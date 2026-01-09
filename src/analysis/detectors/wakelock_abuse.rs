//! WakeLock Abuse Detector
//!
//! Detects WakeLock that may not be properly released.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! fun startOperation() {
//!     wakeLock.acquire()  // Never released!
//!     doLongOperation()
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Prevents device from sleeping
//! - Drains battery rapidly
//! - Can cause device to overheat
//! - Users will uninstall your app
//!
//! ## Better Alternatives
//!
//! - Use acquire(timeout) with reasonable timeout
//! - Always release in finally block
//! - Consider using WorkManager instead

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for WakeLock abuse
pub struct WakeLockAbuseDetector {
    /// Minimum method size to check
    min_method_bytes: usize,
}

impl WakeLockAbuseDetector {
    pub fn new() -> Self {
        Self {
            min_method_bytes: 100,
        }
    }

    /// Check if method name suggests WakeLock usage
    fn suggests_wakelock_usage(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("wakelock")
            || lower.contains("wake")
            || lower.contains("acquire")
            || lower.contains("power")
    }

    /// Check if class name suggests WakeLock handling
    fn class_handles_wakelock(decl: &crate::graph::Declaration, graph: &Graph) -> bool {
        if let Some(ref parent_id) = decl.parent {
            if let Some(parent) = graph.get_declaration(parent_id) {
                let lower = parent.name.to_lowercase();
                return lower.contains("wakelock")
                    || lower.contains("power")
                    || lower.contains("service");
            }
        }
        false
    }
}

impl Default for WakeLockAbuseDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for WakeLockAbuseDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Only check methods
            if !matches!(decl.kind, DeclarationKind::Method) {
                continue;
            }

            // Only check Android code (Kotlin/Java)
            if !matches!(decl.language, Language::Kotlin | Language::Java) {
                continue;
            }

            // Check method size
            let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
            if byte_size < self.min_method_bytes {
                continue;
            }

            // Check if suggests WakeLock usage
            let name_suggests = Self::suggests_wakelock_usage(&decl.name);
            let class_handles = Self::class_handles_wakelock(decl, graph);

            if name_suggests || class_handles {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::WakeLockAbuse);
                dead = dead.with_message(format!(
                    "Method '{}' may handle WakeLock. Ensure timeout and proper release in finally block.",
                    decl.name
                ));
                dead = dead.with_confidence(Confidence::Low);
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
    use crate::graph::{Declaration, DeclarationId, Location};
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
        let detector = WakeLockAbuseDetector::new();
        assert!(detector.min_method_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = WakeLockAbuseDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_wakelock_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("acquireWakeLock", 1, 200));

        let detector = WakeLockAbuseDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_power_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("handlePowerState", 1, 200));

        let detector = WakeLockAbuseDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("acquireWakeLock", 1, 50));

        let detector = WakeLockAbuseDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_unrelated_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processData", 1, 200));

        let detector = WakeLockAbuseDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
