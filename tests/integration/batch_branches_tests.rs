//! Integration tests for --batch-branches: one local git branch per
//! dead top-level class, each holding one commit whose message carries
//! the proof of death. CI validates each branch, a human merges —
//! nothing is pushed, nothing touches the working branch.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn bin(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
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
    git(dir, &["init", "--quiet", "-b", "work"]);
    git(dir, &["config", "core.autocrlf", "false"]);
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@e.c",
            "commit",
            "--quiet",
            "-m",
            "seed",
        ],
    );
}

fn write_project(dir: &Path) {
    fs::write(
        dir.join("DeadA.kt"),
        "package sample\n\nclass DeadA {\n    fun a() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("DeadB.kt"),
        "package sample\n\nclass DeadB {\n    fun b() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

#[test]
fn one_branch_per_dead_class_with_proof_in_the_message() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    init_repo(temp.path());

    let out = bin(temp.path(), &["--batch-branches"]);
    assert!(out.status.success(), "batch failed:\n{out:?}");

    let branches =
        String::from_utf8_lossy(&git(temp.path(), &["branch", "--list", "deadcode/*"]).stdout)
            .to_string();
    assert!(
        branches.contains("deada") && branches.contains("deadb"),
        "one branch per corpse, branches were:\n{branches}"
    );

    let log = String::from_utf8_lossy(
        &git(temp.path(), &["log", "-1", "--format=%B", "deadcode/deada"]).stdout,
    )
    .to_string();
    assert!(
        log.contains("DeadA") && log.to_lowercase().contains("reference"),
        "the commit message carries the proof, message was:\n{log}"
    );
}

#[test]
fn the_working_branch_stays_untouched() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    init_repo(temp.path());

    let out = bin(temp.path(), &["--batch-branches"]);
    assert!(out.status.success(), "batch failed:\n{out:?}");

    let head =
        String::from_utf8_lossy(&git(temp.path(), &["rev-parse", "--abbrev-ref", "HEAD"]).stdout)
            .trim()
            .to_string();
    assert_eq!(head, "work", "we come back to the starting branch");
    assert!(
        temp.path().join("DeadA.kt").exists(),
        "the deletions live on their branches, not here"
    );
    // the analysis cache is an expected untracked artifact — the
    // invariant is: no TRACKED modifications survive the run
    let status = String::from_utf8_lossy(
        &git(
            temp.path(),
            &["status", "--porcelain", "--untracked-files=no"],
        )
        .stdout,
    )
    .trim()
    .to_string();
    assert!(
        status.is_empty(),
        "clean tracked state after the run: {status}"
    );
}

#[test]
fn a_dirty_worktree_is_refused() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    init_repo(temp.path());
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--batch-branches"]);
    assert!(
        !out.status.success(),
        "uncommitted changes must block branch surgery, output was:\n{out:?}"
    );
}

#[test]
fn without_a_repo_its_a_clean_error() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let out = bin(temp.path(), &["--batch-branches"]);
    assert!(
        !out.status.success(),
        "no git, no branches, output was:\n{out:?}"
    );
}
