//! Hardcoded Dispatcher Detector
//!
//! Detects hardcoded Dispatchers.IO/Main/Default usage.
//! Hardcoded dispatchers make testing difficult.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! suspend fun fetchData() = withContext(Dispatchers.IO) {
//!     // Network call
//! }
//!
//! fun loadData() {
//!     viewModelScope.launch(Dispatchers.IO) {
//!         // ...
//!     }
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Cannot control threading in tests
//! - Tests become flaky
//! - Hard to debug race conditions
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! class Repository(
//!     private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO
//! ) {
//!     suspend fun fetchData() = withContext(ioDispatcher) {
//!         // Network call
//!     }
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for hardcoded Dispatcher usage
pub struct HardcodedDispatcherDetector {
    /// Minimum method size to consider
    min_method_bytes: usize,
}

impl HardcodedDispatcherDetector {
    pub fn new() -> Self {
        Self {
            min_method_bytes: 100,
        }
    }

    /// Check if method name suggests coroutine usage
    fn suggests_coroutine_usage(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("fetch")
            || lower.contains("load")
            || lower.contains("async")
            || lower.contains("suspend")
            || lower.contains("launch")
            || lower.contains("with")
            || lower.contains("flow")
    }

    /// Check if in a test file
    fn is_test_file(decl: &crate::graph::Declaration) -> bool {
        let file_path = decl.location.file.to_string_lossy().to_lowercase();
        file_path.contains("test") || file_path.contains("androidtest")
    }
}

impl Default for HardcodedDispatcherDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for HardcodedDispatcherDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Only check methods and functions
            if !matches!(
                decl.kind,
                DeclarationKind::Method | DeclarationKind::Function
            ) {
                continue;
            }

            // Skip test files (it's OK to use Dispatchers directly in tests)
            if Self::is_test_file(decl) {
                continue;
            }

            // Check method size
            let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
            if byte_size < self.min_method_bytes {
                continue;
            }

            // Check if method suggests coroutine usage
            if !Self::suggests_coroutine_usage(&decl.name) {
                continue;
            }

            // Flag methods that likely use dispatchers
            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::HardcodedDispatcher);
            dead = dead.with_message(format!(
                "Method '{}' may use hardcoded Dispatchers. Consider injecting CoroutineDispatcher for testability.",
                decl.name
            ));
            dead = dead.with_confidence(Confidence::Low);
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
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::path::PathBuf;

    fn create_method(name: &str, file: &str, line: usize, byte_size: usize) -> Declaration {
        let path = PathBuf::from(file);
        let start_byte = line * 100;
        let end_byte = start_byte + byte_size;
        Declaration::new(
            DeclarationId::new(path.clone(), start_byte, end_byte),
            name.to_string(),
            DeclarationKind::Method,
            Location::new(path, line, 1, start_byte, end_byte),
            Language::Kotlin,
        )
    }

    #[test]
    fn test_detector_creation() {
        let detector = HardcodedDispatcherDetector::new();
        assert!(detector.min_method_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = HardcodedDispatcherDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_fetch_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("fetchData", "Repository.kt", 1, 200));

        let detector = HardcodedDispatcherDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("fetchData"));
    }

    #[test]
    fn test_load_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("loadFromNetwork", "DataSource.kt", 1, 200));

        let detector = HardcodedDispatcherDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_test_file_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("fetchData", "RepositoryTest.kt", 1, 200));

        let detector = HardcodedDispatcherDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Test files should be OK");
    }

    #[test]
    fn test_small_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("fetchData", "Repository.kt", 1, 50));

        let detector = HardcodedDispatcherDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Small methods should be OK");
    }

    #[test]
    fn test_non_coroutine_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processData", "Processor.kt", 1, 200));

        let detector = HardcodedDispatcherDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Non-coroutine methods should be OK");
    }
}
