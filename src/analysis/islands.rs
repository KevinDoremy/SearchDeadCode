//! Dead islands: groups of declarations that reference only each other and
//! are referenced by nothing else — the mutual pair, the chain behind a dead
//! entry point, the interface plus its only implementation nobody constructs.
//!
//! `is_referenced` cannot see them: each member HAS an incoming edge, from
//! its dead sibling. Reachability-from-blessed-roots overshoots the other
//! way: one over-broad root resurrects its whole closure. The island model
//! (docs/dead-islands-algorithm.md) fixes the error direction instead:
//! everything that cannot be PLACED is a root, and a root means life —
//! entry points, annotated declarations, names in string literals or any
//! non-code file, targets of ambiguous edges. Liveness is the least fixpoint
//! from those roots; deadness is its complement (the co-induction that kills
//! mutual recursion); islands are the connected components of the dead set,
//! computed by the same `dead_clusters` the kill-list already uses.
//!
//! Every approximation error makes code live, never dead. Measured on the
//! reference corpus by the Kotlin Jump implementation of this model:
//! 22 islands, each hand-verified, the fixable ones deleted with a green
//! build matrix.

use crate::analysis::kill_list::dead_clusters;
use crate::analysis::test_refs::is_test_file;
use crate::discovery::{FileType, SourceFile};
use crate::graph::{DeclarationId, DeclarationKind, Graph, ReferenceKind};
use regex::Regex;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::sync::LazyLock;

