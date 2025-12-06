//! CLI integration tests
//!
//! These tests verify that the CLI works correctly with various options.

use std::path::PathBuf;
use std::process::Command;

/// Get the path to the test fixtures directory
fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Get the path to the searchdeadcode binary
fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("searchdeadcode")
}

/// Run searchdeadcode with arguments and return (stdout, stderr, success)
fn run_cli(args: &[&str]) -> (String, String, bool) {
    let binary = binary_path();
    if !binary.exists() {
        // Try release build
        let release_binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("release")
            .join("searchdeadcode");

        if !release_binary.exists() {
            panic!("Binary not found. Run 'cargo build' first.");
        }

        let output = Command::new(release_binary)
            .args(args)
            .output()
            .expect("Failed to execute command");

        return (
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
            output.status.success(),
        );
    }

    let output = Command::new(binary)
        .args(args)
        .output()
        .expect("Failed to execute command");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

// ============================================================================
// Basic CLI Tests
// ============================================================================

#[test]
fn test_cli_help() {
    let (stdout, _, success) = run_cli(&["--help"]);

    assert!(success, "Help should succeed");
    assert!(
        stdout.contains("searchdeadcode"),
        "Should show program name"
    );
    assert!(stdout.contains("--deep"), "Should show --deep option");
    assert!(
        stdout.contains("--parallel"),
        "Should show --parallel option"
    );
}

#[test]
fn test_cli_version() {
    let (stdout, _, success) = run_cli(&["--version"]);

    assert!(success, "Version should succeed");
    assert!(
        stdout.contains("searchdeadcode"),
        "Should show program name"
    );
}

#[test]
fn test_cli_analyze_fixtures() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        println!("Fixtures not found, skipping");
        return;
    }

    let (stdout, stderr, success) = run_cli(&[fixtures.to_str().unwrap()]);

    println!("stdout: {}", stdout);
    println!("stderr: {}", stderr);

    // Should complete (may have warnings)
    assert!(
        success || stderr.contains("Found"),
        "Should analyze fixtures"
    );
}

// ============================================================================
// Detection Flag Tests
// ============================================================================

#[test]
fn test_cli_write_only_flag() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--write-only"]);

    let combined = format!("{}{}", stdout, stderr);
    // Should run write-only detection
    println!("Write-only output: {}", combined);
}

#[test]
fn test_cli_sealed_variants_flag() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--sealed-variants"]);

    let combined = format!("{}{}", stdout, stderr);
    println!("Sealed variants output: {}", combined);
}

#[test]
fn test_cli_unused_params_flag() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--unused-params"]);

    let combined = format!("{}{}", stdout, stderr);
    println!("Unused params output: {}", combined);
}

#[test]
fn test_cli_redundant_overrides_flag() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--redundant-overrides"]);

    let combined = format!("{}{}", stdout, stderr);
    println!("Redundant overrides output: {}", combined);
}

#[test]
fn test_cli_deep_mode() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--deep"]);

    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Deep mode") || combined.contains("deep"),
        "Should run in deep mode"
    );
}

#[test]
fn test_cli_parallel_mode() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--parallel"]);

    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Parallel") || combined.contains("parallel"),
        "Should run in parallel mode"
    );
}

// ============================================================================
// Output Format Tests
// ============================================================================

#[test]
fn test_cli_json_output() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, success) = run_cli(&[
        fixtures.to_str().unwrap(),
        "--format",
        "json",
        "--quiet", // Suppress INFO logs
    ]);

    println!("JSON stdout: {}", &stdout[..stdout.len().min(200)]);
    println!("JSON stderr: {}", &stderr[..stderr.len().min(200)]);

    if success && !stdout.is_empty() {
        // Should be valid JSON (starts with { or [)
        let trimmed = stdout.trim();
        if !trimmed.is_empty() {
            assert!(
                trimmed.starts_with('{') || trimmed.starts_with('['),
                "JSON output should start with {{ or [, got: {}",
                &trimmed[..trimmed.len().min(100)]
            );
        }
    }
}

#[test]
fn test_cli_quiet_mode() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--quiet"]);

    // Quiet mode should have minimal output
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        !combined.contains("INFO") || combined.len() < 1000,
        "Quiet mode should reduce output"
    );
}

// ============================================================================
// Confidence Filter Tests
// ============================================================================

#[test]
fn test_cli_min_confidence_high() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--min-confidence", "high"]);

    let combined = format!("{}{}", stdout, stderr);
    // Should only show high confidence results
    println!("High confidence output: {}", combined);
}

#[test]
fn test_cli_min_confidence_low() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--min-confidence", "low"]);

    let combined = format!("{}{}", stdout, stderr);
    // Should show all results including low confidence
    println!("Low confidence output: {}", combined);
}

// ============================================================================
// Combined Options Tests
// ============================================================================

#[test]
fn test_cli_all_detectors() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[
        fixtures.to_str().unwrap(),
        "--deep",
        "--parallel",
        "--unused-params",
        "--write-only",
        "--sealed-variants",
        "--redundant-overrides",
    ]);

    let combined = format!("{}{}", stdout, stderr);
    println!("All detectors output length: {} chars", combined.len());

    // Should run all detectors
    assert!(!combined.is_empty(), "Should produce output");
}

#[test]
fn test_cli_detect_cycles() {
    let fixtures = fixtures_path().join("kotlin");
    if !fixtures.exists() {
        return;
    }

    let (stdout, stderr, _) = run_cli(&[fixtures.to_str().unwrap(), "--detect-cycles"]);

    let combined = format!("{}{}", stdout, stderr);
    println!("Cycle detection output: {}", combined);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_cli_nonexistent_path() {
    let (stdout, stderr, success) = run_cli(&["/nonexistent/path/to/analyze"]);

    // Should handle gracefully
    let combined = format!("{}{}", stdout, stderr);
    println!("Nonexistent path output: {}", combined);

    // May fail or warn, but shouldn't crash
    assert!(
        !success
            || combined.contains("No Kotlin")
            || combined.contains("not found")
            || combined.is_empty(),
        "Should handle nonexistent path gracefully"
    );
}

#[test]
fn test_cli_empty_directory() {
    use tempfile::tempdir;

    let temp = tempdir().expect("Failed to create temp dir");
    let empty_dir = temp.path();

    let (stdout, stderr, success) = run_cli(&[empty_dir.to_str().unwrap()]);

    let combined = format!("{}{}", stdout, stderr);
    println!("Empty directory output: {}", combined);

    // Should handle empty directory
    if success {
        assert!(
            combined.contains("No Kotlin") || combined.contains("0 files") || combined.is_empty()
        );
    }
}

#[test]
fn test_cli_single_file() {
    let fixture = fixtures_path().join("kotlin").join("dead_code.kt");
    if !fixture.exists() {
        return;
    }

    let (stdout, stderr, success) = run_cli(&[fixture.to_str().unwrap()]);

    let combined = format!("{}{}", stdout, stderr);
    println!("Single file output: {}", combined);

    assert!(success, "Should analyze single file successfully");
}
