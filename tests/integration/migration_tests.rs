//! Integration tests for --compare: v1/v2 migration diff.
//!
//! Given an old world and a new world, everything in the old world that no
//! outside code references anymore is deletable the day the migration flips;
//! old-world symbols still referenced from outside are the blockers.

use std::fs;
use std::path::Path;
use std::process::Output;

/// legacy/: OldHome (uses OldRenderer + SharedUtil), OldRenderer, OldBridge.
/// modern/: NewHome (uses SharedUtil). Main uses NewHome AND OldBridge.
fn write_sample_project(dir: &Path) {
    let legacy = dir.join("legacy");
    let modern = dir.join("modern");
    fs::create_dir_all(&legacy).unwrap();
    fs::create_dir_all(&modern).unwrap();

    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    NewHome().open()\n    OldBridge().carry()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("SharedUtil.kt"),
        "package sample\n\nclass SharedUtil {\n    fun help() {}\n}\n",
    )
    .unwrap();
    fs::write(
        legacy.join("OldHome.kt"),
        "package sample.legacy\n\nclass OldHome {\n    fun open() {\n        OldRenderer().draw()\n        SharedUtil().help()\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        legacy.join("OldRenderer.kt"),
        "package sample.legacy\n\nclass OldRenderer {\n    fun draw() {}\n}\n",
    )
    .unwrap();
    fs::write(
        legacy.join("OldBridge.kt"),
        "package sample.legacy\n\nclass OldBridge {\n    fun carry() {}\n}\n",
    )
    .unwrap();
    fs::write(
        modern.join("NewHome.kt"),
        "package sample.modern\n\nclass NewHome {\n    fun open() {\n        SharedUtil().help()\n    }\n}\n",
    )
    .unwrap();
}

fn run_compare(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--compare", "legacy=modern"])
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn compare_lists_old_world_symbols_deletable_at_flip() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_compare(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.to_lowercase().contains("deletable"),
        "the compare view has a deletable section, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("OldHome") && stdout.contains("OldRenderer"),
        "old-world symbols with no outside referencers are deletable, stdout was:\n{stdout}"
    );
}

#[test]
fn compare_flags_blockers_with_their_referencers() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_compare(temp.path());

    let stdout = stdout_of(&output);
    let bridge_line = stdout
        .lines()
        .find(|l| l.contains("OldBridge"))
        .expect("OldBridge appears in the compare view");
    assert!(
        bridge_line.contains("used by"),
        "a blocker shows who still references it, line was:\n{bridge_line}"
    );
    let home_line = stdout
        .lines()
        .find(|l| l.contains("OldHome"))
        .expect("OldHome appears in the compare view");
    assert!(
        !home_line.contains("used by"),
        "deletable symbols carry no referencer, line was:\n{home_line}"
    );
}

#[test]
fn compare_spares_the_shared_world() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_compare(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("SharedUtil"),
        "code outside the old world is not part of the diff, stdout was:\n{stdout}"
    );
}

#[test]
fn compare_reports_line_totals() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_compare(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("lines"),
        "the compare view reports estimated sizes, stdout was:\n{stdout}"
    );
}
