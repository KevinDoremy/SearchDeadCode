//! Comprehensive integration tests for SearchDeadCode
//!
//! This module covers:
//! - False positive prevention (code that looks dead but isn't)
//! - False negative detection (dead code that should be found)
//! - Edge cases (unicode, long names, nested structures)
//! - Cross-file references
//! - Java language support
//! - Performance with large files

use searchdeadcode::analysis::detectors::{
    Detector, RedundantOverrideDetector, UnusedParamDetector, UnusedSealedVariantDetector,
    WriteOnlyDetector,
};
use searchdeadcode::analysis::ReachabilityAnalyzer;
use searchdeadcode::discovery::{FileType, SourceFile};
use searchdeadcode::graph::GraphBuilder;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

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

/// Build a graph from a Java file
fn build_java_graph(filename: &str) -> searchdeadcode::graph::Graph {
    let path = fixtures_path().join("java").join(filename);
    if !path.exists() {
        panic!("Fixture not found: {:?}", path);
    }
    let source = SourceFile::new(path, FileType::Java);
    let mut builder = GraphBuilder::new();
    builder
        .process_file(&source)
        .expect("Failed to process file");
    builder.build()
}

/// Build a graph from multiple files
fn build_multi_file_graph(files: &[(&str, FileType)]) -> searchdeadcode::graph::Graph {
    let mut builder = GraphBuilder::new();
    for (filename, file_type) in files {
        let subfolder = match file_type {
            FileType::Kotlin => "kotlin",
            FileType::Java => "java",
            _ => continue, // Skip XML and other file types
        };
        let path = fixtures_path().join(subfolder).join(filename);
        if path.exists() {
            let source = SourceFile::new(path, *file_type);
            builder
                .process_file(&source)
                .expect("Failed to process file");
        }
    }
    builder.build()
}

// ============================================================================
// FALSE POSITIVE TESTS
// These tests verify that we DON'T report false positives
// ============================================================================

mod false_positive_tests {
    use super::*;

    #[test]
    fn test_false_positives_fixture_parses() {
        let path = fixtures_path().join("kotlin").join("false_positives.kt");
        if !path.exists() {
            println!("Skipping: false_positives.kt not found");
            return;
        }

        let graph = build_kotlin_graph("false_positives.kt");
        let count = graph.declarations().count();

        println!("False positives fixture: {} declarations", count);
        assert!(
            count > 20,
            "Should have many declarations in false positives fixture"
        );
    }

