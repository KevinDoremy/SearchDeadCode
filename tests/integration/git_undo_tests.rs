//! Integration tests for git-aware undo: instead of relying on a bash
//! heredoc script (useless on Windows), a real deletion in a git repo
//! records the pre-delete state under refs/searchdeadcode/undo via
//! `git stash create` — restoration is one standard git command away.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

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

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

fn init_repo(dir: &Path) {
    git(dir, &["init", "--quiet"]);
    // Windows runners default to autocrlf=true: restoration would come
    // back CRLF and fail the byte-for-byte check for line-ending
    // reasons unrelated to the undo mechanism
    git(dir, &["config", "core.autocrlf", "false"]);
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=t@example.com",
            "commit",
            "--quiet",
            "-m",
            "seed",
        ],
    );
}

fn run(dir: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn a_real_deletion_records_a_git_undo_ref() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    init_repo(temp.path());

    let output = run(temp.path(), &["--delete", "--yes"]);
    assert!(output.status.success(), "output was:\n{output:?}");

    let ref_check = git(temp.path(), &["rev-parse", "refs/searchdeadcode/undo"]);
    assert!(
        ref_check.status.success(),
        "the undo ref exists after a real deletion"
    );
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        stdout.contains("refs/searchdeadcode/undo"),
        "the restore command is shown, stdout was:\n{stdout}"
    );
}

#[test]
fn the_git_ref_restores_the_corpse_byte_for_byte() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let before = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap();
    init_repo(temp.path());

    run(temp.path(), &["--delete", "--yes"]);
    let deleted = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap_or_default();
    assert_ne!(before, deleted, "the corpse is gone before restoring");

    let restored = git(
        temp.path(),
        &["restore", "--source", "refs/searchdeadcode/undo", "--", "."],
    );
    assert!(
        restored.status.success(),
        "git restore accepts the ref, stderr:\n{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    let after = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap();
    assert_eq!(before, after, "restoration is byte-for-byte");
}

#[test]
fn without_a_repo_the_deletion_still_works() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path(), &["--delete", "--yes"]);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "no git, no drama, output was:\n{output:?}"
    );
    assert!(
        !stdout.contains("refs/searchdeadcode/undo"),
        "no repo means no git undo to advertise, stdout was:\n{stdout}"
    );
}
