//! Integration tests for --format reviewdog: Reviewdog Diagnostic
//! JSON Lines (rdjsonl) — one JSON object per finding, pluggable into
//! any CI review flow without a dedicated action.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--format", "reviewdog"])
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_project(dir: &Path) {
    fs::write(
        dir.join("DeadThing.kt"),
        "package sample\n\nclass DeadThing {\n    fun rot() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

#[test]
fn each_line_is_a_valid_rdjson_diagnostic() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let out = run(temp.path());
    let stdout = stdout_of(&out);
    let diagnostics: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|l| l.trim_start().starts_with('{'))
        .map(|l| serde_json::from_str(l).expect("each line parses as JSON"))
        .collect();
    assert!(
        !diagnostics.is_empty(),
        "at least the dead class is a diagnostic, stdout was:\n{stdout}"
    );
    let dead = diagnostics
        .iter()
        .find(|d| d["message"].as_str().unwrap_or("").contains("DeadThing"))
        .expect("DeadThing has a diagnostic");
    assert!(
        dead["location"]["range"]["start"]["line"].as_u64().unwrap() >= 1,
        "1-indexed line present:\n{dead}"
    );
    assert!(
        dead["code"]["value"].as_str().unwrap().starts_with("DC"),
        "the rule code rides along:\n{dead}"
    );
}

#[test]
fn paths_are_relative_to_the_repo() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    let line = stdout
        .lines()
        .find(|l| l.contains("DeadThing"))
        .expect("diagnostic exists");
    let json: serde_json::Value = serde_json::from_str(line).unwrap();
    let path = json["location"]["path"].as_str().unwrap();
    assert!(
        !path.starts_with('/'),
        "reviewdog wants repo-relative paths, got: {path}"
    );
}

#[test]
fn a_clean_project_emits_no_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = run(temp.path());
    let stdout = stdout_of(&out);
    assert!(out.status.success());
    assert!(
        !stdout.lines().any(|l| l.trim_start().starts_with('{')),
        "no findings, no diagnostic lines, stdout was:\n{stdout}"
    );
}
