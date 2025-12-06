//! Unused Sealed Variant Detector
//!
//! Detects sealed class/interface subclasses that are never instantiated.
//! Sealed variants are often created for exhaustive when expressions but
//! some may become unused over time.
//!
//! ## Detection Algorithm
//!
//! 1. Find all sealed classes/interfaces (have "sealed" modifier)
//! 2. Find all their subclasses (classes extending the sealed type)
//! 3. For each subclass, check if it's ever instantiated:
//!    - Constructor called directly
//!    - Referenced via Instantiation
//! 4. Report never-instantiated subclasses
//!
//! ## Examples Detected
//!
//! ```kotlin
//! sealed class UiState {
//!     object Loading : UiState()          // Used
//!     data class Success(val d: Data) : UiState()  // Used
//!     object Empty : UiState()            // DEAD: never emitted
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, ReferenceKind};
use std::collections::HashSet;

/// Detector for unused sealed class/interface variants
pub struct UnusedSealedVariantDetector {
    /// Minimum confidence to report
    min_confidence: Confidence,
}

impl UnusedSealedVariantDetector {
    pub fn new() -> Self {
        Self {
            min_confidence: Confidence::Medium,
        }
    }

    /// Check if a declaration is a sealed class or interface
    fn is_sealed(&self, decl: &crate::graph::Declaration) -> bool {
        decl.modifiers.iter().any(|m| m == "sealed")
    }

    /// Check if a declaration is a subclass of a sealed type
    fn is_sealed_subclass(
        &self,
        decl: &crate::graph::Declaration,
        sealed_types: &HashSet<String>,
    ) -> bool {
        // Check if any of the super types is a sealed class/interface
        decl.super_types.iter().any(|st| {
            // Handle generic types like "Foo<Bar>" -> "Foo"
            let base_type = st.split('<').next().unwrap_or(st);
            // Strip constructor parens: "UiState()" -> "UiState"
            let base_type = base_type.split('(').next().unwrap_or(base_type);
            // Trim whitespace
            let base_type = base_type.trim();
            sealed_types.contains(base_type)
        })
    }

    /// Check if a declaration is ever instantiated
    fn is_instantiated(&self, decl: &crate::graph::Declaration, graph: &Graph) -> bool {
        // Get all references to this declaration
        let refs = graph.get_references_to(&decl.id);

        // Check for instantiation references (constructor calls)
        for (_, reference) in &refs {
            match reference.kind {
                ReferenceKind::Instantiation | ReferenceKind::Call => {
                    return true;
                }
                // Reflection references (::class) often indicate serialization/factory usage
                ReferenceKind::Reflection => {
                    return true;
                }
                // Type references in certain patterns indicate potential instantiation
                ReferenceKind::Type => {
                    // If referenced as a type, it might be instantiated via factory/reflection
                    // This is a conservative check to reduce false positives
                    return true;
                }
                _ => {}
            }
        }

        // For Kotlin objects, they're "instantiated" by just referencing them
        if decl.kind == DeclarationKind::Object {
            // Any reference to an object means it's used
            return !refs.is_empty();
        }

        false
    }
}

