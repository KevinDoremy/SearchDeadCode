//! Integration tests for --format markdown: findings as a paste-ready
//! table for PRs and tickets.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Zombie.kt"),
        "package sample\n\nclass MdZombie {\n    fun groan() {}\n}\n",
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
        .args(["--format", "markdown"])
        .args(extra)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn findings_render_as_a_markdown_table() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("| DC001 |") && stdout.contains("MdZombie"),
        "the corpse sits in a table row, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("| Code |") || stdout.contains("| code |"),
        "a header row exists, stdout was:\n{stdout}"
    );
}

#[test]
fn the_report_carries_a_title_and_count() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.starts_with("# ") || stdout.starts_with("## "),
        "a heading opens the report, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("1 finding"),
        "the count is stated, stdout was:\n{stdout}"
    );
}

#[test]
fn output_flag_writes_the_file() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let out = temp.path().join("report.md");

    let output = run(temp.path(), &["--output", out.to_str().unwrap()]);
    assert!(output.status.success());
    let content = fs::read_to_string(&out).expect("report.md exists");
    assert!(
        content.contains("MdZombie"),
        "the written file carries the findings, content was:\n{content}"
    );
}

#[test]
fn a_healthy_project_says_so_in_markdown() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path(), &[]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success() && stdout.to_lowercase().contains("no dead code"),
        "a clean verdict still reads well in a ticket, stdout was:\n{stdout}"
    );
}
