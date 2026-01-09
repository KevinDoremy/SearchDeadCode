//! Init On Draw Detector
//!
//! Detects object allocation in onDraw() methods.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! override fun onDraw(canvas: Canvas) {
//!     val paint = Paint()  // Allocates every frame!
//!     val rect = Rect(0, 0, width, height)
//!     canvas.drawRect(rect, paint)
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - onDraw() called 60+ times per second
//! - Causes GC pressure and jank
//! - Creates unnecessary object churn
//! - Can cause dropped frames
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! private val paint = Paint()
//! private val rect = Rect()
//!
//! override fun onDraw(canvas: Canvas) {
//!     rect.set(0, 0, width, height)
//!     canvas.drawRect(rect, paint)
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for object allocation in onDraw
pub struct InitOnDrawDetector {
    /// Minimum method size to consider
    min_method_bytes: usize,
}

impl InitOnDrawDetector {
    pub fn new() -> Self {
        Self {
            min_method_bytes: 100,
        }
    }

    /// Check if method is onDraw or similar drawing method
    fn is_draw_method(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower == "ondraw"
            || lower == "dispatchdraw"
            || lower == "drawchild"
            || lower == "ondrawforeground"
            || lower == "ondrawshadow"
    }

    /// Check if class is a View subclass
    fn is_view_class(decl: &crate::graph::Declaration, graph: &Graph) -> bool {
        if let Some(ref parent_id) = decl.parent {
            if let Some(parent) = graph.get_declaration(parent_id) {
                let lower = parent.name.to_lowercase();
                return lower.contains("view")
                    || lower.contains("canvas")
                    || lower.contains("drawable")
                    || parent
                        .super_types
                        .iter()
                        .any(|s| s.contains("View") || s.contains("Drawable"));
            }
        }
        false
    }
}

impl Default for InitOnDrawDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for InitOnDrawDetector {
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

            // Check if it's a draw method
            if !Self::is_draw_method(&decl.name) {
                continue;
            }

            // Check method size (larger methods more likely to have allocations)
            let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
            if byte_size < self.min_method_bytes {
                continue;
            }

            // Optionally check if parent is View class (increases confidence)
            let is_view = Self::is_view_class(decl, graph);

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::InitOnDraw);
            dead = dead.with_message(format!(
                "Method '{}' may allocate objects. Move Paint/Rect/Path to class fields.",
                decl.name
            ));
            dead = dead.with_confidence(if is_view {
                Confidence::Medium
            } else {
                Confidence::Low
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
        let detector = InitOnDrawDetector::new();
        assert!(detector.min_method_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = InitOnDrawDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_ondraw_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("onDraw", 1, 200));

        let detector = InitOnDrawDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("allocate"));
    }

    #[test]
    fn test_dispatchdraw_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("dispatchDraw", 1, 200));

        let detector = InitOnDrawDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_ondraw_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("onDraw", 1, 50));

        let detector = InitOnDrawDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_unrelated_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processData", 1, 200));

        let detector = InitOnDrawDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
