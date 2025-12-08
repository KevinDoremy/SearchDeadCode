//! Redundant This Reference Detector
//!
//! Detects unnecessary 'this.' references in Kotlin/Java code where the
//! reference is not needed for disambiguation.
//!
//! ## Detection Algorithm
//!
//! 1. Find all references that use 'this.'
//! 2. Check if there's a local variable/parameter with the same name
//! 3. If no shadowing, report as redundant
//!
//! ## Examples Detected
//!
//! ```kotlin
//! class Example {
//!     private var name: String = ""
//!
//!     fun setName(value: String) {
//!         this.name = value  // REDUNDANT: 'value' doesn't shadow 'name'
//!     }
//! }
//! ```
//!
//! ## Not Detected (this. is required)
//!
//! ```kotlin
//! class Example {
//!     private var name: String = ""
//!
//!     fun setName(name: String) {  // Parameter shadows field
//!         this.name = name  // REQUIRED: disambiguates
//!     }
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::Graph;

/// Detector for redundant this references
pub struct RedundantThisDetector {
    /// Report this. in getter/setter methods
    check_accessors: bool,
}

impl RedundantThisDetector {
    pub fn new() -> Self {
        Self {
            check_accessors: true,
        }
    }

    /// Skip checking accessor methods (get/set)
    #[allow(dead_code)]
    pub fn skip_accessors(mut self) -> Self {
        self.check_accessors = false;
        self
    }
}

impl Default for RedundantThisDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for RedundantThisDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // This detector requires AST-level analysis to:
        // 1. Find 'this.' references in method bodies
        // 2. Check parameter names in the containing method
        // 3. Determine if shadowing exists
        //
        // Current implementation is a placeholder.
        // Full implementation requires extending the parser to track:
        // - this.field references
        // - Method parameter names
        // - Local variable declarations

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
        let detector = RedundantThisDetector::new();
        assert!(detector.check_accessors);
    }

    #[test]
    fn test_skip_accessors_mode() {
        let detector = RedundantThisDetector::new().skip_accessors();
        assert!(!detector.check_accessors);
    }

    #[test]
    fn test_default_implementation() {
        let detector = RedundantThisDetector::default();
        assert!(detector.check_accessors);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = RedundantThisDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    // Note: More comprehensive tests will be added once AST-level
    // analysis is implemented to track 'this.' references and
    // parameter shadowing.
}
