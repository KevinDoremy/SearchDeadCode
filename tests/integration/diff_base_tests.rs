//! Integration tests for --diff-base <ref>: report only symbols that
//! became dead since a git reference — the CI case par excellence.
//! The reference state is analyzed in a temporary worktree and compared
//! by fingerprint, so pre-existing corpses stay out of the way.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--quiet",
            "-m",
            message,
        ],
    );
}

fn seeded_repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("OldZombie.kt"),
        "package sample\n\nclass OldZombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();
    git(temp.path(), &["init", "--quiet"]);
    commit_all(temp.path(), "seed with one old corpse");
    temp
}

fn run(dir: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn only_the_newly_dead_symbol_is_reported() {
    let temp = seeded_repo();
    fs::write(
        temp.path().join("FreshZombie.kt"),
        "package sample\n\nclass FreshZombie {\n    fun wail() {}\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--diff-base", "HEAD"]));
    assert!(
        stdout.contains("FreshZombie"),
        "the corpse born after HEAD is the news, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("OldZombie"),
        "the pre-existing corpse is not news, stdout was:\n{stdout}"
    );
}

#[test]
fn no_new_corpses_is_a_clean_pass() {
    let temp = seeded_repo();

    let output = run(temp.path(), &["--diff-base", "HEAD"]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "nothing new since HEAD, output was:\n{output:?}"
    );
    assert!(
        !stdout.contains("OldZombie"),
        "unchanged corpses stay silent, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unknown_ref_fails_loudly() {
    let temp = seeded_repo();

    let output = run(temp.path(), &["--diff-base", "no-such-branch"]);
    assert!(
        !output.status.success(),
        "an unresolvable reference cannot be compared against, output was:\n{output:?}"
    );
}

#[test]
fn outside_a_git_repo_the_flag_fails_loudly() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path(), &["--diff-base", "HEAD"]);
    assert!(
        !output.status.success(),
        "no repo means no reference state, output was:\n{output:?}"
    );
}
