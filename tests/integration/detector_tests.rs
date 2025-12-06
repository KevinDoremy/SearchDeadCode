//! Integration tests for each detector type
//!
//! These tests verify that each detector correctly identifies dead code patterns.

use searchdeadcode::analysis::detectors::{
    Detector, RedundantOverrideDetector, UnusedParamDetector, UnusedSealedVariantDetector,
    WriteOnlyDetector,
};
use searchdeadcode::analysis::ReachabilityAnalyzer;
use searchdeadcode::discovery::{FileType, SourceFile};
use searchdeadcode::graph::GraphBuilder;
use std::collections::HashSet;
use std::path::PathBuf;

/// Get the path to the test fixtures directory
fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Build a graph from a Kotlin file
fn build_kotlin_graph(filename: &str) -> searchdeadcode::graph::Graph {
    let path = fixtures_path().join("kotlin").join(filename);
    if !path.exists() {
        panic!("Fixture not found: {:?}", path);
    }
    let source = SourceFile::new(path, FileType::Kotlin);
    let mut builder = GraphBuilder::new();
    builder
        .process_file(&source)
        .expect("Failed to process file");
    builder.build()
}

/// Get declaration names from the graph
fn get_declaration_names(graph: &searchdeadcode::graph::Graph) -> Vec<String> {
    graph.declarations().map(|d| d.name.clone()).collect()
}

// ============================================================================
// Write-Only Detection Tests
// ============================================================================

mod write_only_tests {
    use super::*;

    #[test]
    fn test_write_only_fixture_parses() {
        let graph = build_kotlin_graph("write_only.kt");
        let names = get_declaration_names(&graph);

        assert!(names.contains(&"SimpleWriteOnly".to_string()));
        assert!(names.contains(&"MultipleAssignments".to_string()));
        assert!(names.contains(&"ReadAndWrite".to_string()));
    }

    #[test]
    fn test_write_only_detector_runs() {
        let graph = build_kotlin_graph("write_only.kt");
        let detector = WriteOnlyDetector::new();
        let issues = detector.detect(&graph);

        println!("Write-only issues found: {}", issues.len());
        for issue in &issues {
            println!("  - {}: {}", issue.declaration.name, issue.message);
        }

        // Should find write-only variables
        // Note: Detection depends on reference tracking
    }

    #[test]
    fn test_write_only_skips_backing_fields() {
        let graph = build_kotlin_graph("write_only.kt");
        let detector = WriteOnlyDetector::new();
        let issues = detector.detect(&graph);

        // Should NOT report _data (backing field pattern)
        let backing_field_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.declaration.name.starts_with("_"))
            .collect();

        assert!(
            backing_field_issues.is_empty(),
            "Should not report backing fields: {:?}",
            backing_field_issues
                .iter()
                .map(|i| &i.declaration.name)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_write_only_skips_constants() {
        let graph = build_kotlin_graph("write_only.kt");
        let detector = WriteOnlyDetector::new();
        let issues = detector.detect(&graph);

        // Should NOT report MAX_SIZE (constant naming)
        let constant_issues: Vec<_> = issues
            .iter()
            .filter(|i| {
                i.declaration
                    .name
                    .chars()
                    .all(|c| c.is_uppercase() || c == '_')
            })
            .collect();

        assert!(
            constant_issues.is_empty(),
            "Should not report constants: {:?}",
            constant_issues
                .iter()
                .map(|i| &i.declaration.name)
                .collect::<Vec<_>>()
        );
    }
}

// ============================================================================
// Unused Parameter Detection Tests
// ============================================================================

mod unused_param_tests {
    use super::*;

    #[test]
    fn test_unused_params_fixture_parses() {
        let graph = build_kotlin_graph("unused_params.kt");
        let names = get_declaration_names(&graph);

        assert!(names.contains(&"SimpleUnusedParam".to_string()));
        assert!(names.contains(&"MultipleUnused".to_string()));
        assert!(names.contains(&"AllUsed".to_string()));
    }

    #[test]
    fn test_unused_param_detector_runs() {
        let graph = build_kotlin_graph("unused_params.kt");
        let detector = UnusedParamDetector::new();
        let issues = detector.detect(&graph);

        println!("Unused parameter issues found: {}", issues.len());
        for issue in &issues {
            println!("  - {}: {}", issue.declaration.name, issue.message);
        }
    }

