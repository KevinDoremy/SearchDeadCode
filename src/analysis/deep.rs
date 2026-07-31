// Deep dead code analyzer - more aggressive detection
//
// Unlike the basic reachability analyzer, this one:
// 1. Does NOT mark all class members as reachable automatically
// 2. Tracks actual references to each member individually
// 3. Detects unused members even in reachable classes
// 4. Uses heuristics for common dead code patterns

use super::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{Declaration, DeclarationId, DeclarationKind, Graph, Language, ReferenceKind};
use petgraph::visit::Dfs;
use rayon::prelude::*;
use std::collections::HashSet;
use tracing::info;

/// Deep analyzer for more aggressive dead code detection
pub struct DeepAnalyzer {
    /// Detect unused members in reachable classes
    detect_unused_members: bool,
    /// Use parallel processing
    parallel: bool,
}

impl DeepAnalyzer {
    pub fn new() -> Self {
        Self {
            detect_unused_members: true,
            parallel: true,
        }
    }

    pub fn with_unused_members(mut self, detect: bool) -> Self {
        self.detect_unused_members = detect;
        self
    }

    pub fn with_parallel(mut self, parallel: bool) -> Self {
        self.parallel = parallel;
        self
    }

    /// Analyze the graph and find dead code
    pub fn analyze(
        &self,
        graph: &Graph,
        entry_points: &HashSet<DeclarationId>,
    ) -> (Vec<DeadCode>, HashSet<DeclarationId>) {
        info!("Running deep analysis...");

        // Step 1: Find truly reachable declarations (not all class members)
        let reachable = self.find_reachable_strict(graph, entry_points);

        info!(
            "Deep reachability: {} strictly reachable, {} total",
            reachable.len(),
            graph.declarations().count()
        );

        // Step 2: Find unreachable declarations
        let mut dead_code = self.find_unreachable(graph, &reachable);

        // Step 3: Find unused members in reachable classes
        if self.detect_unused_members {
            let unused_members = self.find_unused_members(graph, &reachable, entry_points);
            info!(
                "Found {} unused members in reachable classes",
                unused_members.len()
            );
            dead_code.extend(unused_members);
        }

        // Step 4: Apply pattern-based detection
        let pattern_dead = self.detect_dead_patterns(graph, &reachable);
        dead_code.extend(pattern_dead);

        // Sort and deduplicate
        dead_code.sort_by(|a, b| {
            let file_cmp = a
                .declaration
                .location
                .file
                .cmp(&b.declaration.location.file);
            if file_cmp != std::cmp::Ordering::Equal {
                return file_cmp;
            }
            a.declaration
                .location
                .line
                .cmp(&b.declaration.location.line)
        });

        // Deduplicate by declaration ID
        let mut seen = HashSet::new();
        dead_code.retain(|dc| seen.insert(dc.declaration.id.clone()));

        info!("Deep analysis found {} dead code items", dead_code.len());

        (dead_code, reachable)
    }

