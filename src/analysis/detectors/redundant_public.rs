//! Redundant Public Detector (DC006)
//!
//! A public Kotlin declaration whose references all live in its own
//! module could be `internal`. The verdict requires at least two modules
//! (otherwise `internal` changes nothing) and at least one reference —
//! a declaration nobody uses is dead code, which is a different report.
//! Ambiguous references count as usage wherever they come from, so a
//! possible cross-module consumer silences the suggestion.

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language, Visibility};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub struct RedundantPublicDetector;

impl RedundantPublicDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RedundantPublicDetector {
    fn default() -> Self {
        Self::new()
    }
}

fn common_root(files: &BTreeSet<&Path>) -> Option<PathBuf> {
    let mut iter = files.iter();
    let mut root = iter.next()?.parent()?.to_path_buf();
    for file in iter {
        while !file.starts_with(&root) {
            root = root.parent()?.to_path_buf();
        }
    }
    Some(root)
}

/// First path component under the shared root; None for files sitting
/// directly at the root (they belong to no module).
fn module_of(root: &Path, file: &Path) -> Option<String> {
    let relative = file.strip_prefix(root).ok()?;
    let mut components = relative.components();
    let first = components.next()?;
    components.next()?; // a module needs at least <dir>/<file>
    Some(first.as_os_str().to_string_lossy().into_owned())
}

impl Detector for RedundantPublicDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let files: BTreeSet<&Path> = graph
            .declarations()
            .map(|d| d.location.file.as_path())
            .collect();
        let Some(root) = common_root(&files) else {
            return Vec::new();
        };
        let modules: BTreeSet<String> = files.iter().filter_map(|f| module_of(&root, f)).collect();
        if modules.len() < 2 {
            return Vec::new();
        }

        let mut issues = Vec::new();
        for decl in graph.declarations() {
            if decl.language != Language::Kotlin
                || decl.visibility != Visibility::Public
                || decl.parent.is_some()
                || !decl.annotations.is_empty()
            {
                continue;
            }
            if !matches!(
                decl.kind,
                DeclarationKind::Class
                    | DeclarationKind::Interface
                    | DeclarationKind::Object
                    | DeclarationKind::Enum
                    | DeclarationKind::Function
                    | DeclarationKind::Property
            ) {
                continue;
            }
            let Some(decl_module) = module_of(&root, &decl.location.file) else {
                continue;
            };
            let references = graph.get_references_to(&decl.id);
            if references.is_empty() {
                continue; // unreferenced = dead code, another detector's job
            }
            let all_local = references.iter().all(|(source, _)| {
                module_of(&root, &source.location.file).as_deref() == Some(decl_module.as_str())
            });
            if !all_local {
                continue;
            }
            let dead = DeadCode::new(decl.clone(), DeadCodeIssue::RedundantPublic)
                .with_message(format!(
                    "Public but only referenced from module '{decl_module}' — could be internal"
                ))
                .with_confidence(Confidence::Medium);
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
    use crate::graph::{Declaration, DeclarationId, Location, Reference, ReferenceKind};
    use std::path::PathBuf;

    fn class_decl(file: &str, name: &str, line: usize) -> Declaration {
        let path = PathBuf::from(file);
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 10),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 10),
            Language::Kotlin,
        )
    }

    fn reference_to(graph: &mut Graph, from: &DeclarationId, to: &DeclarationId, name: &str) {
        let location = Location::new(from.file.clone(), 1, 1, 0, 1);
        graph.add_reference(
            from,
            to,
            Reference::new(ReferenceKind::Type, location, name.to_string()),
        );
    }

    #[test]
    fn local_only_usage_suggests_internal() {
        let mut graph = Graph::new();
        let helper = graph.add_declaration(class_decl("/p/core/src/Helper.kt", "Helper", 1));
        let user = graph.add_declaration(class_decl("/p/core/src/User.kt", "User", 1));
        let _other = graph.add_declaration(class_decl("/p/app/src/App.kt", "App", 1));
        reference_to(&mut graph, &user, &helper, "Helper");

        let issues = RedundantPublicDetector::new().detect(&graph);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].declaration.name, "Helper");
    }

    #[test]
    fn cross_module_usage_stays_public() {
        let mut graph = Graph::new();
        let helper = graph.add_declaration(class_decl("/p/core/src/Helper.kt", "Helper", 1));
        let app = graph.add_declaration(class_decl("/p/app/src/App.kt", "App", 1));
        reference_to(&mut graph, &app, &helper, "Helper");

        let issues = RedundantPublicDetector::new().detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn a_single_module_reports_nothing() {
        let mut graph = Graph::new();
        let helper = graph.add_declaration(class_decl("/p/core/src/Helper.kt", "Helper", 1));
        let user = graph.add_declaration(class_decl("/p/core/src/User.kt", "User", 1));
        reference_to(&mut graph, &user, &helper, "Helper");

        let issues = RedundantPublicDetector::new().detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn an_unreferenced_declaration_is_not_this_detectors_business() {
        let mut graph = Graph::new();
        graph.add_declaration(class_decl("/p/core/src/Helper.kt", "Helper", 1));
        graph.add_declaration(class_decl("/p/app/src/App.kt", "App", 1));

        let issues = RedundantPublicDetector::new().detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn a_java_declaration_is_skipped() {
        let mut graph = Graph::new();
        let path = PathBuf::from("/p/core/src/Helper.java");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), 0, 10),
            "JHelper".to_string(),
            DeclarationKind::Class,
            Location::new(path, 1, 1, 0, 10),
            Language::Java,
        );
        decl.visibility = Visibility::Public;
        let helper = graph.add_declaration(decl);
        let user = graph.add_declaration(class_decl("/p/core/src/User.kt", "User", 1));
        let _other = graph.add_declaration(class_decl("/p/app/src/App.kt", "App", 1));
        reference_to(&mut graph, &user, &helper, "JHelper");

        let issues = RedundantPublicDetector::new().detect(&graph);

        assert!(issues.is_empty(), "Java has no internal keyword to suggest");
    }

    #[test]
    fn an_annotated_declaration_is_left_alone() {
        let mut graph = Graph::new();
        let mut decl = class_decl("/p/core/src/Helper.kt", "Injected", 1);
        decl.annotations.push("Singleton".to_string());
        let helper = graph.add_declaration(decl);
        let user = graph.add_declaration(class_decl("/p/core/src/User.kt", "User", 1));
        let _other = graph.add_declaration(class_decl("/p/app/src/App.kt", "App", 1));
        reference_to(&mut graph, &user, &helper, "Injected");

        let issues = RedundantPublicDetector::new().detect(&graph);

        assert!(
            issues.is_empty(),
            "frameworks reach annotated declarations from outside the graph"
        );
    }
}
