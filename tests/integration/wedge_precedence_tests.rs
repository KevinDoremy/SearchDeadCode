//! Integration tests for wedge precedence: ~40 flags short-circuit the
//! report, and combining two must be deterministic — first wedge in
//! dispatch order wins, silently ignoring the later one is the
//! documented contract. These pin representative pairs so a dispatch
//! reorder cannot change behavior unnoticed.

use std::fs;
use std::path::Path;
use std::process::Output;

fn bin(dir: &Path, args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn sample(dir: &Path) {
    fs::write(
        dir.join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

#[test]
fn explain_wins_over_health() {
    let temp = tempfile::tempdir().unwrap();
    sample(temp.path());

    let stdout = stdout_of(&bin(temp.path(), &["--explain", "Ghost", "--health"]));
    assert!(
        stdout.contains("Explain") || stdout.contains("Ghost"),
        "the symbol query answers, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("Module health"),
        "one wedge at a time — health is silently superseded, stdout was:\n{stdout}"
    );
}

#[test]
fn health_wins_over_pr_description() {
    let temp = tempfile::tempdir().unwrap();
    sample(temp.path());

    let stdout = stdout_of(&bin(temp.path(), &["--health", "--pr-description"]));
    assert!(
        stdout.contains("Module health"),
        "health dispatches first, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("Remove dead code"),
        "the PR body does not also print, stdout was:\n{stdout}"
    );
}

#[test]
fn the_precedence_rule_is_documented() {
    let doc = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/cli-reference.md"
    ))
    .unwrap();
    let lower = doc.to_lowercase();
    assert!(
        lower.contains("precedence"),
        "docs/cli-reference.md documents what happens when two report-replacing flags combine"
    );
}
