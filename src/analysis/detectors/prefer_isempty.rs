//! Prefer isEmpty() Detector
//!
//! Detects size/length comparisons to zero that should use isEmpty()/isNotEmpty().
//!
//! ## Detection Algorithm
//!
//! 1. Find comparison expressions (==, !=, >, <, >=, <=)
//! 2. Check if one side is .size or .length
//! 3. Check if other side is 0
//! 4. Report with suggestion to use isEmpty()/isNotEmpty()
//!
//! ## Examples Detected
//!
//! ```kotlin
//! fun example(list: List<String>) {
//!     if (list.size == 0) { }     // PREFER: list.isEmpty()
//!     if (list.size != 0) { }     // PREFER: list.isNotEmpty()
//!     if (list.size > 0) { }      // PREFER: list.isNotEmpty()
//!     if (s.length == 0) { }      // PREFER: s.isEmpty()
//! }
//! ```
//!
//! ## Not Detected (correct or different)
//!
//! ```kotlin
//! fun example(list: List<String>) {
//!     if (list.isEmpty()) { }     // Already correct
//!     if (list.size == 5) { }     // Checking specific size
//!     if (list.size >= 3) { }     // Checking minimum size
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::Graph;

/// Detector for size/length comparisons that should use isEmpty()
pub struct PreferIsEmptyDetector {
    /// Check string.length comparisons
    check_strings: bool,
    /// Check collection.size comparisons
    check_collections: bool,
    /// Check array.size comparisons
    check_arrays: bool,
}

impl PreferIsEmptyDetector {
    pub fn new() -> Self {
        Self {
            check_strings: true,
            check_collections: true,
            check_arrays: true,
        }
    }

    /// Only check collections, not strings
    #[allow(dead_code)]
    pub fn collections_only(mut self) -> Self {
        self.check_strings = false;
        self.check_arrays = false;
        self
    }
}

impl Default for PreferIsEmptyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for PreferIsEmptyDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // This detector requires AST-level analysis to:
        // 1. Find comparison expressions
        // 2. Check for .size or .length on one side
        // 3. Check for 0 on the other side
        // 4. Determine the comparison operator
        //
        // Current implementation is a placeholder.
        // Full implementation requires extending the parser to:
        // - Track comparison expressions
        // - Identify property access (.size, .length)
        // - Match literal values (0)

        // Placeholder - will be enhanced with full AST analysis

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

    #[test]
    fn test_detector_creation() {
        let detector = PreferIsEmptyDetector::new();
        assert!(detector.check_strings);
        assert!(detector.check_collections);
        assert!(detector.check_arrays);
    }

    #[test]
    fn test_collections_only_mode() {
        let detector = PreferIsEmptyDetector::new().collections_only();
        assert!(!detector.check_strings);
        assert!(detector.check_collections);
        assert!(!detector.check_arrays);
    }

    #[test]
    fn test_default_implementation() {
        let detector = PreferIsEmptyDetector::default();
        assert!(detector.check_strings);
        assert!(detector.check_collections);
        assert!(detector.check_arrays);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = PreferIsEmptyDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    // Note: More comprehensive tests will be added once AST-level
    // analysis is implemented to detect size/length comparisons.
}
