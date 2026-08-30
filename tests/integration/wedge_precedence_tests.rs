//! Integration tests for wedge precedence: ~40 flags short-circuit the
//! report, and combining two must be deterministic — first wedge in
//! dispatch order wins, the later one is ignored and a stderr warning
//! names the winner (stdout stays clean for scripts). These pin
//! representative pairs so a dispatch reorder cannot change behavior
//! unnoticed.

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

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn two_wedges_warn_on_stderr_and_name_the_winner() {
    let temp = tempfile::tempdir().unwrap();
    sample(temp.path());

    let output = bin(temp.path(), &["--twins", "--dead-modules"]);
    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("--twins") && stderr.contains("--dead-modules"),
        "the warning names both flags, stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("--dead-modules wins"),
        "the winner is the first in dispatch order, not in argv order, stderr was:\n{stderr}"
    );
}

#[test]
fn a_single_wedge_stays_quiet() {
    let temp = tempfile::tempdir().unwrap();
    sample(temp.path());

    let stderr = stderr_of(&bin(temp.path(), &["--twins"]));
    assert!(
        !stderr.contains("wins"),
        "one wedge is the normal case, no warning, stderr was:\n{stderr}"
    );
}

#[test]
fn quiet_suppresses_the_wedge_warning() {
    let temp = tempfile::tempdir().unwrap();
    sample(temp.path());

    let stderr = stderr_of(&bin(temp.path(), &["--twins", "--dead-modules", "--quiet"]));
    assert!(
        !stderr.contains("wins"),
        "--quiet silences the warning, stderr was:\n{stderr}"
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
