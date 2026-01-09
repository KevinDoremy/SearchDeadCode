//! Business Logic In Composable Detector
//!
//! Detects non-UI logic in @Composable functions.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! @Composable
//! fun BadUserProfile(userId: String) {
//!     LaunchedEffect(userId) {
//!         // BAD: Network call in composable
//!         val response = retrofit.userService.getUser(userId)
//!         // ...
//!     }
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Violates separation of concerns
//! - Hard to test
//! - Difficult to reuse
//! - UI tightly coupled to data layer
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! // ViewModel handles business logic
//! class UserProfileViewModel : ViewModel() {
//!     fun loadUser(userId: String) { ... }
//! }
//!
//! @Composable
//! fun GoodUserProfile(viewModel: UserProfileViewModel) {
//!     val user by viewModel.user.collectAsState()
//!     // Only UI logic here
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for business logic in composables
pub struct BusinessLogicInComposableDetector {
    /// Minimum function size to check
    min_function_bytes: usize,
}

impl BusinessLogicInComposableDetector {
    pub fn new() -> Self {
        Self {
            min_function_bytes: 200,
        }
    }

    /// Check if function is a Composable
    fn is_composable(decl: &crate::graph::Declaration) -> bool {
        decl.annotations
            .iter()
            .any(|a| a.contains("Composable") || a == "Composable")
    }

    /// Check if function name suggests data fetching/processing
    fn name_suggests_data_handling(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("fetch")
            || lower.contains("load")
            || lower.contains("process")
            || lower.contains("validate")
            || lower.contains("calculate")
            || lower.contains("compute")
            || lower.contains("transform")
    }

    /// Check if function name suggests it's a screen with business logic
    fn is_screen_with_logic(name: &str, byte_size: usize) -> bool {
        let lower = name.to_lowercase();
        // Large screens are more likely to have embedded business logic
        byte_size > 500
            && (lower.contains("screen")
                || lower.contains("page")
                || lower.contains("view")
                || lower.contains("content"))
    }
}

impl Default for BusinessLogicInComposableDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for BusinessLogicInComposableDetector {
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

            // Check function size
            let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
            if byte_size < self.min_function_bytes {
                continue;
            }

            // Check if name suggests data handling or is a large screen
            let suggests_logic = Self::name_suggests_data_handling(&decl.name);
            let is_large_screen = Self::is_screen_with_logic(&decl.name, byte_size);

            if !suggests_logic && !is_large_screen {
                continue;
            }

            let confidence = if suggests_logic {
                Confidence::Medium
            } else {
                Confidence::Low
            };

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::BusinessLogicInComposable);
            dead = dead.with_message(format!(
                "@Composable '{}' may contain business logic. Move data operations to ViewModel.",
                decl.name
            ));
            dead = dead.with_confidence(confidence);
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

    fn create_composable(name: &str, line: usize, byte_size: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let start_byte = line * 100;
        let end_byte = start_byte + byte_size;
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
        let detector = BusinessLogicInComposableDetector::new();
        assert!(detector.min_function_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = BusinessLogicInComposableDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_fetch_composable_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("fetchUserData", 1, 300));

        let detector = BusinessLogicInComposableDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("business logic"));
    }

    #[test]
    fn test_validate_composable_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("validateAndSubmit", 1, 300));

        let detector = BusinessLogicInComposableDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_large_screen_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("HomeScreen", 1, 600));

        let detector = BusinessLogicInComposableDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_composable_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("fetchData", 1, 100));

        let detector = BusinessLogicInComposableDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_ui_composable_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("UserCard", 1, 300));

        let detector = BusinessLogicInComposableDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