static STRING_LITERAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""([^"\\]|\\.)*""#).expect("Invalid string literal regex"));
static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("Invalid word regex"));

/// Annotations that do not, by themselves, imply a runtime caller. Anything
/// else roots its declaration: the framework or processor that consumes the
/// annotation is a caller the graph cannot see.
const BENIGN_ANNOTATIONS: &[&str] = &[
    // An uncalled composable is dead code like any other function — the
    // same stance entry_points takes by leaving Composable out of its list.
    "Composable",
    "Override",
    "Deprecated",
    "Suppress",
    "SuppressWarnings",
    "SuppressLint",
    "Nullable",
    "NonNull",
    "JvmStatic",
    "JvmField",
    "JvmOverloads",
    "Throws",
];

pub struct IslandMember {
    pub id: DeclarationId,
    pub name: String,
    pub file: std::path::PathBuf,
    pub line: usize,
    /// Dead declarations whose references are this member's only mentions.
    pub kept_alive_by: Vec<String>,
}

pub struct DeadIsland {
    /// Outermost members (a member whose parent is in the island is folded
    /// into it — deleting the parent deletes the child).
    pub members: Vec<IslandMember>,
    pub total_declarations: usize,
    pub estimated_lines: usize,
    /// Some member is referenced from a test source set: the island is dead
    /// in production but deleting it means deleting its tests — a human call.
    pub test_only: bool,
    /// Some member matches a wildcard -keep rule: the rule keeps runtime
    /// bytes for the shrinker, not the source — retire it with the island.
    pub keep_covered: bool,
}

/// Islands over the whole graph. `max_size` bounds the OUTERMOST member
/// count: a bigger component is more likely an analysis gap than a corpse,
/// so it is withheld whole, never partially reported.
pub fn find_islands(
    graph: &Graph,
    entry_points: &HashSet<DeclarationId>,
    files: &[SourceFile],
    root: &std::path::Path,
    max_size: usize,
) -> Vec<DeadIsland> {
    // A -keep that spells an EXACT name is a reflection signal: root it.
    // A wildcard blanket ("**.DO.**") proves nothing about the source —
    // its matches are reported with a retire-the-rule label instead.
    let keep_specs = crate::analysis::keep_rules::collect_keep_specs(root);
    let exact_keep_names: HashSet<String> = keep_specs
        .iter()
        .filter(|s| !s.contains('*') && !s.contains('?'))
        .map(|s| s.rsplit('.').next().unwrap_or(s).to_string())
        .collect();
    let wildcard_keeps = crate::analysis::keep_rules::collect_keep_patterns(root);
    // ── Population. Test declarations are never members: their references
    // mark islands testOnly instead of making them alive. Enum cases stay
    // out too — iteration (`values()`, map `.entries`) reaches them without
    // an edge, and DC005 owns them with the guards that know that.
    // A foreign annotation or an override marks a declaration the guards
    // must SAVE — and a saved node leaves the population rather than rooting
    // it: its contents root through the source-outside-population rule, but
    // it vivifies neither its parent nor its class. (Rooting it instead let
    // a member's @MainThread resurrect its whole island.)
    let guarded = |d: &&crate::graph::Declaration| -> bool {
        d.annotations
            .iter()
            .any(|a| !BENIGN_ANNOTATIONS.iter().any(|b| a.contains(b)))
            // Java stores "@Override" in modifiers; Kotlin stores "override".
            || d.modifiers
                .iter()
                .any(|m| m.trim_start_matches('@').eq_ignore_ascii_case("override"))
            // KJ's M3: a Java method under a class with any supertype may
            // implement an interface the corpus never declares (onPreDraw
            // under ViewTreeObserver.OnPreDrawListener) — @Override being
            // optional, only supertype-free classes keep their methods in
            // the population. Accepted false negative, named in the docs.
            || (d.language == crate::graph::Language::Java
                && d.kind == DeclarationKind::Method
                && d.parent.as_ref().is_some_and(|p| {
                    graph
                        .get_declaration(p)
                        .is_some_and(|parent| !parent.super_types.is_empty())
                }))
    };
    let eligible: HashSet<&DeclarationId> = graph
        .declarations()
        .filter(|d| {
            !matches!(
                d.kind,
                DeclarationKind::File
                    | DeclarationKind::Package
                    | DeclarationKind::Import
                    | DeclarationKind::EnumCase
            )
        })
        .filter(|d| !is_testish(&d.location.file))
        .filter(|d| !guarded(d))
        .map(|d| &d.id)
        .collect();

    // ── Non-attributable mentions root by name; an ATTRIBUTED string
    // literal (one lying inside an eligible declaration's own byte extent)
    // becomes an edge instead: `X = "X"` and a class logging its own name
    // must not resurrect themselves. Tokens of non-code files always root —
    // nothing there has an extent. Tool artifacts (lint baselines, R8
    // outputs) are suppressions, not callers: skipped entirely.
    let known_names: HashSet<&str> = graph
        .declarations()
        .filter(|d| eligible.contains(&d.id))
        .map(|d| d.name.as_str())
        .collect();
    let mut extents_by_file: std::collections::HashMap<
        &std::path::Path,
        Vec<(usize, usize, &DeclarationId)>,
    > = std::collections::HashMap::new();
    for decl in graph.declarations() {
        if eligible.contains(&decl.id) {
            extents_by_file
                .entry(decl.location.file.as_path())
                .or_default()
                .push((decl.id.start, decl.id.end, &decl.id));
        }
    }
    for spans in extents_by_file.values_mut() {
        spans.sort_by_key(|(start, _, _)| *start);
    }

    let mut rooted_names: HashSet<String> = HashSet::new();
    let mut literal_edges: std::collections::HashMap<DeclarationId, HashSet<String>> =
        std::collections::HashMap::new();
    for file in files {
        let name = file
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase());
        if name.as_deref().is_some_and(|n| n.contains("baseline")) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file.path) else {
            continue;
        };
        match file.file_type {
            FileType::Kotlin | FileType::Java => {
                // A test's string literal is a test mention, not production
                // life: it must never root an island (it may only mark it
                // testOnly through its graph edges).
                if is_testish(&file.path) {
                    continue;
                }
                let spans = extents_by_file.get(file.path.as_path());
                for m in STRING_LITERAL.find_iter(&content) {
                    // Innermost eligible extent covering this literal, if any.
                    let owner = spans.and_then(|spans| {
                        spans
                            .iter()
                            .filter(|(start, end, _)| *start <= m.start() && m.end() <= *end)
                            .max_by_key(|(start, _, _)| *start)
                            .map(|(_, _, id)| (*id).clone())
                    });
                    for word in WORD.find_iter(m.as_str()) {
                        if !known_names.contains(word.as_str()) {
                            continue;
                        }
                        match &owner {
                            Some(id) => {
                                literal_edges
                                    .entry(id.clone())
                                    .or_default()
                                    .insert(word.as_str().to_string());
                            }
                            None => {
                                rooted_names.insert(word.as_str().to_string());
                            }
                        }
                    }
                }
            }
            // XML, Gradle, ProGuard, properties, TOML, manifests: nothing
            // here has an extent in the pool, so every token is a root.
            _ => {
                for word in WORD.find_iter(&content) {
                    rooted_names.insert(word.as_str().to_string());
                }
            }
        }
    }
    let mut ids_by_name: std::collections::HashMap<&str, Vec<&DeclarationId>> =
        std::collections::HashMap::new();
    for decl in graph.declarations() {
        if eligible.contains(&decl.id) {
            ids_by_name
                .entry(decl.name.as_str())
                .or_default()
                .push(&decl.id);
        }
    }

    // ── Roots: everything with a caller the graph cannot see. Each rule
    // errs toward life.
    let mut alive: HashSet<DeclarationId> = HashSet::new();
    let mut queue: VecDeque<DeclarationId> = VecDeque::new();
    let mut deferred_edges: std::collections::HashMap<DeclarationId, Vec<DeclarationId>> =
        std::collections::HashMap::new();
    let seed = |id: &DeclarationId,
                alive: &mut HashSet<DeclarationId>,
                queue: &mut VecDeque<DeclarationId>| {
        if alive.insert(id.clone()) {
            queue.push_back(id.clone());
        }
    };

    for decl in graph.declarations() {
        if !eligible.contains(&decl.id) {
            continue;
        }
        let is_root = entry_points.contains(&decl.id)
            || rooted_names.contains(&decl.name)
            || exact_keep_names.contains(&decl.name)
            // A declaration under a @Module class is DI convention: the
            // processor generates factories from its @Provides members into
            // build/, and a companion's own name never appears in source.
            || decl.parent.as_ref().is_some_and(|p| {
                graph.get_declaration(p).is_some_and(|parent| {
                    parent.annotations.iter().any(|a| a.contains("Module"))
                })
            });
        if is_root {
            seed(&decl.id, &mut alive, &mut queue);
        }
        // A reference from OUTSIDE the population attributes to the source's
        // nearest ELIGIBLE ancestor (a guarded override's body belongs to
        // its class, exactly as an extent-less member belongs to its
        // enclosing extent); only a source with no eligible ancestor roots
        // the target. Ambiguous edges are ordinary edges here: followed for
        // life during propagation, never a verdict by themselves. Imports
        // never keep a symbol alive, and test references only mark the
        // verdict.
        for (source, reference) in graph.get_references_to(&decl.id) {
            if reference.kind == ReferenceKind::Import {
                continue;
            }
            if is_testish(&source.location.file) {
                continue; // tallied later as testOnly
            }
            if eligible.contains(&source.id) {
                continue; // an ordinary edge, handled by propagation
            }
            let mut ancestor = source.parent.clone();
            let mut attributed = false;
            while let Some(parent_id) = ancestor {
                if eligible.contains(&parent_id) {
                    deferred_edges
                        .entry(parent_id.clone())
                        .or_default()
                        .push(decl.id.clone());
                    attributed = true;
                    break;
                }
                ancestor = graph
                    .get_declaration(&parent_id)
                    .and_then(|d| d.parent.clone());
            }
            if !attributed {
                seed(&decl.id, &mut alive, &mut queue);
                break;
            }
        }
    }

    // ── Liveness: least fixpoint. A live declaration vivifies what it
    // references, and needs its ancestors to compile.
    while let Some(id) = queue.pop_front() {
        for (target, reference) in graph.get_references_from(&id) {
            if reference.kind == ReferenceKind::Import {
                continue;
            }
            seed(&target.id, &mut alive, &mut queue);
        }
        // What a guarded child mentioned belongs to its eligible ancestor:
        // the ancestor's life reaches it.
        if let Some(targets) = deferred_edges.get(&id) {
            for target in targets.clone() {
                seed(&target, &mut alive, &mut queue);
            }
        }
        // A live owner's string literals reach every bearer of the names
        // they spell (reflection and serialization live there).
        if let Some(names) = literal_edges.get(&id) {
            for name in names {
                if let Some(ids) = ids_by_name.get(name.as_str()) {
                    for target in ids {
                        seed(target, &mut alive, &mut queue);
                    }
                }
            }
        }
        if let Some(decl) = graph.get_declaration(&id) {
            if let Some(parent) = &decl.parent {
                seed(parent, &mut alive, &mut queue);
            }
        }
    }

    // ── Dead complement, then components via the kill-list machinery.
    let dead: HashSet<DeclarationId> = eligible
        .iter()
        .filter(|id| !alive.contains(**id))
        .map(|id| (*id).clone())
        .collect();
    if dead.is_empty() {
        return Vec::new();
    }

    let mut islands = Vec::new();
    for cluster in dead_clusters(graph, &dead) {
        let in_cluster: HashSet<&DeclarationId> = cluster.iter().collect();
        // Outermost members: a child folds into its clustered parent.
        let outermost: Vec<&DeclarationId> = cluster
            .iter()
            .filter(|id| {
                graph
                    .get_declaration(id)
                    .and_then(|d| d.parent.as_ref())
                    .map_or(true, |p| !in_cluster.contains(p))
            })
            .collect();
        // A single declaration is the per-symbol detectors' finding, not an
        // island: one finding per root cause.
        if outermost.len() < 2 {
            continue;
        }
        if outermost.len() > max_size {
            continue;
        }

        let test_only = cluster.iter().any(|id| {
            graph
                .get_references_to(id)
                .iter()
                .any(|(source, _)| is_testish(&source.location.file))
        });

        let mut estimated_lines = 0usize;
        let mut members = Vec::new();
        for id in &outermost {
            let Some(decl) = graph.get_declaration(id) else {
                continue;
            };
            if let Ok(bytes) = fs::read(&decl.location.file) {
                let start = id.start.min(bytes.len());
                let end = id.end.min(bytes.len());
                estimated_lines += bytes[start..end].iter().filter(|b| **b == b'\n').count() + 1;
            }
            let kept_alive_by: Vec<String> = {
                // A holder sharing this member's name is named by its file
                // instead of being dropped: filtering same-name holders left
                // every homonym island with no reason line at all, which is
                // exactly the island the reader most needs explained. Only a
                // holder that IS this declaration is silent.
                let mut names: Vec<String> = graph
                    .get_references_to(id)
                    .iter()
                    .filter(|(source, reference)| {
                        reference.kind != ReferenceKind::Import
                            && dead.contains(&source.id)
                            && &source.id != *id
                    })
                    .map(|(source, _)| {
                        if source.name == decl.name {
                            let file = source
                                .location
                                .file
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| source.name.clone());
                            format!("{} ({}:{})", source.name, file, source.location.line)
                        } else {
                            source.name.clone()
                        }
                    })
                    .collect();
                names.sort();
                names.dedup();
                names
            };
            members.push(IslandMember {
                id: (*id).clone(),
                name: decl.name.clone(),
                file: decl.location.file.clone(),
                line: decl.location.line,
                kept_alive_by,
            });
        }
        members.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

        let keep_covered = cluster.iter().any(|id| {
            graph
                .get_declaration(id)
                .and_then(|d| d.fully_qualified_name.as_ref())
                .is_some_and(|fqn| wildcard_keeps.iter().any(|p| p.matches(fqn)))
        });

        islands.push(DeadIsland {
            members,
            total_declarations: cluster.len(),
            estimated_lines,
            test_only,
            keep_covered,
        });
    }

    islands.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then(b.estimated_lines.cmp(&a.estimated_lines))
    });
    islands
}

/// Wider than `test_refs::is_test_file`: any `src/<set>` whose set name
/// contains `test` (savedAndroidTest, testShared, ...) is test territory —
/// a parked test source set must not contribute island members.
fn is_testish(path: &std::path::Path) -> bool {
    if is_test_file(path) {
        return true;
    }
    let mut components = path.components().peekable();
    while let Some(c) = components.next() {
        if c.as_os_str() == "src" {
            if let Some(next) = components.peek() {
                let name = next.as_os_str().to_string_lossy();
                if name.to_lowercase().contains("test") {
                    return true;
                }
            }
        }
    }
    false
}

/// A short reason chain for one member, for the report.
pub fn chain_of(member: &IslandMember) -> Option<String> {
    if member.kept_alive_by.is_empty() {
        return None;
    }
    Some(format!(
        "'{}' kept alive only by {} — themselves dead",
        member.name,
        member.kept_alive_by.join(", ")
    ))
}
