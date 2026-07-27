//! Dead Branch Detector (DC007)
//!
//! Flags branches gated on a literal `false` — the only kind of deadness
//! provable from text alone. Runtime conditions (`if (debug)`) and `false`
//! literals outside a condition are never reported. Each finding is
//! attached to the innermost enclosing declaration in the same file.

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};
use regex::Regex;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

static LITERAL_FALSE_CONDITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:if|while)\s*\(\s*false\s*\)").expect("Invalid dead branch regex")
});

pub struct DeadBranchDetector;

impl DeadBranchDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeadBranchDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for DeadBranchDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let files: BTreeSet<&Path> = graph
            .declarations()
            .map(|d| d.location.file.as_path())
            .collect();

        let mut issues = Vec::new();
        for file in files {
            let Ok(content) = fs::read_to_string(file) else {
                continue;
            };
            for (idx, line) in content.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with('*') {
                    continue;
                }
                if !LITERAL_FALSE_CONDITION.is_match(line) {
                    continue;
                }
                let line_no = idx + 1;
                let enclosing = graph
                    .declarations()
                    .filter(|d| d.location.file.as_path() == file && d.location.line <= line_no)
                    .filter(|d| {
                        matches!(
                            d.kind,
                            DeclarationKind::Function
                                | DeclarationKind::Method
                                | DeclarationKind::Constructor
                                | DeclarationKind::Class
                                | DeclarationKind::Object
                        )
                    })
                    .max_by_key(|d| d.location.line);
                let Some(decl) = enclosing else { continue };
                let dead = DeadCode::new(decl.clone(), DeadCodeIssue::DeadBranch)
                    .with_message(format!(
                        "Branch at line {line_no} is gated on a literal 'false' and can never execute"
                    ))
                    .with_confidence(Confidence::High);
                issues.push(dead);
            }
        }

        issues.sort_by(|a, b| {
            a.declaration
                .location
                .file
                .cmp(&b.declaration.location.file)
                .then(a.message.cmp(&b.message))
        });
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::io::Write;
    use std::path::PathBuf;

    fn function_decl(file: &Path, name: &str, line: usize) -> Declaration {
        Declaration::new(
            DeclarationId::new(file.to_path_buf(), line * 100, line * 100 + 10),
            name.to_string(),
            DeclarationKind::Function,
            Location::new(file.to_path_buf(), line, 1, line * 100, line * 100 + 10),
            Language::Kotlin,
        )
    }

    fn graph_over(source: &str) -> (Graph, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Main.kt");
        let mut f = fs::File::create(&file).unwrap();
        f.write_all(source.as_bytes()).unwrap();
        let mut graph = Graph::new();
        graph.add_declaration(function_decl(&file, "main", 1));
        (graph, temp)
    }

    #[test]
    fn if_false_is_reported() {
        let (graph, _tmp) =
            graph_over("fun main() {\n    if (false) {\n        boom()\n    }\n}\n");
        let issues = DeadBranchDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("line 2"));
    }

    #[test]
    fn while_false_is_reported() {
        let (graph, _tmp) = graph_over("fun main() {\n    while (false) { spin() }\n}\n");
        let issues = DeadBranchDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn runtime_conditions_stay_silent() {
        let (graph, _tmp) = graph_over("fun main(debug: Boolean) {\n    if (debug) { log() }\n}\n");
        let issues = DeadBranchDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn false_outside_a_condition_stays_silent() {
        let (graph, _tmp) = graph_over("fun main() {\n    val flags = listOf(false)\n}\n");
        let issues = DeadBranchDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn commented_out_code_stays_silent() {
        let (graph, _tmp) = graph_over("fun main() {\n    // if (false) { boom() }\n}\n");
        let issues = DeadBranchDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn an_empty_graph_reports_nothing() {
        let issues = DeadBranchDetector::new().detect(&Graph::new());
        assert!(issues.is_empty());
    }
}
