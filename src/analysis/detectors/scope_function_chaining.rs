//! Scope Function Chaining Detector
//!
//! Detects excessive chaining of Kotlin scope functions (let, apply, also, run, with).
//! While scope functions are powerful, over-chaining reduces readability.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! // BAD: Too many chained scope functions
//! user?.let { u ->
//!     u.apply { name = "Updated" }
//!      .also { log(it) }
//!      .run { save() }
//! }
//!
//! // BAD: Nested let pyramid
//! data?.let { d ->
//!     d.field?.let { f ->
//!         f.nested?.let { n ->
//!             n.value?.let { v ->
//!                 process(v)
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! ## Why It's Bad
//!
//! - Reduces code readability ("what is 'it' here?")
//! - Hard to debug and trace
//! - Complex control flow
//! - Mixing different purposes in one chain
//!
//! ## Better Alternatives
//!
//! - Use a single appropriate scope function
//! - Break into multiple statements
//! - Use early returns with `?: return`
//! - Consider `when` expressions for complex branching

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for excessive scope function chaining
pub struct ScopeFunctionChainingDetector {
    /// Maximum allowed chained scope functions
    max_chain_length: usize,
    /// Maximum allowed nested lets
    max_nested_depth: usize,
    /// Scope function names to check
    scope_functions: Vec<String>,
}

impl ScopeFunctionChainingDetector {
    pub fn new() -> Self {
        Self {
            max_chain_length: 3,
            max_nested_depth: 3,
            scope_functions: vec![
                "let".to_string(),
                "apply".to_string(),
                "also".to_string(),
                "run".to_string(),
                "with".to_string(),
                "takeIf".to_string(),
                "takeUnless".to_string(),
            ],
        }
    }

    /// Set maximum chain length before warning
    #[allow(dead_code)]
    pub fn with_max_chain_length(mut self, max: usize) -> Self {
        self.max_chain_length = max;
        self
    }

    /// Set maximum nested depth before warning
    #[allow(dead_code)]
    pub fn with_max_nested_depth(mut self, max: usize) -> Self {
        self.max_nested_depth = max;
        self
    }

    /// Check if a name contains scope function indicators
    fn contains_scope_function(&self, name: &str) -> bool {
        self.scope_functions.iter().any(|sf| {
            // Check for scope function in camelCase method names
            let lower = name.to_lowercase();
            lower.contains(&sf.to_lowercase())
        })
    }

    /// Count scope functions in a method name/signature
    fn count_scope_functions_in_name(&self, name: &str) -> usize {
        let lower = name.to_lowercase();
        self.scope_functions
            .iter()
            .filter(|sf| lower.contains(&sf.to_lowercase()))
            .count()
    }
}

impl Default for ScopeFunctionChainingDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for ScopeFunctionChainingDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        // Check methods for scope function chaining patterns in their names
        for decl in graph.declarations() {
            if !matches!(decl.kind, DeclarationKind::Method | DeclarationKind::Function) {
                continue;
            }

            // Simple heuristic: check if method name contains multiple scope functions
            let scope_count = self.count_scope_functions_in_name(&decl.name);
            if scope_count >= self.max_chain_length {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::ScopeFunctionChaining);
                dead = dead.with_message(format!(
                    "Method '{}' appears to chain {} scope functions. Consider breaking into separate statements.",
                    decl.name, scope_count
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
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::path::PathBuf;

    fn create_method(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 50),
            name.to_string(),
            DeclarationKind::Method,
            Location::new(path, line, 1, line * 100, line * 100 + 50),
            Language::Kotlin,
        )
    }

    #[test]
    fn test_detector_creation() {
        let detector = ScopeFunctionChainingDetector::new();
        assert_eq!(detector.max_chain_length, 3);
        assert_eq!(detector.max_nested_depth, 3);
    }

    #[test]
    fn test_with_max_chain_length() {
        let detector = ScopeFunctionChainingDetector::new().with_max_chain_length(5);
        assert_eq!(detector.max_chain_length, 5);
    }

    #[test]
    fn test_contains_scope_function() {
        let detector = ScopeFunctionChainingDetector::new();
        assert!(detector.contains_scope_function("processWithLet"));
        assert!(detector.contains_scope_function("configureApply"));
        assert!(detector.contains_scope_function("doAlso"));
        assert!(detector.contains_scope_function("runOperation"));
        assert!(!detector.contains_scope_function("processData"));
        assert!(!detector.contains_scope_function("mapItems"));
    }

    #[test]
    fn test_count_scope_functions_in_name() {
        let detector = ScopeFunctionChainingDetector::new();
        // "processWithLetApplyAlso" contains: with, let, apply, also = 4
        assert_eq!(
            detector.count_scope_functions_in_name("processWithLetApplyAlso"),
            4
        );
        // "configureWithApply" contains: with, apply = 2
        assert_eq!(
            detector.count_scope_functions_in_name("configureWithApply"),
            2
        );
        assert_eq!(detector.count_scope_functions_in_name("processData"), 0);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = ScopeFunctionChainingDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_simple_method_no_issues() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processUser", 1));
        graph.add_declaration(create_method("saveData", 2));

        let detector = ScopeFunctionChainingDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_method_with_single_scope_function() {
        let mut graph = Graph::new();
        graph.add_declaration(create_method("processWithLet", 1));

        let detector = ScopeFunctionChainingDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Single scope function should be OK");
    }

    #[test]
    fn test_method_with_chained_scope_functions() {
        let mut graph = Graph::new();
        // Contains: with, let, apply, also = 4 scope functions
        graph.add_declaration(create_method("processWithLetApplyAlso", 1));

        let detector = ScopeFunctionChainingDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("4 scope functions"));
    }
}
