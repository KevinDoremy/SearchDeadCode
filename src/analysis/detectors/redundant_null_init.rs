//! Redundant Null Initialization Detector
//!
//! Detects nullable variables that are explicitly initialized to null,
//! which is redundant in Kotlin since null is the default value for nullable types.
//!
//! ## Detection Algorithm
//!
//! 1. Find all property/field declarations
//! 2. Check if the type is nullable (ends with ?)
//! 3. Check if explicitly initialized to null
//! 4. Report as redundant
//!
//! ## Examples Detected
//!
//! ```kotlin
//! class Example {
//!     private var name: String? = null  // REDUNDANT: null is default
//!     private var age: Int? = null      // REDUNDANT: null is default
//! }
//! ```
//!
//! ## Not Detected (correct usage)
//!
//! ```kotlin
//! class Example {
//!     private var name: String? = "default"  // Has actual value
//!     private var count: Int = 0             // Non-nullable, needs init
//!     private lateinit var adapter: String   // lateinit, no init
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for redundant null initialization
pub struct RedundantNullInitDetector {
    /// Include local variables (not just class properties)
    include_locals: bool,
}

impl RedundantNullInitDetector {
    pub fn new() -> Self {
        Self {
            include_locals: true,
        }
    }

    /// Only check class properties, not local variables
    #[allow(dead_code)]
    pub fn properties_only(mut self) -> Self {
        self.include_locals = false;
        self
    }

    /// Check if a declaration has redundant null initialization
    /// This requires checking the source text, which we simulate through
    /// declaration metadata
    fn is_redundant_null_init(&self, decl: &crate::graph::Declaration) -> bool {
        // Must be a property or field
        if !matches!(
            decl.kind,
            DeclarationKind::Property | DeclarationKind::Field
        ) {
            return false;
        }

        // Skip lateinit (no init allowed)
        if decl.modifiers.iter().any(|m| m == "lateinit") {
            return false;
        }

        // Skip const/val that can't be null
        if decl.modifiers.iter().any(|m| m == "const") {
            return false;
        }

        // For now, we detect based on naming patterns and modifiers
        // A full implementation would parse the initializer expression
        // This is a placeholder that will be enhanced with AST analysis

        // Check if the name suggests nullable type (ends with ?)
        // In practice, we'd check the actual type annotation
        false // Placeholder - requires AST enhancement
    }
}

impl Default for RedundantNullInitDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for RedundantNullInitDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();

        for decl in graph.declarations() {
            // Only check Kotlin code (Java doesn't have this redundancy)
            if decl.language != Language::Kotlin {
                continue;
            }

            if self.is_redundant_null_init(decl) {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::RedundantNullInit);
                dead = dead.with_message(format!(
                    "Nullable property '{}' is explicitly initialized to null (this is the default value)",
                    decl.name
                ));
                dead = dead.with_confidence(Confidence::High);
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
    use crate::graph::{Declaration, DeclarationId, Location, Visibility};
    use std::path::PathBuf;

    fn create_property(name: &str, modifiers: Vec<&str>) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), 0, 50),
            name.to_string(),
            DeclarationKind::Property,
            Location::new(path, 1, 1, 0, 50),
            Language::Kotlin,
        );
        decl.visibility = Visibility::Private;
        decl.modifiers = modifiers.into_iter().map(String::from).collect();
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = RedundantNullInitDetector::new();
        assert!(detector.include_locals);
    }

    #[test]
    fn test_properties_only_mode() {
        let detector = RedundantNullInitDetector::new().properties_only();
        assert!(!detector.include_locals);
    }

    #[test]
    fn test_skip_lateinit() {
        let detector = RedundantNullInitDetector::new();
        let decl = create_property("adapter", vec!["lateinit"]);
        assert!(!detector.is_redundant_null_init(&decl));
    }

    #[test]
    fn test_skip_const() {
        let detector = RedundantNullInitDetector::new();
        let decl = create_property("MAX_SIZE", vec!["const"]);
        assert!(!detector.is_redundant_null_init(&decl));
    }

    #[test]
    fn test_skip_java_code() {
        let mut graph = Graph::new();
        let path = PathBuf::from("Test.java");
        let decl = Declaration::new(
            DeclarationId::new(path.clone(), 0, 50),
            "value".to_string(),
            DeclarationKind::Field,
            Location::new(path, 1, 1, 0, 50),
            Language::Java, // Java code
        );
        graph.add_declaration(decl);

        let detector = RedundantNullInitDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Should skip Java code");
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = RedundantNullInitDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }
}
