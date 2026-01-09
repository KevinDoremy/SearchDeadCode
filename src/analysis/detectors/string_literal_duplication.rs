//! String Literal Duplication Detector
//!
//! Detects repeated string literals (magic strings).
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! fun saveData() {
//!     prefs.putString("user_name", name)
//! }
//! fun loadData() {
//!     prefs.getString("user_name", "")  // Duplicated!
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Typos cause silent bugs
//! - Hard to refactor (find/replace is error-prone)
//! - No IDE support for navigation
//! - Can't be verified at compile time
//!
//! ## Better Alternatives
//!
//! - Extract to constants
//! - Use sealed class/enum for keys
//! - Use object with const properties

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for duplicated string literals
pub struct StringLiteralDuplicationDetector {
    /// Minimum class size to consider
    min_class_bytes: usize,
}

impl StringLiteralDuplicationDetector {
    pub fn new() -> Self {
        Self {
            min_class_bytes: 500, // ~12 lines minimum
        }
    }

    /// Check if class name suggests it might have magic strings
    fn suggests_magic_strings(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("preferences")
            || lower.contains("prefs")
            || lower.contains("intent")
            || lower.contains("bundle")
            || lower.contains("api")
            || lower.contains("endpoint")
            || lower.contains("constants")
            || lower.contains("keys")
    }

    /// Check if class has a companion object (where constants should be)
    fn has_companion_object(decl: &crate::graph::Declaration, graph: &Graph) -> bool {
        graph
            .get_children(&decl.id)
            .iter()
            .filter_map(|id| graph.get_declaration(id))
            .any(|child| {
                matches!(child.kind, DeclarationKind::Object)
                    && child.name.to_lowercase().contains("companion")
            })
    }
}

impl Default for StringLiteralDuplicationDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for StringLiteralDuplicationDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Only check classes
            if !matches!(decl.kind, DeclarationKind::Class) {
                continue;
            }

            // Check class size
            let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
            if byte_size < self.min_class_bytes {
                continue;
            }

            // Check if class suggests magic strings
            if !Self::suggests_magic_strings(&decl.name) {
                continue;
            }

            // Classes that handle prefs/intents but don't have companion objects
            // likely have magic strings
            if Self::has_companion_object(decl, graph) {
                continue; // Already has constants
            }

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::StringLiteralDuplication);
            dead = dead.with_message(format!(
                "Class '{}' may have duplicated string literals. Consider extracting to constants in a companion object.",
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

    fn create_class(name: &str, line: usize, byte_size: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let start_byte = line * 100;
        let end_byte = start_byte + byte_size;
        Declaration::new(
            DeclarationId::new(path.clone(), start_byte, end_byte),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, start_byte, end_byte),
            Language::Kotlin,
        )
    }

    fn create_companion(parent_id: DeclarationId, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 100),
            "Companion".to_string(),
            DeclarationKind::Object,
            Location::new(path, line, 1, line * 100, line * 100 + 100),
            Language::Kotlin,
        );
        decl.parent = Some(parent_id);
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = StringLiteralDuplicationDetector::new();
        assert!(detector.min_class_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = StringLiteralDuplicationDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_prefs_class_without_companion() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class("UserPreferences", 1, 600));

        let detector = StringLiteralDuplicationDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_intent_class_without_companion() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class("IntentBuilder", 1, 600));

        let detector = StringLiteralDuplicationDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_class_with_companion_ok() {
        let mut graph = Graph::new();
        let cls = create_class("UserPreferences", 1, 600);
        let cls_id = cls.id.clone();
        graph.add_declaration(cls);
        graph.add_declaration(create_companion(cls_id, 2));

        let detector = StringLiteralDuplicationDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_small_class_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class("UserPreferences", 1, 200));

        let detector = StringLiteralDuplicationDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_unrelated_class_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class("UserViewModel", 1, 600));

        let detector = StringLiteralDuplicationDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
