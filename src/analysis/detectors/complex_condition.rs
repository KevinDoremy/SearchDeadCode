//! Complex Condition Detector
//!
//! Detects conditions with too many boolean operators.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! if (user.isActive && user.age >= 18 && user.age <= 120 &&
//!     user.email.isNotEmpty() && user.isVerified && !user.isBanned &&
//!     user.country == "US" || user.isPremium && user.hasSubscription) {
//!     // What does this even check?
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Hard to understand
//! - Easy to introduce bugs
//! - Difficult to test all branches
//! - Prone to operator precedence errors
//!
//! ## Better Alternatives
//!
//! - Extract to named boolean variables
//! - Create helper methods
//! - Use extension functions

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for complex boolean conditions
pub struct ComplexConditionDetector {
    /// Minimum method size to consider (larger = more likely to have complex conditions)
    min_method_bytes: usize,
}

impl ComplexConditionDetector {
    pub fn new() -> Self {
        Self {
            min_method_bytes: 300, // ~7-8 lines minimum
        }
    }

    /// Check if method name suggests conditional logic
    fn suggests_conditional_logic(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("valid")
            || lower.contains("check")
            || lower.contains("verify")
            || lower.contains("should")
            || lower.contains("can")
            || lower.contains("is")
            || lower.contains("has")
            || lower.starts_with("if")
    }
}

impl Default for ComplexConditionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for ComplexConditionDetector {
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

            // Check method size
            let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
            if byte_size < self.min_method_bytes {
                continue;
            }

            // Check if method suggests conditional logic
            if !Self::suggests_conditional_logic(&decl.name) {
                continue;
            }

            // Large validation/check methods likely have complex conditions
            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::ComplexCondition);
            dead = dead.with_message(format!(
                "Method '{}' may have complex conditions. Consider extracting to named booleans or helper methods.",
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
        let detector = ComplexConditionDetector::new();
        assert!(detector.min_method_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = ComplexConditionDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_validate_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("validateUser", 1, 400));

        let detector = ComplexConditionDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_check_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("checkPermissions", 1, 400));

        let detector = ComplexConditionDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_should_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("shouldProcess", 1, 400));

        let detector = ComplexConditionDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("isValid", 1, 100));

        let detector = ComplexConditionDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_non_conditional_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processData", 1, 400));

        let detector = ComplexConditionDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
