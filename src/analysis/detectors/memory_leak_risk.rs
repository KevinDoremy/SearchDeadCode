//! Memory Leak Risk Detector
//!
//! Detects common memory leak patterns in Android applications.
//! These patterns often lead to Activity/Context leaks.
//!
//! ## Anti-Patterns Detected
//!
//! 1. Static references to Context, Activity, or View
//! 2. Singleton holding Activity reference
//! 3. Companion object with Context/Activity/View properties
//!
//! ## Why It's Bad
//!
//! - Activity/Fragment cannot be garbage collected
//! - Memory grows unbounded as user navigates
//! - Eventually leads to OutOfMemoryError
//! - App becomes sluggish and unresponsive
//!
//! ## Better Alternatives
//!
//! - Use Application context for long-lived references
//! - Use WeakReference for callbacks
//! - Clear references in onDestroy/onCleared
//! - Use lifecycle-aware components

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for memory leak risks in Android code
pub struct MemoryLeakRiskDetector {
    /// Types that should not be held statically
    leak_prone_types: Vec<String>,
}

impl MemoryLeakRiskDetector {
    pub fn new() -> Self {
        Self {
            leak_prone_types: vec![
                "Context".to_string(),
                "Activity".to_string(),
                "Fragment".to_string(),
                "View".to_string(),
                "Bitmap".to_string(),
                "Drawable".to_string(),
                "Handler".to_string(),
                "Dialog".to_string(),
                "Toast".to_string(),
                "WindowManager".to_string(),
            ],
        }
    }

    /// Check if a type name is leak-prone (case-insensitive)
    fn is_leak_prone_type(&self, name: &str) -> bool {
        let lower_name = name.to_lowercase();
        self.leak_prone_types
            .iter()
            .any(|t| lower_name.contains(&t.to_lowercase()))
    }

    /// Check if declaration is in a static context (object, companion object)
    fn is_static_context(decl: &crate::graph::Declaration) -> bool {
        decl.is_static
            || decl.modifiers.iter().any(|m| m == "static")
    }

    /// Check if parent is a Kotlin object or companion object
    fn is_in_object_or_companion(&self, decl: &crate::graph::Declaration, graph: &Graph) -> bool {
        if let Some(ref parent_id) = decl.parent {
            if let Some(parent) = graph.get_declaration(parent_id) {
                return matches!(parent.kind, DeclarationKind::Object)
                    || parent.name.contains("Companion");
            }
        }
        false
    }
}

impl Default for MemoryLeakRiskDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for MemoryLeakRiskDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Check properties and fields
            if !matches!(decl.kind, DeclarationKind::Property | DeclarationKind::Field) {
                continue;
            }

            // Check if the property name or type indicates a leak-prone type
            if !self.is_leak_prone_type(&decl.name) {
                continue;
            }

            // Check if it's in a static context
            let is_static = Self::is_static_context(decl);
            let is_in_object = self.is_in_object_or_companion(decl, graph);

            if is_static || is_in_object {
                let context = if is_in_object {
                    "object/companion object"
                } else {
                    "static field"
                };

                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::MemoryLeakRisk);
                dead = dead.with_message(format!(
                    "Property '{}' in {} holds a leak-prone type. Consider using WeakReference or Application context.",
                    decl.name, context
                ));
                dead = dead.with_confidence(Confidence::Medium);
                issues.push(dead);
            }
        }

        // Also check objects that hold leak-prone properties
        for decl in graph.declarations() {
            if !matches!(decl.kind, DeclarationKind::Object) {
                continue;
            }

            // Check if the object name suggests it holds context
            if decl.name.contains("Holder")
                || decl.name.contains("Cache")
                || decl.name.contains("Manager")
            {
                // Check children for leak-prone types
                for child_id in graph.get_children(&decl.id) {
                    if let Some(child) = graph.get_declaration(child_id) {
                        if matches!(child.kind, DeclarationKind::Property | DeclarationKind::Field)
                            && self.is_leak_prone_type(&child.name)
                        {
                            let mut dead =
                                DeadCode::new(decl.clone(), DeadCodeIssue::MemoryLeakRisk);
                            dead = dead.with_message(format!(
                                "Object '{}' contains leak-prone property '{}'. Singleton with Context/Activity reference causes memory leaks.",
                                decl.name, child.name
                            ));
                            dead = dead.with_confidence(Confidence::High);
                            issues.push(dead);
                            break; // One warning per object
                        }
                    }
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

        // Deduplicate
        issues.dedup_by(|a, b| a.declaration.id == b.declaration.id);

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::path::PathBuf;

    fn create_object(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Object,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        )
    }

    fn create_property(
        name: &str,
        parent_id: Option<DeclarationId>,
        line: usize,
        is_static: bool,
    ) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 30),
            name.to_string(),
            DeclarationKind::Property,
            Location::new(path, line, 1, line * 100, line * 100 + 30),
            Language::Kotlin,
        );
        decl.parent = parent_id;
        decl.is_static = is_static;
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = MemoryLeakRiskDetector::new();
        assert!(!detector.leak_prone_types.is_empty());
    }

    #[test]
    fn test_is_leak_prone_type() {
        let detector = MemoryLeakRiskDetector::new();
        assert!(detector.is_leak_prone_type("context"));
        assert!(detector.is_leak_prone_type("appContext"));
        assert!(detector.is_leak_prone_type("activityRef"));
        assert!(detector.is_leak_prone_type("mainView"));
        assert!(!detector.is_leak_prone_type("userName"));
        assert!(!detector.is_leak_prone_type("settings"));
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = MemoryLeakRiskDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_static_context_property() {
        let mut graph = Graph::new();
        graph.add_declaration(create_property("context", None, 1, true));

        let detector = MemoryLeakRiskDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("static field"));
    }

    #[test]
    fn test_object_with_context_property() {
        let mut graph = Graph::new();
        let obj = create_object("ContextHolder", 1);
        let obj_id = obj.id.clone();
        graph.add_declaration(obj);
        graph.add_declaration(create_property("context", Some(obj_id), 2, false));

        let detector = MemoryLeakRiskDetector::new();
        let issues = detector.detect(&graph);

        // Should detect property in object
        assert!(!issues.is_empty());
    }

    #[test]
    fn test_non_leak_prone_property() {
        let mut graph = Graph::new();
        graph.add_declaration(create_property("userName", None, 1, true));

        let detector = MemoryLeakRiskDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Non-leak-prone types should not be flagged");
    }

    #[test]
    fn test_instance_property_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_property("context", None, 1, false));

        let detector = MemoryLeakRiskDetector::new();
        let issues = detector.detect(&graph);

        assert!(
            issues.is_empty(),
            "Instance properties without object parent should be OK"
        );
    }
}
