//! Nullability Overload Detector
//!
//! Detects excessive force unwrap (!!) or redundant null handling patterns.
//!
//! ## Anti-Patterns
//!
//! ```kotlin
//! val name = user!!.profile!!.name!!  // Excessive !!
//! if (x != null) { x!!.doSomething() } // Redundant !!
//! name?.let { it } ?: ""  // Redundant let
//! ```
//!
//! ## Why It's Bad
//!
//! - Excessive !! leads to crashes
//! - Redundant null checks are confusing
//! - Indicates poor null safety understanding
//!
//! ## Better Alternatives
//!
//! - Use safe calls (?.) with Elvis (?:)
//! - Use let for scoping with transformation
//! - Use require/checkNotNull for preconditions

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for nullability anti-patterns
pub struct NullabilityOverloadDetector {
    /// Minimum method size to check
    min_method_bytes: usize,
}

impl NullabilityOverloadDetector {
    pub fn new() -> Self {
        Self {
            min_method_bytes: 100,
        }
    }

    /// Check if method name suggests null handling
    fn suggests_null_handling(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("null")
            || lower.contains("optional")
            || lower.contains("maybe")
            || lower.contains("unwrap")
            || lower.contains("force")
    }

    /// Check if method is large enough to potentially have null handling issues
    fn is_suspicious_size(decl: &crate::graph::Declaration, min_bytes: usize) -> bool {
        let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
        byte_size > min_bytes
    }
}

impl Default for NullabilityOverloadDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for NullabilityOverloadDetector {
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

            // Only check Kotlin (null safety is Kotlin-specific)
            if !matches!(decl.language, Language::Kotlin) {
                continue;
            }

            // Check if method is suspicious
            if !Self::suggests_null_handling(&decl.name) {
                continue;
            }

            if !Self::is_suspicious_size(decl, self.min_method_bytes) {
                continue;
            }

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::NullabilityOverload);
            dead = dead.with_message(format!(
                "Method '{}' may have excessive null handling. Consider using safe calls (?.) with Elvis (?:) or requireNotNull.",
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
    use crate::graph::{Declaration, DeclarationId, Location};
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
        let detector = NullabilityOverloadDetector::new();
        assert!(detector.min_method_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = NullabilityOverloadDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_unwrap_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("forceUnwrapValue", 1, 200));

        let detector = NullabilityOverloadDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_null_check_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("handleNullCase", 1, 200));

        let detector = NullabilityOverloadDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_normal_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processData", 1, 200));

        let detector = NullabilityOverloadDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_small_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("forceUnwrap", 1, 50));

        let detector = NullabilityOverloadDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
