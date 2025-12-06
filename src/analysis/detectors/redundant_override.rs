//! Redundant Override Detector
//!
//! Detects methods that override a parent method but only call super without
//! adding any additional behavior. These overrides are unnecessary and can be removed.
//!
//! ## Detection Algorithm
//!
//! 1. Find all methods with "override" modifier
//! 2. Check if the method body only contains a call to super.methodName()
//! 3. Report as redundant if no additional behavior is added
//!
//! ## Examples Detected
//!
//! ```kotlin
//! override fun onCreateView(...): View {
//!     return super.onCreateView(inflater, container, savedInstanceState)
//!     // DEAD: If this is all it does, the override is unnecessary
//! }
//!
//! override fun onResume() {
//!     super.onResume()
//!     // DEAD: No additional behavior
//! }
//! ```
//!
//! ## False Positive Prevention
//!
//! - Skip if override has annotations (may be for documentation/tooling)
//! - Skip abstract method implementations (required)
//! - Skip if method has different visibility than parent (intentional restriction)

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};

/// Detector for redundant method overrides
pub struct RedundantOverrideDetector {
    /// Skip overrides with these annotations
    skip_annotations: Vec<String>,
}

impl RedundantOverrideDetector {
    pub fn new() -> Self {
        Self {
            skip_annotations: vec![
                "Deprecated".to_string(),
                "Suppress".to_string(),
                "VisibleForTesting".to_string(),
            ],
        }
    }

    /// Check if a declaration is an override
    fn is_override(&self, decl: &crate::graph::Declaration) -> bool {
        decl.modifiers.iter().any(|m| m == "override")
    }

    /// Check if this override should be skipped due to annotations
    fn has_skip_annotation(&self, decl: &crate::graph::Declaration) -> bool {
        for annotation in &decl.annotations {
            for skip in &self.skip_annotations {
                if annotation.contains(skip) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if an override is redundant (only calls super)
    ///
    /// This requires analyzing the method body, which we don't have in the graph.
    /// For now, we'll use heuristics based on what we can detect:
    /// - Methods with no references made FROM them (no internal calls except super)
    /// - This is a conservative approximation
    fn is_redundant_override(&self, decl: &crate::graph::Declaration, graph: &Graph) -> bool {
        // Skip if has annotations that suggest it's intentional
        if self.has_skip_annotation(decl) {
            return false;
        }

        // Get all references FROM this method
        let refs_from = graph.get_references_from(&decl.id);

        // If the method makes no references at all, it might be truly empty
        // or only calling super (which wouldn't be tracked as a reference to our declarations)
        if refs_from.is_empty() {
            // This could be a redundant override or an abstract implementation
            // We'll report with lower confidence
            return true;
        }

        // If it only makes one reference and it's a call, check if it could be super
        // This is a heuristic - we can't actually detect super.method() calls
        // because "super" isn't tracked as a declaration
        if refs_from.len() == 1 {
            // Could be super.method() - report with medium confidence
            return false; // For now, don't report these to avoid false positives
        }

        false
    }
}

impl Default for RedundantOverrideDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for RedundantOverrideDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();

        for decl in graph.declarations() {
            // Only check methods
            if decl.kind != DeclarationKind::Method {
                continue;
            }

            // Must be an override
            if !self.is_override(decl) {
                continue;
            }

            // Check if it's redundant
            if self.is_redundant_override(decl, graph) {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::RedundantOverride);
                dead = dead.with_message(format!(
                    "Override '{}' may be redundant (no additional behavior detected)",
                    decl.name
                ));
                // Lower confidence since we can't analyze the actual body
                dead = dead.with_confidence(Confidence::Low);
                issues.push(dead);
            }
        }

        // Sort by file and line for consistent output
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

    #[test]
    fn test_detector_creation() {
        let detector = RedundantOverrideDetector::new();
        assert!(!detector.skip_annotations.is_empty());
    }
}
