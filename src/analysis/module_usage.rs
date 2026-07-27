//! Module usage attribution: who actually uses a shared module.
//!
//! Symbols are grouped under their outermost declaration; each group is
//! classified by its real referencers — unreferenced, internal-only
//! (visibility-narrowing candidate), or used by named consumer directories.
//! Ambiguous simple-name edges are ignored: a guessed match must never
//! invent a consumer.

use crate::graph::{Declaration, DeclarationId, Graph};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

/// Classification of one outermost module symbol
#[derive(Debug)]
pub enum Usage {
    Unreferenced,
    InternalOnly,
    UsedBy(BTreeSet<String>),
}

pub struct SymbolUsage {
    pub id: DeclarationId,
    pub usage: Usage,
}

fn in_module(decl: &Declaration, token: &str) -> bool {
    decl.location.file.to_string_lossy().contains(token)
}

/// First path segment under the analysis root: the consumer's directory
fn consumer_dir(file: &Path, root: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    rel.components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
}

/// Attribute every outermost symbol of the module
pub fn module_usage(graph: &Graph, module_token: &str, root: &Path) -> Vec<SymbolUsage> {
    // Group module declarations under their outermost module ancestor
    let mut groups: HashMap<DeclarationId, Vec<&Declaration>> = HashMap::new();
    for decl in graph.declarations() {
        if !in_module(decl, module_token) {
            continue;
        }
        let mut outermost = decl;
        while let Some(parent_id) = &outermost.parent {
            match graph.get_declaration(parent_id) {
                Some(parent) if in_module(parent, module_token) => outermost = parent,
                _ => break,
            }
        }
        groups.entry(outermost.id.clone()).or_default().push(decl);
    }

    let mut result: Vec<SymbolUsage> = groups
        .into_iter()
        .map(|(outermost_id, members)| {
            let member_ids: HashSet<&DeclarationId> = members.iter().map(|m| &m.id).collect();
            let mut internal = false;
            let mut consumers: BTreeSet<String> = BTreeSet::new();

            for member in &members {
                for (referencer, reference) in graph.get_references_to(&member.id) {
                    if reference.ambiguous {
                        continue; // a guess is not a consumer
                    }
                    if member_ids.contains(&referencer.id) {
                        continue; // self group
                    }
                    if in_module(referencer, module_token) {
                        internal = true;
                    } else if let Some(dir) = consumer_dir(&referencer.location.file, root) {
                        consumers.insert(dir);
                    }
                }
            }

            let usage = if !consumers.is_empty() {
                Usage::UsedBy(consumers)
            } else if internal {
                Usage::InternalOnly
            } else {
                Usage::Unreferenced
            };
            SymbolUsage {
                id: outermost_id,
                usage,
            }
        })
        .collect();

    result.sort_by(|a, b| a.id.file.cmp(&b.id.file).then(a.id.start.cmp(&b.id.start)));
    result
}
