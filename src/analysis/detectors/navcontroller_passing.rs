//! NavController Passing Detector
//!
//! Detects NavController passed to child composables.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! @Composable
//! fun BadNavigation() {
//!     val navController = rememberNavController()
//!     HomeScreen(navController = navController)  // BAD
//! }
//!
//! @Composable
//! fun HomeScreen(navController: NavController) {  // BAD
//!     ItemList(navController = navController)     // BAD
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Tight coupling between composables
//! - Hard to test (need mock NavController)
//! - Difficult to preview
//! - Violates unidirectional data flow
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! @Composable
//! fun GoodNavigation() {
//!     val navController = rememberNavController()
//!     HomeScreen(onNavigateToDetails = { id -> navController.navigate("details/$id") })
//! }
//!
//! @Composable
//! fun HomeScreen(onNavigateToDetails: (String) -> Unit) {
//!     ItemList(onItemClick = onNavigateToDetails)
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for NavController passed to children
pub struct NavControllerPassingDetector;

impl NavControllerPassingDetector {
    pub fn new() -> Self {
        Self
    }

    /// Check if function is a Composable
    fn is_composable(decl: &crate::graph::Declaration) -> bool {
        decl.annotations
            .iter()
            .any(|a| a.contains("Composable") || a == "Composable")
    }

    /// Check if function name suggests NavController usage
    fn name_suggests_navcontroller(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("navcontroller") || lower.contains("navigator")
    }

    /// Check if function name suggests it's a child composable (not NavHost)
    fn is_child_screen_with_nav(name: &str) -> bool {
        let lower = name.to_lowercase();

        // Check if it's a screen/page that might receive navcontroller
        let is_screen = lower.contains("screen")
            || lower.contains("page")
            || lower.contains("content")
            || lower.contains("view");

        // NavHost setup functions are expected to have navController
        let is_navhost = lower.contains("navhost")
            || lower.contains("navigation")
            || lower.contains("navgraph")
            || lower.contains("router")
            || lower == "app"
            || lower == "main"
            || lower.contains("root");

        is_screen && !is_navhost
    }
}

impl Default for NavControllerPassingDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for NavControllerPassingDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Only check functions
            if !matches!(decl.kind, DeclarationKind::Function | DeclarationKind::Method) {
                continue;
            }

            // Only check Kotlin files
            if !matches!(decl.language, Language::Kotlin) {
                continue;
            }

            // Check if it's a Composable
            if !Self::is_composable(decl) {
                continue;
            }

            // Check if name suggests NavController usage
            let has_navcontroller_name = Self::name_suggests_navcontroller(&decl.name);

            // Or if it's a child screen (screens shouldn't receive navcontroller)
            let is_child_screen = Self::is_child_screen_with_nav(&decl.name);

            // Flag screens that might be receiving NavController
            // This is a heuristic - we can't see parameters without full parsing
            if !has_navcontroller_name && !is_child_screen {
                continue;
            }

            // Only flag screens, not functions with navcontroller in the name
            if has_navcontroller_name && !is_child_screen {
                continue;
            }

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::NavControllerPassing);
            dead = dead.with_message(format!(
                "@Composable '{}' is a screen that may receive NavController. Consider using navigation callbacks instead.",
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
    use crate::graph::{Declaration, DeclarationId, Location};
    use std::path::PathBuf;

    fn create_composable(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let start_byte = line * 100;
        let end_byte = start_byte + 200;
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), start_byte, end_byte),
            name.to_string(),
            DeclarationKind::Function,
            Location::new(path, line, 1, start_byte, end_byte),
            Language::Kotlin,
        );
        decl.annotations.push("Composable".to_string());
        decl
    }

    #[test]
    fn test_detector_creation() {
        let _detector = NavControllerPassingDetector::new();
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = NavControllerPassingDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_home_screen_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("HomeScreen", 1));

        let detector = NavControllerPassingDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("callbacks"));
    }

    #[test]
    fn test_settings_page_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("SettingsPage", 1));

        let detector = NavControllerPassingDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_navhost_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("AppNavHost", 1));

        let detector = NavControllerPassingDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_navigation_setup_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("SetupNavigation", 1));

        let detector = NavControllerPassingDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_card_composable_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("UserCard", 1));

        let detector = NavControllerPassingDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_root_app_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("App", 1));

        let detector = NavControllerPassingDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
