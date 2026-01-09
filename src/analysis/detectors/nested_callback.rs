//! Nested Callback Detector
//!
//! Detects deeply nested callbacks (callback hell).
//! Deep nesting indicates complex async flows that should be refactored.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! fun loadData() {
//!     userService.getUser { user ->
//!         orderService.getOrders(user.id) { orders ->
//!             paymentService.getPayments { payments ->
//!                 shippingService.getAddresses { addresses ->
//!                     // Pyramid of doom!
//!                 }
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Hard to read and maintain
//! - Error handling is complex
//! - Difficult to test
//! - Called "callback hell" or "pyramid of doom"
//!
//! ## Better Alternatives
//!
//! - Use coroutines (suspend functions)
//! - Use RxJava/Flow operators
//! - Break into smaller functions

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for deeply nested callbacks
pub struct NestedCallbackDetector {
    /// Minimum method size to consider (larger = more likely to have nested callbacks)
    min_method_bytes: usize,
}

impl NestedCallbackDetector {
    pub fn new() -> Self {
        Self {
            min_method_bytes: 500, // ~12 lines minimum
        }
    }

    /// Check if method name suggests callback-heavy code
    fn suggests_callback_usage(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("load")
            || lower.contains("fetch")
            || lower.contains("request")
            || lower.contains("async")
            || lower.contains("callback")
            || lower.contains("listener")
    }

    /// Check if method is large enough to potentially have nested callbacks
    fn is_large_method(decl: &crate::graph::Declaration, min_bytes: usize) -> bool {
        let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
        byte_size > min_bytes
    }
}

impl Default for NestedCallbackDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for NestedCallbackDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Only check methods and functions
            if !matches!(
                decl.kind,
                DeclarationKind::Method | DeclarationKind::Function
            ) {
                continue;
            }

            // Check if method name suggests callback usage
            if !Self::suggests_callback_usage(&decl.name) {
                continue;
            }

            // Check if method is large enough to have nested callbacks
            if !Self::is_large_method(decl, self.min_method_bytes) {
                continue;
            }

            // Large async-looking methods are suspicious
            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::NestedCallback);
            dead = dead.with_message(format!(
                "Method '{}' may have deeply nested callbacks. Consider using coroutines or breaking into smaller functions.",
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
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::path::PathBuf;

    fn create_method(name: &str, line: usize, byte_size: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let start_byte = line * 100;
        let end_byte = start_byte + byte_size;
        Declaration::new(
            DeclarationId::new(path.clone(), start_byte, end_byte),
            name.to_string(),
            DeclarationKind::Method,
            Location::new(path, line, 1, start_byte, end_byte),
            Language::Kotlin,
        )
    }

    #[test]
    fn test_detector_creation() {
        let detector = NestedCallbackDetector::new();
        assert!(detector.min_method_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = NestedCallbackDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_large_load_method_detected() {
        let mut graph = Graph::new();
        // 600 bytes = large enough
        graph.add_declaration(create_method("loadUserData", 1, 600));

        let detector = NestedCallbackDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("loadUserData"));
    }

    #[test]
    fn test_large_fetch_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("fetchAllOrders", 1, 600));

        let detector = NestedCallbackDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_load_method_ok() {
        let mut graph = Graph::new();
        // 200 bytes = too small
        graph.add_declaration(create_method("loadUser", 1, 200));

        let detector = NestedCallbackDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Small methods should be OK");
    }

    #[test]
    fn test_non_async_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processData", 1, 600));

        let detector = NestedCallbackDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Non-async methods should be OK");
    }
}
