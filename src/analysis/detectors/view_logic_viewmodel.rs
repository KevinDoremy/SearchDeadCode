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
    /// Types that should not be in ViewModel (exact matches on the base type)
    forbidden_types: Vec<&'static str>,
    /// Safe wrapper types that can contain any inner type
    safe_wrapper_types: Vec<&'static str>,
}

impl ViewLogicInViewModelDetector {
    pub fn new() -> Self {
        Self {
            forbidden_types: vec![
                // Views
                "View",
                "TextView",
                "Button",
                "ImageButton",
                "ImageView",
                "RecyclerView",
                "EditText",
                "CheckBox",
                "RadioButton",
                "Switch",
                "SeekBar",
                "ProgressBar",
                "WebView",
                "ViewGroup",
                "LinearLayout",
                "RelativeLayout",
                "FrameLayout",
                "ConstraintLayout",
                "CoordinatorLayout",
                "ScrollView",
                "NestedScrollView",
                "CardView",
                "Toolbar",
                "AppBarLayout",
                "TabLayout",
                "ViewPager",
                "ViewPager2",
                // Android components
                "Fragment",
                "DialogFragment",
                "BottomSheetDialogFragment",
                "Activity",
                "FragmentActivity",
                "AppCompatActivity",
                "ComponentActivity",
                "Context",
                "Dialog",
                "AlertDialog",
                "BottomSheetDialog",
                "Toast",
                "Snackbar",
                "PopupWindow",
                "PopupMenu",
                // Layout-related
                "LayoutInflater",
                "Window",
                "Drawable",
                "Bitmap",
            ],
            safe_wrapper_types: vec![
                // These types wrap other types and are safe in ViewModels
                "StateFlow",
                "MutableStateFlow",
                "SharedFlow",
                "MutableSharedFlow",
                "LiveData",
                "MutableLiveData",
                "Flow",
                "Observable",
                "Single",
                "Maybe",
                "Completable",
                "Flowable",
                "List",
                "Set",
                "Map",
                "Array",
                "Pair",
                "Triple",
            ],
        }
    }

    /// Strip generic parameters from a type string
    /// e.g., "BaseFeedFragment<NewsFeedViewModel, NewsToolbarViewModel>" -> "BaseFeedFragment"
    fn strip_generics(type_str: &str) -> &str {
        type_str.split('<').next().unwrap_or(type_str)
    }

    /// Check if a type string represents a forbidden View/Context type
    /// This checks the actual type, not the property name
    fn is_forbidden_type(&self, type_str: &str) -> bool {
        // Strip nullable marker and generics
        let base_type = type_str.trim_end_matches('?');
        let base_type = Self::strip_generics(base_type);

        // Get just the simple name (last component of qualified name)
        let simple_name = base_type.split('.').next_back().unwrap_or(base_type);

        // Check for exact match (case-insensitive) against forbidden types
        self.forbidden_types
            .iter()
            .any(|t| t.eq_ignore_ascii_case(simple_name))
    }

    /// Check if the type is a safe wrapper type (StateFlow, LiveData, etc.)
    fn is_safe_wrapper_type(&self, type_str: &str) -> bool {
        let base_type = Self::strip_generics(type_str);
        let simple_name = base_type.split('.').next_back().unwrap_or(base_type);

        self.safe_wrapper_types
            .iter()
            .any(|t| t.eq_ignore_ascii_case(simple_name))
    }

