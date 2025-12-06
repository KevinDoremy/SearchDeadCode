// Cycle detector - finds "zombie code" that only references itself
//
// Zombie code is code that forms a closed cycle with no external entry points.
// For example:
// - Class A uses Class B
// - Class B uses Class A
// - Neither A nor B is used by anything else
//
// This is inspired by Meta's SCARF system which detects mutually dependent dead code.

use crate::graph::{DeclarationId, DeclarationKind, Graph};
use petgraph::algo::tarjan_scc;
use std::collections::HashSet;
use tracing::debug;

/// Result of cycle detection
#[derive(Debug, Clone)]
pub struct CycleInfo {
    /// IDs of declarations in this cycle
    pub members: Vec<DeclarationId>,
    /// Human-readable names
    pub names: Vec<String>,
    /// Whether this cycle is entirely dead (no external references)
    pub is_dead_cycle: bool,
    /// Size of the cycle
    pub size: usize,
}

/// Detector for zombie/cycle code
pub struct CycleDetector;

impl CycleDetector {
    pub fn new() -> Self {
        Self
    }

    /// Find all strongly connected components (cycles) that are not reachable
    /// from any entry points.
    ///
    /// Returns cycles sorted by size (largest first) - larger cycles are more
    /// impactful to clean up.
    pub fn find_dead_cycles(
        &self,
        graph: &Graph,
        reachable: &HashSet<DeclarationId>,
    ) -> Vec<CycleInfo> {
        // Use Tarjan's algorithm to find strongly connected components
        let inner = graph.inner();
        let sccs = tarjan_scc(inner);

        let mut dead_cycles = Vec::new();

        for scc in sccs {
            // Skip single-node SCCs (not really cycles, unless self-referential)
            if scc.len() < 2 {
                continue;
            }

            // Get declaration IDs for this SCC
            let member_ids: Vec<DeclarationId> = scc
                .iter()
                .filter_map(|&idx| inner.node_weight(idx).cloned())
                .collect();

            // Check if ANY member is reachable from entry points
            let any_reachable = member_ids.iter().any(|id| reachable.contains(id));

            if any_reachable {
                // This cycle is reachable - not dead
                continue;
            }

            // Check if this cycle has any external incoming edges
            let member_set: HashSet<_> = member_ids.iter().collect();
            let has_external_reference = self.has_external_incoming_edge(graph, &member_set);

            if has_external_reference {
                // Something outside the cycle references it
                continue;
            }

            // This is a dead cycle!
            let names: Vec<String> = member_ids
                .iter()
                .filter_map(|id| graph.get_declaration(id))
                .filter(|decl| {
                    // Only include significant declarations
                    matches!(
                        decl.kind,
                        DeclarationKind::Class
                            | DeclarationKind::Interface
                            | DeclarationKind::Object
                            | DeclarationKind::Function
                            | DeclarationKind::Method
                    )
                })
                .map(|decl| format!("{} '{}'", decl.kind.display_name(), decl.name))
                .collect();

            if names.is_empty() {
                continue; // Skip cycles with no significant declarations
            }

            debug!("Found dead cycle with {} members: {:?}", names.len(), names);

            dead_cycles.push(CycleInfo {
                members: member_ids,
                names,
                is_dead_cycle: true,
                size: scc.len(),
            });
        }

        // Sort by size (largest first)
        dead_cycles.sort_by(|a, b| b.size.cmp(&a.size));

        dead_cycles
    }

    /// Check if any declaration outside the given set references a member of the set
    fn has_external_incoming_edge(&self, graph: &Graph, members: &HashSet<&DeclarationId>) -> bool {
        for member_id in members {
            let refs = graph.get_references_to(member_id);
            for (ref_decl, _) in refs {
                if !members.contains(&ref_decl.id) {
                    // External reference found
                    return true;
                }
            }
        }
        false
    }

    /// Find potential zombie code - declarations that only reference each other
    /// without being part of a proper cycle (for smaller mutual references)
    pub fn find_zombie_pairs(
        &self,
        graph: &Graph,
        reachable: &HashSet<DeclarationId>,
    ) -> Vec<(DeclarationId, DeclarationId)> {
        let mut zombie_pairs = Vec::new();

        // Look for pairs A -> B where both are unreachable
        // and B -> A also exists
        for decl in graph.declarations() {
            if reachable.contains(&decl.id) {
                continue;
            }

            let refs_from = graph.get_references_from(&decl.id);
            for (target_decl, _) in refs_from {
                if reachable.contains(&target_decl.id) {
                    continue;
                }

                // Check if target references back
                let refs_back = graph.get_references_from(&target_decl.id);
                let references_back = refs_back.iter().any(|(d, _)| d.id == decl.id);

                if references_back {
                    // Avoid duplicates by comparing string representations
                    let id_a = decl.id.to_string();
                    let id_b = target_decl.id.to_string();
                    if id_a < id_b {
                        zombie_pairs.push((decl.id.clone(), target_decl.id.clone()));
                    }
                }
            }
        }

        zombie_pairs
    }

    /// Get statistics about cycles in the codebase
    pub fn get_cycle_stats(&self, graph: &Graph, reachable: &HashSet<DeclarationId>) -> CycleStats {
        let dead_cycles = self.find_dead_cycles(graph, reachable);
        let zombie_pairs = self.find_zombie_pairs(graph, reachable);

        let total_declarations_in_cycles: usize = dead_cycles.iter().map(|c| c.size).sum();

        CycleStats {
            num_dead_cycles: dead_cycles.len(),
            largest_cycle_size: dead_cycles.first().map(|c| c.size).unwrap_or(0),
            total_declarations_in_cycles,
            num_zombie_pairs: zombie_pairs.len(),
        }
    }
}

impl Default for CycleDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about cycles
#[derive(Debug, Clone)]
pub struct CycleStats {
    pub num_dead_cycles: usize,
    pub largest_cycle_size: usize,
    pub total_declarations_in_cycles: usize,
    pub num_zombie_pairs: usize,
}

impl CycleStats {
    pub fn has_cycles(&self) -> bool {
        self.num_dead_cycles > 0 || self.num_zombie_pairs > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cycle_detector_creation() {
        let detector = CycleDetector::new();
        let graph = Graph::new();
        let reachable = HashSet::new();

        let cycles = detector.find_dead_cycles(&graph, &reachable);
        assert!(cycles.is_empty());
    }
}
