//! Mutable State Exposed Detector
//!
//! Detects public MutableLiveData/MutableStateFlow properties.
//! These should be private with read-only public exposure.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! class BadViewModel : ViewModel() {
//!     // BAD: Public mutable state
//!     val userData = MutableLiveData<User>()
//!     val uiState = MutableStateFlow(UiState.Loading)
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Breaks encapsulation
//! - External code can modify ViewModel state
//! - Makes state changes unpredictable
//! - Violates unidirectional data flow
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! class GoodViewModel : ViewModel() {
//!     private val _userData = MutableLiveData<User>()
//!     val userData: LiveData<User> = _userData
//!
//!     private val _uiState = MutableStateFlow(UiState.Loading)
//!     val uiState: StateFlow<UiState> = _uiState.asStateFlow()
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Visibility};

/// Detector for publicly exposed mutable state
pub struct MutableStateExposedDetector {
    /// Mutable state types to detect
    mutable_types: Vec<&'static str>,
}

impl MutableStateExposedDetector {
    pub fn new() -> Self {
        Self {
            mutable_types: vec![
                "MutableLiveData",
                "MutableStateFlow",
                "MutableSharedFlow",
                "MutableState", // Compose
                "BehaviorSubject",
                "PublishSubject",
                "ReplaySubject",
            ],
        }
    }

    /// Check if property name contains a mutable type indicator
    fn is_mutable_state_property(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.mutable_types
            .iter()
            .any(|t| lower.contains(&t.to_lowercase()))
    }

    /// Check if property is in a ViewModel class
    fn is_in_viewmodel(decl: &crate::graph::Declaration, graph: &Graph) -> bool {
        if let Some(ref parent_id) = decl.parent {
            if let Some(parent) = graph.get_declaration(parent_id) {
                let name_lower = parent.name.to_lowercase();
                let is_viewmodel = name_lower.contains("viewmodel")
                    || parent
                        .super_types
                        .iter()
                        .any(|s| s.to_lowercase().contains("viewmodel"));
                return is_viewmodel;
            }
        }
        false
    }
}

impl Default for MutableStateExposedDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for MutableStateExposedDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Only check properties
            if !matches!(decl.kind, DeclarationKind::Property | DeclarationKind::Field) {
                continue;
            }

            // Only check public/protected (non-private) properties
            if matches!(decl.visibility, Visibility::Private) {
                continue;
            }

            // Check if it's a mutable state type
            if !self.is_mutable_state_property(&decl.name) {
                continue;
            }

            // Check if in ViewModel (higher confidence)
            let in_viewmodel = Self::is_in_viewmodel(decl, graph);

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::MutableStateExposed);
            dead = dead.with_message(format!(
                "Property '{}' exposes mutable state publicly. Use private backing property with read-only exposure (e.g., LiveData, StateFlow).",
                decl.name
            ));
            dead = dead.with_confidence(if in_viewmodel {
                Confidence::High
            } else {
                Confidence::Medium
            });
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

    fn create_property(name: &str, line: usize, visibility: Visibility) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Property,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.visibility = visibility;
        decl
    }

    fn create_viewmodel(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 500),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 500),
            Language::Kotlin,
        );
        decl.super_types.push("ViewModel".to_string());
        decl
    }

    fn create_property_with_parent(
        name: &str,
        parent_id: DeclarationId,
        line: usize,
        visibility: Visibility,
    ) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Property,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.visibility = visibility;
        decl.parent = Some(parent_id);
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = MutableStateExposedDetector::new();
        assert!(!detector.mutable_types.is_empty());
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = MutableStateExposedDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_public_mutablelivedata_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_property(
            "userDataMutableLiveData",
            1,
            Visibility::Public,
        ));

        let detector = MutableStateExposedDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_public_mutablestateflow_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_property(
            "uiStateMutableStateFlow",
            1,
            Visibility::Public,
        ));

        let detector = MutableStateExposedDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_private_mutable_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_property(
            "_dataMutableLiveData",
            1,
            Visibility::Private,
        ));

        let detector = MutableStateExposedDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Private mutable state should be OK");
    }

    #[test]
    fn test_in_viewmodel_higher_confidence() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("UserViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_parent(
            "dataMutableLiveData",
            vm_id,
            2,
            Visibility::Public,
        ));

        let detector = MutableStateExposedDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].confidence, Confidence::High);
    }

    #[test]
    fn test_non_mutable_property_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_property("userData", 1, Visibility::Public));

        let detector = MutableStateExposedDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Non-mutable properties should be OK");
    }
}
