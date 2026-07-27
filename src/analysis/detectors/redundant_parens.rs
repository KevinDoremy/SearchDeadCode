//! Redundant Parentheses Detector (DC015)
//!
//! Flags `if ((x))` / `while ((x))` where the inner pair wraps a
//! parenthesis-free expression, and `return (x)` around a bare
//! identifier. Anything with nested parentheses is left alone: the
//! outer pair may be doing real grouping work.

use super::{enclosing_declaration, graph_files, Detector};
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::Graph;
use regex::Regex;
use std::fs;
use std::sync::LazyLock;

static DOUBLED_CONDITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:if|while)\s*\(\(([^()]*)\)\)").expect("Invalid doubled-parens regex")
});

static PARENTHESIZED_RETURN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\breturn\s*\((\w+)\)\s*;?\s*$").expect("Invalid return regex"));

pub struct RedundantParenthesesDetector;

impl RedundantParenthesesDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RedundantParenthesesDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for RedundantParenthesesDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();
        for file in graph_files(graph) {
            let Ok(content) = fs::read_to_string(file) else {
                continue;
            };
            for (idx, line) in content.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with('*') {
                    continue;
                }
                let doubled = DOUBLED_CONDITION.is_match(line);
                let wrapped_return = PARENTHESIZED_RETURN.is_match(line);
                if !doubled && !wrapped_return {
                    continue;
                }
                let line_no = idx + 1;
                let Some(decl) = enclosing_declaration(graph, file, line_no) else {
                    continue;
                };
                let what = if doubled {
                    "doubled parentheses around the condition"
                } else {
                    "parentheses around a bare return value"
                };
                let dead = DeadCode::new(decl.clone(), DeadCodeIssue::RedundantParentheses)
                    .with_message(format!("{what} at line {line_no}"))
                    .with_confidence(Confidence::High);
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

    fn graph_over(source: &str) -> (Graph, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Main.kt");
        fs::write(&file, source).unwrap();
        let mut graph = Graph::new();
        let decl = Declaration::new(
            DeclarationId::new(file.to_path_buf(), 0, 10),
            "main".to_string(),
            DeclarationKind::Function,
            Location::new(file.to_path_buf(), 1, 1, 0, 10),
            Language::Kotlin,
        );
        graph.add_declaration(decl);
        (graph, temp)
    }

    #[test]
    fn doubled_condition_is_flagged() {
        let (graph, _tmp) = graph_over("fun main(x: Boolean) {\n    if ((x)) return\n}\n");
        let issues = RedundantParenthesesDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("line 2"));
    }

    #[test]
    fn grouping_parentheses_are_respected() {
        let (graph, _tmp) = graph_over(
            "fun main(a: Boolean, b: Boolean, c: Boolean) {\n    if ((a || b) && c) return\n}\n",
        );
        let issues = RedundantParenthesesDetector::new().detect(&graph);
        assert!(issues.is_empty(), "the outer pair groups a || b");
    }

    #[test]
    fn bare_return_wrapped_in_parens_is_flagged() {
        let (graph, _tmp) = graph_over("fun main(x: Int): Int {\n    return (x)\n}\n");
        let issues = RedundantParenthesesDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn return_of_an_expression_is_left_alone() {
        let (graph, _tmp) = graph_over("fun main(x: Int): Int {\n    return (x + 1) * 2\n}\n");
        let issues = RedundantParenthesesDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn comments_never_fire() {
        let (graph, _tmp) = graph_over("fun main() {\n    // if ((x)) return\n}\n");
        let issues = RedundantParenthesesDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }
}
