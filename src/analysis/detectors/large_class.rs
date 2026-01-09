//! Large Class Detector
//!
//! Detects classes that have too many methods or properties.
//! Also known as "God Class" anti-pattern.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! class GodClass {
//!     // 50+ properties
//!     // 100+ methods
//!     // Handles user management, email, payments, inventory...
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Violates Single Responsibility Principle
//! - Hard to understand and navigate
//! - Difficult to test comprehensively
//! - Changes have far-reaching effects
//! - Merge conflicts are common
//!
//! ## Better Alternatives
//!
//! - Split into smaller, focused classes
//! - Use composition over inheritance
//! - Apply SOLID principles
//! - Create domain-specific services

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationId, DeclarationKind, Graph};
use std::collections::HashMap;

/// Detector for classes that are too large
pub struct LargeClassDetector {
    /// Maximum allowed methods per class
    max_methods: usize,
    /// Maximum allowed properties per class
    max_properties: usize,
}

impl LargeClassDetector {
    pub fn new() -> Self {
        Self {
            max_methods: 30,
            max_properties: 20,
        }
    }

    /// Set maximum methods before warning
    #[allow(dead_code)]
    pub fn with_max_methods(mut self, max: usize) -> Self {
        self.max_methods = max;
        self
    }

    /// Set maximum properties before warning
    #[allow(dead_code)]
    pub fn with_max_properties(mut self, max: usize) -> Self {
        self.max_properties = max;
        self
    }
}

impl Default for LargeClassDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for LargeClassDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // Count methods and properties per class
        let mut class_methods: HashMap<&DeclarationId, usize> = HashMap::new();
        let mut class_properties: HashMap<&DeclarationId, usize> = HashMap::new();

        // First pass: count children by type
        for decl in graph.declarations() {
            if let Some(ref parent_id) = decl.parent {
                match decl.kind {
                    DeclarationKind::Method | DeclarationKind::Function => {
                        *class_methods.entry(parent_id).or_insert(0) += 1;
                    }
                    DeclarationKind::Property | DeclarationKind::Field => {
                        *class_properties.entry(parent_id).or_insert(0) += 1;
                    }
                    _ => {}
                }
            }
        }

        // Second pass: check each class
        for decl in graph.declarations() {
            if !matches!(decl.kind, DeclarationKind::Class) {
                continue;
            }

            let method_count = class_methods.get(&decl.id).copied().unwrap_or(0);
            let property_count = class_properties.get(&decl.id).copied().unwrap_or(0);

            // Check if exceeds thresholds
            let exceeds_methods = method_count > self.max_methods;
            let exceeds_properties = property_count > self.max_properties;

            if exceeds_methods || exceeds_properties {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::LargeClass);

                let message = if exceeds_methods && exceeds_properties {
                    format!(
                        "Class '{}' has {} methods (max: {}) and {} properties (max: {}). Consider splitting into smaller classes.",
                        decl.name, method_count, self.max_methods, property_count, self.max_properties
                    )
                } else if exceeds_methods {
                    format!(
                        "Class '{}' has {} methods (max recommended: {}). Consider extracting functionality into separate classes.",
                        decl.name, method_count, self.max_methods
                    )
                } else {
                    format!(
                        "Class '{}' has {} properties (max recommended: {}). Consider grouping related properties into separate classes.",
                        decl.name, property_count, self.max_properties
                    )
                };

                dead = dead.with_message(message);
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

    fn create_method(name: &str, parent_id: DeclarationId, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 30),
            name.to_string(),
            DeclarationKind::Method,
            Location::new(path, line, 1, line * 100, line * 100 + 30),
            Language::Kotlin,
        );
        decl.parent = Some(parent_id);
        decl
    }

    fn create_property(name: &str, parent_id: DeclarationId, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 20),
            name.to_string(),
            DeclarationKind::Property,
            Location::new(path, line, 1, line * 100, line * 100 + 20),
            Language::Kotlin,
        );
        decl.parent = Some(parent_id);
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = LargeClassDetector::new();
        assert_eq!(detector.max_methods, 30);
        assert_eq!(detector.max_properties, 20);
    }

    #[test]
    fn test_with_max_methods() {
        let detector = LargeClassDetector::new().with_max_methods(50);
        assert_eq!(detector.max_methods, 50);
    }

    #[test]
    fn test_with_max_properties() {
        let detector = LargeClassDetector::new().with_max_properties(30);
        assert_eq!(detector.max_properties, 30);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = LargeClassDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_small_class_ok() {
        let mut graph = Graph::new();
        let cls = create_class("SmallClass", 1);
        let cls_id = cls.id.clone();
        graph.add_declaration(cls);

        for i in 0..5 {
            graph.add_declaration(create_method(&format!("method{}", i), cls_id.clone(), 2 + i));
        }
        for i in 0..5 {
            graph.add_declaration(create_property(&format!("prop{}", i), cls_id.clone(), 10 + i));
        }

        let detector = LargeClassDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Small classes should be OK");
    }

    #[test]
    fn test_too_many_methods() {
        let mut graph = Graph::new();
        let cls = create_class("MethodHeavyClass", 1);
        let cls_id = cls.id.clone();
        graph.add_declaration(cls);

        for i in 0..35 {
            graph.add_declaration(create_method(&format!("method{}", i), cls_id.clone(), 2 + i));
        }

        let detector = LargeClassDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("35 methods"));
    }

    #[test]
    fn test_too_many_properties() {
        let mut graph = Graph::new();
        let cls = create_class("PropertyHeavyClass", 1);
        let cls_id = cls.id.clone();
        graph.add_declaration(cls);

        for i in 0..25 {
            graph.add_declaration(create_property(&format!("prop{}", i), cls_id.clone(), 2 + i));
        }

        let detector = LargeClassDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("25 properties"));
    }

    #[test]
    fn test_both_exceeded() {
        let mut graph = Graph::new();
        let cls = create_class("GodClass", 1);
        let cls_id = cls.id.clone();
        graph.add_declaration(cls);

        for i in 0..35 {
            graph.add_declaration(create_method(&format!("method{}", i), cls_id.clone(), 2 + i));
        }
        for i in 0..25 {
            graph.add_declaration(create_property(&format!("prop{}", i), cls_id.clone(), 50 + i));
        }

        let detector = LargeClassDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("35 methods"));
        assert!(issues[0].message.contains("25 properties"));
    }

    #[test]
    fn test_custom_thresholds() {
        let mut graph = Graph::new();
        let cls = create_class("MediumClass", 1);
        let cls_id = cls.id.clone();
        graph.add_declaration(cls);

        for i in 0..15 {
            graph.add_declaration(create_method(&format!("method{}", i), cls_id.clone(), 2 + i));
        }

        // Default (30) should not flag
        let detector = LargeClassDetector::new();
        assert!(detector.detect(&graph).is_empty());

        // Stricter threshold (10) should flag
        let strict_detector = LargeClassDetector::new().with_max_methods(10);
        let issues = strict_detector.detect(&graph);
        assert_eq!(issues.len(), 1);
    }
}
