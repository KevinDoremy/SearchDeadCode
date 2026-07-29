//! Migration diff: old world vs new world.
//!
//! During a v1/v2 migration two implementations coexist. The question that
//! matters is "what can I delete the day the switch flips?": every old-world
//! symbol that no outside code references anymore. Old-world symbols still
//! referenced from outside are the blockers, listed with their referencers.

use crate::graph::{Declaration, DeclarationId, Graph};
use std::collections::HashMap;

/// One outermost old-world entry and, when blocked, an outside referencer
#[derive(Debug)]
pub struct MigrationEntry {
    pub id: DeclarationId,
    /// An outside declaration that still references this group, if any
    pub blocked_by: Option<DeclarationId>,
}

/// Diff of the old world against the rest of the codebase
#[derive(Debug, Default)]
pub struct MigrationReport {
    /// Old-world groups nobody outside references: deletable at the flip
    pub deletable: Vec<MigrationEntry>,
    /// Old-world groups still referenced from outside
    pub blockers: Vec<MigrationEntry>,
}

/// Does a declaration belong to a world named by `token`?
/// Matches on the FQN prefix or on the file path containing the token.
fn in_world(decl: &Declaration, token: &str) -> bool {
    if let Some(fqn) = &decl.fully_qualified_name {
        if fqn.starts_with(token) {
            return true;
        }
    }
    decl.location.file.to_string_lossy().contains(token)
}

/// Compare the old world against everything else.
///
/// `new_token` doit être exclu explicitement : quand le vieux monde est un
/// préfixe de chaîne du nouveau (main / mainV2 — le cas typique d'une
/// migration), `contains(old)` avale tout le nouveau monde. Les internes du
/// nouveau monde sortaient « deletable », et un symbole du vieux monde
/// référencé PAR le nouveau était classé deletable (référence « interne »)
/// alors que sa suppression casserait le nouveau monde.
pub fn compare(graph: &Graph, old_token: &str, new_token: &str) -> MigrationReport {
    let in_old_world = |decl: &Declaration| -> bool {
        in_world(decl, old_token) && (new_token.is_empty() || !in_world(decl, new_token))
    };

    // Group old-world declarations under their outermost old-world ancestor
    let mut groups: HashMap<DeclarationId, Vec<&Declaration>> = HashMap::new();
    for decl in graph.declarations() {
        if !in_old_world(decl) {
            continue;
        }
        let mut outermost = decl;
        while let Some(parent_id) = &outermost.parent {
            match graph.get_declaration(parent_id) {
                Some(parent) if in_old_world(parent) => outermost = parent,
                _ => break,
            }
        }
        groups.entry(outermost.id.clone()).or_default().push(decl);
    }

    let mut report = MigrationReport::default();
    for (outermost_id, members) in groups {
        let mut blocked_by = None;
        'members: for member in &members {
            for (referencer, reference) in graph.get_references_to(&member.id) {
                // Ambiguous simple-name matches are guesses, and v2 worlds
                // mirror v1 names by construction: only certain references
                // count as blockers
                if reference.ambiguous {
                    continue;
                }
                if !in_old_world(referencer) {
                    blocked_by = Some(referencer.id.clone());
                    break 'members;
                }
            }
        }
        let entry = MigrationEntry {
            id: outermost_id,
            blocked_by,
        };
        if entry.blocked_by.is_some() {
            report.blockers.push(entry);
        } else {
            report.deletable.push(entry);
        }
    }

    report
        .deletable
        .sort_by(|a, b| a.id.file.cmp(&b.id.file).then(a.id.start.cmp(&b.id.start)));
    report
        .blockers
        .sort_by(|a, b| a.id.file.cmp(&b.id.file).then(a.id.start.cmp(&b.id.start)));
    report
}
