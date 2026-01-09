//! Collection Without Sequence Detector
//!
//! Detects chained collection operations without asSequence().
//! Without sequences, each operation creates an intermediate collection.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! items
//!     .filter { it.isActive }
//!     .map { it.name }
//!     .filter { it.isNotEmpty() }
//!     // Each step creates a new List
//! ```
//!
//! ## Why It's Bad
//!
//! - Creates intermediate collections at each step
//! - O(n) memory for each operation
//! - Slower for large collections
//! - Unnecessary allocations and GC pressure
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! items.asSequence()
//!     .filter { it.isActive }
//!     .map { it.name }
//!     .filter { it.isNotEmpty() }
//!     .toList()
//! // Single pass, lazy evaluation
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for chained collection operations without asSequence()
pub struct CollectionWithoutSequenceDetector {
    /// Collection operation names to track
    collection_operations: Vec<&'static str>,
    /// Minimum chain length to flag
    min_chain_length: usize,
}

impl CollectionWithoutSequenceDetector {
    pub fn new() -> Self {
        Self {
            collection_operations: vec![
                "filter",
                "map",
                "flatMap",
                "mapNotNull",
                "filterNot",
                "filterNotNull",
                "sortedBy",
                "sortedByDescending",
                "sorted",
                "take",
                "drop",
                "takeWhile",
                "dropWhile",
                "distinctBy",
                "distinct",
            ],
            min_chain_length: 2,
        }
    }

    /// Set minimum chain length before warning
    #[allow(dead_code)]
    pub fn with_min_chain_length(mut self, min: usize) -> Self {
        self.min_chain_length = min;
        self
    }

    /// Check if a method name suggests collection operations
    fn suggests_collection_processing(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        // Methods that commonly process collections
        lower.contains("process")
            || lower.contains("transform")
            || lower.contains("convert")
            || lower.contains("filter")
            || lower.contains("map")
    }

    /// Check if method has annotations suggesting data processing
    fn has_data_processing_annotations(decl: &crate::graph::Declaration) -> bool {
        decl.annotations.iter().any(|a| {
            let lower = a.to_lowercase();
            lower.contains("query") || lower.contains("transform")
        })
    }
}

impl Default for CollectionWithoutSequenceDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for CollectionWithoutSequenceDetector {
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

            // Look for methods that suggest collection processing
            let name_suggests = self.suggests_collection_processing(&decl.name);
            let has_annotations = Self::has_data_processing_annotations(decl);

            // Flag methods that likely process collections without sequence
            // This is a heuristic since we don't have access to method bodies
            if name_suggests || has_annotations {
                // Check method size - larger methods are more likely to have chains
                let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
                let estimated_lines = byte_size / 40;

                // Only flag if method is substantial enough to have multiple operations
                if estimated_lines >= 5 {
                    let mut dead =
                        DeadCode::new(decl.clone(), DeadCodeIssue::CollectionWithoutSequence);
                    dead = dead.with_message(format!(
                        "Method '{}' appears to process collections. Consider using asSequence() for chained operations on large collections.",
                        decl.name
                    ));
                    dead = dead.with_confidence(Confidence::Low);
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
        let detector = CollectionWithoutSequenceDetector::new();
        assert!(!detector.collection_operations.is_empty());
        assert_eq!(detector.min_chain_length, 2);
    }

    #[test]
    fn test_with_min_chain_length() {
        let detector = CollectionWithoutSequenceDetector::new().with_min_chain_length(3);
        assert_eq!(detector.min_chain_length, 3);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = CollectionWithoutSequenceDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_processing_method() {
        let mut graph = Graph::new();
        // Method that processes - 400 bytes ≈ 10 lines
        graph.add_declaration(create_method("processItems", 1, 400));

        let detector = CollectionWithoutSequenceDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("processItems"));
    }

    #[test]
    fn test_transform_method() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("transformData", 1, 400));

        let detector = CollectionWithoutSequenceDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_method_not_flagged() {
        let mut graph = Graph::new();
        // Small method - 100 bytes ≈ 2.5 lines
        graph.add_declaration(create_method("processItems", 1, 100));

        let detector = CollectionWithoutSequenceDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Small methods should not be flagged");
    }

    #[test]
    fn test_non_processing_method() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("getUserName", 1, 400));

        let detector = CollectionWithoutSequenceDetector::new();
        let issues = detector.detect(&graph);

        assert!(
            issues.is_empty(),
            "Methods without processing names should not be flagged"
        );
    }
}
