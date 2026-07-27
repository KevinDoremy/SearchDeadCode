//! Long Parameter List Detector
//!
//! Detects functions with too many parameters.
//!
//! ## Anti-Pattern
//!
//! ```kotlin
//! fun createUser(
//!     firstName: String,
//!     lastName: String,
//!     email: String,
//!     phone: String,
//!     address: String,
//!     city: String,
//!     country: String,
//!     postalCode: String
//! ): User
//! ```
//!
//! ## Why It's Bad
//!
//! - Hard to call correctly (easy to swap arguments)
//! - Boolean parameters are especially confusing
//! - Indicates the function does too much
//!
//! ## Better Alternatives
//!
//! - Use data class for related parameters
//! - Use builder pattern
//! - Split into smaller functions

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for functions with too many parameters
pub struct LongParameterListDetector {
    /// Maximum allowed parameters
    max_parameters: usize,
}

impl LongParameterListDetector {
    pub fn new() -> Self {
        Self { max_parameters: 6 }
    }

    /// Set maximum parameters before warning
    #[allow(dead_code)]
    pub fn with_max_parameters(mut self, max: usize) -> Self {
        self.max_parameters = max;
        self
    }

    /// Check if method has @Inject annotation (DI is OK)
    fn has_inject_annotation(decl: &crate::graph::Declaration) -> bool {
        decl.annotations
            .iter()
            .any(|a| a.to_lowercase().contains("inject"))
    }

    /// Check if it's a constructor
    fn is_constructor(decl: &crate::graph::Declaration) -> bool {
        matches!(decl.kind, DeclarationKind::Constructor)
            || decl.name == "init"
            || decl.name.starts_with("<init>")
    }

    /// Count parameters by looking at child declarations
    fn count_parameters(decl: &crate::graph::Declaration, graph: &Graph) -> usize {
        let graph_count = graph
            .get_children(&decl.id)
            .iter()
            .filter_map(|id| graph.get_declaration(id))
            .filter(|child| matches!(child.kind, DeclarationKind::Parameter))
            .count();
        if graph_count > 0 {
            return graph_count;
        }
        // The parsers do not emit Parameter children; count from the
        // signature text instead. Commas inside generics stay ignored
        // by tracking <> depth alongside () depth.
        Self::count_parameters_textually(decl).unwrap_or(0)
    }

    fn count_parameters_textually(decl: &crate::graph::Declaration) -> Option<usize> {
        let content = std::fs::read_to_string(&decl.location.file).ok()?;
        let span = content.get(decl.location.start_byte..decl.location.end_byte)?;
        let open = span.find('(')?;
        let mut paren_depth = 0usize;
        let mut angle_depth = 0usize;
        let mut commas = 0usize;
        let mut has_content = false;
        for ch in span[open..].chars() {
            match ch {
                '(' => paren_depth += 1,
                ')' => {
                    paren_depth -= 1;
                    if paren_depth == 0 {
                        break;
                    }
                }
                '<' => angle_depth += 1,
                '>' => angle_depth = angle_depth.saturating_sub(1),
                ',' if paren_depth == 1 && angle_depth == 0 => commas += 1,
                c if !c.is_whitespace() && paren_depth >= 1 => has_content = true,
                _ => {}
            }
        }
        Some(if has_content { commas + 1 } else { 0 })
    }
}

