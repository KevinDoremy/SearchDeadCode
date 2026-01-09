//! Unclosed Resource Detector
//!
//! Detects resources (Cursor, Stream, etc.) that may not be properly closed.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! fun readFile(file: File): String {
//!     val stream = FileInputStream(file)
//!     return stream.bufferedReader().readText()
//!     // stream.close() is missing!
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Resource leaks (file handles, connections)
//! - Memory pressure
//! - Can cause app crashes
//! - Database cursors are especially problematic
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! fun readFile(file: File): String {
//!     return FileInputStream(file).use { stream ->
//!         stream.bufferedReader().readText()
//!     }
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for unclosed resources
pub struct UnclosedResourceDetector {
    /// Resource-related keywords
    resource_keywords: Vec<&'static str>,
}

impl UnclosedResourceDetector {
    pub fn new() -> Self {
        Self {
            resource_keywords: vec![
                "cursor",
                "stream",
                "reader",
                "writer",
                "connection",
                "socket",
                "channel",
                "input",
                "output",
            ],
        }
    }

    /// Check if method name suggests resource handling
    fn handles_resources(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.resource_keywords
            .iter()
            .any(|&kw| lower.contains(kw))
            || lower.contains("read")
            || lower.contains("write")
            || lower.contains("open")
            || lower.contains("query")
    }

    /// Check if method is large enough to potentially have resource issues
    fn is_large_method(decl: &crate::graph::Declaration) -> bool {
        let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
        byte_size > 150 // ~4 lines minimum
    }
}

impl Default for UnclosedResourceDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for UnclosedResourceDetector {
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

            // Check if method handles resources
            if !self.handles_resources(&decl.name) {
                continue;
            }

            // Check method size
            if !Self::is_large_method(decl) {
                continue;
            }

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::UnclosedResource);
            dead = dead.with_message(format!(
                "Method '{}' handles resources. Ensure proper cleanup with .use {{}} or try-finally.",
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

    fn create_method(name: &str, line: usize, byte_size: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
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
        let detector = UnclosedResourceDetector::new();
        assert!(!detector.resource_keywords.is_empty());
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = UnclosedResourceDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_read_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("readFile", 1, 200));

        let detector = UnclosedResourceDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_cursor_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("queryCursor", 1, 200));

        let detector = UnclosedResourceDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_stream_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("openInputStream", 1, 200));

        let detector = UnclosedResourceDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("readFile", 1, 50));

        let detector = UnclosedResourceDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_unrelated_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processData", 1, 200));

        let detector = UnclosedResourceDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
