//! Main Thread Database Detector
//!
//! Detects database operations that may block the main thread.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! fun onButtonClick() {
//!     val users = userDao.getAllUsers()  // Blocks UI thread!
//!     updateUI(users)
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Blocks UI thread (causes jank)
//! - Can cause ANR (Application Not Responding)
//! - Database operations can take 100ms+
//!
//! ## Better Alternatives
//!
//! - Use suspend functions with Dispatchers.IO
//! - Use LiveData/Flow (Room handles threading)
//! - Use background thread with callback

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for main thread database operations
pub struct MainThreadDatabaseDetector {
    /// Database-related keywords
    db_keywords: Vec<&'static str>,
}

impl MainThreadDatabaseDetector {
    pub fn new() -> Self {
        Self {
            db_keywords: vec![
                "dao",
                "database",
                "query",
                "insert",
                "update",
                "delete",
                "repository",
                "sqlite",
                "room",
            ],
        }
    }

    /// Check if class/method name suggests database access
    fn suggests_database_access(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.db_keywords.iter().any(|&kw| lower.contains(kw))
    }

    /// Check if method is a DAO method (non-suspend = blocking)
    fn is_dao_method(decl: &crate::graph::Declaration, graph: &Graph) -> bool {
        if let Some(ref parent_id) = decl.parent {
            if let Some(parent) = graph.get_declaration(parent_id) {
                return parent.name.to_lowercase().contains("dao")
                    || parent
                        .annotations
                        .iter()
                        .any(|a| a.to_lowercase().contains("dao"));
            }
        }
        false
    }

    /// Check if method has suspend modifier (safe)
    fn is_suspend_function(decl: &crate::graph::Declaration) -> bool {
        decl.modifiers.iter().any(|m| m == "suspend")
    }
}

impl Default for MainThreadDatabaseDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for MainThreadDatabaseDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Only check methods
            if !matches!(decl.kind, DeclarationKind::Method) {
                continue;
            }

            // Only check Kotlin/Java
            if !matches!(decl.language, Language::Kotlin | Language::Java) {
                continue;
            }

            // Check if it's a DAO method
            if !Self::is_dao_method(decl, graph) {
                continue;
            }

            // Skip suspend functions (they're safe)
            if Self::is_suspend_function(decl) {
                continue;
            }

            // Check if name suggests database operation
            if !self.suggests_database_access(&decl.name) {
                continue;
            }

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::MainThreadDatabase);
            dead = dead.with_message(format!(
                "DAO method '{}' is not a suspend function. May block main thread causing ANR.",
                decl.name
            ));
            dead = dead.with_confidence(Confidence::Medium);
            issues.push(dead);
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

    fn create_dao_interface(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 500),
            name.to_string(),
            DeclarationKind::Interface,
            Location::new(path, line, 1, line * 100, line * 100 + 500),
            Language::Kotlin,
        );
        decl.annotations.push("Dao".to_string());
        decl
    }

    fn create_dao_method(
        name: &str,
        parent_id: DeclarationId,
        line: usize,
        is_suspend: bool,
    ) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Method,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.parent = Some(parent_id);
        if is_suspend {
            decl.modifiers.push("suspend".to_string());
        }
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = MainThreadDatabaseDetector::new();
        assert!(!detector.db_keywords.is_empty());
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = MainThreadDatabaseDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_blocking_dao_method_detected() {
        let mut graph = Graph::new();
        let dao = create_dao_interface("UserDao", 1);
        let dao_id = dao.id.clone();
        graph.add_declaration(dao);
        graph.add_declaration(create_dao_method("queryAllUsers", dao_id, 2, false));

        let detector = MainThreadDatabaseDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_suspend_dao_method_ok() {
        let mut graph = Graph::new();
        let dao = create_dao_interface("UserDao", 1);
        let dao_id = dao.id.clone();
        graph.add_declaration(dao);
        graph.add_declaration(create_dao_method("queryAllUsers", dao_id, 2, true));

        let detector = MainThreadDatabaseDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
