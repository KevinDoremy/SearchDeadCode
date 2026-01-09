//! Long Method Detector
//!
//! Detects methods that exceed a configurable line threshold.
//! Long methods are harder to understand, test, and maintain.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! fun processData(data: List<Item>): Result {
//!     // 100+ lines of code doing multiple things
//!     // validation, transformation, logging, error handling...
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Hard to understand at a glance
//! - Difficult to test individual logic
//! - Often indicates multiple responsibilities
//! - Higher cognitive complexity
//!
//! ## Better Alternatives
//!
//! - Extract smaller, focused methods
//! - Use meaningful method names as documentation
//! - Apply single responsibility principle
//! - Consider using strategy or command pattern

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for methods that are too long
pub struct LongMethodDetector {
    /// Maximum allowed lines per method
    max_lines: usize,
}

impl LongMethodDetector {
    pub fn new() -> Self {
        Self { max_lines: 50 }
    }

    /// Set maximum lines before warning
    #[allow(dead_code)]
    pub fn with_max_lines(mut self, max: usize) -> Self {
        self.max_lines = max;
        self
    }

    /// Calculate approximate line count from byte range
    fn estimate_lines(decl: &crate::graph::Declaration) -> usize {
        let byte_range = decl.location.end_byte.saturating_sub(decl.location.start_byte);
        // Rough estimate: average 40 bytes per line
        (byte_range / 40).max(1)
    }
}

impl Default for LongMethodDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for LongMethodDetector {
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

            // Skip constructors - they can legitimately be long for initialization
            if matches!(decl.kind, DeclarationKind::Constructor) {
                continue;
            }

            // Estimate line count
            let estimated_lines = Self::estimate_lines(decl);

            if estimated_lines > self.max_lines {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::LongMethod);
                dead = dead.with_message(format!(
                    "Method '{}' has approximately {} lines (max recommended: {}). Consider breaking into smaller methods.",
                    decl.name, estimated_lines, self.max_lines
                ));
                dead = dead.with_confidence(Confidence::Medium);
                issues.push(dead);
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
        let detector = LongMethodDetector::new();
        assert_eq!(detector.max_lines, 50);
    }

    #[test]
    fn test_with_max_lines() {
        let detector = LongMethodDetector::new().with_max_lines(100);
        assert_eq!(detector.max_lines, 100);
    }

    #[test]
    fn test_estimate_lines() {
        // 2000 bytes / 40 = 50 lines
        let method = create_method("longMethod", 1, 2000);
        assert_eq!(LongMethodDetector::estimate_lines(&method), 50);

        // 4000 bytes / 40 = 100 lines
        let longer = create_method("longerMethod", 1, 4000);
        assert_eq!(LongMethodDetector::estimate_lines(&longer), 100);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = LongMethodDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_short_method_ok() {
        let mut graph = Graph::new();
        // 800 bytes ≈ 20 lines
        graph.add_declaration(create_method("shortMethod", 1, 800));

        let detector = LongMethodDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Short methods should be OK");
    }

    #[test]
    fn test_long_method_detected() {
        let mut graph = Graph::new();
        // 2400 bytes ≈ 60 lines (exceeds 50)
        graph.add_declaration(create_method("veryLongMethod", 1, 2400));

        let detector = LongMethodDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].declaration.name, "veryLongMethod");
        assert!(issues[0].message.contains("60 lines"));
    }

    #[test]
    fn test_custom_threshold() {
        let mut graph = Graph::new();
        // 1200 bytes ≈ 30 lines
        graph.add_declaration(create_method("mediumMethod", 1, 1200));

        // Default (50) should not flag
        let detector = LongMethodDetector::new();
        assert!(detector.detect(&graph).is_empty());

        // Lower threshold (25) should flag
        let strict_detector = LongMethodDetector::new().with_max_lines(25);
        let issues = strict_detector.detect(&graph);
        assert_eq!(issues.len(), 1);
    }
}