    /// Check if class is a ViewModel (not a Fragment, Activity, etc.)
    fn is_viewmodel_class(decl: &crate::graph::Declaration) -> bool {
        // First, exclude classes that are clearly NOT ViewModels
        let excluded_base_types = [
            "Fragment",
            "DialogFragment",
            "BottomSheetDialogFragment",
            "Activity",
            "FragmentActivity",
            "AppCompatActivity",
            "ComponentActivity",
            "Service",
            "BroadcastReceiver",
            "ContentProvider",
            "View",
            "ViewGroup",
            "RecyclerView",
            "Adapter",
        ];

        // Check if any supertype is an excluded type (strip generics first)
        for super_type in &decl.super_types {
            let base_type = Self::strip_generics(super_type);
            let simple_name = base_type.split('.').next_back().unwrap_or(base_type);

            for excluded in &excluded_base_types {
                if excluded.eq_ignore_ascii_case(simple_name) {
                    return false;
                }
            }
        }

        // Now check if it's actually a ViewModel
        let name_lower = decl.name.to_lowercase();
        if name_lower.ends_with("viewmodel") || name_lower.ends_with("vm") {
            return true;
        }

        // Check supertypes for ViewModel (strip generics)
        for super_type in &decl.super_types {
            let base_type = Self::strip_generics(super_type);
            let base_lower = base_type.to_lowercase();

            if base_lower.ends_with("viewmodel") || base_lower == "viewmodel" {
                return true;
            }
        }

        false
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
            if !matches!(
                decl.kind,
                DeclarationKind::Property | DeclarationKind::Field
            ) {
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

            // Check the actual type if available
            if let Some(ref type_name) = decl.type_name {
                // Skip if it's a safe wrapper type (StateFlow, LiveData, etc.)
                if self.is_safe_wrapper_type(type_name) {
                    continue;
                }

                // Check if the type is a forbidden View/Context type
                if self.is_forbidden_type(type_name) {
                    let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::ViewLogicInViewModel);
                    dead = dead.with_message(format!(
                        "Property '{}' of type '{}' in ViewModel holds View/Context reference. This causes memory leaks and violates MVVM.",
                        decl.name, type_name
                    ));
                    dead = dead.with_confidence(Confidence::High);
                    issues.push(dead);
                }
            }
            // If type_name is not available, fall back to checking property name
            // but only for exact matches (not substring matches)
            else {
                let name_lower = decl.name.to_lowercase();
                // Only flag if the name exactly matches a forbidden type (case-insensitive)
                // This avoids false positives like "isToastShowing" or "viewProperties"
                let is_exact_match = self.forbidden_types.iter().any(|t| {
                    let t_lower = t.to_lowercase();
                    name_lower == t_lower
                        || name_lower == format!("_{}", t_lower)
                        || name_lower == format!("m{}", t_lower)
                        || name_lower == format!("my{}", t_lower)
                });

                if is_exact_match {
                    let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::ViewLogicInViewModel);
                    dead = dead.with_message(format!(
                        "Property '{}' in ViewModel may hold View/Context reference. This causes memory leaks and violates MVVM.",
                        decl.name
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

    fn create_fragment(name: &str, line: usize, super_type: &str) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 500),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 500),
            Language::Kotlin,
        );
        decl.super_types.push(super_type.to_string());
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

    fn create_property_with_type(
        name: &str,
        type_name: &str,
        parent_id: DeclarationId,
        line: usize,
    ) -> Declaration {
        let mut decl = create_property_with_parent(name, parent_id, line);
        decl.type_name = Some(type_name.to_string());
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
    fn test_textview_type_in_viewmodel() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("UserViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_type(
            "myTextView",
            "TextView",
            vm_id,
            2,
        ));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("myTextView"));
        assert!(issues[0].message.contains("TextView"));
    }

    #[test]
    fn test_recyclerview_type_in_viewmodel() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("UserViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_type(
            "recyclerView",
            "RecyclerView",
            vm_id,
            2,
        ));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_nullable_view_type_in_viewmodel() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("UserViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_type("view", "View?", vm_id, 2));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_stateflow_with_view_properties_ok() {
        // StateFlow<VideoViewProperties> should NOT be flagged
        // because VideoViewProperties is a data class, not a View
        let mut graph = Graph::new();
        let vm = create_viewmodel("VideoVM", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_type(
            "_videoViewProperties",
            "MutableStateFlow<VideoViewProperties?>",
            vm_id,
            2,
        ));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert!(
            issues.is_empty(),
            "StateFlow wrapper should be safe, issues: {:?}",
            issues
        );
    }

    #[test]
    fn test_boolean_flag_with_toast_in_name_ok() {
        // isToastShowing: Boolean should NOT be flagged
        let mut graph = Graph::new();
        let vm = create_viewmodel("NewsFeedViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_type(
            "isToastShowing",
            "Boolean",
            vm_id,
            2,
        ));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert!(
            issues.is_empty(),
            "Boolean flag should be OK, issues: {:?}",
            issues
        );
    }

    #[test]
    fn test_fragment_not_identified_as_viewmodel() {
        // NewsFeedFragment extending BaseFeedFragment<NewsFeedViewModel, ...>
        // should NOT be identified as a ViewModel
        let mut graph = Graph::new();
        let fragment = create_fragment(
            "NewsFeedFragment",
            1,
            "BaseFeedFragment<NewsFeedViewModel, NewsToolbarViewModel>",
        );
        let fragment_id = fragment.id.clone();
        graph.add_declaration(fragment);
        graph.add_declaration(create_property_with_type(
            "fragmentActivity",
            "FragmentActivity",
            fragment_id,
            2,
        ));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        // Fragment is NOT a ViewModel, so View references in Fragment are OK
        assert!(
            issues.is_empty(),
            "Fragment should not be identified as ViewModel, issues: {:?}",
            issues
        );
    }

    #[test]
    fn test_fragment_with_recyclerview_ok() {
        // ShowcaseFragment with recyclerView: RecyclerView should be OK
        let mut graph = Graph::new();
        let fragment = create_fragment(
            "ShowcaseFragment",
            1,
            "BaseFeedFragment<ShowcaseViewModel, FeedToolbarViewModel>",
        );
        let fragment_id = fragment.id.clone();
        graph.add_declaration(fragment);
        graph.add_declaration(create_property_with_type(
            "recyclerView",
            "RecyclerView",
            fragment_id,
            2,
        ));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert!(
            issues.is_empty(),
            "RecyclerView in Fragment is OK, issues: {:?}",
            issues
        );
    }

    #[test]
    fn test_context_type_in_viewmodel() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("MainViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_type("context", "Context", vm_id, 2));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_activity_type_in_viewmodel() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("HomeViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_type(
            "activity",
            "FragmentActivity",
            vm_id,
            2,
        ));

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
        graph.add_declaration(create_property_with_type("userData", "UserData", vm_id, 2));

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
        graph.add_declaration(create_property_with_type("textView", "TextView", cls_id, 2));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Views in non-ViewModel classes are OK");
    }

    #[test]
    fn test_livedata_wrapper_ok() {
        let mut graph = Graph::new();
        let vm = create_viewmodel("UserViewModel", 1);
        let vm_id = vm.id.clone();
        graph.add_declaration(vm);
        graph.add_declaration(create_property_with_type(
            "users",
            "LiveData<List<User>>",
            vm_id,
            2,
        ));

        let detector = ViewLogicInViewModelDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "LiveData wrapper should be safe");
    }

    #[test]
    fn test_strip_generics() {
        assert_eq!(
            ViewLogicInViewModelDetector::strip_generics("List<String>"),
            "List"
        );
        assert_eq!(
            ViewLogicInViewModelDetector::strip_generics("Map<String, Int>"),
            "Map"
        );
        assert_eq!(
            ViewLogicInViewModelDetector::strip_generics(
                "BaseFeedFragment<NewsFeedViewModel, NewsToolbarViewModel>"
            ),
            "BaseFeedFragment"
        );
        assert_eq!(
            ViewLogicInViewModelDetector::strip_generics("SimpleClass"),
            "SimpleClass"
        );
    }

    #[test]
    fn test_class_ending_with_vm_is_viewmodel() {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), 100, 600),
            "VideoVM".to_string(),
            DeclarationKind::Class,
            Location::new(path.clone(), 1, 1, 100, 600),
            Language::Kotlin,
        );
        decl.super_types.push("ModuleViewModel".to_string());

        assert!(
            ViewLogicInViewModelDetector::is_viewmodel_class(&decl),
            "VideoVM should be identified as ViewModel"
        );
    }
}
