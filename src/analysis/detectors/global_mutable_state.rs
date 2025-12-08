//! Global Mutable State Detector
//!
//! Detects Kotlin `object` declarations that have public mutable properties (`var`).
//! This is a common anti-pattern that leads to hard-to-debug state issues.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! object GlobalState {
//!     var currentUser: String? = null  // BAD: global mutable state
//!     var isLoggedIn: Boolean = false  // BAD: anyone can modify
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Hard to unit test (state persists between tests)
//! - Race conditions in multi-threaded code
//! - Implicit dependencies (any code can read/write)
//! - Difficult to debug (state can change from anywhere)
//!
//! ## Better Alternatives
//!
//! - Use dependency injection for state management
//! - Use `val` for immutable state
//! - Use private `var` with controlled access
//! - Use proper state management (ViewModel, StateFlow)

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Visibility};

/// Detector for global mutable state in Kotlin objects
pub struct GlobalMutableStateDetector {
    /// Also check companion objects
    check_companion_objects: bool,
    /// Minimum number of public vars to report
    min_public_vars: usize,
}

impl GlobalMutableStateDetector {
    pub fn new() -> Self {
        Self {
            check_companion_objects: true,
            min_public_vars: 1,
        }
    }

    /// Set minimum number of public vars to report
    #[allow(dead_code)]
    pub fn with_min_vars(mut self, min: usize) -> Self {
        self.min_public_vars = min;
        self
    }

    /// Check if a declaration is a Kotlin object
    fn is_kotlin_object(&self, decl: &crate::graph::Declaration) -> bool {
        decl.kind == DeclarationKind::Object
    }

    /// Check if a property is a mutable public var
    fn is_public_mutable_var(&self, decl: &crate::graph::Declaration) -> bool {
        // Must be a property
        if decl.kind != DeclarationKind::Property {
            return false;
        }

        // Must be public (or default visibility in Kotlin which is public)
        if decl.visibility == Visibility::Private || decl.visibility == Visibility::Internal {
            return false;
        }

        // Check if it's a var (mutable) - we detect this through modifiers
        // In Kotlin, vars don't have a "val" modifier, vals do
        // We check if the property has "var" in modifiers or doesn't have "val"
        let has_val = decl.modifiers.iter().any(|m| m == "val");
        let has_const = decl.modifiers.iter().any(|m| m == "const");

        // If it has val or const, it's immutable
        !has_val && !has_const
    }
}

impl Default for GlobalMutableStateDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for GlobalMutableStateDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();

        // Find all Kotlin objects
        for decl in graph.declarations() {
            if !self.is_kotlin_object(decl) {
                continue;
            }

            // Get children (properties) of this object
            let children = graph.get_children(&decl.id);

            // Count public mutable vars
            let public_vars: Vec<_> = children
                .iter()
                .filter_map(|child_id| graph.get_declaration(child_id))
                .filter(|child| self.is_public_mutable_var(child))
                .collect();

            if public_vars.len() >= self.min_public_vars {
                let var_names: Vec<_> = public_vars.iter().map(|v| v.name.as_str()).collect();
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::GlobalMutableState);
                dead = dead.with_message(format!(
                    "Object '{}' has {} public mutable var(s): {}. Consider using dependency injection or making them private.",
                    decl.name,
                    public_vars.len(),
                    var_names.join(", ")
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

    fn create_property(name: &str, line: usize, visibility: Visibility, modifiers: Vec<&str>) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 30),
            name.to_string(),
            DeclarationKind::Property,
            Location::new(path, line, 1, line * 100, line * 100 + 30),
            Language::Kotlin,
        );
        decl.visibility = visibility;
        decl.modifiers = modifiers.into_iter().map(String::from).collect();
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = GlobalMutableStateDetector::new();
        assert!(detector.check_companion_objects);
        assert_eq!(detector.min_public_vars, 1);
    }

    #[test]
    fn test_is_kotlin_object() {
        let detector = GlobalMutableStateDetector::new();
        let object = create_object("GlobalState", 1);
        assert!(detector.is_kotlin_object(&object));
    }

    #[test]
    fn test_is_public_mutable_var() {
        let detector = GlobalMutableStateDetector::new();

        // Public var (no val/const modifier)
        let public_var = create_property("state", 1, Visibility::Public, vec![]);
        assert!(detector.is_public_mutable_var(&public_var));

        // Public val (immutable)
        let public_val = create_property("constant", 2, Visibility::Public, vec!["val"]);
        assert!(!detector.is_public_mutable_var(&public_val));

        // Private var (not exposed)
        let private_var = create_property("internal", 3, Visibility::Private, vec![]);
        assert!(!detector.is_public_mutable_var(&private_var));

        // Const val (immutable)
        let const_val = create_property("MAX", 4, Visibility::Public, vec!["const"]);
        assert!(!detector.is_public_mutable_var(&const_val));
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = GlobalMutableStateDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_object_without_vars() {
        let mut graph = Graph::new();

        let object = create_object("Utils", 1);
        let object_id = graph.add_declaration(object);

        // Add a val (immutable)
        let mut val = create_property("VERSION", 2, Visibility::Public, vec!["val"]);
        val.parent = Some(object_id);
        graph.add_declaration(val);

        let detector = GlobalMutableStateDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Object with only vals should not be reported");
    }
}
