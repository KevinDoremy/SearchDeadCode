//! Lateinit Abuse Detector
//!
//! Detects excessive use of `lateinit` properties in Kotlin classes.
//! While lateinit is useful for dependency injection, overuse is a code smell.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! class TooManyLateinit {
//!     lateinit var a: String
//!     lateinit var b: String
//!     lateinit var c: String
//!     lateinit var d: String
//!     lateinit var e: String  // 5+ lateinit is a smell
//!
//!     // Risk of UninitializedPropertyAccessException
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Risk of `UninitializedPropertyAccessException` at runtime
//! - Harder to reason about object state
//! - Often indicates missing constructor injection
//! - Can hide initialization order bugs
//!
//! ## Better Alternatives
//!
//! - Constructor injection for required dependencies
//! - `lazy { }` for expensive computed properties
//! - Nullable types with default null for truly optional
//! - Only use lateinit with @Inject annotation for DI

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationId, DeclarationKind, Graph};
use std::collections::HashMap;

/// Detector for excessive lateinit usage
pub struct LateinitAbuseDetector {
    /// Maximum allowed lateinit properties per class
    max_lateinit: usize,
}

impl LateinitAbuseDetector {
    pub fn new() -> Self {
        Self { max_lateinit: 5 }
    }

    /// Set maximum lateinit properties before warning
    #[allow(dead_code)]
    pub fn with_max_lateinit(mut self, max: usize) -> Self {
        self.max_lateinit = max;
        self
    }

    /// Check if a property is marked as lateinit
    fn is_lateinit(decl: &crate::graph::Declaration) -> bool {
        decl.modifiers.iter().any(|m| m == "lateinit")
    }

    /// Check if a property has @Inject annotation (acceptable lateinit usage)
    fn has_inject_annotation(decl: &crate::graph::Declaration) -> bool {
        decl.annotations
            .iter()
            .any(|a| a.contains("Inject") || a.contains("Autowired"))
    }
}

impl Default for LateinitAbuseDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for LateinitAbuseDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // Group lateinit properties by their parent class
        let mut class_lateinit_count: HashMap<&DeclarationId, Vec<&crate::graph::Declaration>> =
            HashMap::new();

        // Find all lateinit properties
        for decl in graph.declarations() {
            if !matches!(decl.kind, DeclarationKind::Property | DeclarationKind::Field) {
                continue;
            }

            if !Self::is_lateinit(decl) {
                continue;
            }

            // Skip if it has @Inject annotation (valid use case)
            if Self::has_inject_annotation(decl) {
                continue;
            }

            // Group by parent class
            if let Some(ref parent_id) = decl.parent {
                class_lateinit_count
                    .entry(parent_id)
                    .or_default()
                    .push(decl);
            }
        }

        // Check each class for excessive lateinit
        for (parent_id, lateinit_props) in class_lateinit_count {
            if lateinit_props.len() > self.max_lateinit {
                // Find the class declaration
                if let Some(class_decl) = graph.get_declaration(parent_id) {
                    let prop_names: Vec<_> =
                        lateinit_props.iter().map(|p| p.name.as_str()).collect();
                    let mut dead =
                        DeadCode::new(class_decl.clone(), DeadCodeIssue::LateinitAbuse);
                    dead = dead.with_message(format!(
                        "Class '{}' has {} lateinit properties ({}). Consider constructor injection or lazy initialization.",
                        class_decl.name,
                        lateinit_props.len(),
                        prop_names.join(", ")
                    ));
                    dead = dead.with_confidence(Confidence::Medium);
                    issues.push(dead);
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

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::path::PathBuf;

    fn create_class(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        )
    }

    fn create_lateinit_property(
        name: &str,
        parent_id: DeclarationId,
        line: usize,
        has_inject: bool,
    ) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Property,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.modifiers = vec!["lateinit".to_string()];
        decl.parent = Some(parent_id);
        if has_inject {
            decl.annotations = vec!["Inject".to_string()];
        }
        decl
    }

    fn create_regular_property(name: &str, parent_id: DeclarationId, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Property,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.parent = Some(parent_id);
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = LateinitAbuseDetector::new();
        assert_eq!(detector.max_lateinit, 5);
    }

    #[test]
    fn test_with_max_lateinit() {
        let detector = LateinitAbuseDetector::new().with_max_lateinit(10);
        assert_eq!(detector.max_lateinit, 10);
    }

    #[test]
    fn test_is_lateinit() {
        let class = create_class("User", 1);
        let lateinit_prop = create_lateinit_property("name", class.id.clone(), 2, false);
        let regular_prop = create_regular_property("age", class.id, 3);

        assert!(LateinitAbuseDetector::is_lateinit(&lateinit_prop));
        assert!(!LateinitAbuseDetector::is_lateinit(&regular_prop));
    }

    #[test]
    fn test_has_inject_annotation() {
        let class = create_class("Service", 1);
        let with_inject = create_lateinit_property("repo", class.id.clone(), 2, true);
        let without_inject = create_lateinit_property("name", class.id, 3, false);

        assert!(LateinitAbuseDetector::has_inject_annotation(&with_inject));
        assert!(!LateinitAbuseDetector::has_inject_annotation(&without_inject));
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = LateinitAbuseDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_within_limit() {
        let mut graph = Graph::new();
        let class = create_class("User", 1);
        let class_id = class.id.clone();
        graph.add_declaration(class);
        graph.add_declaration(create_lateinit_property("a", class_id.clone(), 2, false));
        graph.add_declaration(create_lateinit_property("b", class_id.clone(), 3, false));
        graph.add_declaration(create_lateinit_property("c", class_id, 4, false));

        let detector = LateinitAbuseDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "3 lateinit should be OK");
    }

    #[test]
    fn test_exceeds_limit() {
        let mut graph = Graph::new();
        let class = create_class("TooManyLateinit", 1);
        let class_id = class.id.clone();
        graph.add_declaration(class);
        graph.add_declaration(create_lateinit_property("a", class_id.clone(), 2, false));
        graph.add_declaration(create_lateinit_property("b", class_id.clone(), 3, false));
        graph.add_declaration(create_lateinit_property("c", class_id.clone(), 4, false));
        graph.add_declaration(create_lateinit_property("d", class_id.clone(), 5, false));
        graph.add_declaration(create_lateinit_property("e", class_id.clone(), 6, false));
        graph.add_declaration(create_lateinit_property("f", class_id, 7, false));

        let detector = LateinitAbuseDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("6 lateinit"));
    }

    #[test]
    fn test_inject_properties_excluded() {
        let mut graph = Graph::new();
        let class = create_class("InjectedService", 1);
        let class_id = class.id.clone();
        graph.add_declaration(class);
        graph.add_declaration(create_lateinit_property("a", class_id.clone(), 2, true));
        graph.add_declaration(create_lateinit_property("b", class_id.clone(), 3, true));
        graph.add_declaration(create_lateinit_property("c", class_id.clone(), 4, true));
        graph.add_declaration(create_lateinit_property("d", class_id.clone(), 5, true));
        graph.add_declaration(create_lateinit_property("e", class_id.clone(), 6, true));
        graph.add_declaration(create_lateinit_property("f", class_id, 7, true));

        let detector = LateinitAbuseDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Injected lateinit should be OK");
    }
}
