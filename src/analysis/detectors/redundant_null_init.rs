//! Redundant Null Initialization Detector (DC013)
//!
//! Flags Java fields explicitly initialized to null: the JLS (4.12.5)
//! already defaults reference-typed fields to null, so `= null` says
//! nothing. Final fields are excluded — dropping their initializer
//! breaks definite assignment. Kotlin is excluded entirely: properties
//! there REQUIRE an initializer, so nothing is redundant.

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

static NULL_INIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"=\s*null\s*;").expect("Invalid null-init regex"));

pub struct RedundantNullInitDetector;

impl RedundantNullInitDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RedundantNullInitDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for RedundantNullInitDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut file_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut issues = Vec::new();

        for decl in graph.declarations() {
            if decl.language != Language::Java || decl.kind != DeclarationKind::Field {
                continue;
            }
            if decl.modifiers.iter().any(|m| m == "final") {
                continue;
            }
            let lines = file_cache
                .entry(decl.location.file.clone())
                .or_insert_with(|| {
                    fs::read_to_string(&decl.location.file)
                        .map(|c| c.lines().map(String::from).collect())
                        .unwrap_or_default()
                });
            let Some(line) = lines.get(decl.location.line.saturating_sub(1)) else {
                continue;
            };
            if line.contains("final") {
                continue; // belt and braces: modifiers may not carry final
            }
            if !NULL_INIT.is_match(line) {
                continue;
            }
            let dead = DeadCode::new(decl.clone(), DeadCodeIssue::RedundantNullInit)
                .with_message(format!(
                    "Java field '{}' defaults to null — the explicit '= null' is redundant",
                    decl.name
                ))
                .with_confidence(Confidence::High);
            issues.push(dead);
        }

        issues.sort_by(|a, b| {
            a.declaration
                .location
                .file
                .cmp(&b.declaration.location.file)
                .then(
                    a.declaration
                        .location
                        .line
                        .cmp(&b.declaration.location.line),
                )
        });
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Declaration, DeclarationId, Location};
    use std::path::Path;

    fn field_decl(file: &Path, name: &str, line: usize, modifiers: Vec<&str>) -> Declaration {
        let mut decl = Declaration::new(
            DeclarationId::new(file.to_path_buf(), line * 100, line * 100 + 10),
            name.to_string(),
            DeclarationKind::Field,
            Location::new(file.to_path_buf(), line, 1, line * 100, line * 100 + 10),
            Language::Java,
        );
        decl.modifiers = modifiers.into_iter().map(String::from).collect();
        decl
    }

    fn graph_over(
        source: &str,
        field_line: usize,
        modifiers: Vec<&str>,
    ) -> (Graph, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Holder.java");
        fs::write(&file, source).unwrap();
        let mut graph = Graph::new();
        graph.add_declaration(field_decl(&file, "cache", field_line, modifiers));
        (graph, temp)
    }

    #[test]
    fn a_null_initialized_field_is_flagged() {
        let (graph, _tmp) = graph_over(
            "class Holder {\n    private String cache = null;\n}\n",
            2,
            vec!["private"],
        );
        let issues = RedundantNullInitDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("cache"));
    }

    #[test]
    fn a_field_with_a_real_initializer_is_fine() {
        let (graph, _tmp) = graph_over(
            "class Holder {\n    private String cache = \"warm\";\n}\n",
            2,
            vec!["private"],
        );
        let issues = RedundantNullInitDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn a_final_field_keeps_its_initializer() {
        let (graph, _tmp) = graph_over(
            "class Holder {\n    private final String cache = null;\n}\n",
            2,
            vec!["private", "final"],
        );
        let issues = RedundantNullInitDetector::new().detect(&graph);
        assert!(issues.is_empty(), "final needs definite assignment");
    }

    #[test]
    fn kotlin_properties_are_out_of_scope() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("State.kt");
        fs::write(&file, "class State {\n    var name: String? = null\n}\n").unwrap();
        let mut graph = Graph::new();
        let mut decl = field_decl(&file, "name", 2, vec![]);
        decl.language = Language::Kotlin;
        decl.kind = DeclarationKind::Property;
        graph.add_declaration(decl);

        let issues = RedundantNullInitDetector::new().detect(&graph);
        assert!(
            issues.is_empty(),
            "Kotlin requires property initializers, nothing is redundant"
        );
    }

    #[test]
    fn an_empty_graph_reports_nothing() {
        let issues = RedundantNullInitDetector::new().detect(&Graph::new());
        assert!(issues.is_empty());
    }
}
