//! Redundant This Detector (DC014)
//!
//! Flags `this.field = value` in Kotlin where nothing shadows `field`:
//! neither a parameter of the enclosing function nor a local declared
//! above the assignment. `this.name = name` is the classic disambiguation
//! and is never reported. Conservative by construction — when the
//! enclosing signature cannot be found, nothing is reported.

use super::{enclosing_declaration, graph_files, Detector};
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::Graph;
use regex::Regex;
use std::fs;
use std::sync::LazyLock;

static THIS_ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"this\.(\w+)\s*=\s*(\w+)\s*$").expect("Invalid this-assignment regex")
});

pub struct RedundantThisDetector;

impl RedundantThisDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RedundantThisDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// True when `name` is shadowed between the enclosing `fun` signature and
/// the assignment: declared as a parameter (`name:`) or a local
/// (`val/var name`). Unknown signature counts as shadowed — stay quiet.
fn is_shadowed(lines: &[&str], assignment_idx: usize, name: &str) -> bool {
    let param_pattern = Regex::new(&format!(r"\b{}\s*:", regex::escape(name))).unwrap();
    let local_pattern = Regex::new(&format!(r"\bva[lr]\s+{}\b", regex::escape(name))).unwrap();
    for idx in (0..assignment_idx).rev() {
        let line = lines[idx];
        if line.contains("fun ") {
            return param_pattern.is_match(line);
        }
        if local_pattern.is_match(line) {
            return true;
        }
    }
    true // no signature found above: not enough context to judge
}

impl Detector for RedundantThisDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();
        for file in graph_files(graph) {
            // map_or, not is_none_or: MSRV is 1.80, is_none_or landed in 1.82
            if file.extension().map_or(true, |e| e != "kt") {
                continue;
            }
            let Ok(content) = fs::read_to_string(file) else {
                continue;
            };
            let lines: Vec<&str> = content.lines().collect();
            for (idx, line) in lines.iter().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with('*') {
                    continue;
                }
                let Some(captures) = THIS_ASSIGNMENT.captures(line) else {
                    continue;
                };
                let (field, rhs) = (&captures[1], &captures[2]);
                if field == rhs {
                    continue; // this.name = name disambiguates, keep it
                }
                if is_shadowed(&lines, idx, field) {
                    continue;
                }
                let line_no = idx + 1;
                let Some(decl) = enclosing_declaration(graph, file, line_no) else {
                    continue;
                };
                let dead = DeadCode::new(decl.clone(), DeadCodeIssue::RedundantThis)
                    .with_message(format!(
                        "'this.{field}' at line {line_no}: nothing shadows '{field}', 'this.' is redundant"
                    ))
                    .with_confidence(Confidence::Medium);
                issues.push(dead);
            }
        }
        issues.sort_by(|a, b| a.message.cmp(&b.message));
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Declaration, DeclarationId, DeclarationKind, Language, Location};
    use std::path::Path;

    fn graph_over(source: &str) -> (Graph, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Main.kt");
        fs::write(&file, source).unwrap();
        let mut graph = Graph::new();
        graph.add_declaration(class_decl(&file, "Account", 1));
        (graph, temp)
    }

    fn class_decl(file: &Path, name: &str, line: usize) -> Declaration {
        Declaration::new(
            DeclarationId::new(file.to_path_buf(), line * 100, line * 100 + 10),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(file.to_path_buf(), line, 1, line * 100, line * 100 + 10),
            Language::Kotlin,
        )
    }

    #[test]
    fn unshadowed_this_is_flagged() {
        let (graph, _tmp) = graph_over(
            "class Account {\n    var balance = 0\n    fun deposit(amount: Int) {\n        this.balance = amount\n    }\n}\n",
        );
        let issues = RedundantThisDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("this.balance"));
    }

    #[test]
    fn parameter_shadowing_keeps_this() {
        let (graph, _tmp) = graph_over(
            "class Account {\n    var balance = 0\n    fun reset(balance: Int) {\n        this.balance = balance\n    }\n}\n",
        );
        let issues = RedundantThisDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn a_local_shadow_keeps_this() {
        let (graph, _tmp) = graph_over(
            "class Account {\n    var balance = 0\n    fun oddReset(amount: Int) {\n        val balance = amount / 2\n        this.balance = amount\n        println(balance)\n    }\n}\n",
        );
        let issues = RedundantThisDetector::new().detect(&graph);
        assert!(issues.is_empty(), "a local named balance shadows the field");
    }

    #[test]
    fn different_param_name_without_shadow_is_flagged() {
        let (graph, _tmp) = graph_over(
            "class Account {\n    var balance = 0\n    fun set(value: Int) {\n        this.balance = value\n    }\n}\n",
        );
        let issues = RedundantThisDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn java_files_are_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Main.java");
        fs::write(
            &file,
            "class Account {\n    void set(int value) {\n        this.balance = value\n    }\n}\n",
        )
        .unwrap();
        let mut graph = Graph::new();
        graph.add_declaration(class_decl(&file, "Account", 1));
        let issues = RedundantThisDetector::new().detect(&graph);
        assert!(issues.is_empty(), "Java conventions favour explicit this");
    }
}