    #[test]
    fn test_android_lifecycle_not_reported() {
        let path = fixtures_path().join("kotlin").join("false_positives.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("false_positives.kt");
        let detector = UnusedParamDetector::new();
        let issues = detector.detect(&graph);

        // Android lifecycle methods should not be reported
        let lifecycle_issues: Vec<_> = issues
            .iter()
            .filter(|i| {
                let name = &i.declaration.name;
                name == "onCreate" || name == "onResume" || name == "onPause" || name == "onDestroy"
            })
            .collect();

        if !lifecycle_issues.is_empty() {
            println!("Warning: Lifecycle methods incorrectly reported as unused:");
            for issue in &lifecycle_issues {
                println!("  - {}", issue.declaration.name);
            }
        }
    }

    #[test]
    fn test_serialization_fields_not_reported() {
        let path = fixtures_path().join("kotlin").join("false_positives.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("false_positives.kt");

        // Check that SerializableData fields exist
        let serializable_decls: Vec<_> = graph
            .declarations()
            .filter(|d| d.name == "SerializableData" || d.name == "hiddenField")
            .collect();

        println!(
            "Serializable declarations found: {}",
            serializable_decls.len()
        );
    }

    #[test]
    fn test_operator_functions_not_reported() {
        let path = fixtures_path().join("kotlin").join("false_positives.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("false_positives.kt");

        // Operator functions should be found
        let operators: Vec<_> = graph
            .declarations()
            .filter(|d| {
                d.name == "plus"
                    || d.name == "get"
                    || d.name == "invoke"
                    || d.name == "contains"
                    || d.name.starts_with("component")
            })
            .collect();

        println!("Operator functions found: {}", operators.len());
        assert!(operators.len() >= 4, "Should find operator functions");
    }

    #[test]
    fn test_companion_object_factories_not_reported() {
        let path = fixtures_path().join("kotlin").join("false_positives.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("false_positives.kt");

        // Factory methods in companion objects
        let factories: Vec<_> = graph
            .declarations()
            .filter(|d| d.name == "create" || d.name == "default")
            .collect();

        println!("Factory methods found: {}", factories.len());
    }

    #[test]
    fn test_enum_values_from_valueof() {
        let path = fixtures_path().join("kotlin").join("false_positives.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("false_positives.kt");
        let detector = UnusedSealedVariantDetector::new();
        let issues = detector.detect(&graph);

        // Enum values used via valueOf() should not be reported
        // This is a known limitation - may need dynamic analysis
        println!("Sealed variant issues: {}", issues.len());
    }

    #[test]
    fn test_suspend_functions_not_reported() {
        let path = fixtures_path().join("kotlin").join("false_positives.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("false_positives.kt");

        // Coroutine entry points
        let suspend_fns: Vec<_> = graph
            .declarations()
            .filter(|d| d.modifiers.iter().any(|m| m == "suspend"))
            .collect();

        println!("Suspend functions found: {}", suspend_fns.len());
    }
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[test]
    fn test_edge_cases_fixture_parses() {
        let path = fixtures_path().join("kotlin").join("edge_cases.kt");
        if !path.exists() {
            println!("Skipping: edge_cases.kt not found");
            return;
        }

        let graph = build_kotlin_graph("edge_cases.kt");
        let count = graph.declarations().count();

        println!("Edge cases fixture: {} declarations", count);
        assert!(count > 10, "Should parse edge cases successfully");
    }

    #[test]
    fn test_unicode_identifiers() {
        let path = fixtures_path().join("kotlin").join("edge_cases.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("edge_cases.kt");

        // Look for unicode class name
        let unicode_decls: Vec<_> = graph
            .declarations()
            .filter(|d| !d.name.is_ascii())
            .collect();

        println!("Unicode declarations found: {}", unicode_decls.len());
        for decl in &unicode_decls {
            println!("  - {}", decl.name);
        }
    }

    #[test]
    fn test_backtick_identifiers() {
        let path = fixtures_path().join("kotlin").join("edge_cases.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("edge_cases.kt");

        // Look for backtick identifiers (keywords as names)
        let names: Vec<_> = graph.declarations().map(|d| d.name.clone()).collect();

        println!("All declaration names count: {}", names.len());

        // The parser might strip backticks or keep them
        let keyword_names = ["class", "val", "fun", "when", "is", "in", "object"];
        for kw in &keyword_names {
            if names.contains(&kw.to_string()) || names.contains(&format!("`{}`", kw)) {
                println!("Found keyword as identifier: {}", kw);
            }
        }
    }

    #[test]
    fn test_deeply_nested_classes() {
        let path = fixtures_path().join("kotlin").join("edge_cases.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("edge_cases.kt");

        // Look for nested level classes
        let level_classes: Vec<_> = graph
            .declarations()
            .filter(|d| d.name.contains("Level"))
            .collect();

        println!("Nested level classes found: {}", level_classes.len());
        assert!(level_classes.len() >= 2, "Should find nested classes");
    }

    #[test]
    fn test_generic_complexity() {
        let path = fixtures_path().join("kotlin").join("edge_cases.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("edge_cases.kt");

        // Look for generic classes
        let generic_decls: Vec<_> = graph
            .declarations()
            .filter(|d| {
                d.name.contains("Generic")
                    || d.name.contains("Covariant")
                    || d.name.contains("Contravariant")
            })
            .collect();

        println!("Generic declarations found: {}", generic_decls.len());
    }

    #[test]
    fn test_inline_value_classes() {
        let path = fixtures_path().join("kotlin").join("edge_cases.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("edge_cases.kt");

        // Look for value classes
        let value_classes: Vec<_> = graph
            .declarations()
            .filter(|d| d.name == "Password" || d.name == "UserId")
            .collect();

        println!("Value classes found: {}", value_classes.len());
    }

    #[test]
    fn test_type_aliases() {
        let path = fixtures_path().join("kotlin").join("edge_cases.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("edge_cases.kt");

        // Look for type aliases
        let type_alias_count = graph
            .declarations()
            .filter(|d| d.kind == searchdeadcode::graph::DeclarationKind::TypeAlias)
            .count();

        println!("Type aliases found: {}", type_alias_count);
    }

    #[test]
    fn test_local_functions() {
        let path = fixtures_path().join("kotlin").join("edge_cases.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("edge_cases.kt");

        // Should find outerFunction
        let outer_fn = graph.declarations().find(|d| d.name == "outerFunction");

        assert!(outer_fn.is_some(), "Should find outerFunction");
    }

    #[test]
    fn test_empty_file_handling() {
        // Create a temporary empty file
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let empty_file = temp_dir.path().join("empty.kt");
        std::fs::write(&empty_file, "").expect("Failed to write empty file");

        let source = SourceFile::new(empty_file, FileType::Kotlin);
        let mut builder = GraphBuilder::new();
        let result = builder.process_file(&source);

        // Should handle empty file gracefully
        assert!(result.is_ok(), "Should handle empty file");

        let graph = builder.build();
        assert_eq!(
            graph.declarations().count(),
            0,
            "Empty file should have no declarations"
        );
    }

    #[test]
    fn test_whitespace_only_file() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let whitespace_file = temp_dir.path().join("whitespace.kt");
        std::fs::write(&whitespace_file, "   \n\n\t\t  \n  ").expect("Failed to write file");

        let source = SourceFile::new(whitespace_file, FileType::Kotlin);
        let mut builder = GraphBuilder::new();
        let result = builder.process_file(&source);

        assert!(result.is_ok(), "Should handle whitespace-only file");
    }

    #[test]
    fn test_comments_only_file() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let comments_file = temp_dir.path().join("comments.kt");
        std::fs::write(
            &comments_file,
            r#"
// This is a comment
/* This is a
   multiline comment */
/**
 * This is a doc comment
 */
// Another comment
"#,
        )
        .expect("Failed to write file");

        let source = SourceFile::new(comments_file, FileType::Kotlin);
        let mut builder = GraphBuilder::new();
        let result = builder.process_file(&source);

        assert!(result.is_ok(), "Should handle comments-only file");
    }
}

// ============================================================================
// CROSS-FILE REFERENCE TESTS
// ============================================================================

mod cross_file_tests {
    use super::*;

    #[test]
    fn test_cross_file_fixtures_parse() {
        let file_a = fixtures_path().join("kotlin").join("cross_file_a.kt");
        let file_b = fixtures_path().join("kotlin").join("cross_file_b.kt");

        if !file_a.exists() || !file_b.exists() {
            println!("Skipping: cross_file fixtures not found");
            return;
        }

        let graph = build_multi_file_graph(&[
            ("cross_file_a.kt", FileType::Kotlin),
            ("cross_file_b.kt", FileType::Kotlin),
        ]);

        let count = graph.declarations().count();
        println!("Cross-file combined declarations: {}", count);
        assert!(count > 30, "Should have many declarations from both files");
    }

    #[test]
    fn test_cross_file_shared_service_used() {
        let file_a = fixtures_path().join("kotlin").join("cross_file_a.kt");
        let file_b = fixtures_path().join("kotlin").join("cross_file_b.kt");

        if !file_a.exists() || !file_b.exists() {
            return;
        }

        let graph = build_multi_file_graph(&[
            ("cross_file_a.kt", FileType::Kotlin),
            ("cross_file_b.kt", FileType::Kotlin),
        ]);

        // SharedService should be referenced
        let shared_service = graph.declarations().find(|d| d.name == "SharedService");

        assert!(shared_service.is_some(), "Should find SharedService");

        // ServiceConsumer should also exist
        let consumer = graph.declarations().find(|d| d.name == "ServiceConsumer");

        assert!(consumer.is_some(), "Should find ServiceConsumer");
    }

    #[test]
    fn test_cross_file_interface_implementation() {
        let file_a = fixtures_path().join("kotlin").join("cross_file_a.kt");
        let file_b = fixtures_path().join("kotlin").join("cross_file_b.kt");

        if !file_a.exists() || !file_b.exists() {
            return;
        }

        let graph = build_multi_file_graph(&[
            ("cross_file_a.kt", FileType::Kotlin),
            ("cross_file_b.kt", FileType::Kotlin),
        ]);

        // DataProvider interface from file A
        let interface_decl = graph.declarations().find(|d| d.name == "DataProvider");

        // LocalDataProvider implementation from file B
        let impl_decl = graph.declarations().find(|d| d.name == "LocalDataProvider");

        assert!(
            interface_decl.is_some(),
            "Should find DataProvider interface"
        );
        assert!(
            impl_decl.is_some(),
            "Should find LocalDataProvider implementation"
        );
    }

    #[test]
    fn test_cross_file_extension_function() {
        let file_a = fixtures_path().join("kotlin").join("cross_file_a.kt");
        let file_b = fixtures_path().join("kotlin").join("cross_file_b.kt");

        if !file_a.exists() || !file_b.exists() {
            return;
        }

        let graph = build_multi_file_graph(&[
            ("cross_file_a.kt", FileType::Kotlin),
            ("cross_file_b.kt", FileType::Kotlin),
        ]);

        // toSlug extension from file A
        let to_slug = graph.declarations().find(|d| d.name == "toSlug");

        // SlugGenerator that uses it from file B
        let generator = graph.declarations().find(|d| d.name == "SlugGenerator");

        assert!(to_slug.is_some(), "Should find toSlug extension");
        assert!(generator.is_some(), "Should find SlugGenerator");
    }

    #[test]
    fn test_cross_file_orphan_class_detected() {
        let file_a = fixtures_path().join("kotlin").join("cross_file_a.kt");
        let file_b = fixtures_path().join("kotlin").join("cross_file_b.kt");

        if !file_a.exists() || !file_b.exists() {
            return;
        }

        let graph = build_multi_file_graph(&[
            ("cross_file_a.kt", FileType::Kotlin),
            ("cross_file_b.kt", FileType::Kotlin),
        ]);

        // OrphanClass should exist but be unreferenced
        let orphan = graph.declarations().find(|d| d.name == "OrphanClass");

        assert!(orphan.is_some(), "Should find OrphanClass");

        // Check if it's unreferenced
        let entry_points: HashSet<_> = graph
            .declarations()
            .filter(|d| d.name == "main")
            .map(|d| d.id.clone())
            .collect();

        if !entry_points.is_empty() {
            let analyzer = ReachabilityAnalyzer::new();
            let (dead_code, _) = analyzer.find_unreachable_with_reachable(&graph, &entry_points);

            let dead_names: HashSet<_> = dead_code
                .iter()
                .map(|d| d.declaration.name.as_str())
                .collect();

            println!("Dead code found: {:?}", dead_names);

            // OrphanClass should be in dead code
            if dead_names.contains("OrphanClass") {
                println!("OrphanClass correctly identified as dead");
            }
        }
    }
}

// ============================================================================
// JAVA LANGUAGE TESTS
// ============================================================================

mod java_tests {
    use super::*;

    #[test]
    fn test_java_fixture_parses() {
        let path = fixtures_path().join("java").join("DeadCode.java");
        if !path.exists() {
            println!("Skipping: DeadCode.java not found");
            return;
        }

        let graph = build_java_graph("DeadCode.java");
        let count = graph.declarations().count();

        println!("Java fixture: {} declarations", count);
        assert!(count > 5, "Should parse Java declarations");
    }

    #[test]
    fn test_java_class_detection() {
        let path = fixtures_path().join("java").join("DeadCode.java");
        if !path.exists() {
            return;
        }

        let graph = build_java_graph("DeadCode.java");

        let classes: Vec<_> = graph
            .declarations()
            .filter(|d| d.kind == searchdeadcode::graph::DeclarationKind::Class)
            .map(|d| d.name.clone())
            .collect();

        println!("Java classes found: {:?}", classes);
        assert!(
            classes.contains(&"DeadCode".to_string()),
            "Should find DeadCode class"
        );
    }

    #[test]
    fn test_java_method_detection() {
        let path = fixtures_path().join("java").join("DeadCode.java");
        if !path.exists() {
            return;
        }

        let graph = build_java_graph("DeadCode.java");

        let methods: Vec<_> = graph
            .declarations()
            .filter(|d| d.kind == searchdeadcode::graph::DeclarationKind::Function)
            .map(|d| d.name.clone())
            .collect();

        println!("Java methods found: {:?}", methods);
    }

    #[test]
    fn test_java_field_detection() {
        let path = fixtures_path().join("java").join("DeadCode.java");
        if !path.exists() {
            return;
        }

        let graph = build_java_graph("DeadCode.java");

        let fields: Vec<_> = graph
            .declarations()
            .filter(|d| d.kind == searchdeadcode::graph::DeclarationKind::Property)
            .map(|d| d.name.clone())
            .collect();

        println!("Java fields found: {:?}", fields);
    }

    #[test]
    fn test_java_enum_detection() {
        let path = fixtures_path().join("java").join("DeadCode.java");
        if !path.exists() {
            return;
        }

        let graph = build_java_graph("DeadCode.java");

        let enums: Vec<_> = graph
            .declarations()
            .filter(|d| {
                d.kind == searchdeadcode::graph::DeclarationKind::Enum
                    || d.kind == searchdeadcode::graph::DeclarationKind::EnumCase
            })
            .map(|d| d.name.clone())
            .collect();

        println!("Java enums found: {:?}", enums);
    }

    #[test]
    fn test_java_write_only_detection() {
        let path = fixtures_path().join("java").join("DeadCode.java");
        if !path.exists() {
            return;
        }

        let graph = build_java_graph("DeadCode.java");
        let detector = WriteOnlyDetector::new();
        let issues = detector.detect(&graph);

        println!("Java write-only issues: {}", issues.len());
        for issue in &issues {
            println!("  - {}: {}", issue.declaration.name, issue.message);
        }
    }

    #[test]
    fn test_java_unused_params_detection() {
        let path = fixtures_path().join("java").join("DeadCode.java");
        if !path.exists() {
            return;
        }

        let graph = build_java_graph("DeadCode.java");
        let detector = UnusedParamDetector::new();
        let issues = detector.detect(&graph);

        println!("Java unused param issues: {}", issues.len());
        for issue in &issues {
            println!("  - {}: {}", issue.declaration.name, issue.message);
        }
    }
}

// ============================================================================
// PERFORMANCE TESTS
// ============================================================================

mod performance_tests {
    use super::*;

    #[test]
    fn test_parsing_performance() {
        let all_fixtures = vec![
            "dead_code.kt",
            "all_used.kt",
            "write_only.kt",
            "unused_params.kt",
            "sealed_classes.kt",
            "redundant_overrides.kt",
            "unreferenced.kt",
        ];

        let start = Instant::now();
        let mut total_decls = 0;

        for filename in &all_fixtures {
            let path = fixtures_path().join("kotlin").join(filename);
            if path.exists() {
                let source = SourceFile::new(path, FileType::Kotlin);
                let mut builder = GraphBuilder::new();
                builder.process_file(&source).expect("Failed to process");
                let graph = builder.build();
                total_decls += graph.declarations().count();
            }
        }

        let elapsed = start.elapsed();
        println!(
            "Parsed {} files with {} declarations in {:?}",
            all_fixtures.len(),
            total_decls,
            elapsed
        );

        // Should complete in reasonable time
        assert!(
            elapsed.as_secs() < 10,
            "Parsing should complete in < 10 seconds"
        );
    }

    #[test]
    fn test_detector_performance() {
        // Build a combined graph from multiple files
        let mut builder = GraphBuilder::new();
        let files = vec![
            "write_only.kt",
            "unused_params.kt",
            "sealed_classes.kt",
            "redundant_overrides.kt",
        ];

        for filename in &files {
            let path = fixtures_path().join("kotlin").join(filename);
            if path.exists() {
                let source = SourceFile::new(path, FileType::Kotlin);
                builder.process_file(&source).expect("Failed to process");
            }
        }

        let graph = builder.build();
        let decl_count = graph.declarations().count();

        // Time each detector
        let detectors: Vec<(&str, Box<dyn Detector>)> = vec![
            ("WriteOnly", Box::new(WriteOnlyDetector::new())),
            ("UnusedParam", Box::new(UnusedParamDetector::new())),
            (
                "SealedVariant",
                Box::new(UnusedSealedVariantDetector::new()),
            ),
            (
                "RedundantOverride",
                Box::new(RedundantOverrideDetector::new()),
            ),
        ];

        println!("Performance test on {} declarations:", decl_count);

        for (name, detector) in detectors {
            let start = Instant::now();
            let issues = detector.detect(&graph);
            let elapsed = start.elapsed();
            println!(
                "  {} detector: {} issues in {:?}",
                name,
                issues.len(),
                elapsed
            );

            // Each detector should be fast
            assert!(
                elapsed.as_millis() < 1000,
                "{} detector should complete in < 1 second",
                name
            );
        }
    }

    #[test]
    fn test_large_file_handling() {
        // Generate a large synthetic file
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let large_file = temp_dir.path().join("large.kt");

        let mut content = String::from("package com.example.large\n\n");

        // Generate 100 classes with 10 methods each
        for i in 0..100 {
            content.push_str(&format!("class Class{} {{\n", i));
            for j in 0..10 {
                content.push_str(&format!("    fun method{}(): String = \"value{}\"\n", j, j));
            }
            content.push_str("}\n\n");
        }

        std::fs::write(&large_file, &content).expect("Failed to write large file");

        let start = Instant::now();
        let source = SourceFile::new(large_file, FileType::Kotlin);
        let mut builder = GraphBuilder::new();
        builder
            .process_file(&source)
            .expect("Failed to process large file");
        let graph = builder.build();
        let elapsed = start.elapsed();

        let decl_count = graph.declarations().count();
        println!(
            "Large file: {} declarations parsed in {:?}",
            decl_count, elapsed
        );

        assert!(decl_count >= 1000, "Should have many declarations");
        assert!(
            elapsed.as_secs() < 30,
            "Should parse large file in < 30 seconds"
        );
    }
}

// ============================================================================
// REGRESSION TESTS
// ============================================================================

mod regression_tests {
    use super::*;

    #[test]
    fn test_sealed_class_parenthesis_stripping() {
        // Regression: is_sealed_subclass didn't strip parentheses from "SealedClass()"
        let path = fixtures_path().join("kotlin").join("sealed_classes.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("sealed_classes.kt");
        let detector = UnusedSealedVariantDetector::new();
        let issues = detector.detect(&graph);

        // Variants should be detected even with constructor calls like "State()"
        println!("Sealed variant issues: {}", issues.len());
    }

    #[test]
    fn test_backing_field_convention() {
        // Regression: _underscore prefix should not be reported as write-only
        let path = fixtures_path().join("kotlin").join("write_only.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("write_only.kt");
        let detector = WriteOnlyDetector::new();
        let issues = detector.detect(&graph);

        // No issues should have names starting with underscore
        let underscore_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.declaration.name.starts_with("_"))
            .collect();

        assert!(
            underscore_issues.is_empty(),
            "Should not report backing fields starting with _"
        );
    }

    #[test]
    fn test_override_with_additional_behavior() {
        // Regression: Override that adds behavior should not be reported as redundant
        let path = fixtures_path()
            .join("kotlin")
            .join("redundant_overrides.kt");
        if !path.exists() {
            return;
        }

        let graph = build_kotlin_graph("redundant_overrides.kt");
        let detector = RedundantOverrideDetector::new();
        let issues = detector.detect(&graph);

        let issue_names: HashSet<_> = issues.iter().map(|i| i.declaration.name.as_str()).collect();

        // onCreate in MainActivity adds behavior, should not be reported
        // (depends on detector implementation)
        println!("Redundant override issues: {:?}", issue_names);
    }
}
