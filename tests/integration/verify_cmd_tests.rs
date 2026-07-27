//! Integration tests for --verify-cmd: after --delete, run a check
//! command (a compile, a test suite) and restore every touched file
//! automatically when it fails. The riskiest operation of the tool
//! gets a seatbelt.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Zombie.kt"),
        "package sample\n\nclass Zombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path, extra: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn a_passing_verification_keeps_the_deletion() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let before = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap();

    let output = run(
        temp.path(),
        &["--delete", "--yes", "--verify-cmd", "exit 0"],
    );
    assert!(output.status.success(), "output was:\n{output:?}");
    let after = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap_or_default();
    assert_ne!(
        before, after,
        "the corpse stays deleted when the check passes"
    );
}

#[test]
fn a_failing_verification_restores_every_file() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let before = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap();

    let output = run(
        temp.path(),
        &["--delete", "--yes", "--verify-cmd", "exit 1"],
    );
    assert!(
        !output.status.success(),
        "a broken build is a failed run, output was:\n{output:?}"
    );
    let after = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap();
    assert_eq!(
        before, after,
        "the deletion must be rolled back byte-for-byte"
    );
}

#[test]
fn verify_cmd_without_delete_is_an_error() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path(), &["--verify-cmd", "exit 0"]);
    assert!(
        !output.status.success(),
        "a check command with nothing to check is a config mistake, output was:\n{output:?}"
    );
}

#[test]
fn dry_run_never_runs_the_verification() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let before = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap();

    // 'exit 1' would fail the run if it were executed
    let output = run(
        temp.path(),
        &["--delete", "--yes", "--dry-run", "--verify-cmd", "exit 1"],
    );
    assert!(
        output.status.success(),
        "dry-run touches nothing so there is nothing to verify, output was:\n{output:?}"
    );
    let after = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap();
    assert_eq!(before, after, "dry-run leaves files intact");
}
