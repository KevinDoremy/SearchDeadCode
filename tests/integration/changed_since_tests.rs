//! Integration tests for --changed-since: PR-scoped analysis.
//!
//! A partial subgraph cannot prove global deadness — except when a changed
//! symbol's name appears nowhere else in the project. Only those stable
//! verdicts are emitted; everything else stays silent.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?} failed");
}

/// Base commit: Main + UsedTool. Second commit adds the changed files.
fn write_repo(dir: &Path, changed_files: &[(&str, &str)]) -> String {
    git(dir, &["init", "-q"]);
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    UsedTool().work()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("UsedTool.kt"),
        "package sample\n\nclass UsedTool {\n    fun work() {}\n}\n",
    )
    .unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-qm", "base"]);
    let base = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    for (name, content) in changed_files {
        fs::write(dir.join(name), content).unwrap();
    }
    git(dir, &["add", "."]);
    git(dir, &["commit", "-qm", "change"]);
    base
}

fn run(dir: &Path, base: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--changed-since", base])
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_new_unmentioned_symbol_is_a_stable_finding() {
    let temp = tempfile::tempdir().unwrap();
    let base = write_repo(
        temp.path(),
        &[(
            "DeadNew.kt",
            "package sample\n\nclass DeadNew {\n    fun rot() {}\n}\n",
        )],
    );

    let output = run(temp.path(), &base);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("DeadNew"),
        "a name absent from the whole project is stably dead, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("UsedTool"),
        "unchanged files are not judged, stdout was:\n{stdout}"
    );
}

#[test]
fn a_mention_anywhere_silences_the_verdict() {
    let temp = tempfile::tempdir().unwrap();
    let base = write_repo(
        temp.path(),
        &[
            (
                "MaybeDead.kt",
                "package sample\n\nclass MaybeDead {\n    fun rot() {}\n}\n",
            ),
            (
                "Notes.kt",
                "package sample\n\n// TODO: wire MaybeDead into the flow\nclass Notes\n",
            ),
        ],
    );

    let output = run(temp.path(), &base);

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("MaybeDead"),
        "any mention, even a comment, keeps PR scope silent, stdout was:\n{stdout}"
    );
}

#[test]
fn an_android_component_in_the_diff_is_not_reported() {
    let temp = tempfile::tempdir().unwrap();
    let base = write_repo(
        temp.path(),
        &[(
            "NewScreenActivity.kt",
            "package sample\n\nclass NewScreenActivity : Activity() {\n    fun show() {}\n}\n",
        )],
    );

    let output = run(temp.path(), &base);

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("NewScreenActivity"),
        "framework entry points are roots even in PR scope, stdout was:\n{stdout}"
    );
}

#[test]
fn outside_a_git_repo_the_error_is_helpful() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--changed-since", "HEAD~1"])
        .output()
        .unwrap();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("git"),
        "the failure names git as the missing piece, output was:\n{combined}"
    );
}
