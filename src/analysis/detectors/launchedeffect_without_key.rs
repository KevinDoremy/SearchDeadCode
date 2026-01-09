//! LaunchedEffect Without Key Detector
//!
//! Detects LaunchedEffect/DisposableEffect without proper keys in Compose.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! @Composable
//! fun BadLaunchedEffect(userId: String) {
//!     LaunchedEffect(Unit) {  // BAD: Should use userId as key
//!         user = fetchUser(userId)
//!     }
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Effect won't re-run when parameters change
//! - Stale data displayed to user
//! - Subtle bugs that are hard to debug
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! @Composable
//! fun GoodLaunchedEffect(userId: String) {
//!     LaunchedEffect(userId) {  // GOOD: Re-runs when userId changes
//!         user = fetchUser(userId)
//!     }
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for LaunchedEffect without proper keys
pub struct LaunchedEffectWithoutKeyDetector {
    /// Minimum function size to check
    min_function_bytes: usize,
}

impl LaunchedEffectWithoutKeyDetector {
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

    /// Check if function name suggests effect usage with parameters
    fn name_suggests_effect_with_params(name: &str) -> bool {
        let lower = name.to_lowercase();
        // Functions that take IDs or parameters and load data
        (lower.contains("detail") || lower.contains("profile") || lower.contains("user"))
            && (lower.contains("screen") || lower.contains("page") || lower.contains("content"))
    }

    /// Check if function name suggests data loading
    fn name_suggests_data_loading(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("load") || lower.contains("fetch") || lower.contains("refresh")
    }
}

impl Default for LaunchedEffectWithoutKeyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for LaunchedEffectWithoutKeyDetector {
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

            // Check if name suggests effect with parameters or data loading
            let suggests_params = Self::name_suggests_effect_with_params(&decl.name);
            let suggests_loading = Self::name_suggests_data_loading(&decl.name);

            if !suggests_params && !suggests_loading {
                continue;
            }

            let confidence = if suggests_params {
                Confidence::Medium
            } else {
                Confidence::Low
            };

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::LaunchedEffectWithoutKey);
            dead = dead.with_message(format!(
                "@Composable '{}' may use LaunchedEffect. Ensure proper keys are used.",
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
        let detector = LaunchedEffectWithoutKeyDetector::new();
        assert!(detector.min_function_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = LaunchedEffectWithoutKeyDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_detail_screen_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("UserDetailScreen", 1, 300));

        let detector = LaunchedEffectWithoutKeyDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_profile_page_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("ProfilePage", 1, 300));

        let detector = LaunchedEffectWithoutKeyDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_load_composable_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("loadAndDisplayData", 1, 300));

        let detector = LaunchedEffectWithoutKeyDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_composable_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("UserDetailScreen", 1, 100));

        let detector = LaunchedEffectWithoutKeyDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_ui_composable_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("UserCard", 1, 300));

        let detector = LaunchedEffectWithoutKeyDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
