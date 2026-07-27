//! Duplicate Import Detector (DC012)
//!
//! Scans import lines directly: the parsers keep imports out of the
//! declaration graph (feeding them in would make every import look
//! unreachable to the analyzers), so this detector reads the files the
//! graph knows about and reports the second and later occurrences of
//! an identical import.

use super::{graph_files, Detector};
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{Declaration, DeclarationId, DeclarationKind, Graph, Language, Location};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::sync::LazyLock;

static IMPORT_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*import\s+(.+?);?\s*$").expect("Invalid import regex"));

pub struct DuplicateImportDetector;

impl DuplicateImportDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DuplicateImportDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for DuplicateImportDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();
        for file in graph_files(graph) {
            let Ok(content) = fs::read_to_string(file) else {
                continue;
            };
            let language = if file.extension().is_some_and(|e| e == "java") {
                Language::Java
            } else {
                Language::Kotlin
            };
            let mut first_seen: HashMap<String, usize> = HashMap::new();
            let mut byte_offset = 0usize;
            for (idx, line) in content.lines().enumerate() {
                let line_start = byte_offset;
                byte_offset += line.len() + 1;
                let Some(captures) = IMPORT_LINE.captures(line) else {
                    continue;
                };
                let import_path = captures[1].trim().to_string();
                let line_no = idx + 1;
                match first_seen.get(&import_path) {
                    None => {
                        first_seen.insert(import_path, line_no);
                    }
                    Some(first_line) => {
                        let id = DeclarationId::new(
                            file.to_path_buf(),
                            line_start,
                            line_start + line.len(),
                        );
                        let location = Location::new(
                            file.to_path_buf(),
                            line_no,
                            1,
                            line_start,
                            line_start + line.len(),
                        );
                        let decl = Declaration::new(
                            id,
                            import_path.clone(),
                            DeclarationKind::Import,
                            location,
                            language,
                        );
                        let dead = DeadCode::new(decl, DeadCodeIssue::DuplicateImport)
                            .with_message(format!(
                                "Import '{import_path}' is duplicated (first occurrence at line {first_line})"
                            ))
                            .with_confidence(Confidence::High);
                        issues.push(dead);
                    }
                }
            }
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

    fn graph_over(name: &str, source: &str) -> (Graph, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join(name);
        fs::write(&file, source).unwrap();
        let mut graph = Graph::new();
        let decl = Declaration::new(
            DeclarationId::new(file.to_path_buf(), 0, 5),
            "anchor".to_string(),
            DeclarationKind::Class,
            Location::new(file.to_path_buf(), 1, 1, 0, 5),
            Language::Kotlin,
        );
        graph.add_declaration(decl);
        (graph, temp)
    }

    #[test]
    fn a_repeated_import_is_flagged_once_per_repeat() {
        let (graph, _tmp) = graph_over(
            "Main.kt",
            "import kotlin.math.abs\nimport kotlin.math.abs\nimport kotlin.math.abs\n",
        );
        let issues = DuplicateImportDetector::new().detect(&graph);
        assert_eq!(issues.len(), 2, "second and third occurrences");
        assert!(issues[0].message.contains("first occurrence at line 1"));
    }

    #[test]
    fn different_imports_are_fine() {
        let (graph, _tmp) = graph_over(
            "Main.kt",
            "import kotlin.math.abs\nimport kotlin.math.max\n",
        );
        let issues = DuplicateImportDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn java_semicolons_do_not_hide_duplicates() {
        let (graph, _tmp) = graph_over(
            "Main.java",
            "import java.util.List;\nimport java.util.List;\n",
        );
        let issues = DuplicateImportDetector::new().detect(&graph);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn a_commented_import_is_not_an_import() {
        let (graph, _tmp) = graph_over(
            "Main.kt",
            "import kotlin.math.abs\n// import kotlin.math.abs\n",
        );
        let issues = DuplicateImportDetector::new().detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn the_word_import_in_code_is_ignored() {
        let (graph, _tmp) = graph_over(
            "Main.kt",
            "import kotlin.math.abs\nval s = \"import kotlin.math.abs\"\n",
        );
        let issues = DuplicateImportDetector::new().detect(&graph);
        assert!(
            issues.is_empty(),
            "a string containing the word import is not an import line"
        );
    }
}
