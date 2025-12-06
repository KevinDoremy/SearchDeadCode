//! Write-Only Variable Detector
//!
//! Detects variables (properties/fields) that are assigned values but never read.
//! This is a common form of dead code where developers store information they never use.
//!
//! ## Detection Algorithm
//!
//! 1. Find all private properties/fields (public ones might be read externally)
//! 2. For each property, count:
//!    - Write references (assignments)
//!    - Read references (usages in expressions, function args, etc.)
//! 3. Report as "write-only" if: writes > 0 AND reads == 0
//!
//! ## Examples Detected
//!
//! ```kotlin
//! class Example {
//!     private var lastUpdateTime: Long = 0  // DEAD: never read
//!
//!     fun update() {
//!         lastUpdateTime = System.currentTimeMillis()  // write-only!
//!     }
//! }
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, Visibility};

/// Detector for write-only variables (assigned but never read)
pub struct WriteOnlyDetector {
    /// Only check private variables (public might be read externally)
    private_only: bool,
    /// Minimum number of writes to report (avoid reporting uninitialized vars)
    min_writes: usize,
}

impl WriteOnlyDetector {
    pub fn new() -> Self {
        Self {
            private_only: true,
            min_writes: 1,
        }
    }

    /// Include non-private variables in detection (use with caution)
    #[allow(dead_code)]
    pub fn include_public(mut self) -> Self {
        self.private_only = false;
        self
    }

    /// Check if a declaration is a variable/property that could be write-only
    fn is_candidate(&self, decl: &crate::graph::Declaration) -> bool {
        // Must be a property or field
        if !matches!(
            decl.kind,
            DeclarationKind::Property | DeclarationKind::Field
        ) {
            return false;
        }

        // If private_only, must be private
        if self.private_only && decl.visibility != Visibility::Private {
            return false;
        }

        // Skip constants (val in Kotlin, final in Java) - they're meant to be read
        // We detect this by checking if the name is all caps (convention for constants)
        if decl.name.chars().all(|c| c.is_uppercase() || c == '_') {
            return false;
        }

        // Skip backing fields (start with underscore, typically for StateFlow)
        if decl.name.starts_with('_') {
            return false;
        }

        // Skip common framework-required fields
        let skip_names = [
            "binding",       // ViewBinding
            "viewModel",     // ViewModel
            "adapter",       // RecyclerView adapter
            "layoutManager", // RecyclerView layoutManager
        ];
        if skip_names.contains(&decl.name.as_str()) {
            return false;
        }

        true
    }
}

impl Default for WriteOnlyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for WriteOnlyDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();

        for decl in graph.declarations() {
            // Check if this is a candidate for write-only detection
            if !self.is_candidate(decl) {
                continue;
            }

            // Count reads and writes
            let read_count = graph.count_reads(&decl.id);
            let write_count = graph.count_writes(&decl.id);

            // Also count any other references (calls, etc.) as reads
            // This handles cases like property delegation
            let refs = graph.get_references_to(&decl.id);
            let other_refs = refs.iter().filter(|(_, r)| !r.kind.is_write()).count();
            let total_reads = read_count + other_refs;

            // Report if:
            // - Has at least min_writes
            // - Has zero reads
            if write_count >= self.min_writes && total_reads == 0 {
                let mut dead = DeadCode::new(decl.clone(), DeadCodeIssue::AssignOnly);
                dead = dead.with_message(format!(
                    "Property '{}' is assigned {} time(s) but never read",
                    decl.name, write_count
                ));
                dead = dead.with_confidence(Confidence::High);
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
        let detector = WriteOnlyDetector::new();
        assert!(detector.private_only);
        assert_eq!(detector.min_writes, 1);
    }

    #[test]
    fn test_skip_constants() {
        use std::path::PathBuf;
        let detector = WriteOnlyDetector::new();

        // Create a mock declaration for a constant
        let mut decl = crate::graph::Declaration::new(
            crate::graph::DeclarationId::new(PathBuf::from("test.kt"), 0, 10),
            "MAX_COUNT".to_string(),
            DeclarationKind::Property,
            crate::graph::Location::new(PathBuf::from("test.kt"), 1, 1, 0, 10),
            crate::graph::Language::Kotlin,
        );
        decl.visibility = Visibility::Private;

        // Constants (ALL_CAPS) should be skipped
        assert!(!detector.is_candidate(&decl));
    }

    #[test]
    fn test_skip_backing_fields() {
        use std::path::PathBuf;
        let detector = WriteOnlyDetector::new();

        let mut decl = crate::graph::Declaration::new(
            crate::graph::DeclarationId::new(PathBuf::from("test.kt"), 0, 10),
            "_state".to_string(),
            DeclarationKind::Property,
            crate::graph::Location::new(PathBuf::from("test.kt"), 1, 1, 0, 10),
            crate::graph::Language::Kotlin,
        );
        decl.visibility = Visibility::Private;

        // Backing fields (_underscore prefix) should be skipped
        assert!(!detector.is_candidate(&decl));
    }
}
