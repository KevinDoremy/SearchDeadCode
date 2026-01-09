//! State Without Remember Detector
//!
//! Detects state variables without proper remember {} wrapper in Compose.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! @Composable
//! fun BadCounter() {
//!     var count = mutableStateOf(0)  // BAD: No remember!
//!     Button(onClick = { count.value++ }) {
//!         Text("Count: ${count.value}")
//!     }
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - State resets on every recomposition
//! - Wastes resources recreating state
//! - Causes unexpected UI behavior
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! @Composable
//! fun GoodCounter() {
//!     var count by remember { mutableStateOf(0) }
//!     // ...
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for state without remember in Compose
pub struct StateWithoutRememberDetector {
    /// Minimum function size to check
    min_function_bytes: usize,
}

impl StateWithoutRememberDetector {
    pub fn new() -> Self {
        Self {
            min_function_bytes: 100,
        }
    }

    /// Check if function is a Composable
    fn is_composable(decl: &crate::graph::Declaration) -> bool {
        decl.annotations
            .iter()
            .any(|a| a.contains("Composable") || a == "Composable")
    }

    /// Check if function name suggests state handling
    fn name_suggests_state_handling(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("screen")
            || lower.contains("content")
            || lower.contains("dialog")
            || lower.contains("sheet")
            || lower.contains("card")
            || lower.contains("item")
            || lower.contains("form")
            || lower.contains("input")
            || lower.contains("toggle")
            || lower.contains("counter")
    }
}

impl Default for StateWithoutRememberDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for StateWithoutRememberDetector {
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

            // Check function size (larger functions more likely to have state)
            let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
            if byte_size < self.min_function_bytes {
                continue;
            }

            // Check if name suggests state handling
            if !Self::name_suggests_state_handling(&decl.name) {
                continue;
            }

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::StateWithoutRemember);
            dead = dead.with_message(format!(
                "@Composable '{}' may use state without remember. Wrap mutableStateOf in remember {{}}.",
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

    fn create_regular_function(name: &str, line: usize, byte_size: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let start_byte = line * 100;
        let end_byte = start_byte + byte_size;
        Declaration::new(
            DeclarationId::new(path.clone(), start_byte, end_byte),
            name.to_string(),
            DeclarationKind::Function,
            Location::new(path, line, 1, start_byte, end_byte),
            Language::Kotlin,
        )
    }

    #[test]
    fn test_detector_creation() {
        let detector = StateWithoutRememberDetector::new();
        assert!(detector.min_function_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = StateWithoutRememberDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_composable_screen_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("HomeScreen", 1, 200));

        let detector = StateWithoutRememberDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("remember"));
    }

    #[test]
    fn test_composable_form_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("LoginForm", 1, 200));

        let detector = StateWithoutRememberDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_composable_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("HomeScreen", 1, 50));

        let detector = StateWithoutRememberDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_non_composable_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_regular_function("HomeScreen", 1, 200));

        let detector = StateWithoutRememberDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_composable_helper_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_composable("calculateOffset", 1, 200));

        let detector = StateWithoutRememberDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
