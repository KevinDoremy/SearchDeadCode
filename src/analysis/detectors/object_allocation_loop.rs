//! Object Allocation in Loop Detector
//!
//! Detects methods that are likely to have object allocation inside loops.
//! Especially important for Android's onDraw() which is called 60 times per second.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! override fun onDraw(canvas: Canvas) {
//!     val paint = Paint()  // BAD: Created every frame!
//!     val rect = Rect()    // BAD: GC pressure
//!     // ...
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - onDraw called 60x/sec = 60 allocations/sec per object
//! - Triggers frequent garbage collection
//! - Causes jank and frame drops
//! - Memory churn affects battery life
//!
//! ## Better Alternatives
//!
//! ```kotlin
//! // Pre-allocate as instance fields
//! private val paint = Paint()
//! private val rect = Rect()
//!
//! override fun onDraw(canvas: Canvas) {
//!     rect.set(0, 0, width, height)  // Reuse existing object
//!     canvas.drawRect(rect, paint)
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for object allocation in loops or performance-critical methods
pub struct ObjectAllocationInLoopDetector {
    /// Methods that are called frequently (e.g., every frame)
    hot_methods: Vec<&'static str>,
    /// Minimum method size to consider (to avoid flagging empty overrides)
    min_method_bytes: usize,
}

impl ObjectAllocationInLoopDetector {
    pub fn new() -> Self {
        Self {
            hot_methods: vec![
                "onDraw",
                "onMeasure",
                "onLayout",
                "dispatchDraw",
                "draw",
                "onTouchEvent",
                "onInterceptTouchEvent",
                "onBindViewHolder",
                "onScrollChanged",
                "onScrolled",
                "onAnimationUpdate",
                "computeScroll",
            ],
            min_method_bytes: 200, // ~5 lines minimum
        }
    }

    /// Check if method is a hot path (called frequently)
    fn is_hot_method(&self, name: &str) -> bool {
        self.hot_methods.iter().any(|&hot| name == hot)
    }

    /// Check if method likely contains loops based on size
    fn likely_has_loops(&self, decl: &crate::graph::Declaration) -> bool {
        let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
        // Larger methods are more likely to contain loops
        byte_size > 400 // ~10 lines
    }

    /// Check if method name suggests iteration
    fn name_suggests_iteration(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("foreach")
            || lower.contains("iterate")
            || lower.contains("loop")
            || lower.contains("process")
            || lower.contains("batch")
            || lower.contains("all")
    }
}

impl Default for ObjectAllocationInLoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for ObjectAllocationInLoopDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Only check methods
            if !matches!(decl.kind, DeclarationKind::Method) {
                continue;
            }

            // Only check Kotlin/Java (Android-specific)
            if !matches!(decl.language, Language::Kotlin | Language::Java) {
                continue;
            }

            let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);

            // Skip methods that are too small to have allocations
            if byte_size < self.min_method_bytes {
                continue;
            }

            // Check if this is a hot method (onDraw, etc.)
            let is_hot = self.is_hot_method(&decl.name);
            let has_loops = self.likely_has_loops(decl);
            let name_suggests = Self::name_suggests_iteration(&decl.name);

            // Flag hot methods that are substantial enough to have allocations
            if is_hot {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::ObjectAllocationInLoop);
                dead = dead.with_message(format!(
                    "Method '{}' is called frequently. Avoid creating objects like Paint, Rect, Path inside - pre-allocate as instance fields.",
                    decl.name
                ));
                dead = dead.with_confidence(Confidence::Medium);
                issues.push(dead);
            }
            // Also flag iteration methods that are large enough to have loops with allocations
            else if name_suggests && has_loops {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::ObjectAllocationInLoop);
                dead = dead.with_message(format!(
                    "Method '{}' may allocate objects in loops. Consider pre-allocating and reusing objects.",
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

    fn create_method(name: &str, line: usize, byte_size: usize, lang: Language) -> Declaration {
        let path = PathBuf::from("Test.kt");
        let start_byte = line * 100;
        let end_byte = start_byte + byte_size;
        Declaration::new(
            DeclarationId::new(path.clone(), start_byte, end_byte),
            name.to_string(),
            DeclarationKind::Method,
            Location::new(path, line, 1, start_byte, end_byte),
            lang,
        )
    }

    #[test]
    fn test_detector_creation() {
        let detector = ObjectAllocationInLoopDetector::new();
        assert!(!detector.hot_methods.is_empty());
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = ObjectAllocationInLoopDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_ondraw_detected() {
        let mut graph = Graph::new();
        // 300 bytes method - substantial enough
        graph.add_declaration(create_method("onDraw", 1, 300, Language::Kotlin));

        let detector = ObjectAllocationInLoopDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("onDraw"));
        assert!(issues[0].message.contains("Paint"));
    }

    #[test]
    fn test_onmeasure_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("onMeasure", 1, 300, Language::Kotlin));

        let detector = ObjectAllocationInLoopDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_onbindviewholder_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("onBindViewHolder", 1, 300, Language::Kotlin));

        let detector = ObjectAllocationInLoopDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_small_ondraw_not_flagged() {
        let mut graph = Graph::new();
        // 100 bytes - too small to have meaningful allocations
        graph.add_declaration(create_method("onDraw", 1, 100, Language::Kotlin));

        let detector = ObjectAllocationInLoopDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Small onDraw should not be flagged");
    }

    #[test]
    fn test_normal_method_not_flagged() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processData", 1, 300, Language::Kotlin));

        let detector = ObjectAllocationInLoopDetector::new();
        let issues = detector.detect(&graph);

        assert!(
            issues.is_empty(),
            "Normal methods without loop indicators should not be flagged"
        );
    }

    #[test]
    fn test_foreach_method_large() {
        let mut graph = Graph::new();
        // Large method with iteration-suggesting name
        graph.add_declaration(create_method("processAllItems", 1, 500, Language::Kotlin));

        let detector = ObjectAllocationInLoopDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("loops"));
    }

    #[test]
    fn test_java_ondraw_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("onDraw", 1, 300, Language::Java));

        let detector = ObjectAllocationInLoopDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }
}