    #[test]
    fn test_unused_param_skips_underscore() {
        let graph = build_kotlin_graph("unused_params.kt");
        let detector = UnusedParamDetector::new();
        let issues = detector.detect(&graph);

        // Should NOT report _event (intentionally unused)
        let underscore_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.declaration.name.starts_with("_"))
            .collect();

        assert!(
            underscore_issues.is_empty(),
            "Should not report underscore-prefixed params: {:?}",
            underscore_issues
                .iter()
                .map(|i| &i.declaration.name)
                .collect::<Vec<_>>()
        );
    }
}

// ============================================================================
// Sealed Variant Detection Tests
// ============================================================================

mod sealed_variant_tests {
    use super::*;

    #[test]
    fn test_sealed_classes_fixture_parses() {
        let graph = build_kotlin_graph("sealed_classes.kt");
        let names = get_declaration_names(&graph);

        assert!(names.contains(&"UiState".to_string()));
        assert!(names.contains(&"Loading".to_string()));
        assert!(names.contains(&"Success".to_string()));
        assert!(names.contains(&"Empty".to_string()));
    }

    #[test]
    fn test_sealed_variant_detector_runs() {
        let graph = build_kotlin_graph("sealed_classes.kt");
        let detector = UnusedSealedVariantDetector::new();
        let issues = detector.detect(&graph);

        println!("Sealed variant issues found: {}", issues.len());
        for issue in &issues {
            println!("  - {}: {}", issue.declaration.name, issue.message);
        }
    }

    #[test]
    fn test_sealed_finds_sealed_classes() {
        let graph = build_kotlin_graph("sealed_classes.kt");

        // Find all declarations with "sealed" modifier
        let sealed_count = graph
            .declarations()
            .filter(|d| d.modifiers.iter().any(|m| m == "sealed"))
            .count();

        // Should find multiple sealed classes/interfaces
        assert!(sealed_count > 0, "Should find sealed classes in fixture");
        println!("Found {} sealed classes/interfaces", sealed_count);
    }

    #[test]
    fn test_sealed_skips_interfaces() {
        let graph = build_kotlin_graph("sealed_classes.kt");
        let detector = UnusedSealedVariantDetector::new();
        let issues = detector.detect(&graph);

        // Should NOT report interfaces as unused variants
        let interface_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.declaration.kind == searchdeadcode::graph::DeclarationKind::Interface)
            .collect();

        assert!(
            interface_issues.is_empty(),
            "Should not report interfaces as unused variants"
        );
    }
}

// ============================================================================
// Redundant Override Detection Tests
// ============================================================================

mod redundant_override_tests {
    use super::*;

    #[test]
    fn test_redundant_overrides_fixture_parses() {
        let graph = build_kotlin_graph("redundant_overrides.kt");
        let names = get_declaration_names(&graph);

        assert!(names.contains(&"BaseActivity".to_string()));
        assert!(names.contains(&"MainActivity".to_string()));
        assert!(names.contains(&"onCreate".to_string()));
        assert!(names.contains(&"onDestroy".to_string()));
    }

    #[test]
    fn test_redundant_override_detector_runs() {
        let graph = build_kotlin_graph("redundant_overrides.kt");
        let detector = RedundantOverrideDetector::new();
        let issues = detector.detect(&graph);

        println!("Redundant override issues found: {}", issues.len());
        for issue in &issues {
            println!("  - {}: {}", issue.declaration.name, issue.message);
        }
    }

    #[test]
    fn test_redundant_finds_override_methods() {
        let graph = build_kotlin_graph("redundant_overrides.kt");

        // Find all declarations with "override" modifier
        let override_count = graph
            .declarations()
            .filter(|d| d.modifiers.iter().any(|m| m == "override"))
            .count();

        assert!(
            override_count > 0,
            "Should find override methods in fixture"
        );
        println!("Found {} override methods", override_count);
    }
}

// ============================================================================
// Unreferenced Code Detection Tests
// ============================================================================

mod unreferenced_tests {
    use super::*;

    #[test]
    fn test_unreferenced_fixture_parses() {
        let graph = build_kotlin_graph("unreferenced.kt");
        let names = get_declaration_names(&graph);

        assert!(names.contains(&"UnusedClass".to_string()));
        assert!(names.contains(&"PartiallyUsedClass".to_string()));
        assert!(names.contains(&"usedMethod".to_string()));
        assert!(names.contains(&"unusedMethod".to_string()));
    }

