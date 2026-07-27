//! Unused Enum Case Detector (DC005)
//!
//! Flags enum entries with no incoming reference anywhere in the graph.
//! An enum iterated reflectively (`Enum.values()`, `Enum.entries`,
//! `Enum.valueOf`, `enumValues<Enum>()`) keeps every case: iteration
//! reaches them all, so nothing is reported for that enum. Annotated
//! cases are skipped too — serialization names live in string form.

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{Declaration, DeclarationId, DeclarationKind, Graph};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

pub struct UnusedEnumCaseDetector;

impl UnusedEnumCaseDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnusedEnumCaseDetector {
    fn default() -> Self {
        Self::new()
    }
}

fn project_corpus(graph: &Graph) -> String {
    let files: BTreeSet<&Path> = graph
        .declarations()
        .map(|d| d.location.file.as_path())
        .collect();
    let mut corpus = String::new();
    for file in files {
        if let Ok(content) = fs::read_to_string(file) {
            corpus.push_str(&content);
            corpus.push('\n');
        }
    }
    corpus
}

fn is_iterated_reflectively(corpus: &str, enum_name: &str) -> bool {
    [
        format!("{enum_name}.values"),
        format!("{enum_name}.entries"),
        format!("{enum_name}.valueOf"),
        format!("enumValues<{enum_name}>"),
    ]
    .iter()
    .any(|needle| corpus.contains(needle.as_str()))
}

/// Enums iterated reflectively (`values()`, `entries`, `valueOf`,
/// `enumValues<T>()`): iteration reaches every case, so none of their
/// cases should ever be reported as unused.
pub fn reflectively_iterated_enum_ids(graph: &Graph) -> std::collections::HashSet<DeclarationId> {
    let enums: Vec<&Declaration> = graph
        .declarations()
        .filter(|d| d.kind == DeclarationKind::Enum)
        .collect();
    if enums.is_empty() {
        return Default::default();
    }
    let corpus = project_corpus(graph);
    enums
        .into_iter()
        .filter(|e| is_iterated_reflectively(&corpus, &e.name))
        .map(|e| e.id.clone())
        .collect()
}

impl Detector for UnusedEnumCaseDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut candidates: HashMap<&DeclarationId, Vec<&Declaration>> = HashMap::new();
        for decl in graph.declarations() {
            if decl.kind != DeclarationKind::EnumCase {
                continue;
            }
            if graph.is_referenced(&decl.id) {
                continue;
            }
            if !decl.annotations.is_empty() {
                continue;
            }
            if let Some(parent) = &decl.parent {
                candidates.entry(parent).or_default().push(decl);
            }
        }
        if candidates.is_empty() {
            return Vec::new();
        }

        let corpus = project_corpus(graph);
        let mut issues = Vec::new();
        for (enum_id, cases) in candidates {
            let Some(enum_decl) = graph.get_declaration(enum_id) else {
                continue;
            };
            if is_iterated_reflectively(&corpus, &enum_decl.name) {
                continue;
            }
            for case in cases {
                let dead = DeadCode::new(case.clone(), DeadCodeIssue::UnusedEnumCase)
                    .with_message(format!(
                        "Enum case '{}' of '{}' is never referenced",
                        case.name, enum_decl.name
                    ))
                    .with_confidence(Confidence::Medium);
                issues.push(dead);
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
    use crate::graph::{Language, Location};
    use std::path::PathBuf;

    fn decl(file: &str, name: &str, kind: DeclarationKind, line: usize) -> Declaration {
        let path = PathBuf::from(file);
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 10),
            name.to_string(),
            kind,
            Location::new(path, line, 1, line * 100, line * 100 + 10),
            Language::Kotlin,
        )
    }

    fn enum_with_cases(graph: &mut Graph, names: &[&str]) -> DeclarationId {
        let enum_decl = decl("Status.kt", "Status", DeclarationKind::Enum, 1);
        let enum_id = graph.add_declaration(enum_decl);
        for (i, name) in names.iter().enumerate() {
            let mut case = decl("Status.kt", name, DeclarationKind::EnumCase, i + 2);
            case.parent = Some(enum_id.clone());
            graph.add_declaration(case);
        }
        enum_id
    }

    #[test]
    fn an_unreferenced_case_is_flagged() {
        let mut graph = Graph::new();
        enum_with_cases(&mut graph, &["LEGACY"]);

        let issues = UnusedEnumCaseDetector::new().detect(&graph);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].declaration.name, "LEGACY");
    }

    #[test]
    fn an_annotated_case_is_left_alone() {
        let mut graph = Graph::new();
        let enum_decl = decl("Status.kt", "Status", DeclarationKind::Enum, 1);
        let enum_id = graph.add_declaration(enum_decl);
        let mut case = decl("Status.kt", "WIRE_NAME", DeclarationKind::EnumCase, 2);
        case.parent = Some(enum_id);
        case.annotations.push("SerializedName".to_string());
        graph.add_declaration(case);

        let issues = UnusedEnumCaseDetector::new().detect(&graph);

        assert!(
            issues.is_empty(),
            "annotated cases may be reached by their serialized name"
        );
    }

    #[test]
    fn a_case_without_parent_is_ignored() {
        let mut graph = Graph::new();
        graph.add_declaration(decl("Loose.kt", "STRAY", DeclarationKind::EnumCase, 1));

        let issues = UnusedEnumCaseDetector::new().detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn an_empty_graph_reports_nothing() {
        let issues = UnusedEnumCaseDetector::new().detect(&Graph::new());
        assert!(issues.is_empty());
    }
}