    /// Find reachable declarations - STRICT mode (doesn't auto-mark class members)
    fn find_reachable_strict(
        &self,
        graph: &Graph,
        entry_points: &HashSet<DeclarationId>,
    ) -> HashSet<DeclarationId> {
        let inner_graph = graph.inner();

        // Use a shared visited set for efficient DFS
        // Instead of running separate DFS from each entry point, we use a single traversal
        let mut reachable = HashSet::new();

        // Collect all starting node indices
        let start_indices: Vec<_> = entry_points
            .iter()
            .filter_map(|id| {
                reachable.insert(id.clone());
                graph.node_index(id)
            })
            .collect();

        // Single DFS traversal using a worklist (more efficient than per-entry DFS)
        let mut visited_indices = HashSet::new();
        let mut stack: Vec<_> = start_indices;

        while let Some(node_idx) = stack.pop() {
            if !visited_indices.insert(node_idx) {
                continue;
            }

            if let Some(node_id) = inner_graph.node_weight(node_idx) {
                reachable.insert(node_id.clone());
            }

            // Add all neighbors to stack
            for neighbor in inner_graph.neighbors(node_idx) {
                if !visited_indices.contains(&neighbor) {
                    stack.push(neighbor);
                }
            }
        }

        // Mark ancestors as reachable (batch collect to avoid repeated lookups)
        let ancestor_ids: Vec<_> = reachable.iter().cloned().collect();
        for id in ancestor_ids {
            Self::collect_ancestors(graph, &id, &mut reachable);
        }

        // When a class/type is reachable, follow references from its members (fields, etc.)
        // This ensures inner classes referenced by field initializers are reachable
        let reachable_types: Vec<_> = reachable
            .iter()
            .filter(|id| {
                if let Some(decl) = graph.get_declaration(id) {
                    decl.kind.is_type()
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        for type_id in reachable_types {
            // Find all members of this type
            for decl in graph.declarations() {
                if decl.parent.as_ref() == Some(&type_id) {
                    // Only follow property/field initializers: following method
                    // edges would resurrect everything a dead method references
                    // (e.g. an unconsumed @Provides body)
                    if !matches!(
                        decl.kind,
                        crate::graph::DeclarationKind::Property
                            | crate::graph::DeclarationKind::Field
                    ) {
                        continue;
                    }
                    // Follow edges from this member
                    if let Some(member_idx) = graph.node_index(&decl.id) {
                        for neighbor in inner_graph.neighbors(member_idx) {
                            if let Some(neighbor_id) = inner_graph.node_weight(neighbor) {
                                if !reachable.contains(neighbor_id) {
                                    // DFS from this newly discovered node
                                    let mut dfs = Dfs::new(inner_graph, neighbor);
                                    while let Some(node_idx) = dfs.next(inner_graph) {
                                        if let Some(node_id) = inner_graph.node_weight(node_idx) {
                                            reachable.insert(node_id.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // IMPORTANT: Only mark certain members as reachable:
        // 1. Override methods (called via polymorphism)
        // 2. Constructors of instantiated classes
        // 3. Serialization-related members
        // 4. Companion object members that are accessed

        // Le blessing (overrides, ctors, sealed, interfaces) et la DFS
        // incrémentale se nourrissent l'un l'autre : une chaîne
        // override → appel → override casse si la passe ne tourne
        // qu'une fois — le deuxième override n'est jamais blessé.
        // Itérer jusqu'au point fixe (croissance monotone, bornée par
        // le nombre de nœuds).
        loop {
            // Pre-compute which classes are instantiated (avoid repeated lookups)
            let instantiated_classes: HashSet<_> = reachable
                .iter()
                .filter(|id| {
                    graph.get_references_to(id).iter().any(|(_, r)| {
                        matches!(r.kind, ReferenceKind::Call | ReferenceKind::Instantiation)
                    })
                })
                .cloned()
                .collect();

            // Single pass over declarations to find additional reachable items
            let additional: Vec<_> = if self.parallel {
                let declarations: Vec<_> = graph.declarations().collect();
                declarations
                    .par_iter()
                    .filter_map(|decl| {
                        if reachable.contains(&decl.id) {
                            return None;
                        }

                        let parent_id = decl.parent.as_ref()?;
                        if !reachable.contains(parent_id) {
                            return None;
                        }

                        // Override methods are reachable via polymorphism
                        if decl.modifiers.iter().any(|m| m == "override")
                            || decl.annotations.iter().any(|a| a.contains("Override"))
                        {
                            return Some(decl.id.clone());
                        }

                        // Primary constructor is reachable if class is instantiated
                        if decl.kind == DeclarationKind::Constructor
                            && (instantiated_classes.contains(parent_id)
                                || entry_points.contains(parent_id))
                        {
                            return Some(decl.id.clone());
                        }

                        // Serialization members
                        if self.is_serialization_member(decl, graph) {
                            return Some(decl.id.clone());
                        }

                        // Companion object
                        if decl.kind == DeclarationKind::Object
                            && decl.modifiers.iter().any(|m| m == "companion")
                        {
                            return Some(decl.id.clone());
                        }

                        // Lazy/delegated properties
                        if decl.kind == DeclarationKind::Property
                            && decl.modifiers.iter().any(|m| m == "delegated")
                        {
                            return Some(decl.id.clone());
                        }

                        // Suspend functions in reachable classes
                        if self.is_suspend_function(decl) {
                            return Some(decl.id.clone());
                        }

                        // Flow-related declarations
                        if self.is_flow_pattern(decl) {
                            return Some(decl.id.clone());
                        }

                        None
                    })
                    .collect()
            } else {
                graph
                    .declarations()
                    .filter_map(|decl| {
                        if reachable.contains(&decl.id) {
                            return None;
                        }

                        let parent_id = decl.parent.as_ref()?;
                        if !reachable.contains(parent_id) {
                            return None;
                        }

                        if decl.modifiers.iter().any(|m| m == "override")
                            || decl.annotations.iter().any(|a| a.contains("Override"))
                        {
                            return Some(decl.id.clone());
                        }

                        if decl.kind == DeclarationKind::Constructor
                            && (instantiated_classes.contains(parent_id)
                                || entry_points.contains(parent_id))
                        {
                            return Some(decl.id.clone());
                        }

                        if self.is_serialization_member(decl, graph) {
                            return Some(decl.id.clone());
                        }

                        if decl.kind == DeclarationKind::Object
                            && decl.modifiers.iter().any(|m| m == "companion")
                        {
                            return Some(decl.id.clone());
                        }

                        if decl.kind == DeclarationKind::Property
                            && decl.modifiers.iter().any(|m| m == "delegated")
                        {
                            return Some(decl.id.clone());
                        }

                        if self.is_suspend_function(decl) {
                            return Some(decl.id.clone());
                        }

                        if self.is_flow_pattern(decl) {
                            return Some(decl.id.clone());
                        }

                        None
                    })
                    .collect()
            };

            // Collect sealed class subtypes and interface implementations
            let sealed_subtypes = self.collect_sealed_subtypes(graph, &reachable);
            let interface_impls = self.collect_interface_implementations(graph, &reachable);

            // Combine all newly discovered items for incremental DFS
            // This includes: override methods, sealed subtypes, interface implementations
            let new_items: Vec<_> = additional
                .iter()
                .chain(sealed_subtypes.iter())
                .chain(interface_impls.iter())
                .filter(|id| !reachable.contains(*id))
                .cloned()
                .collect();

            // Invariant du point fixe : tout nœud de `reachable` doit avoir
            // ses arêtes sortantes suivies. Les insertions directes
            // (ancestors, blessing, membres sérialisation…) ne passent pas
            // par une DFS — sans ce rattrapage, un override raccroché
            // tardivement reste "vivant" pendant que ses callees meurent.
            let unvisited: Vec<_> = reachable
                .iter()
                .filter(|id| {
                    graph
                        .node_index(id)
                        .is_some_and(|ix| !visited_indices.contains(&ix))
                })
                .cloned()
                .collect();

            if new_items.is_empty() && unvisited.is_empty() {
                break;
            }

            reachable.extend(additional);
            reachable.extend(sealed_subtypes);
            reachable.extend(interface_impls);

            // Incremental DFS from new items and from any reachable node
            // whose edges were never walked
            for id in new_items.iter().chain(unvisited.iter()) {
                if let Some(start_idx) = graph.node_index(id) {
                    if visited_indices.contains(&start_idx) {
                        continue;
                    }
                    let mut dfs = Dfs::new(inner_graph, start_idx);
                    while let Some(node_idx) = dfs.next(inner_graph) {
                        visited_indices.insert(node_idx);
                        if let Some(node_id) = inner_graph.node_weight(node_idx) {
                            reachable.insert(node_id.clone());
                        }
                    }
                }
            }
        }

        reachable
    }

    /// Check if a member is serialization-related
    fn is_serialization_member(&self, decl: &Declaration, graph: &Graph) -> bool {
        // Check for serialization annotations
        let serialization_annotations = [
            "Serializable",
            "SerializedName",
            "JsonProperty",
            "JsonField",
            "Parcelize",
            "Parcelable",
            "Entity",
            "ColumnInfo",
            "PrimaryKey",
        ];

        for ann in &decl.annotations {
            for pattern in &serialization_annotations {
                if ann.contains(pattern) {
                    return true;
                }
            }
        }

        // A member of a serializable class is reached reflectively: Java
        // serialization reads private fields without ever spelling them.
        if matches!(
            decl.kind,
            DeclarationKind::Field | DeclarationKind::Property
        ) {
            if let Some(parent_id) = &decl.parent {
                if let Some(parent) = graph.get_declaration(parent_id) {
                    let parent_is_serializable = parent.super_types.iter().any(|s| {
                        let last = s.rsplit('.').next().unwrap_or(s);
                        matches!(last, "Serializable" | "Parcelable" | "Externalizable")
                    }) || parent
                        .annotations
                        .iter()
                        .any(|a| serialization_annotations.iter().any(|p| a.contains(p)));
                    if parent_is_serializable {
                        return true;
                    }
                }
            }
        }

        // Check for common serialization method names, including the
        // reflective hooks of java.io.Serializable
        let serialization_methods = [
            "writeToParcel",
            "describeContents",
            "createFromParcel",
            "newArray",
            "readFromParcel",
            "writeObject",
            "readObject",
            "readObjectNoData",
            "writeReplace",
            "readResolve",
        ];

        if matches!(
            decl.kind,
            DeclarationKind::Function | DeclarationKind::Method
        ) {
            for method in &serialization_methods {
                if decl.name == *method {
                    return true;
                }
            }
        }

        false
    }

    /// Collect ancestors
    fn collect_ancestors(
        graph: &Graph,
        id: &DeclarationId,
        ancestors: &mut HashSet<DeclarationId>,
    ) {
        if let Some(decl) = graph.get_declaration(id) {
            if let Some(parent_id) = &decl.parent {
                if ancestors.insert(parent_id.clone()) {
                    Self::collect_ancestors(graph, parent_id, ancestors);
                }
            }
        }
    }

    /// Find unreachable declarations
    fn find_unreachable(&self, graph: &Graph, reachable: &HashSet<DeclarationId>) -> Vec<DeadCode> {
        let declarations: Vec<_> = graph.declarations().collect();

        let dead_code: Vec<_> = if self.parallel {
            declarations
                .par_iter()
                .filter_map(|decl| {
                    if reachable.contains(&decl.id) {
                        return None;
                    }
                    if self.should_skip_declaration(decl, graph, reachable) {
                        return None;
                    }
                    let issue = self.determine_issue_type(decl);
                    Some(zombie_aware_finding(graph, decl, issue))
                })
                .collect()
        } else {
            declarations
                .iter()
                .filter_map(|decl| {
                    if reachable.contains(&decl.id) {
                        return None;
                    }
                    if self.should_skip_declaration(decl, graph, reachable) {
                        return None;
                    }
                    let issue = self.determine_issue_type(decl);
                    Some(zombie_aware_finding(graph, decl, issue))
                })
                .collect()
        };

        dead_code
    }

    /// Find unused members in reachable classes
    fn find_unused_members(
        &self,
        graph: &Graph,
        reachable: &HashSet<DeclarationId>,
        entry_points: &HashSet<DeclarationId>,
    ) -> Vec<DeadCode> {
        let mut unused = Vec::new();

        for decl in graph.declarations() {
            // Skip if already marked unreachable
            if !reachable.contains(&decl.id) {
                continue;
            }

            // An entry point is invoked from outside the graph (static
            // main, Lombok-accessor-reached fields, framework hooks)
            if entry_points.contains(&decl.id) {
                continue;
            }

            // Only check members of classes
            let Some(parent_id) = &decl.parent else {
                continue;
            };

            // Le framework instancie lui-même les classes déclarées au
            // manifest / dans un layout : leur constructeur n'a aucun
            // appelant dans le code, par construction
            if decl.kind == DeclarationKind::Constructor && entry_points.contains(parent_id) {
                continue;
            }

            // Parent must be reachable too
            if !reachable.contains(parent_id) {
                continue;
            }

            // Skip certain kinds
            if decl.kind == DeclarationKind::Class
                || decl.kind == DeclarationKind::Interface
                || decl.kind == DeclarationKind::Object
                || decl.kind == DeclarationKind::File
                || decl.kind == DeclarationKind::Package
            {
                continue;
            }

            // Skip override methods
            if decl.modifiers.iter().any(|m| m == "override")
                || decl.annotations.iter().any(|a| a.contains("Override"))
            {
                continue;
            }

            // Java's @Override is optional: a lifecycle-named method in a
            // class that extends something is a framework callback
            if is_lifecycle_callback(graph, decl) {
                continue;
            }

            // Skip constructors
            if decl.kind == DeclarationKind::Constructor {
                continue;
            }

            // Skip serialization members
            if self.is_serialization_member(decl, graph) {
                continue;
            }

            // Skip const val (inlined at compile time)
            if self.is_const_val(decl) {
                continue;
            }

            // Skip Dagger/DI annotated methods (they're entry points called by framework)
            if self.is_di_entry_point(decl) {
                continue;
            }

            // Skip data class auto-generated methods
            if self.is_data_class_generated_method(decl, graph) {
                continue;
            }

            // Skip declarations with @Suppress("unused") or @Suppress("UnusedPrivateMember")
            if decl.annotations.iter().any(|a| {
                a.contains("Suppress")
                    && (a.contains("unused") || a.contains("UnusedPrivateMember"))
            }) {
                continue;
            }

            // Skip declarations with @VisibleForTesting
            if decl
                .annotations
                .iter()
                .any(|a| a.contains("VisibleForTesting"))
            {
                continue;
            }

            // Skip test methods and test rules (called by test framework)
            let is_test = decl.annotations.iter().any(|a| {
                a.contains("Test")
                    || a.contains("Before")
                    || a.contains("After")
                    || a.contains("BeforeEach")
                    || a.contains("AfterEach")
                    || a.contains("BeforeAll")
                    || a.contains("AfterAll")
                    || a.contains("ParameterizedTest")
                    || a.contains("RepeatedTest")
                    || a.contains("Rule")
                    || a.contains("ClassRule")
            });
            if is_test {
                continue;
            }

            // Skip public API (might be used externally)
            if decl.visibility == crate::graph::Visibility::Public {
                // But still report if it's not referenced at all
                if graph.is_referenced(&decl.id) {
                    continue;
                }
            }

            // Check if this member is actually referenced
            if !graph.is_referenced(&decl.id) {
                let mut dc = DeadCode::new(decl.clone(), DeadCodeIssue::Unreferenced);
                dc.confidence = Confidence::Medium;
                unused.push(dc);
            }

            // Check for write-only properties
            if decl.kind == DeclarationKind::Property {
                if let Some(issue) = self.detect_write_only_property(decl, graph) {
                    unused.push(issue);
                }
            }
        }

        unused
    }

    /// Detect write-only properties - properties that are written but never read
    fn detect_write_only_property(&self, decl: &Declaration, graph: &Graph) -> Option<DeadCode> {
        // Only check properties
        if decl.kind != DeclarationKind::Property {
            return None;
        }

        // Get all references to this property
        let refs = graph.get_references_to(&decl.id);

        if refs.is_empty() {
            return None; // Already reported as unreferenced
        }

        // Check if all references are writes.
        // Invoquer une propriété de type fonction (`params.onDone(true)`)
        // la LIT avant de l'appeler — la référence est classée Call, ce
        // qui faisait passer les porteurs de callbacks pour write-only.
        let has_writes = refs.iter().any(|(_, r)| r.kind == ReferenceKind::Write);
        let has_reads = refs
            .iter()
            .any(|(_, r)| matches!(r.kind, ReferenceKind::Read | ReferenceKind::Call));

        if has_writes && !has_reads {
            let mut dc = DeadCode::new(decl.clone(), DeadCodeIssue::AssignOnly);
            dc.confidence = Confidence::Medium;
            dc.message = format!("Property '{}' is written but never read", decl.name);
            return Some(dc);
        }

        None
    }

    /// Detect dead code patterns
    fn detect_dead_patterns(
        &self,
        graph: &Graph,
        reachable: &HashSet<DeclarationId>,
    ) -> Vec<DeadCode> {
        let mut pattern_dead = Vec::new();

        for decl in graph.declarations() {
            if reachable.contains(&decl.id) {
                continue;
            }

            // Pattern 1: Debug-only classes
            if self.is_debug_only_pattern(decl) {
                let mut dc = DeadCode::new(decl.clone(), DeadCodeIssue::Unreferenced);
                dc.confidence = Confidence::High;
                dc.message = format!(
                    "{} '{}' appears to be debug-only code",
                    decl.kind.display_name(),
                    decl.name
                );
                pattern_dead.push(dc);
                continue;
            }

            // Pattern 2: Test helper classes in main source
            if self.is_test_helper_pattern(decl) {
                let mut dc = DeadCode::new(decl.clone(), DeadCodeIssue::Unreferenced);
                dc.confidence = Confidence::High;
                dc.message = format!(
                    "{} '{}' appears to be test code in main source",
                    decl.kind.display_name(),
                    decl.name
                );
                pattern_dead.push(dc);
                continue;
            }

            // Pattern 3: Deprecated code without usages
            if self.is_deprecated_unused(decl, graph) {
                let mut dc = DeadCode::new(decl.clone(), DeadCodeIssue::Unreferenced);
                dc.confidence = Confidence::High;
                dc.message = format!(
                    "{} '{}' is deprecated and has no usages",
                    decl.kind.display_name(),
                    decl.name
                );
                pattern_dead.push(dc);
                continue;
            }

            // Pattern 4: Empty/stub implementations
            if self.is_stub_implementation(decl) {
                let mut dc = DeadCode::new(decl.clone(), DeadCodeIssue::Unreferenced);
                dc.confidence = Confidence::Medium;
                dc.message = format!(
                    "{} '{}' appears to be a stub/empty implementation",
                    decl.kind.display_name(),
                    decl.name
                );
                pattern_dead.push(dc);
            }
        }

        pattern_dead
    }

    /// Check if declaration is debug-only pattern
    fn is_debug_only_pattern(&self, decl: &Declaration) -> bool {
        let debug_patterns = [
            "Debug",
            "Debugger",
            "DebugMenu",
            "DebugHelper",
            "DebugPanel",
            "DebugScreen",
            "DebugActivity",
            "DebugFragment",
            "DebugView",
            "DebugListener",
            "DebugReceiver",
        ];

        for pattern in &debug_patterns {
            if decl.name.contains(pattern) {
                return true;
            }
        }

        // Check if in debug source set
        if path_has_segment(&decl.location.file, "debug")
            || path_has_segment(&decl.location.file, "staging")
        {
            return true;
        }

        false
    }

    /// Check if declaration is a test helper pattern
    fn is_test_helper_pattern(&self, decl: &Declaration) -> bool {
        let test_patterns = [
            "Mock",
            "Fake",
            "Stub",
            "TestHelper",
            "TestUtil",
            "TestData",
            "ForTest",
            "InTest",
        ];

        // Only flag if in main source (not in test directories)
        if path_has_segment(&decl.location.file, "test")
            || path_has_segment(&decl.location.file, "androidTest")
        {
            return false;
        }

        for pattern in &test_patterns {
            if decl.name.contains(pattern) {
                return true;
            }
        }

        false
    }

    /// Check if declaration is deprecated and unused
    fn is_deprecated_unused(&self, decl: &Declaration, graph: &Graph) -> bool {
        let is_deprecated = decl.annotations.iter().any(|a| a.contains("Deprecated"));
        if !is_deprecated {
            return false;
        }
        !graph.is_referenced(&decl.id)
    }

    /// Check if declaration is a stub implementation
    fn is_stub_implementation(&self, decl: &Declaration) -> bool {
        // Check for TODO/FIXME in name suggesting incomplete implementation
        let stub_indicators = ["Stub", "Empty", "Noop", "NoOp", "Dummy", "Placeholder"];

        for indicator in &stub_indicators {
            if decl.name.contains(indicator) {
                return true;
            }
        }

        false
    }

    /// Check if declaration should be skipped
    fn should_skip_declaration(
        &self,
        decl: &Declaration,
        graph: &Graph,
        reachable: &HashSet<DeclarationId>,
    ) -> bool {
        // Skip file-level declarations
        if decl.kind == DeclarationKind::File || decl.kind == DeclarationKind::Package {
            return true;
        }

        // Skip parameters - let the dedicated UnusedParamDetector handle them
        // The detector checks if parameters are actually used in their function body,
        // which is more accurate than reachability-based detection
        if decl.kind == DeclarationKind::Parameter {
            return true;
        }

        // Skip members of unreachable classes (report class instead)
        if let Some(parent_id) = &decl.parent {
            if !reachable.contains(parent_id) {
                if let Some(parent) = graph.get_declaration(parent_id) {
                    if parent.kind.is_type() {
                        return true;
                    }
                }
            }
        }

        // Skip constructors of unreachable classes
        if decl.kind == DeclarationKind::Constructor {
            if let Some(parent_id) = &decl.parent {
                if !reachable.contains(parent_id) {
                    return true;
                }
            }
        }

        // Un constructeur privé sans paramètre est l'idiome
        // anti-instanciation des classes utilitaires : son but EST de ne
        // jamais être appelé, le supprimer réintroduirait le ctor public
        if decl.kind == DeclarationKind::Constructor
            && decl.visibility == crate::graph::Visibility::Private
            && graph.get_children(&decl.id).iter().all(|c| {
                graph
                    .get_declaration(c)
                    .map_or(true, |d| d.kind != DeclarationKind::Parameter)
            })
        {
            return true;
        }

        // Skip Kotlin const val properties (they are inlined at compile time)
        if self.is_const_val(decl) {
            return true;
        }

        // Skip serialization members: a private field of a Serializable
        // class is read reflectively, never by name
        if self.is_serialization_member(decl, graph) {
            return true;
        }

        // Skip data class auto-generated methods (copy, componentN, equals, hashCode, toString)
        if self.is_data_class_generated_method(decl, graph) {
            return true;
        }

        // Skip overridden methods (they might be called via interface/base class)
        // Check both Java-style @Override annotation and Kotlin override modifier
        if decl.annotations.iter().any(|a| a.contains("Override")) {
            return true;
        }
        if decl.modifiers.iter().any(|m| m == "override") {
            return true;
        }

        // Java's @Override is optional: a lifecycle-named method in a
        // class that extends something is a framework callback
        if is_lifecycle_callback(graph, decl) {
            return true;
        }

        // Skip declarations with @Suppress("unused") or @Suppress("UnusedPrivateMember")
        // Developer explicitly acknowledges the code is unused but wants to keep it
        if decl.annotations.iter().any(|a| {
            a.contains("Suppress") && (a.contains("unused") || a.contains("UnusedPrivateMember"))
        }) {
            return true;
        }

        // Skip declarations with @VisibleForTesting - they are public only for testing
        if decl
            .annotations
            .iter()
            .any(|a| a.contains("VisibleForTesting"))
        {
            return true;
        }

        // Skip test methods and test rules (called by test framework)
        if decl.annotations.iter().any(|a| {
            a.contains("Test")
                || a.contains("Before")
                || a.contains("After")
                || a.contains("BeforeEach")
                || a.contains("AfterEach")
                || a.contains("BeforeAll")
                || a.contains("AfterAll")
                || a.contains("ParameterizedTest")
                || a.contains("RepeatedTest")
                || a.contains("Rule")
                || a.contains("ClassRule")
        }) {
            return true;
        }

        false
    }

    /// Check if a declaration is a Kotlin const val property
    /// These are inlined at compile time, so they appear unused even when used
    fn is_const_val(&self, decl: &Declaration) -> bool {
        if decl.kind != DeclarationKind::Property {
            return false;
        }

        // Only Kotlin has const val
        if decl.language != Language::Kotlin {
            return false;
        }

        // Check for const modifier
        decl.modifiers.iter().any(|m| m == "const")
    }

    /// Check if a declaration is a data class
    fn is_data_class(&self, decl: &Declaration) -> bool {
        if decl.kind != DeclarationKind::Class {
            return false;
        }

        if decl.language != Language::Kotlin {
            return false;
        }

        decl.modifiers.iter().any(|m| m == "data")
    }

    /// Check if a declaration is a sealed class
    fn is_sealed_class(&self, decl: &Declaration) -> bool {
        if decl.kind != DeclarationKind::Class && decl.kind != DeclarationKind::Interface {
            return false;
        }

        if decl.language != Language::Kotlin {
            return false;
        }

        decl.modifiers.iter().any(|m| m == "sealed")
    }

    /// Check if a method is an auto-generated data class method
    /// Data classes generate: copy(), componentN(), equals(), hashCode(), toString()
    fn is_data_class_generated_method(&self, decl: &Declaration, graph: &Graph) -> bool {
        // Only check methods
        if decl.kind != DeclarationKind::Method && decl.kind != DeclarationKind::Function {
            return false;
        }

        // Check if parent is a data class
        if let Some(parent_id) = &decl.parent {
            if let Some(parent) = graph.get_declaration(parent_id) {
                if self.is_data_class(parent) {
                    // Check for auto-generated method names
                    let generated_methods = ["copy", "equals", "hashCode", "toString"];
                    if generated_methods.contains(&decl.name.as_str()) {
                        return true;
                    }
                    // componentN methods (component1, component2, etc.)
                    if decl.name.starts_with("component") && decl.name[9..].parse::<u32>().is_ok() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Find all sealed class subtypes and mark them as reachable when the parent is reachable
    fn collect_sealed_subtypes(
        &self,
        graph: &Graph,
        reachable: &HashSet<DeclarationId>,
    ) -> HashSet<DeclarationId> {
        // First, find all sealed classes that are reachable - build a HashSet for O(1) lookup
        let sealed_names: HashSet<String> = graph
            .declarations()
            .filter(|d| reachable.contains(&d.id) && self.is_sealed_class(d))
            .flat_map(|d| {
                let fqn = d
                    .fully_qualified_name
                    .clone()
                    .unwrap_or_else(|| d.name.clone());
                let simple = fqn.split('.').next_back().unwrap_or(&fqn).to_string();
                vec![fqn, simple]
            })
            .collect();

        if sealed_names.is_empty() {
            return HashSet::new();
        }

        // Find all classes that extend these sealed classes - single pass with HashSet lookups
        let declarations: Vec<_> = graph.declarations().collect();

        if self.parallel {
            declarations
                .par_iter()
                .filter_map(|decl| {
                    if reachable.contains(&decl.id) {
                        return None;
                    }

                    for super_type in &decl.super_types {
                        if sealed_names.contains(super_type) {
                            return Some(decl.id.clone());
                        }
                        let simple = super_type.split('.').next_back().unwrap_or(super_type);
                        if sealed_names.contains(simple) {
                            return Some(decl.id.clone());
                        }
                    }
                    None
                })
                .collect()
        } else {
            declarations
                .iter()
                .filter_map(|decl| {
                    if reachable.contains(&decl.id) {
                        return None;
                    }

                    for super_type in &decl.super_types {
                        if sealed_names.contains(super_type) {
                            return Some(decl.id.clone());
                        }
                        let simple = super_type.split('.').next_back().unwrap_or(super_type);
                        if sealed_names.contains(simple) {
                            return Some(decl.id.clone());
                        }
                    }
                    None
                })
                .collect()
        }
    }

    /// Find all interface implementations and mark them as reachable when the interface is reachable
    fn collect_interface_implementations(
        &self,
        graph: &Graph,
        reachable: &HashSet<DeclarationId>,
    ) -> HashSet<DeclarationId> {
        // Build a HashSet of interface names for O(1) lookup
        let interface_names: HashSet<String> = graph
            .declarations()
            .filter(|d| reachable.contains(&d.id) && d.kind == DeclarationKind::Interface)
            .flat_map(|d| {
                let fqn = d
                    .fully_qualified_name
                    .clone()
                    .unwrap_or_else(|| d.name.clone());
                let simple = fqn.split('.').next_back().unwrap_or(&fqn).to_string();
                vec![fqn, simple]
            })
            .collect();

        if interface_names.is_empty() {
            return HashSet::new();
        }

        // Find all classes that implement these interfaces - single pass with HashSet lookups
        let declarations: Vec<_> = graph.declarations().collect();

        if self.parallel {
            declarations
                .par_iter()
                .filter_map(|decl| {
                    if reachable.contains(&decl.id) {
                        return None;
                    }

                    for super_type in &decl.super_types {
                        if interface_names.contains(super_type) {
                            return Some(decl.id.clone());
                        }
                        let simple = super_type.split('.').next_back().unwrap_or(super_type);
                        if interface_names.contains(simple) {
                            return Some(decl.id.clone());
                        }
                    }
                    None
                })
                .collect()
        } else {
            declarations
                .iter()
                .filter_map(|decl| {
                    if reachable.contains(&decl.id) {
                        return None;
                    }

                    for super_type in &decl.super_types {
                        if interface_names.contains(super_type) {
                            return Some(decl.id.clone());
                        }
                        let simple = super_type.split('.').next_back().unwrap_or(super_type);
                        if interface_names.contains(simple) {
                            return Some(decl.id.clone());
                        }
                    }
                    None
                })
                .collect()
        }
    }

    /// Check if a function is a suspend function (used in coroutines)
    fn is_suspend_function(&self, decl: &Declaration) -> bool {
        if decl.kind != DeclarationKind::Function && decl.kind != DeclarationKind::Method {
            return false;
        }

        decl.modifiers.iter().any(|m| m == "suspend")
    }

    /// Check if a declaration is a Flow-related pattern
    fn is_flow_pattern(&self, decl: &Declaration) -> bool {
        // Check for Flow types in name or annotations
        let flow_patterns = [
            "Flow",
            "StateFlow",
            "SharedFlow",
            "MutableStateFlow",
            "MutableSharedFlow",
        ];

        for pattern in &flow_patterns {
            if decl.name.contains(pattern) {
                return true;
            }
        }

        // Check for flow-related annotations
        decl.annotations
            .iter()
            .any(|a| a.contains("FlowPreview") || a.contains("ExperimentalCoroutinesApi"))
    }

    /// Check if a declaration is a DI/framework entry point (Dagger, Hilt, etc.)
    fn is_di_entry_point(&self, decl: &Declaration) -> bool {
        let di_annotations = [
            // Dagger/Hilt providers
            "Provides",
            "Binds",
            "BindsOptionalOf",
            "BindsInstance",
            "IntoMap",
            "IntoSet",
            "ElementsIntoSet",
            "Multibinds",
            // Dagger injection
            "Inject",
            "AssistedInject",
            "AssistedFactory",
            // Koin
            "Factory",
            "Single",
            "KoinViewModel",
            // Room
            "Query",
            "Insert",
            "Update",
            "Delete",
            "RawQuery",
            "Transaction",
            // Retrofit
            "GET",
            "POST",
            "PUT",
            "DELETE",
            "PATCH",
            "HEAD",
            "OPTIONS",
            "HTTP",
            // Lifecycle
            "OnLifecycleEvent",
            // Data binding
            "BindingAdapter",
            "InverseBindingAdapter",
            "BindingMethod",
            "BindingMethods",
            "BindingConversion",
            // Event handlers
            "Subscribe",
            "OnClick",
            // Compose — Composable deliberately absent (an uncalled
            // composable is dead code); previews are IDE-invoked roots
            "Preview",
        ];

        for annotation in &decl.annotations {
            for di_ann in &di_annotations {
                if annotation.contains(di_ann) {
                    return true;
                }
            }
        }

        false
    }

    /// Determine issue type
    fn determine_issue_type(&self, decl: &Declaration) -> DeadCodeIssue {
        match decl.kind {
            DeclarationKind::Import => DeadCodeIssue::UnusedImport,
            DeclarationKind::Parameter => DeadCodeIssue::UnusedParameter,
            DeclarationKind::EnumCase => DeadCodeIssue::UnusedEnumCase,
            _ => DeadCodeIssue::Unreferenced,
        }
    }
}

impl Default for DeepAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Android framework callbacks the runtime invokes by name. Retention
/// requires the parent to extend something — the name alone earns
/// nothing in a base-less class.
const LIFECYCLE_METHODS: &[&str] = &[
    "onCreate",
    "onStart",
    "onRestart",
    "onResume",
    "onPause",
    "onStop",
    "onDestroy",
    "onAttach",
    "onDetach",
    "onCreateView",
    "onViewCreated",
    "onDestroyView",
    "onActivityCreated",
    "onBind",
    "onUnbind",
    "onStartCommand",
    "onReceive",
    "onNewIntent",
    "onActivityResult",
    "onRequestPermissionsResult",
    "onSaveInstanceState",
    "onRestoreInstanceState",
    "onConfigurationChanged",
    "onLowMemory",
    "onTrimMemory",
];

fn is_lifecycle_callback(graph: &Graph, decl: &Declaration) -> bool {
    if decl.kind != DeclarationKind::Method || !LIFECYCLE_METHODS.contains(&decl.name.as_str()) {
        return false;
    }
    let Some(parent_id) = &decl.parent else {
        return false;
    };
    graph
        .get_declaration(parent_id)
        .map(|parent| !parent.super_types.is_empty())
        .unwrap_or(false)
}

/// "is never used" would be false for a zombie: it IS referenced, but
/// only from code that is itself dead. Name the real condition.
fn zombie_aware_finding(
    graph: &Graph,
    decl: &Declaration,
    issue: super::DeadCodeIssue,
) -> super::DeadCode {
    let finding = super::DeadCode::new(decl.clone(), issue);
    if matches!(issue, super::DeadCodeIssue::Unreferenced)
        && !graph.get_references_to(&decl.id).is_empty()
    {
        finding.with_message(format!(
            "{} '{}' is only referenced from dead code — unreachable from any entry point",
            decl.kind.display_name(),
            decl.name
        ))
    } else {
        finding
    }
}

/// Does the path contain this directory segment? Separator-agnostic:
/// Windows backslash paths name the same source sets.
pub(crate) fn path_has_segment(path: &std::path::Path, segment: &str) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized.contains(&format!("/{segment}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_analyzer_creation() {
        let analyzer = DeepAnalyzer::new();
        let graph = Graph::new();
        let entry_points = HashSet::new();

        let (dead_code, _) = analyzer.analyze(&graph, &entry_points);
        assert!(dead_code.is_empty());
    }

    #[test]
    fn path_segments_match_windows_separators_too() {
        use std::path::Path;
        assert!(path_has_segment(Path::new("src/debug/Hook.kt"), "debug"));
        assert!(
            path_has_segment(Path::new("src\\debug\\Hook.kt"), "debug"),
            "a backslash path is the same source set"
        );
        assert!(
            !path_has_segment(Path::new("src/debugging/Hook.kt"), "debug"),
            "segment match, not substring match"
        );
        assert!(!path_has_segment(Path::new("Debug.kt"), "debug"));
    }
}
