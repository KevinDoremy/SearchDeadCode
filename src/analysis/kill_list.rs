//! Kill-list: given a target symbol, everything that only stays alive through it.
//!
//! Algorithm: forward closure of the target (with its members) minus what is
//! still reachable from the entry points when traversal may not pass through
//! the target. What remains falls with the target.

use crate::graph::{DeclarationId, Graph};
use std::collections::{HashSet, VecDeque};

/// Declarations that fall if the targets are deleted.
pub fn kill_list(
    graph: &Graph,
    entry_points: &HashSet<DeclarationId>,
    targets: &HashSet<DeclarationId>,
) -> Vec<DeclarationId> {
    let targets = with_members(graph, targets);
    let closure = forward_closure(graph, &targets);
    let alive_without = reachable_avoiding(graph, entry_points, &targets);

    let mut result: Vec<DeclarationId> = closure
        .into_iter()
        .filter(|id| !alive_without.contains(id))
        .collect();
    result.sort_by(|a, b| a.file.cmp(&b.file).then(a.start.cmp(&b.start)));
    result
}

/// Expand a set of declarations with every declaration nested inside them
fn with_members(graph: &Graph, targets: &HashSet<DeclarationId>) -> HashSet<DeclarationId> {
    let mut set = targets.clone();
    loop {
        let mut added = false;
        for decl in graph.declarations() {
            if set.contains(&decl.id) {
                continue;
            }
            if let Some(parent) = &decl.parent {
                if set.contains(parent) {
                    set.insert(decl.id.clone());
                    added = true;
                }
            }
        }
        if !added {
            break;
        }
    }
    set
}

/// Everything transitively referenced from the start set (start set included)
fn forward_closure(graph: &Graph, start: &HashSet<DeclarationId>) -> HashSet<DeclarationId> {
    let mut seen = start.clone();
    let mut queue: VecDeque<DeclarationId> = start.iter().cloned().collect();
    while let Some(id) = queue.pop_front() {
        for (to_decl, _) in graph.get_references_from(&id) {
            if seen.insert(to_decl.id.clone()) {
                queue.push_back(to_decl.id.clone());
            }
        }
    }
    seen
}

/// Reachability from the entry points where traversal never enters `avoid`
fn reachable_avoiding(
    graph: &Graph,
    entry_points: &HashSet<DeclarationId>,
    avoid: &HashSet<DeclarationId>,
) -> HashSet<DeclarationId> {
    let mut seen: HashSet<DeclarationId> = entry_points
        .iter()
        .filter(|id| !avoid.contains(*id))
        .cloned()
        .collect();
    let mut queue: VecDeque<DeclarationId> = seen.iter().cloned().collect();
    while let Some(id) = queue.pop_front() {
        for (to_decl, _) in graph.get_references_from(&id) {
            if avoid.contains(&to_decl.id) {
                continue;
            }
            if seen.insert(to_decl.id.clone()) {
                queue.push_back(to_decl.id.clone());
            }
        }
    }
    seen
}
