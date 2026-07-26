//! Flag cleanup: what dies when a feature flag is settled?
//!
//! Every `if` whose condition mentions the flag splits the code into a
//! winning and a losing branch. Symbols referenced by losing branches and
//! nowhere else are dead the day the flag is burned in.

use crate::discovery::{FileType, SourceFile};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use tree_sitter::{Node, Parser};

/// Occurrence ranges of losing branches, per file (byte ranges)
type LosingRanges = HashMap<std::path::PathBuf, Vec<(usize, usize)>>;

/// Result of the flag analysis
#[derive(Debug, Default)]
pub struct FlagReport {
    /// Symbols only reachable through losing branches
    pub dead_symbols: Vec<String>,
    /// Number of gate sites found for the flag
    pub gate_count: usize,
}

pub fn dead_under_flag(files: &[SourceFile], flag: &str, enabled: bool) -> FlagReport {
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_kotlin::language())
        .is_err()
    {
        return FlagReport::default();
    }

    let mut losing_names: HashSet<String> = HashSet::new();
    let mut winning_names: HashSet<String> = HashSet::new();
    let mut losing_ranges: LosingRanges = HashMap::new();
    let mut gate_count = 0usize;

    let sources: Vec<(&SourceFile, String)> = files
        .iter()
        .filter(|f| matches!(f.file_type, FileType::Kotlin))
        .filter_map(|f| fs::read_to_string(&f.path).ok().map(|c| (f, c)))
        .collect();

    for (file, content) in &sources {
        let Some(tree) = parser.parse(content, None) else {
            continue;
        };
        walk_ifs(tree.root_node(), content, flag, enabled, &mut |site| {
            gate_count += 1;
            collect_identifiers(site.losing, content, &mut losing_names);
            if let Some(winning) = site.winning {
                collect_identifiers(winning, content, &mut winning_names);
            }
            losing_ranges
                .entry(file.path.clone())
                .or_default()
                .push((site.losing.start_byte(), site.losing.end_byte()));
        });
    }

    // A candidate dies only if every project-wide occurrence of its name is
    // inside a losing branch or on its own declaration line
    let mut dead: Vec<String> = losing_names
        .difference(&winning_names)
        .filter(|name| only_used_in_losing_branches(name, &sources, &losing_ranges))
        .cloned()
        .collect();
    dead.sort();

    FlagReport {
        dead_symbols: dead,
        gate_count,
    }
}

struct GateSite<'a> {
    losing: Node<'a>,
    winning: Option<Node<'a>>,
}

/// Walk the tree; for each if-expression gated on the flag, hand the losing
/// and winning branches to the callback.
fn walk_ifs<'a>(
    node: Node<'a>,
    content: &str,
    flag: &str,
    enabled: bool,
    on_gate: &mut impl FnMut(GateSite<'a>),
) {
    if node.kind() == "if_expression" {
        let mut condition_mentions_flag = false;
        let mut branches: Vec<Node> = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "control_structure_body" => branches.push(child),
                "if_expression" => branches.push(child), // else-if chain
                _ => {
                    if child.kind() != "else" && branches.is_empty() {
                        let text = &content[child.byte_range()];
                        if text.contains(flag) {
                            condition_mentions_flag = true;
                        }
                    }
                }
            }
        }

        if condition_mentions_flag && !branches.is_empty() {
            let then_branch = branches[0];
            let else_branch = branches.get(1).copied();
            let (losing, winning) = if enabled {
                match else_branch {
                    Some(e) => (Some(e), Some(then_branch)),
                    None => (None, Some(then_branch)),
                }
            } else {
                (Some(then_branch), else_branch)
            };
            if let Some(losing) = losing {
                on_gate(GateSite { losing, winning });
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_ifs(child, content, flag, enabled, on_gate);
    }
}

/// Collect capitalized identifiers (type usages) in a subtree
fn collect_identifiers(node: Node, content: &str, out: &mut HashSet<String>) {
    if node.kind() == "simple_identifier" {
        let text = &content[node.byte_range()];
        if text.chars().next().is_some_and(|c| c.is_uppercase()) {
            out.insert(text.to_string());
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identifiers(child, content, out);
    }
}

/// Every occurrence of `name` must be in a losing range or a declaration line
fn only_used_in_losing_branches(
    name: &str,
    sources: &[(&SourceFile, String)],
    losing_ranges: &LosingRanges,
) -> bool {
    let Ok(word) = Regex::new(&format!(r"\b{}\b", regex::escape(name))) else {
        return false;
    };
    let declaration = Regex::new(&format!(
        r"\b(class|interface|object|fun|val|var|enum class)\s+{}\b",
        regex::escape(name)
    ))
    .ok();

    for (file, content) in sources {
        let ranges = losing_ranges.get(&file.path);
        for m in word.find_iter(content) {
            let in_losing = ranges
                .map(|rs| rs.iter().any(|(s, e)| m.start() >= *s && m.end() <= *e))
                .unwrap_or(false);
            if in_losing {
                continue;
            }
            let line_start = content[..m.start()].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_end = content[m.start()..]
                .find('\n')
                .map(|i| m.start() + i)
                .unwrap_or(content.len());
            let line = &content[line_start..line_end];
            let is_declaration = declaration
                .as_ref()
                .map(|d| d.is_match(line))
                .unwrap_or(false);
            if !is_declaration {
                return false;
            }
        }
    }
    true
}
