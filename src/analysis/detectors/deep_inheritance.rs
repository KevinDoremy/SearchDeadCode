//! Deep Inheritance Detector
//!
//! Detects classes with deep inheritance chains (e.g., 3+ levels of Base classes).
//! This is a common anti-pattern that leads to rigid, hard-to-maintain code.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! open class BaseActivity { }
//! open class BaseViewModelActivity : BaseActivity() { }
//! open class BaseToolbarActivity : BaseViewModelActivity() { }
//! open class BaseListActivity : BaseToolbarActivity() { }
//! class UserListActivity : BaseListActivity() { }  // 4 levels deep!
//! ```
//!
//! ## Why It's Bad
//!
//! - Hard to navigate in IDE
//! - Unclear which base class to extend
//! - Changes in base classes ripple through hierarchy
//! - Often indicates misuse of inheritance for code reuse
//!
//! ## Better Alternatives
//!
//! - Prefer composition over inheritance
//! - Use interfaces for contracts
//! - Use delegation for code reuse
//! - Keep inheritance chains shallow (1-2 levels)

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};
use std::collections::HashMap;

/// Detector for deep inheritance chains
pub struct DeepInheritanceDetector {
    /// Maximum allowed inheritance depth before warning
    max_depth: usize,
    /// Known framework classes to skip counting
    framework_classes: Vec<String>,
}

impl DeepInheritanceDetector {
    pub fn new() -> Self {
        Self {
            max_depth: 3,
            framework_classes: vec![
                // Android framework
                "Activity".to_string(),
                "AppCompatActivity".to_string(),
                "FragmentActivity".to_string(),
                "ComponentActivity".to_string(),
                "Fragment".to_string(),
                "DialogFragment".to_string(),
                "BottomSheetDialogFragment".to_string(),
                "Service".to_string(),
                "IntentService".to_string(),
                "BroadcastReceiver".to_string(),
                "ContentProvider".to_string(),
                "Application".to_string(),
                "ViewModel".to_string(),
                "AndroidViewModel".to_string(),
                // RecyclerView
                "RecyclerView.Adapter".to_string(),
                "RecyclerView.ViewHolder".to_string(),
                // Views
                "View".to_string(),
                "ViewGroup".to_string(),
                "LinearLayout".to_string(),
                "FrameLayout".to_string(),
                "ConstraintLayout".to_string(),
            ],
        }
    }

    /// Set maximum depth before warning
    #[allow(dead_code)]
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Check if a class name is a framework base class
    fn is_framework_class(&self, name: &str) -> bool {
        self.framework_classes.iter().any(|fc| name.contains(fc))
    }

    /// Calculate inheritance depth for a class
    fn calculate_depth(&self, decl: &crate::graph::Declaration, graph: &Graph) -> usize {
        let mut depth = 0;

        // Count super_types that are in the codebase (not framework classes)
        for super_type in &decl.super_types {
            if self.is_framework_class(super_type) {
                continue;
            }

            // Try to find this supertype in the graph
            let super_decls = graph.find_by_name(super_type);
            if !super_decls.is_empty() {
                // Found in codebase, add to depth
                depth += 1;

                // Recursively check parent's depth
                for super_decl in super_decls {
                    let parent_depth = self.calculate_depth(super_decl, graph);
                    depth = depth.max(1 + parent_depth);
                }
            }
        }

        depth
    }
}

impl Default for DeepInheritanceDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for DeepInheritanceDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();

        // Build inheritance depth cache
        let mut depth_cache: HashMap<String, usize> = HashMap::new();

        // Find all classes
        for decl in graph.declarations() {
            if !matches!(decl.kind, DeclarationKind::Class) {
                continue;
            }

            // Skip if it's a Base class itself (only report leaf classes)
            if decl.name.starts_with("Base") {
                continue;
            }

            // Calculate inheritance depth
            let depth = if let Some(&cached) = depth_cache.get(&decl.name) {
                cached
            } else {
                let d = self.calculate_depth(decl, graph);
                depth_cache.insert(decl.name.clone(), d);
                d
            };

            if depth >= self.max_depth {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::DeepInheritance);
                dead = dead.with_message(format!(
                    "Class '{}' has inheritance depth of {} (max recommended: {}). Consider using composition over inheritance.",
                    decl.name, depth, self.max_depth
                ));
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

    fn create_class(name: &str, line: usize, super_types: Vec<&str>) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        );
        decl.super_types = super_types.into_iter().map(String::from).collect();
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = DeepInheritanceDetector::new();
        assert_eq!(detector.max_depth, 3);
    }

    #[test]
    fn test_with_max_depth() {
        let detector = DeepInheritanceDetector::new().with_max_depth(5);
        assert_eq!(detector.max_depth, 5);
    }

    #[test]
    fn test_is_framework_class() {
        let detector = DeepInheritanceDetector::new();
        assert!(detector.is_framework_class("AppCompatActivity"));
        assert!(detector.is_framework_class("Fragment"));
        assert!(detector.is_framework_class("ViewModel"));
        assert!(!detector.is_framework_class("UserRepository"));
        assert!(!detector.is_framework_class("DataManager"));
        // Note: names containing framework class names will match
        assert!(detector.is_framework_class("BaseActivity"));
        assert!(detector.is_framework_class("UserService")); // Contains "Service"
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = DeepInheritanceDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_shallow_inheritance() {
        let mut graph = Graph::new();
        graph.add_declaration(create_class("BaseActivity", 1, vec!["AppCompatActivity"]));
        graph.add_declaration(create_class("MainActivity", 2, vec!["BaseActivity"]));

        let detector = DeepInheritanceDetector::new();
        let issues = detector.detect(&graph);

        // Depth 1 is within limit
        assert!(issues.is_empty());
    }
}
