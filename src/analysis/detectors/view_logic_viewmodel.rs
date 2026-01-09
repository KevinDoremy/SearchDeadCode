//! View Logic in ViewModel Detector
//!
//! Detects View/Context references in ViewModel classes.
//! ViewModels should not hold references to Views or Activity Context.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! class BadViewModel : ViewModel() {
//!     private var textView: TextView? = null  // Memory leak!
//!     private var context: Context? = null    // Memory leak!
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Activity/Fragment outlives View references = memory leak
//! - ViewModel survives configuration changes, View doesn't
//! - Violates MVVM architecture separation
//!
//! ## Better Alternatives
//!
//! - Use AndroidViewModel for Application context only
//! - Pass data, not Views
//! - Use LiveData/StateFlow to communicate with UI

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for View/Context references in ViewModel
pub struct ViewLogicInViewModelDetector {
    /// Types that should not be in ViewModel
    forbidden_types: Vec<&'static str>,
}

impl ViewLogicInViewModelDetector {
    pub fn new() -> Self {
        Self {
            forbidden_types: vec![
                "View",
                "TextView",
                "Button",
                "ImageView",
                "RecyclerView",
                "EditText",
                "Fragment",
                "Activity",
                "Context",
                "Dialog",
                "Toast",
                "Snackbar",
                "LayoutInflater",
                "Window",
                "ViewGroup",
            ],
        }
    }

    /// Check if declaration name suggests a forbidden type
    fn has_forbidden_type(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.forbidden_types
            .iter()
            .any(|t| lower.contains(&t.to_lowercase()))
    }

    /// Check if class is a ViewModel
    fn is_viewmodel_class(decl: &crate::graph::Declaration) -> bool {
        let name_lower = decl.name.to_lowercase();
        name_lower.contains("viewmodel")
            || decl
                .super_types
                .iter()
                .any(|s| s.to_lowercase().contains("viewmodel"))
    }
}

impl Default for ViewLogicInViewModelDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for ViewLogicInViewModelDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // Find all ViewModels first
        let viewmodel_ids: Vec<_> = graph
            .declarations()
            .filter(|d| matches!(d.kind, DeclarationKind::Class) && Self::is_viewmodel_class(d))
            .map(|d| d.id.clone())
            .collect();

        // Check properties in ViewModels
        for decl in graph.declarations() {
            if !matches!(decl.kind, DeclarationKind::Property | DeclarationKind::Field) {
                continue;
            }

            // Check if parent is a ViewModel
            let in_viewmodel = decl
                .parent
                .as_ref()
                .map(|p| viewmodel_ids.iter().any(|vm| vm == p))
                .unwrap_or(false);

            if !in_viewmodel {
                continue;
            }

            // Check if property has forbidden type
            if self.has_forbidden_type(&decl.name) {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::ViewLogicInViewModel);
                dead = dead.with_message(format!(
                    "Property '{}' in ViewModel holds View/Context reference. This causes memory leaks and violates MVVM.",
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
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::path::PathBuf;

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
    ) -> Declaration {
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
        let detector = ViewLogicInViewModelDetector::new();
        assert!(!detector.forbidden_types.is_empty());
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_textview_in_viewmodel() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("UserViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_parent("textView", vm_id, 2));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("textView"));
    }

    #[test]
    fn test_context_in_viewmodel() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("MainViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_parent("context", vm_id, 2));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_activity_in_viewmodel() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("HomeViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_parent("activity", vm_id, 2));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_normal_property_ok() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("UserViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_parent("userData", vm_id, 2));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Normal properties should be OK");
    }

    #[test]
    fn test_view_outside_viewmodel_ok() {
        let mut graph = Graph::new();
        let path = PathBuf::from("test.kt");
        let cls = Declaration::new(
            DeclarationId::new(path.clone(), 100, 600),
            "RegularClass".to_string(),
            DeclarationKind::Class,
            Location::new(path.clone(), 1, 1, 100, 600),
            Language::Kotlin,
        );
        let cls_id = cls.id.clone();
        graph.add_declaration(cls);
        graph.add_declaration(create_property_with_parent("textView", cls_id, 2));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Views in non-ViewModel classes are OK");
    }
}
