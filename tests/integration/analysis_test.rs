//! Integration tests for SearchDeadCode analysis
//!
//! These tests verify the complete analysis pipeline against test fixtures.

use searchdeadcode::analysis::detectors::{
    Detector, UnusedSealedVariantDetector, WriteOnlyDetector,
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

/// Build a graph from a single Kotlin file
fn build_graph_from_file(path: &PathBuf) -> searchdeadcode::graph::Graph {
    let file_type = if path.to_string_lossy().ends_with(".kt") {
        FileType::Kotlin
    } else if path.to_string_lossy().ends_with(".java") {
        FileType::Java
    } else {
        panic!("Unknown file type: {:?}", path);
    };

    let source_file = SourceFile::new(path.clone(), file_type);
    let mut builder = GraphBuilder::new();
    builder
        .process_file(&source_file)
        .expect("Failed to process file");
    builder.build()
}

#[test]
fn test_dead_code_kotlin_fixture() {
    let fixture = fixtures_path().join("kotlin/dead_code.kt");
    if !fixture.exists() {
        eprintln!("Fixture not found: {:?}", fixture);
        return;
    }

    let graph = build_graph_from_file(&fixture);

    // Verify we parsed declarations
    let decl_count = graph.declarations().count();
    assert!(decl_count > 0, "Should have parsed some declarations");

    // Check for expected declarations
    let names: Vec<_> = graph.declarations().map(|d| d.name.as_str()).collect();

    // Should find the classes
    assert!(names.contains(&"UnusedClass"), "Should find UnusedClass");
    assert!(
        names.contains(&"UsedClassWithDeadMethod"),
        "Should find UsedClassWithDeadMethod"
    );
    assert!(
        names.contains(&"WriteOnlyExample"),
        "Should find WriteOnlyExample"
    );
}

#[test]
fn test_all_used_kotlin_fixture() {
    let fixture = fixtures_path().join("kotlin/all_used.kt");
    if !fixture.exists() {
        eprintln!("Fixture not found: {:?}", fixture);
        return;
    }

    let graph = build_graph_from_file(&fixture);

    // Verify we parsed declarations
    let decl_count = graph.declarations().count();
    assert!(decl_count > 0, "Should have parsed some declarations");

    let names: Vec<_> = graph.declarations().map(|d| d.name.as_str()).collect();
    assert!(
        names.contains(&"DataProcessor"),
        "Should find DataProcessor"
    );
    assert!(names.contains(&"Calculator"), "Should find Calculator");
}

#[test]
fn test_java_fixture() {
    let fixture = fixtures_path().join("java/DeadCode.java");
    if !fixture.exists() {
        eprintln!("Fixture not found: {:?}", fixture);
        return;
    }

    let graph = build_graph_from_file(&fixture);
    let decl_count = graph.declarations().count();
    assert!(
        decl_count > 0,
        "Should have parsed some declarations from Java file"
    );
}

#[test]
fn test_write_only_detector_on_fixture() {
    let fixture = fixtures_path().join("kotlin/dead_code.kt");
    if !fixture.exists() {
        return;
    }

    let graph = build_graph_from_file(&fixture);
    let detector = WriteOnlyDetector::new();
    let issues = detector.detect(&graph);

    // The fixture has a write-only variable: writeOnlyCounter
    // Check that we can run the detector without errors
    println!("Write-only issues found: {}", issues.len());
    for issue in &issues {
        println!(
            "  - {} in {}",
            issue.declaration.name,
            issue.declaration.location.file.display()
        );
    }
}

#[test]
fn test_sealed_variant_detector_on_fixture() {
    let fixture = fixtures_path().join("kotlin/dead_code.kt");
    if !fixture.exists() {
        return;
    }

    let graph = build_graph_from_file(&fixture);
    let detector = UnusedSealedVariantDetector::new();
    let issues = detector.detect(&graph);

    // The fixture has a sealed class with an unused variant: Empty
    println!("Sealed variant issues found: {}", issues.len());
    for issue in &issues {
        println!(
            "  - {} in {}",
            issue.declaration.name,
            issue.declaration.location.file.display()
        );
    }
}

#[test]
fn test_reachability_analysis() {
    let fixture = fixtures_path().join("kotlin/dead_code.kt");
    if !fixture.exists() {
        return;
    }

    let graph = build_graph_from_file(&fixture);

    // Find entry points (main function)
    let entry_points: Vec<_> = graph
        .declarations()
        .filter(|d| d.name == "main")
        .map(|d| d.id.clone())
        .collect();

    if entry_points.is_empty() {
        println!("No main function found, skipping reachability test");
        return;
    }

    let analyzer = ReachabilityAnalyzer::new();
    let entry_set: HashSet<_> = entry_points.into_iter().collect();
    let (dead_code, reachable) = analyzer.find_unreachable_with_reachable(&graph, &entry_set);

    println!("Reachability analysis:");
    println!("  Entry points: {}", entry_set.len());
    println!("  Reachable: {}", reachable.len());
    println!("  Dead code candidates: {}", dead_code.len());

    // We should find some dead code
    assert!(
        !dead_code.is_empty(),
        "Should find some dead code in the fixture"
    );
}

#[test]
fn test_multiple_files() {
    let kotlin_fixture = fixtures_path().join("kotlin/dead_code.kt");
    let java_fixture = fixtures_path().join("java/DeadCode.java");

    if !kotlin_fixture.exists() || !java_fixture.exists() {
        return;
    }

    let kotlin_source = SourceFile::new(kotlin_fixture.clone(), FileType::Kotlin);
    let java_source = SourceFile::new(java_fixture.clone(), FileType::Java);

    let mut builder = GraphBuilder::new();
    builder
        .process_file(&kotlin_source)
        .expect("Failed to process Kotlin file");
    builder
        .process_file(&java_source)
        .expect("Failed to process Java file");
    let graph = builder.build();

    let decl_count = graph.declarations().count();
    println!("Total declarations from both files: {}", decl_count);

    // Should have declarations from both files
    let kotlin_decls = graph
        .declarations()
        .filter(|d| d.location.file.to_string_lossy().ends_with(".kt"))
        .count();
    let java_decls = graph
        .declarations()
        .filter(|d| d.location.file.to_string_lossy().ends_with(".java"))
        .count();

    assert!(kotlin_decls > 0, "Should have Kotlin declarations");
    assert!(java_decls > 0, "Should have Java declarations");
}
