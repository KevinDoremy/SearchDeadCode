//! Reflection Overuse Detector
//!
//! Detects excessive Kotlin reflection usage in hot paths.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! fun processAll(items: List<Any>) {
//!     items.forEach { item ->
//!         item::class.memberProperties.forEach { prop ->
//!             println(prop.getter.call(item))
//!         }
//!     }
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Reflection is slow (10-100x slower than direct access)
//! - Adds kotlin-reflect dependency (~2.5MB)
//! - Breaks proguard/R8 optimizations
//! - Often unnecessary
//!
//! ## Better Alternatives
//!
//! - Direct property access
//! - Interface-based polymorphism
//! - Code generation (kapt/ksp)

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Language};

/// Detector for excessive reflection usage
pub struct ReflectionOveruseDetector {
    /// Minimum method size to check
    min_method_bytes: usize,
}

impl ReflectionOveruseDetector {
    pub fn new() -> Self {
        Self {
            min_method_bytes: 150,
        }
    }

    /// Check if method name suggests reflection usage
    fn suggests_reflection(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("reflect")
            || lower.contains("kclass")
            || lower.contains("property")
            || lower.contains("member")
            || lower.contains("introspect")
            || lower.contains("dynamic")
    }

    /// Check if in test file (reflection in tests is OK)
    fn is_test_file(decl: &crate::graph::Declaration) -> bool {
        let file_path = decl.location.file.to_string_lossy().to_lowercase();
        file_path.contains("test")
    }
}

impl Default for ReflectionOveruseDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for ReflectionOveruseDetector {
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

            // Only check Kotlin
            if !matches!(decl.language, Language::Kotlin) {
                continue;
            }

            // Skip test files
            if Self::is_test_file(decl) {
                continue;
            }

            // Check method size
            let byte_size = decl.location.end_byte.saturating_sub(decl.location.start_byte);
            if byte_size < self.min_method_bytes {
                continue;
            }

            // Check if method suggests reflection
            if !Self::suggests_reflection(&decl.name) {
                continue;
            }

            let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::ReflectionOveruse);
            dead = dead.with_message(format!(
                "Method '{}' may use excessive reflection. Consider direct access or compile-time alternatives.",
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

    fn create_method(name: &str, file: &str, line: usize, byte_size: usize) -> Declaration {
        let path = PathBuf::from(file);
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
        let detector = ReflectionOveruseDetector::new();
        assert!(detector.min_method_bytes > 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = ReflectionOveruseDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_reflect_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("reflectProperties", "Mapper.kt", 1, 200));

        let detector = ReflectionOveruseDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_kclass_method_detected() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("getKClassInfo", "Utils.kt", 1, 200));

        let detector = ReflectionOveruseDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_test_file_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("reflectProperties", "MapperTest.kt", 1, 200));

        let detector = ReflectionOveruseDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_normal_method_ok() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processData", "Processor.kt", 1, 200));

        let detector = ReflectionOveruseDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
