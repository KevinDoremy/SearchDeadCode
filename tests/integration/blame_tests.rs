//! Integration tests for --blame: last author and date per finding, so
//! cleanup can be routed to the right person. Opt-in — one git subprocess
//! per finding is not free. A missing git repo or an untracked file must
//! never break the run.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn write_dead_code(dir: &Path) {
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

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn init_repo_with_author(dir: &Path) {
    git(dir, &["init", "--quiet"]);
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            "user.name=Ghost Author",
            "-c",
            "user.email=ghost@example.com",
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

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn blame_names_the_last_author() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_code(temp.path());
    init_repo_with_author(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--blame"]));
    assert!(
        stdout.contains("Ghost Author"),
        "the finding should carry its last author, stdout was:\n{stdout}"
    );
}

#[test]
fn without_the_flag_no_author_appears() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_code(temp.path());
    init_repo_with_author(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("Ghost Author"),
        "blame is opt-in, stdout was:\n{stdout}"
    );
}

#[test]
fn a_recent_change_warns_about_resurrection() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_code(temp.path());
    init_repo_with_author(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--blame"]));
    assert!(
        stdout.contains("recently modified"),
        "dead but committed today = someone may be working on it, stdout was:\n{stdout}"
    );
}

#[test]
fn an_old_corpse_gets_no_resurrection_warning() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_code(temp.path());
    git(temp.path(), &["init", "--quiet"]);
    git(temp.path(), &["add", "."]);
    let status = Command::new("git")
        .arg("-C")
        .arg(temp.path())
        .env("GIT_AUTHOR_DATE", "2020-01-01T12:00:00")
        .env("GIT_COMMITTER_DATE", "2020-01-01T12:00:00")
        .args([
            "-c",
            "user.name=Ghost Author",
            "-c",
            "user.email=ghost@example.com",
            "commit",
            "--quiet",
            "-m",
            "ancient",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let stdout = stdout_of(&run(temp.path(), &["--blame"]));
    assert!(
        stdout.contains("2020-01-01") && !stdout.contains("recently modified"),
        "frozen since 2020 = serene deletion, stdout was:\n{stdout}"
    );
}

#[test]
fn a_project_without_git_survives_blame() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_code(temp.path());

    let output = run(temp.path(), &["--blame"]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success() && stdout.contains("Zombie"),
        "no repo means no author, never a crash, output was:\n{output:?}"
    );
}

#[test]
fn an_untracked_file_survives_blame() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_code(temp.path());
    init_repo_with_author(temp.path());
    fs::write(
        temp.path().join("Fresh.kt"),
        "package sample\n\nclass FreshZombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();

    let output = run(temp.path(), &["--blame"]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success() && stdout.contains("FreshZombie"),
        "the uncommitted finding still reports, just without an author, output was:\n{output:?}\n{stdout}"
    );
}
