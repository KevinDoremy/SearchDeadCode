//! Redundant Parentheses Detector
//!
//! Detects unnecessary parentheses around expressions that don't need them.
//!
//! ## Detection Algorithm
//!
//! 1. Find parenthesized expressions in AST
//! 2. Check if inner expression is:
//!    - Already parenthesized (double parens)
//!    - A simple literal or identifier
//!    - A function call result
//! 3. Check if parens are needed for:
//!    - Operator precedence
//!    - Method chaining on cast/elvis
//! 4. Report unnecessary parens
//!
//! ## Examples Detected
//!
//! ```kotlin
//! fun example() {
//!     val x = ((42))           // REDUNDANT: double parens
//!     if ((x > 0)) { }         // REDUNDANT: condition already in if()
//!     return (x)               // REDUNDANT: simple variable
//! }
//! ```
//!
//! ## Not Detected (parens are useful)
//!
//! ```kotlin
//! fun example() {
//!     val x = (a + b) * c      // Needed for precedence
//!     val y = (obj as String).length  // Needed for chaining
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::Graph;

/// Detector for redundant parentheses
pub struct RedundantParenthesesDetector {
    /// Report parens around single return values
    check_returns: bool,
    /// Report parens in when expressions
    check_when: bool,
}

impl RedundantParenthesesDetector {
    pub fn new() -> Self {
        Self {
            check_returns: true,
            check_when: true,
        }
    }

    /// Don't check return statements
    #[allow(dead_code)]
    pub fn skip_returns(mut self) -> Self {
        self.check_returns = false;
        self
    }

    /// Don't check when expressions
    #[allow(dead_code)]
    pub fn skip_when(mut self) -> Self {
        self.check_when = false;
        self
    }
}

impl Default for RedundantParenthesesDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for RedundantParenthesesDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // This detector requires AST-level analysis to:
        // 1. Find parenthesized expressions
        // 2. Analyze the inner expression type
        // 3. Check surrounding context (operators, method calls)
        //
        // Current implementation is a placeholder.
        // Full implementation requires extending the parser to:
        // - Track parenthesized expressions
        // - Understand expression precedence
        // - Detect double parentheses

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
        let detector = RedundantParenthesesDetector::new();
        assert!(detector.check_returns);
        assert!(detector.check_when);
    }

    #[test]
    fn test_skip_returns_mode() {
        let detector = RedundantParenthesesDetector::new().skip_returns();
        assert!(!detector.check_returns);
        assert!(detector.check_when);
    }

    #[test]
    fn test_skip_when_mode() {
        let detector = RedundantParenthesesDetector::new().skip_when();
        assert!(detector.check_returns);
        assert!(!detector.check_when);
    }

    #[test]
    fn test_default_implementation() {
        let detector = RedundantParenthesesDetector::default();
        assert!(detector.check_returns);
        assert!(detector.check_when);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = RedundantParenthesesDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    // Note: More comprehensive tests will be added once AST-level
    // analysis is implemented to detect parenthesized expressions.
}
