//! Unused function parameter detection
//!
//! This detector finds function/method parameters that are declared but never
//! used within the function body.

use super::Detector;
use crate::analysis::{DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for unused function parameters
pub struct UnusedParamDetector {
    /// Skip parameters with underscore prefix (Kotlin convention for unused)
    skip_underscore: bool,
    /// Skip parameters in abstract/interface methods
    skip_abstract: bool,
    /// Skip parameters in override methods
    skip_override: bool,
}

impl UnusedParamDetector {
    pub fn new() -> Self {
        Self {
            skip_underscore: true,
            skip_abstract: true,
            skip_override: true,
        }
    }

    /// Configure whether to skip underscore-prefixed parameters
    pub fn with_skip_underscore(mut self, skip: bool) -> Self {
        self.skip_underscore = skip;
        self
    }

    /// Check if a parameter should be skipped based on naming convention
    fn should_skip_name(&self, name: &str) -> bool {
        if self.skip_underscore && name.starts_with('_') {
            return true;
        }
        // Skip common framework parameter names that are often required but unused
        if name == "savedInstanceState" || name == "context" || name == "parent" || name == "view" {
            return true;
        }
        false
    }

    /// The developer already answered this diagnostic by name: `@Suppress` on
    /// the parameter itself, its enclosing function, or an enclosing type.
    fn is_suppressed(annotations: &[String]) -> bool {
        crate::analysis::suppress::annotations_suppress(
            annotations,
            crate::analysis::suppress::UNUSED_PARAMETER,
        )
    }

    /// Check if a parameter's parent function should be skipped
    fn should_skip_parent(&self, graph: &Graph, parent_id: &crate::graph::DeclarationId) -> bool {
        if let Some(parent) = graph.get_declaration(parent_id) {
            // Skip abstract methods
            if self.skip_abstract && parent.is_abstract {
                return true;
            }

            // The JVM entry point's signature is imposed: it takes the
            // argument array whether it reads it or not. A method merely
            // NAMED main is called like any other, so its dead parameters
            // stay reportable. Beyond one parameter the signature is nobody's
            // contract either — `main(config, unused)` is an ordinary
            // function that happens to be called main.
            if parent.name == "main"
                && crate::analysis::entry_points::is_main_entry_shape(parent)
                && graph
                    .get_children(&parent.id)
                    .iter()
                    .filter(|c| {
                        graph
                            .get_declaration(c)
                            .is_some_and(|d| d.kind == DeclarationKind::Parameter)
                    })
                    .count()
                    <= 1
            {
                return true;
            }

            if Self::is_suppressed(&parent.annotations) {
                return true;
            }

            // A @Suppress on an enclosing type covers its members' parameters
            let mut ancestor = parent.parent.clone();
            while let Some(ancestor_id) = ancestor {
                let Some(decl) = graph.get_declaration(&ancestor_id) else {
                    break;
                };
                if Self::is_suppressed(&decl.annotations) {
                    return true;
                }
                ancestor = decl.parent.clone();
            }

            // Skip override methods
            if self.skip_override {
                if parent.modifiers.iter().any(|m| m == "override") {
                    return true;
                }
                if parent.annotations.iter().any(|a| a.contains("Override")) {
                    return true;
                }
            }

            // Skip interface methods
            if let Some(grandparent_id) = &parent.parent {
                if let Some(grandparent) = graph.get_declaration(grandparent_id) {
                    if grandparent.kind == DeclarationKind::Interface {
                        return true;
                    }
                }
            }

            // Skip constructors (parameters often used for property initialization)
            if parent.kind == DeclarationKind::Constructor {
                return true;
            }

            // An operator convention has a signature the language imposes: a
            // property delegate must take `thisRef` and `property` whether it
            // reads them or not, `plus` must take its operand. Reporting those
            // is not actionable, they cannot be removed.
            if crate::analysis::deep::is_operator_convention(parent) {
                return true;
            }

            // Skip @Composable functions (parameters used for recomposition)
            if parent.annotations.iter().any(|a| a.contains("Composable")) {
                return true;
            }

            // Skip common callback/listener patterns
            if parent.name.starts_with("on")
                || parent.name.ends_with("Listener")
                || parent.name.ends_with("Callback")
            {
                return true;
            }
        }
        false
    }
}

impl Detector for UnusedParamDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut dead_code = Vec::new();

        // Find all parameter declarations
        for decl in graph.declarations() {
            if decl.kind != DeclarationKind::Parameter {
                continue;
            }

            // Check naming convention
            if self.should_skip_name(&decl.name) {
                continue;
            }

            if Self::is_suppressed(&decl.annotations) {
                continue;
            }

            // Check if parent function should be skipped
            if let Some(ref parent_id) = decl.parent {
                if self.should_skip_parent(graph, parent_id) {
                    continue;
                }
            }

            // Check if the parameter is referenced anywhere
            if !graph.is_referenced(&decl.id) {
                dead_code.push(DeadCode::new(decl.clone(), DeadCodeIssue::UnusedParameter));
            }
        }

        dead_code
    }
}

impl Default for UnusedParamDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_creation() {
        let detector = UnusedParamDetector::new();
        let graph = Graph::new();
        let results = detector.detect(&graph);
        assert!(results.is_empty());
    }

    #[test]
    fn test_skip_underscore() {
        let detector = UnusedParamDetector::new();
        assert!(detector.should_skip_name("_unused"));
        assert!(detector.should_skip_name("_"));
        assert!(!detector.should_skip_name("used"));
    }

    #[test]
    fn test_skip_framework_params() {
        let detector = UnusedParamDetector::new();
        assert!(detector.should_skip_name("savedInstanceState"));
        assert!(detector.should_skip_name("context"));
        assert!(!detector.should_skip_name("myParam"));
    }
}
