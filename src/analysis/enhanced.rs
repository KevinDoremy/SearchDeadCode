// Enhanced dead code analyzer with parallel processing
// and ProGuard cross-validation

use super::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{Declaration, DeclarationId, DeclarationKind, Graph};
use crate::proguard::ProguardUsage;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::info;

/// Enhanced analyzer that combines static analysis with ProGuard validation
pub struct EnhancedAnalyzer {
    /// ProGuard usage data for cross-validation
    proguard: Option<Arc<ProguardUsage>>,
    /// Whether to use strict mode (report more items)
    strict_mode: bool,
}

impl EnhancedAnalyzer {
    pub fn new() -> Self {
        Self {
            proguard: None,
            strict_mode: false,
        }
    }

    pub fn with_proguard(mut self, proguard: ProguardUsage) -> Self {
        self.proguard = Some(Arc::new(proguard));
        self
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    /// Analyze the graph and find dead code with parallel processing
    pub fn analyze(
        &self,
        graph: &Graph,
        entry_points: &HashSet<DeclarationId>,
    ) -> (Vec<DeadCode>, HashSet<DeclarationId>) {
        info!("Running enhanced analysis with parallelism...");

        // Step 1: Build reachability set (parallel BFS from entry points)
        let reachable = self.find_reachable_parallel(graph, entry_points);

        // Step 2: Find unreachable declarations in parallel
        let dead_code = self.find_dead_code_parallel(graph, &reachable);

        // Step 3: Cross-validate with ProGuard data
        let dead_code = if self.proguard.is_some() {
            self.cross_validate_with_proguard(dead_code, graph)
        } else {
            dead_code
        };

        info!(
            "Enhanced analysis complete: {} dead items, {} reachable",
            dead_code.len(),
            reachable.len()
        );

        (dead_code, reachable)
    }

    /// Find all reachable declarations using single BFS from all entry points
    fn find_reachable_parallel(
        &self,
        graph: &Graph,
        entry_points: &HashSet<DeclarationId>,
    ) -> HashSet<DeclarationId> {
        use std::collections::VecDeque;

        info!("  → Starting BFS from {} entry points...", entry_points.len());

        let inner_graph = graph.inner();
        let mut reachable: HashSet<DeclarationId> = entry_points.clone();
        let mut queue: VecDeque<_> = entry_points.iter().cloned().collect();

        // Single BFS from all entry points at once - each node visited only once
        let mut visited_count = 0;
        while let Some(current_id) = queue.pop_front() {
            visited_count += 1;
            if visited_count % 10000 == 0 {
                info!("    Visited {} nodes, queue size: {}", visited_count, queue.len());
            }

            if let Some(node_idx) = graph.node_index(&current_id) {
                // Visit all neighbors
                for neighbor_idx in inner_graph.neighbors(node_idx) {
                    if let Some(neighbor_id) = inner_graph.node_weight(neighbor_idx) {
                        if reachable.insert(neighbor_id.clone()) {
                            queue.push_back(neighbor_id.clone());
                        }
                    }
                }
            }
        }

        info!("  → Found {} directly reachable declarations", reachable.len());

        // Mark ancestors as reachable
        info!("  → Marking ancestors...");
        let mut ancestors = HashSet::new();
        for id in &reachable {
            Self::collect_ancestors(graph, id, &mut ancestors);
        }
        reachable.extend(ancestors);
        info!("  → Total with ancestors: {}", reachable.len());

        // Mark members of reachable classes - multi-pass for nested
        info!("  → Expanding class members...");
        let mut pass = 0;
        loop {
            pass += 1;
            let mut class_members = HashSet::new();
            for decl in graph.declarations() {
                if let Some(parent_id) = &decl.parent {
                    if reachable.contains(parent_id) && !reachable.contains(&decl.id) {
                        class_members.insert(decl.id.clone());
                    }
                }
            }
            if class_members.is_empty() {
                break;
            }
            info!("    Pass {}: added {} members", pass, class_members.len());
            reachable.extend(class_members);
        }
        info!("  → Final reachable count: {}", reachable.len());

        reachable
    }

    /// Collect all ancestor declarations
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

    /// Find dead code in parallel
    fn find_dead_code_parallel(
        &self,
        graph: &Graph,
        reachable: &HashSet<DeclarationId>,
    ) -> Vec<DeadCode> {
        info!("  → Scanning {} declarations for dead code...", graph.declarations().count());
        let declarations: Vec<_> = graph.declarations().collect();

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
                Some(DeadCode::new((*decl).clone(), issue))
            })
            .collect()
    }

    /// Check if a declaration should be skipped
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

        // Skip members of unreachable classes (report class instead)
        if !self.strict_mode {
            if let Some(parent_id) = &decl.parent {
                if !reachable.contains(parent_id) {
                    if let Some(parent) = graph.get_declaration(parent_id) {
                        if parent.kind.is_type() {
                            return true;
                        }
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

        // Skip override methods
        if decl.annotations.iter().any(|a| a.contains("Override")) {
            return true;
        }
        if decl.modifiers.iter().any(|m| m == "override") {
            return true;
        }

        false
    }

    /// Determine the issue type
    fn determine_issue_type(&self, decl: &Declaration) -> DeadCodeIssue {
        match decl.kind {
            DeclarationKind::Import => DeadCodeIssue::UnusedImport,
            DeclarationKind::Parameter => DeadCodeIssue::UnusedParameter,
            DeclarationKind::EnumCase => DeadCodeIssue::UnusedEnumCase,
            _ => DeadCodeIssue::Unreferenced,
        }
    }

    /// Cross-validate findings with ProGuard data
    fn cross_validate_with_proguard(
        &self,
        mut dead_code: Vec<DeadCode>,
        graph: &Graph,
    ) -> Vec<DeadCode> {
        let Some(ref proguard) = self.proguard else {
            return dead_code;
        };

        // Build a map from simple names to FQN for matching
        let mut fqn_map: HashMap<String, String> = HashMap::new();
        for decl in graph.declarations() {
            if let Some(fqn) = &decl.fully_qualified_name {
                fqn_map.insert(decl.name.clone(), fqn.clone());
            }
        }

        // Update confidence based on ProGuard confirmation
        for dc in &mut dead_code {
            let class_fqn = dc.declaration.fully_qualified_name.as_deref();
            let member_name = &dc.declaration.name;

            if let Some(confidence_boost) = proguard.get_confidence_for(class_fqn, member_name) {
                if confidence_boost >= 1.0 {
                    dc.confidence = Confidence::Confirmed;
                    dc.runtime_confirmed = true;
                    dc.message = format!("{} (confirmed by R8/ProGuard)", dc.message);
                } else if confidence_boost >= 0.7 {
                    dc.confidence = Confidence::High;
                }
            }
        }

        // Also find items that ProGuard reports as dead but we didn't detect
        // These are additional candidates
        let mut additional: Vec<DeadCode> = Vec::new();

        for class_name in proguard.dead_classes() {
            // Try to find this class in our graph
            if let Some(decl) = graph.find_by_fqn(class_name) {
                // Check if we already have this
                let already_reported = dead_code.iter().any(|dc| dc.declaration.id == decl.id);
                if !already_reported {
                    let mut dc = DeadCode::new(decl.clone(), DeadCodeIssue::Unreferenced);
                    dc.confidence = Confidence::Confirmed;
                    dc.runtime_confirmed = true;
                    dc.message = format!(
                        "class '{}' is never used (confirmed by R8/ProGuard - missed by static analysis)",
                        decl.name
                    );
                    additional.push(dc);
                }
            }
        }

        if !additional.is_empty() {
            info!(
                "ProGuard cross-validation found {} additional dead items",
                additional.len()
            );
            dead_code.extend(additional);
        }

        // Sort by file and location
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

        dead_code
    }
}

impl Default for EnhancedAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_analyzer_creation() {
        let analyzer = EnhancedAnalyzer::new();
        let graph = Graph::new();
        let entry_points = HashSet::new();

        let (dead_code, _) = analyzer.analyze(&graph, &entry_points);
        assert!(dead_code.is_empty());
    }
}
