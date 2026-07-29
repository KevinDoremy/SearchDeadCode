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

/// Partition dead declarations into connected clusters.
///
/// Connectivity runs through reference edges in both directions AND through
/// parent/member links, over the dead set expanded with its members — two dead
/// classes linked by a method call belong to the same cluster even when the
/// method itself is not a separate finding.
pub fn dead_clusters(graph: &Graph, dead: &HashSet<DeclarationId>) -> Vec<Vec<DeclarationId>> {
    let expanded = with_members(graph, dead);

    let mut parent_links: std::collections::HashMap<DeclarationId, Vec<DeclarationId>> =
        std::collections::HashMap::new();
    for decl in graph.declarations() {
        if let Some(parent) = &decl.parent {
            if expanded.contains(&decl.id) && expanded.contains(parent) {
                parent_links
                    .entry(parent.clone())
                    .or_default()
                    .push(decl.id.clone());
                parent_links
                    .entry(decl.id.clone())
                    .or_default()
                    .push(parent.clone());
            }
        }
    }

    let mut visited: HashSet<DeclarationId> = HashSet::new();
    let mut clusters = Vec::new();

    for id in &expanded {
        if visited.contains(id) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([id.clone()]);
        visited.insert(id.clone());

        while let Some(current) = queue.pop_front() {
            component.push(current.clone());

            let referenced: Vec<DeclarationId> = graph
                .get_references_from(&current)
                .into_iter()
                .map(|(d, _)| d.id.clone())
                .chain(
                    graph
                        .get_references_to(&current)
                        .into_iter()
                        .map(|(d, _)| d.id.clone()),
                )
                .chain(parent_links.get(&current).cloned().unwrap_or_default())
                .collect();

            for neighbor in referenced {
                if expanded.contains(&neighbor) && visited.insert(neighbor.clone()) {
                    queue.push_back(neighbor);
                }
            }
        }
        component.sort_by(|a, b| a.file.cmp(&b.file).then(a.start.cmp(&b.start)));
        clusters.push(component);
    }

    clusters
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

/// Everything transitively referenced from the start set (start set included).
/// Les arêtes ambiguës (résolution par nom simple, plusieurs candidats) sont
/// ignorées : une devinette par homonymie ne condamne pas un symbole d'un
/// autre module. Sous-approximer la fermeture est le côté sûr — l'autre sens
/// (reachable_avoiding) les suit toujours, ce qui garde vivant.
fn forward_closure(graph: &Graph, start: &HashSet<DeclarationId>) -> HashSet<DeclarationId> {
    let mut seen = start.clone();
    let mut queue: VecDeque<DeclarationId> = start.iter().cloned().collect();
    while let Some(id) = queue.pop_front() {
        for (to_decl, reference) in graph.get_references_from(&id) {
            if reference.ambiguous {
                continue;
            }
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