impl Default for UnusedSealedVariantDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for UnusedSealedVariantDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();

        // Step 1: Find all sealed classes/interfaces
        let sealed_types: HashSet<String> = graph
            .declarations()
            .filter(|d| self.is_sealed(d))
            .filter_map(|d| {
                d.fully_qualified_name
                    .clone()
                    .or_else(|| Some(d.name.clone()))
            })
            .collect();

        // Also collect simple names for matching
        let sealed_simple_names: HashSet<String> = graph
            .declarations()
            .filter(|d| self.is_sealed(d))
            .map(|d| d.name.clone())
            .collect();

        if sealed_types.is_empty() {
            return issues;
        }

        // Step 2: Find all subclasses of sealed types
        for decl in graph.declarations() {
            // Skip if not a class or object (interfaces can't be instantiated)
            if !matches!(decl.kind, DeclarationKind::Class | DeclarationKind::Object) {
                continue;
            }

            // Skip sealed classes themselves - we only care about variants (subclasses)
            if self.is_sealed(decl) {
                continue;
            }

            // Skip enum classes - their constants are referenced, not instantiated
            if decl.modifiers.iter().any(|m| m == "enum") {
                continue;
            }

            // Check if this is a subclass of a sealed type
            let is_sealed_sub = decl.super_types.iter().any(|st| {
                // Strip generic args: "Foo<Bar>" -> "Foo"
                let base_type = st.split('<').next().unwrap_or(st);
                // Strip constructor parens: "UiState()" -> "UiState"
                let base_type = base_type.split('(').next().unwrap_or(base_type);
                // Trim whitespace
                let base_type = base_type.trim();
                sealed_types.contains(base_type) || sealed_simple_names.contains(base_type)
            });

            if !is_sealed_sub {
                continue;
            }

            // Step 3: Check if this variant is ever instantiated
            if !self.is_instantiated(decl, graph) {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::UnusedSealedVariant);
                dead = dead.with_message(format!(
                    "Sealed variant '{}' is never instantiated",
                    decl.name
                ));
                dead = dead.with_confidence(Confidence::High);
                issues.push(dead);
            }
        }

        // Sort by file and line for consistent output
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
    use crate::graph::{
        Declaration, DeclarationId, DeclarationKind, Language, Location, Visibility,
    };
    use std::path::PathBuf;

    fn make_declaration(
        name: &str,
        kind: DeclarationKind,
        modifiers: Vec<&str>,
        super_types: Vec<&str>,
    ) -> Declaration {
        let mut decl = Declaration::new(
            DeclarationId::new(PathBuf::from("test.kt"), 0, 100),
            name.to_string(),
            kind,
            Location::new(PathBuf::from("test.kt"), 1, 1, 0, 100),
            Language::Kotlin,
        );
        decl.modifiers = modifiers.into_iter().map(String::from).collect();
        decl.super_types = super_types.into_iter().map(String::from).collect();
        decl.visibility = Visibility::Public;
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = UnusedSealedVariantDetector::new();
        assert_eq!(detector.min_confidence, Confidence::Medium);
    }

    #[test]
    fn test_is_sealed() {
        let detector = UnusedSealedVariantDetector::new();

        // Sealed class
        let sealed_class =
            make_declaration("UiState", DeclarationKind::Class, vec!["sealed"], vec![]);
        assert!(detector.is_sealed(&sealed_class));

        // Regular class (not sealed)
        let regular_class = make_declaration("MyClass", DeclarationKind::Class, vec![], vec![]);
        assert!(!detector.is_sealed(&regular_class));

        // Sealed interface
        let sealed_interface =
            make_declaration("Action", DeclarationKind::Interface, vec!["sealed"], vec![]);
        assert!(detector.is_sealed(&sealed_interface));
    }

    #[test]
    fn test_is_sealed_subclass() {
        let detector = UnusedSealedVariantDetector::new();
        let sealed_types: std::collections::HashSet<String> =
            ["UiState".to_string()].into_iter().collect();

        // Direct subclass
        let loading = make_declaration("Loading", DeclarationKind::Object, vec![], vec!["UiState"]);
        assert!(detector.is_sealed_subclass(&loading, &sealed_types));

        // Subclass with constructor call
        let success = make_declaration(
            "Success",
            DeclarationKind::Class,
            vec!["data"],
            vec!["UiState()"],
        );
        assert!(detector.is_sealed_subclass(&success, &sealed_types));

        // Not a subclass
        let unrelated =
            make_declaration("Helper", DeclarationKind::Class, vec![], vec!["BaseClass"]);
        assert!(!detector.is_sealed_subclass(&unrelated, &sealed_types));

        // Generic subclass
        let generic = make_declaration(
            "Loaded",
            DeclarationKind::Class,
            vec![],
            vec!["UiState<String>"],
        );
        assert!(detector.is_sealed_subclass(&generic, &sealed_types));
    }
}