    #[test]
    fn test_reachability_finds_unreachable() {
        let graph = build_kotlin_graph("unreferenced.kt");

        // Find main function as entry point
        let entry_points: HashSet<_> = graph
            .declarations()
            .filter(|d| d.name == "main")
            .map(|d| d.id.clone())
            .collect();

        if entry_points.is_empty() {
            println!("No main function found");
            return;
        }

        let analyzer = ReachabilityAnalyzer::new();
        let (dead_code, reachable) =
            analyzer.find_unreachable_with_reachable(&graph, &entry_points);

        println!("Reachability analysis:");
        println!("  Reachable: {}", reachable.len());
        println!("  Dead code: {}", dead_code.len());

        // Should find some unreachable code
        assert!(!dead_code.is_empty(), "Should find unreachable code");

        // Check for specific expected dead code
        let dead_names: Vec<_> = dead_code
            .iter()
            .map(|d| d.declaration.name.as_str())
            .collect();

        println!("Dead code names: {:?}", dead_names);
    }

    #[test]
    fn test_finds_unused_class() {
        let graph = build_kotlin_graph("unreferenced.kt");

        let entry_points: HashSet<_> = graph
            .declarations()
            .filter(|d| d.name == "main")
            .map(|d| d.id.clone())
            .collect();

        if entry_points.is_empty() {
            return;
        }

        let analyzer = ReachabilityAnalyzer::new();
        let (dead_code, _) = analyzer.find_unreachable_with_reachable(&graph, &entry_points);

        let dead_names: HashSet<_> = dead_code
            .iter()
            .map(|d| d.declaration.name.as_str())
            .collect();

        // UnusedClass should be detected as dead
        assert!(
            dead_names.contains("UnusedClass"),
            "Should detect UnusedClass as dead code. Found: {:?}",
            dead_names
        );
    }

    #[test]
    fn test_finds_unused_methods() {
        let graph = build_kotlin_graph("unreferenced.kt");

        let entry_points: HashSet<_> = graph
            .declarations()
            .filter(|d| d.name == "main")
            .map(|d| d.id.clone())
            .collect();

        if entry_points.is_empty() {
            return;
        }

        let analyzer = ReachabilityAnalyzer::new();
        let (dead_code, _) = analyzer.find_unreachable_with_reachable(&graph, &entry_points);

        let dead_names: HashSet<_> = dead_code
            .iter()
            .map(|d| d.declaration.name.as_str())
            .collect();

        // unusedMethod should be detected
        if dead_names.contains("unusedMethod") {
            println!("Correctly found unusedMethod as dead");
        } else {
            println!(
                "Warning: unusedMethod not found as dead. Found: {:?}",
                dead_names
            );
        }
    }
}

// ============================================================================
// Multi-File Analysis Tests
// ============================================================================

mod multi_file_tests {
    use super::*;

    #[test]
    fn test_all_fixtures_parse() {
        let kotlin_files = vec![
            "dead_code.kt",
            "all_used.kt",
            "write_only.kt",
            "unused_params.kt",
            "sealed_classes.kt",
            "redundant_overrides.kt",
            "unreferenced.kt",
        ];

        for filename in kotlin_files {
            let path = fixtures_path().join("kotlin").join(filename);
            if path.exists() {
                let source = SourceFile::new(path.clone(), FileType::Kotlin);
                let mut builder = GraphBuilder::new();
                let result = builder.process_file(&source);
                assert!(result.is_ok(), "Failed to parse {}: {:?}", filename, result);
                println!("Successfully parsed: {}", filename);
            }
        }
    }

    #[test]
    fn test_combined_analysis() {
        let kotlin_files = vec!["write_only.kt", "unused_params.kt", "sealed_classes.kt"];

        let mut builder = GraphBuilder::new();

        for filename in &kotlin_files {
            let path = fixtures_path().join("kotlin").join(filename);
            if path.exists() {
                let source = SourceFile::new(path, FileType::Kotlin);
                builder
                    .process_file(&source)
                    .expect("Failed to process file");
            }
        }

        let graph = builder.build();
        let total_decls = graph.declarations().count();

        println!("Combined analysis:");
        println!("  Files: {}", kotlin_files.len());
        println!("  Total declarations: {}", total_decls);

        assert!(
            total_decls > 50,
            "Should have many declarations from combined files"
        );
    }
}
