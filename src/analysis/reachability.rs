use super::{DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationId, DeclarationKind, Graph};
use petgraph::visit::Dfs;
use std::collections::HashSet;
use tracing::debug;

/// Analyzer for finding unreachable/dead code via graph traversal
pub struct ReachabilityAnalyzer;

impl ReachabilityAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Find all unreachable declarations starting from entry points
    pub fn find_unreachable(
        &self,
        graph: &Graph,
        entry_points: &HashSet<DeclarationId>,
    ) -> Vec<DeadCode> {
        let (dead_code, _) = self.find_unreachable_with_reachable(graph, entry_points);
        dead_code
    }

    /// Find all unreachable declarations and also return the reachable set
    /// This is useful for hybrid analysis that needs to check runtime coverage
    pub fn find_unreachable_with_reachable(
        &self,
        graph: &Graph,
        entry_points: &HashSet<DeclarationId>,
    ) -> (Vec<DeadCode>, HashSet<DeclarationId>) {
        // First, find all reachable nodes via DFS from entry points
        let reachable = self.find_reachable(graph, entry_points);

        // Collect unreachable declarations
        let mut dead_code = Vec::new();

        for decl in graph.declarations() {
            // Skip if reachable
            if reachable.contains(&decl.id) {
                continue;
            }

            // Skip certain kinds that shouldn't be reported
            if self.should_skip_declaration(decl, graph) {
                continue;
            }

            debug!("Unreachable: {} ({})", decl.name, decl.kind.display_name());

            let issue = self.determine_issue_type(decl);
            dead_code.push(DeadCode::new(decl.clone(), issue));
        }

        // Sort by file and location for consistent output
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

        (dead_code, reachable)
    }

    /// Find all reachable nodes from entry points using DFS
    fn find_reachable(
        &self,
        graph: &Graph,
        entry_points: &HashSet<DeclarationId>,
    ) -> HashSet<DeclarationId> {
        let mut reachable = HashSet::new();
        let inner_graph = graph.inner();

        // Step 1: Initial DFS from entry points
        for entry_id in entry_points {
            if let Some(start_idx) = graph.node_index(entry_id) {
                // Add entry point itself
                reachable.insert(entry_id.clone());

                // Perform DFS from this entry point
                let mut dfs = Dfs::new(inner_graph, start_idx);

                while let Some(node_idx) = dfs.next(inner_graph) {
                    if let Some(node_id) = inner_graph.node_weight(node_idx) {
                        reachable.insert(node_id.clone());

                        // Also mark parent declarations as reachable
                        if let Some(decl) = graph.get_declaration(node_id) {
                            if let Some(parent_id) = &decl.parent {
                                reachable.insert(parent_id.clone());
                            }
                        }
                    }
                }
            }
        }

        // Step 2: Mark all ancestors of reachable nodes as reachable
        let mut ancestors = HashSet::new();
        for id in &reachable {
            Self::collect_ancestors(graph, id, &mut ancestors);
        }
        reachable.extend(ancestors);

        // Step 3: Mark all children of reachable classes as reachable (optimized)
        // Use a worklist instead of iterating all declarations
        self.mark_children_reachable(graph, &mut reachable);

        // Step 4: DFS from newly reachable nodes
        let mut additional_reachable = HashSet::new();
        for id in &reachable {
            if let Some(start_idx) = graph.node_index(id) {
                let mut dfs = Dfs::new(inner_graph, start_idx);
                while let Some(node_idx) = dfs.next(inner_graph) {
                    if let Some(node_id) = inner_graph.node_weight(node_idx) {
                        additional_reachable.insert(node_id.clone());
                    }
                }
            }
        }
        reachable.extend(additional_reachable);

        // Step 5: Mark children again (for newly discovered reachable classes)
        self.mark_children_reachable(graph, &mut reachable);

        reachable
    }

    /// Mark all children of reachable declarations as reachable (optimized with children_index)
    fn mark_children_reachable(&self, graph: &Graph, reachable: &mut HashSet<DeclarationId>) {
        // Use a worklist approach instead of iterating all declarations
        let mut worklist: Vec<DeclarationId> = reachable.iter().cloned().collect();
        let mut processed: HashSet<DeclarationId> = HashSet::new();

        while let Some(id) = worklist.pop() {
            if processed.contains(&id) {
                continue;
            }
            processed.insert(id.clone());

            // Get children of this declaration using the index
            for child_id in graph.get_children(&id) {
                if !reachable.contains(child_id) {
                    reachable.insert(child_id.clone());
                    worklist.push(child_id.clone());
                }
            }
        }
    }

    /// Collect all ancestor declarations (parent classes, etc.)
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

    /// Check if a declaration should be skipped from dead code reporting
    fn should_skip_declaration(&self, decl: &crate::graph::Declaration, graph: &Graph) -> bool {
        // Skip file-level declarations
        if decl.kind == DeclarationKind::File || decl.kind == DeclarationKind::Package {
            return true;
        }

        // Skip private/internal members of unreachable classes
        // (they should be reported at the class level, not individually)
        if let Some(parent_id) = &decl.parent {
            if let Some(parent) = graph.get_declaration(parent_id) {
                // If parent is a class/object and also unreferenced,
                // skip the member (parent will be reported instead)
                if parent.kind.is_type() && !graph.is_referenced(parent_id) {
                    return true;
                }
            }
        }

        // Skip constructors of unreachable classes
        if decl.kind == DeclarationKind::Constructor {
            if let Some(parent_id) = &decl.parent {
                if !graph.is_referenced(parent_id) {
                    return true;
                }
            }
        }

        // Skip overridden methods (they might be called via interface/base class)
        // Check both Java-style @Override annotation and Kotlin override modifier
        if decl.annotations.iter().any(|a| a.contains("Override")) {
            return true;
        }
        if decl.modifiers.iter().any(|m| m == "override") {
            return true;
        }

        false
    }

    /// Determine the specific issue type for a dead code declaration
    fn determine_issue_type(&self, decl: &crate::graph::Declaration) -> DeadCodeIssue {
        match decl.kind {
            DeclarationKind::Import => DeadCodeIssue::UnusedImport,
            DeclarationKind::Parameter => DeadCodeIssue::UnusedParameter,
            DeclarationKind::EnumCase => DeadCodeIssue::UnusedEnumCase,
            _ => DeadCodeIssue::Unreferenced,
        }
    }
}

impl Default for ReachabilityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_creation() {
        let analyzer = ReachabilityAnalyzer::new();
        let graph = Graph::new();
        let entry_points = HashSet::new();

        let dead_code = analyzer.find_unreachable(&graph, &entry_points);
        assert!(dead_code.is_empty());
    }
}