impl Default for LongParameterListDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for LongParameterListDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues: Vec<DeadCode> = Vec::new();

        for decl in graph.declarations() {
            // Only check methods, functions, and constructors
            if !matches!(
                decl.kind,
                DeclarationKind::Method | DeclarationKind::Function | DeclarationKind::Constructor
            ) {
                continue;
            }

            // Skip if has @Inject (DI framework handles this)
            if Self::has_inject_annotation(decl) {
                continue;
            }

            // Count parameters
            let param_count = Self::count_parameters(decl, graph);

            if param_count > self.max_parameters {
                let is_ctor = Self::is_constructor(decl);
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::LongParameterList);
                dead = dead.with_message(format!(
                    "{} '{}' has {} parameters (max recommended: {}). Consider using a data class or builder pattern.",
                    if is_ctor { "Constructor" } else { "Function" },
                    decl.name,
                    param_count,
                    self.max_parameters
                ));
                dead = dead.with_confidence(Confidence::Medium);
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

    fn textual_decl(source: &str) -> (Declaration, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Sig.kt");
        std::fs::write(&file, source).unwrap();
        let decl = Declaration::new(
            DeclarationId::new(file.clone(), 0, source.len()),
            "f".to_string(),
            DeclarationKind::Function,
            Location::new(file, 1, 1, 0, source.len()),
            Language::Kotlin,
        );
        (decl, temp)
    }

    #[test]
    fn textual_count_ignores_commas_inside_generics() {
        let (decl, _tmp) = textual_decl("fun f(m: Map<String, Int>, n: Int) {}\n");
        assert_eq!(
            LongParameterListDetector::count_parameters_textually(&decl),
            Some(2),
            "the comma inside Map<String, Int> is not a parameter separator"
        );
    }

    #[test]
    fn textual_count_of_an_empty_signature_is_zero() {
        let (decl, _tmp) = textual_decl("fun f() {}\n");
        assert_eq!(
            LongParameterListDetector::count_parameters_textually(&decl),
            Some(0)
        );
    }

    #[test]
    fn textual_count_ignores_nested_lambda_parens() {
        let (decl, _tmp) = textual_decl("fun f(cb: (Int, Int) -> Int, x: Int) {}\n");
        assert_eq!(
            LongParameterListDetector::count_parameters_textually(&decl),
            Some(2),
            "the comma inside the lambda type sits at paren depth 2"
        );
    }

    fn create_function(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 200),
            name.to_string(),
            DeclarationKind::Function,
            Location::new(path, line, 1, line * 100, line * 100 + 200),
            Language::Kotlin,
        )
    }

    fn create_parameter(name: &str, parent_id: DeclarationId, line: usize) -> Declaration {
        let path = PathBuf::from("test.kt");
        let mut decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 20),
            name.to_string(),
            DeclarationKind::Parameter,
            Location::new(path, line, 1, line * 100, line * 100 + 20),
            Language::Kotlin,
        );
        decl.parent = Some(parent_id);
        decl
    }

    #[test]
    fn test_detector_creation() {
        let detector = LongParameterListDetector::new();
        assert_eq!(detector.max_parameters, 6);
    }

    #[test]
    fn test_with_max_parameters() {
        let detector = LongParameterListDetector::new().with_max_parameters(8);
        assert_eq!(detector.max_parameters, 8);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Graph::new();
        let detector = LongParameterListDetector::new();
        let issues = detector.detect(&graph);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_too_many_parameters() {
        let mut graph = Graph::new();
        let func = create_function("createUser", 1);
        let func_id = func.id.clone();
        graph.add_declaration(func);

        // Add 8 parameters
        for i in 0..8 {
            graph.add_declaration(create_parameter(
                &format!("param{}", i),
                func_id.clone(),
                2 + i,
            ));
        }

        let detector = LongParameterListDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("8 parameters"));
    }

    #[test]
    fn test_few_parameters_ok() {
        let mut graph = Graph::new();
        let func = create_function("formatName", 1);
        let func_id = func.id.clone();
        graph.add_declaration(func);

        // Add 3 parameters
        for i in 0..3 {
            graph.add_declaration(create_parameter(
                &format!("param{}", i),
                func_id.clone(),
                2 + i,
            ));
        }

        let detector = LongParameterListDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_inject_annotation_ok() {
        let mut graph = Graph::new();
        let mut func = create_function("InjectedClass", 1);
        func.annotations.push("Inject".to_string());
        let func_id = func.id.clone();
        graph.add_declaration(func);

        // Add 8 parameters
        for i in 0..8 {
            graph.add_declaration(create_parameter(
                &format!("dep{}", i),
                func_id.clone(),
                2 + i,
            ));
        }

        let detector = LongParameterListDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty());
    }
}
