//! Prefer isEmpty() Detector (DC016)
//!
//! Flags `.size` / `.length` compared to zero where isEmpty() or
//! isNotEmpty() says the same thing. Comparisons to any other number,
//! and `>= 0` (always true, but not this detector's business), are
//! left alone.

use super::{enclosing_declaration, graph_files, Detector};
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::Graph;
use regex::Regex;
use std::fs;
use std::sync::LazyLock;

static SIZE_VS_ZERO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.(?:size|length)\s*(==|!=|>)\s*0\b").expect("Invalid size regex")
});

pub struct PreferIsEmptyDetector;

impl PreferIsEmptyDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PreferIsEmptyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for PreferIsEmptyDetector {
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
                let Some(captures) = SIZE_VS_ZERO.captures(line) else {
                    continue;
                };
                let suggestion = match &captures[1] {
                    "==" => "isEmpty()",
                    _ => "isNotEmpty()",
                };
                let line_no = idx + 1;
                let Some(decl) = enclosing_declaration(graph, file, line_no) else {
                    continue;
                };
                let dead = DeadCode::new(decl.clone(), DeadCodeIssue::PreferIsEmpty)
                    .with_message(format!(
                        "size/length compared to zero at line {line_no} — prefer {suggestion}"
                    ))
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
    fn size_equals_zero_suggests_isempty() {
        let (graph, _tmp) =
            graph_over("fun main(l: List<Int>) {\n    if (l.size == 0) return\n}\n");
        let issues = PreferIsEmptyDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("isEmpty()"));
    }

    #[test]
    fn size_greater_than_zero_suggests_isnotempty() {
        let (graph, _tmp) = graph_over("fun main(l: List<Int>) {\n    if (l.size > 0) return\n}\n");
        let issues = PreferIsEmptyDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("isNotEmpty()"));
    }

    #[test]
    fn length_not_equals_zero_suggests_isnotempty() {
        let (graph, _tmp) = graph_over("fun main(s: String) {\n    if (s.length != 0) return\n}\n");
        let issues = PreferIsEmptyDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("isNotEmpty()"));
    }

    #[test]
    fn specific_sizes_are_fine() {
        let (graph, _tmp) =
            graph_over("fun main(l: List<Int>) {\n    if (l.size == 3) return\n}\n");
        let issues = PreferIsEmptyDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn greater_or_equal_zero_is_not_this_detectors_call() {
        let (graph, _tmp) =
            graph_over("fun main(l: List<Int>) {\n    if (l.size >= 0) return\n}\n");
        let issues = PreferIsEmptyDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn size_compared_to_ten_is_fine() {
        let (graph, _tmp) =
            graph_over("fun main(l: List<Int>) {\n    if (l.size > 10) return\n}\n");
        let issues = PreferIsEmptyDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }
}
