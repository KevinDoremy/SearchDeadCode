//! Duplicate Import Detector
//!
//! Detects import statements that appear multiple times in the same file.
//! This is a common issue that can occur during refactoring or merging.
//!
//! ## Detection Algorithm
//!
//! 1. Group all imports by file
//! 2. For each file, track seen import names
//! 3. Report duplicates (second occurrence onwards)
//!
//! ## Examples Detected
//!
//! ```kotlin
//! import kotlin.collections.List
//! import kotlin.collections.List  // DUPLICATE
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph};
use std::collections::{HashMap, HashSet};

/// Detector for duplicate import statements
pub struct DuplicateImportDetector;

impl DuplicateImportDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DuplicateImportDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for DuplicateImportDetector {
    fn detect(&self, graph: &Graph) -> Vec<DeadCode> {
        let mut issues = Vec::new();

        // Group imports by file
        let mut imports_by_file: HashMap<&std::path::Path, Vec<_>> = HashMap::new();

        for decl in graph.declarations() {
            if decl.kind == DeclarationKind::Import {
                imports_by_file
                    .entry(decl.location.file.as_path())
                    .or_default()
                    .push(decl);
            }
        }

        // Find duplicates in each file
        for (_file, mut imports) in imports_by_file {
            // Sort imports by line number to ensure consistent ordering
            imports.sort_by_key(|i| i.location.line);

            let mut seen: HashSet<&str> = HashSet::new();
            let mut first_occurrence: HashMap<&str, usize> = HashMap::new();

            for (idx, import) in imports.iter().enumerate() {
                let import_name = &import.name;

                if seen.contains(import_name.as_str()) {
                    // This is a duplicate
                    let mut dead = DeadCode::new((*import).clone(), DeadCodeIssue::DuplicateImport);
                    let first_line = imports[*first_occurrence.get(import_name.as_str()).unwrap()]
                        .location
                        .line;
                    dead = dead.with_message(format!(
                        "Import '{}' is duplicated (first occurrence at line {})",
                        import_name, first_line
                    ));
                    dead = dead.with_confidence(Confidence::High);
                    issues.push(dead);
                } else {
                    seen.insert(import_name);
                    first_occurrence.insert(import_name, idx);
                }
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
    use crate::graph::{Declaration, DeclarationId, Language, Location};
    use std::path::PathBuf;

    fn create_import(file: &str, name: &str, line: usize) -> Declaration {
        let path = PathBuf::from(file);
        Declaration::new(
            DeclarationId::new(path.clone(), line * 10, line * 10 + 5),
            name.to_string(),
            DeclarationKind::Import,
            Location::new(path, line, 1, line * 10, line * 10 + 5),
            Language::Kotlin,
        )
    }

    #[test]
    fn test_detector_creation() {
        let detector = DuplicateImportDetector::new();
        let default_detector = DuplicateImportDetector::default();
        // Both should be valid
        let _ = detector;
        let _ = default_detector;
    }

    #[test]
    fn test_no_duplicates() {
        let mut graph = Graph::new();
        graph.add_declaration(create_import("test.kt", "kotlin.collections.List", 1));
        graph.add_declaration(create_import("test.kt", "kotlin.collections.Map", 2));
        graph.add_declaration(create_import("test.kt", "kotlin.collections.Set", 3));

        let detector = DuplicateImportDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Should not find duplicates when all imports are unique");
    }

    #[test]
    fn test_finds_duplicate() {
        let mut graph = Graph::new();
        graph.add_declaration(create_import("test.kt", "kotlin.collections.List", 1));
        graph.add_declaration(create_import("test.kt", "kotlin.collections.List", 2)); // Duplicate

        let detector = DuplicateImportDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 1, "Should find one duplicate");
        assert_eq!(issues[0].declaration.name, "kotlin.collections.List");
        assert_eq!(issues[0].declaration.location.line, 2); // Reports the second occurrence
    }

    #[test]
    fn test_finds_multiple_duplicates() {
        let mut graph = Graph::new();
        graph.add_declaration(create_import("test.kt", "kotlin.collections.List", 1));
        graph.add_declaration(create_import("test.kt", "kotlin.collections.List", 2)); // Duplicate
        graph.add_declaration(create_import("test.kt", "kotlin.collections.List", 3)); // Duplicate

        let detector = DuplicateImportDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues.len(), 2, "Should find two duplicates (second and third occurrence)");
    }

    #[test]
    fn test_different_files_not_duplicates() {
        let mut graph = Graph::new();
        graph.add_declaration(create_import("file1.kt", "kotlin.collections.List", 1));
        graph.add_declaration(create_import("file2.kt", "kotlin.collections.List", 1));

        let detector = DuplicateImportDetector::new();
        let issues = detector.detect(&graph);

        assert!(issues.is_empty(), "Same import in different files should not be duplicate");
    }

    #[test]
    fn test_duplicate_confidence_is_high() {
        let mut graph = Graph::new();
        graph.add_declaration(create_import("test.kt", "kotlin.collections.List", 1));
        graph.add_declaration(create_import("test.kt", "kotlin.collections.List", 2));

        let detector = DuplicateImportDetector::new();
        let issues = detector.detect(&graph);

        assert_eq!(issues[0].confidence, Confidence::High);
    }
}
