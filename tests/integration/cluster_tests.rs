//! Integration tests for --clusters: dead code grouped into deletable clusters.

use std::fs;
use std::path::Path;
use std::process::Output;

/// Live: Main -> SharedFormatter. Dead cluster A: OldScreen -> OldPrinter.
/// Dead cluster B: TinyOrphan alone.
fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    SharedFormatter().format()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("SharedFormatter.kt"),
        "package sample\n\nclass SharedFormatter {\n    fun format() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("OldScreen.kt"),
        "package sample\n\nclass OldScreen {\n    fun show() {\n        OldPrinter().dump()\n    }\n\n    fun hide() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("OldPrinter.kt"),
        "package sample\n\nclass OldPrinter {\n    fun dump() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("TinyOrphan.kt"),
        "package sample\n\nclass TinyOrphan\n",
    )
    .unwrap();
}

fn run_clusters(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--clusters")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn clusters_view_groups_connected_dead_code() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_clusters(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("Cluster"),
        "--clusters shows a cluster view, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("OldScreen") && stdout.contains("OldPrinter"),
        "connected dead classes belong to the same view, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("TinyOrphan"),
        "isolated dead code forms its own cluster, stdout was:\n{stdout}"
    );
}

#[test]
fn clusters_are_sorted_by_size_descending() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_clusters(temp.path());

    let stdout = stdout_of(&output);
    let big = stdout.find("OldScreen").expect("OldScreen in output");
    let small = stdout.find("TinyOrphan").expect("TinyOrphan in output");
    assert!(
        big < small,
        "the bigger cluster comes first, stdout was:\n{stdout}"
    );
}

#[test]
fn clusters_report_line_estimates() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_clusters(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("lines"),
        "each cluster reports an estimated size, stdout was:\n{stdout}"
    );
}
