//! AsyncTask Usage Detector
//!
//! Detects usage of deprecated AsyncTask.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! class LoadTask : AsyncTask<String, Int, List<User>>() {
//!     override fun doInBackground(vararg params: String): List<User> {
//!         return fetchUsers(params[0])
//!     }
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Deprecated in API 30
//! - Memory leaks (holds Activity reference)
//! - No lifecycle awareness
//! - Complex cancellation
//! - Results lost on config change
//!
//! ## Better Alternatives
//!
//! - Kotlin coroutines with viewModelScope
//! - RxJava
//! - java.util.concurrent.Executor
//! - WorkManager for persistent work

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for AsyncTask usage
pub struct AsyncTaskUsageDetector;

impl AsyncTaskUsageDetector {
    pub fn new() -> Self {
        Self
    }

    /// Check if class extends AsyncTask
    fn extends_asynctask(decl: &crate::graph::Declaration) -> bool {
        decl.super_types
            .iter()
            .any(|s| s.contains("AsyncTask"))
    }

    /// Check if class name suggests AsyncTask
    fn name_suggests_asynctask(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("asynctask") || lower.ends_with("task") || lower.ends_with("async")
    }
}

impl Default for AsyncTaskUsageDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for AsyncTaskUsageDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Check classes that extend AsyncTask
            if matches!(decl.kind, DeclarationKind::Class) {
                if !matches!(decl.language, Language::Kotlin | Language::Java) {
                    continue;
                }

                if Self::extends_asynctask(decl) {
                    let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::AsyncTaskUsage);
                    dead = dead.with_message(format!(
                        "Class '{}' extends deprecated AsyncTask. Use coroutines or Executor instead.",
                        decl.name
                    ));
                    dead = dead.with_confidence(Confidence::High);
                    issues.push(dead);
                }
            }
        }

        // Sort by file and line
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
    use std::path::PathBuf;

    fn create_class(name: &str, line: usize, super_types: Vec<&str>) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 500),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 500),
            Language::Kotlin,
        );
        decl.super_types = super_types.into_iter().map(String::from).collect();
        decl
    }

    #[test]
    fn test_detector_creation() {
        let _detector = AsyncTaskUsageDetector::new();
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = AsyncTaskUsageDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_asynctask_class_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class(
            "LoadDataTask",
            1,
            vec!["AsyncTask<String, Int, List>"],
        ));

        let detector = AsyncTaskUsageDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("deprecated"));
    }

    #[test]
    fn test_regular_class_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class("UserRepository", 1, vec!["Repository"]));

        let detector = AsyncTaskUsageDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_coroutine_class_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class("DataLoader", 1, vec!["CoroutineScope"]));

        let detector = AsyncTaskUsageDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
