//! GlobalScope Usage Detector
//!
//! Detects usage of `GlobalScope.launch` and `GlobalScope.async` in coroutines.
//! This is a common anti-pattern that leads to memory leaks and unstructured concurrency.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! // BAD: GlobalScope coroutines can't be cancelled
//! GlobalScope.launch {
//!     loadData()
//! }
//!
//! // BAD: Memory leak risk
//! GlobalScope.async {
//!     fetchData()
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Coroutines launched in GlobalScope can't be cancelled with the component lifecycle
//! - Memory leaks when referencing Activity/Fragment
//! - Violates structured concurrency principles
//! - Hard to test and debug
//!
//! ## Better Alternatives
//!
//! - Use `viewModelScope` in ViewModels (cancelled when ViewModel is cleared)
//! - Use `lifecycleScope` in Activities/Fragments (tied to lifecycle)
//! - Use custom CoroutineScope with proper cancellation
//! - Use WorkManager for background work that should survive process death

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for GlobalScope usage in coroutines
pub struct GlobalScopeUsageDetector {
    /// Also flag runBlocking usage
    flag_run_blocking: bool,
}

impl GlobalScopeUsageDetector {
    pub fn new() -> Self {
        Self {
            flag_run_blocking: true,
        }
    }

    /// Don't flag runBlocking
    #[allow(dead_code)]
    pub fn ignore_run_blocking(mut self) -> Self {
        self.flag_run_blocking = false;
        self
    }

    /// Check if file is a test file
    fn is_test_file(path: &std::path::Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.contains("/test/")
            || path_str.contains("/androidTest/")
            || path_str.contains("Test.kt")
            || path_str.contains("Tests.kt")
            || path_str.ends_with("Spec.kt")
    }

    /// Check if name indicates GlobalScope usage
    fn indicates_globalscope(name: &str) -> bool {
        name.contains("GlobalScope")
    }

    /// Check if name indicates runBlocking usage
    fn indicates_runblocking(name: &str) -> bool {
        name.contains("runBlocking")
    }
}

impl Default for GlobalScopeUsageDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for GlobalScopeUsageDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // Check declarations for GlobalScope usage
        for decl in graph.declarations() {
            // Skip test files - runBlocking and GlobalScope are OK in tests
            if Self::is_test_file(&decl.location.file) {
                continue;
            }

            // Check method names for GlobalScope usage patterns
            if matches!(decl.kind, DeclarationKind::Method | DeclarationKind::Function) {
                if Self::indicates_globalscope(&decl.name) {
                    let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::GlobalScopeUsage);
                    dead = dead.with_message(format!(
                        "Method '{}' appears to use GlobalScope. Use viewModelScope or lifecycleScope instead.",
                        decl.name
                    ));
                    dead = dead.with_confidence(Confidence::Medium);
                    issues.push(dead);
                } else if self.flag_run_blocking && Self::indicates_runblocking(&decl.name) {
                    let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::GlobalScopeUsage);
                    dead = dead.with_message(format!(
                        "Method '{}' appears to use runBlocking. Avoid blocking threads; use suspend functions.",
                        decl.name
                    ));
                    dead = dead.with_confidence(Confidence::Medium);
                    issues.push(dead);
                }
            }

            // Check class for GlobalScope in super types (rare but possible)
            if matches!(decl.kind, DeclarationKind::Class) {
                for super_type in &decl.super_types {
                    if super_type.contains("GlobalScope") {
                        let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::GlobalScopeUsage);
                        dead = dead.with_message(format!(
                            "Class '{}' inherits from GlobalScope. Consider using structured concurrency.",
                            decl.name
                        ));
                        dead = dead.with_confidence(Confidence::High);
                        issues.push(dead);
                    }
                }
            }

            // Check properties/fields for GlobalScope references
            if matches!(decl.kind, DeclarationKind::Property | DeclarationKind::Field) {
                if Self::indicates_globalscope(&decl.name) {
                    let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::GlobalScopeUsage);
                    dead = dead.with_message(format!(
                        "Property '{}' references GlobalScope. Consider using a lifecycle-aware scope.",
                        decl.name
                    ));
                    dead = dead.with_confidence(Confidence::Medium);
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
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::path::PathBuf;

    fn create_method(name: &str, file: &str, line: usize) -> Declaration {
        let path = PathBuf::from(file);
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Method,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        )
    }

    fn create_class(name: &str, file: &str, line: usize, super_types: Vec<&str>) -> Declaration {
        let path = PathBuf::from(file);
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.super_types = super_types.into_iter().map(String::from).collect();
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = GlobalScopeUsageDetector::new();
        assert!(detector.flag_run_blocking);
    }

    #[test]
    fn test_ignore_run_blocking() {
        let detector = GlobalScopeUsageDetector::new().ignore_run_blocking();
        assert!(!detector.flag_run_blocking);
    }

    #[test]
    fn test_indicates_globalscope() {
        assert!(GlobalScopeUsageDetector::indicates_globalscope(
            "launchWithGlobalScope"
        ));
        assert!(GlobalScopeUsageDetector::indicates_globalscope(
            "GlobalScope"
        ));
        assert!(!GlobalScopeUsageDetector::indicates_globalscope(
            "viewModelScope"
        ));
    }

    #[test]
    fn test_indicates_runblocking() {
        assert!(GlobalScopeUsageDetector::indicates_runblocking(
            "runBlocking"
        ));
        assert!(GlobalScopeUsageDetector::indicates_runblocking(
            "userunBlocking"  // lowercase 'r' in runBlocking
        ));
        assert!(!GlobalScopeUsageDetector::indicates_runblocking("launch"));
    }

    #[test]
    fn test_is_test_file() {
        assert!(GlobalScopeUsageDetector::is_test_file(&PathBuf::from(
            "src/test/kotlin/Test.kt"
        )));
        assert!(GlobalScopeUsageDetector::is_test_file(&PathBuf::from(
            "src/androidTest/kotlin/Test.kt"
        )));
        assert!(GlobalScopeUsageDetector::is_test_file(&PathBuf::from(
            "UserServiceTest.kt"
        )));
        assert!(!GlobalScopeUsageDetector::is_test_file(&PathBuf::from(
            "src/main/kotlin/Service.kt"
        )));
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = GlobalScopeUsageDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_method_with_globalscope_name() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("launchWithGlobalScope", "main.kt", 1));

        let detector = GlobalScopeUsageDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("GlobalScope"));
    }

    #[test]
    fn test_method_with_runblocking_name() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("userunBlockingHelper", "main.kt", 1));

        let detector = GlobalScopeUsageDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("runBlocking"));
    }

    #[test]
    fn test_skips_test_files() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method(
            "launchWithGlobalScope",
            "src/test/kotlin/Test.kt",
            1,
        ));

        let detector = GlobalScopeUsageDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Should skip test files");
    }

    #[test]
    fn test_clean_code_no_issues() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("loadData", "main.kt", 1));
        graph.add_declaration(create_class("UserViewModel", "main.kt", 5, vec!["ViewModel"]));

        let detector = GlobalScopeUsageDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_class_inheriting_globalscope() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class(
            "BadScope",
            "main.kt",
            1,
            vec!["GlobalScope", "CoroutineScope"],
        ));

        let detector = GlobalScopeUsageDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("inherits from GlobalScope"));
    }
}
