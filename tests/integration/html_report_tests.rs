//! Integration tests for --format html: one self-contained file with
//! sorting and filtering. A terminal caps out fast at 5000 findings;
//! a page you can filter does not.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Zombie.kt"),
        "package sample\n\nclass HtmlZombie {\n    fun groan() {}\n}\n",
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
        .args(["--format", "html"])
        .args(extra)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn the_page_contains_the_findings() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("<html") && stdout.contains("HtmlZombie") && stdout.contains("DC001"),
        "the corpse and its rule live in the page, stdout was:\n{stdout}"
    );
}

#[test]
fn the_page_is_self_contained() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("src=\"http") && !stdout.contains("href=\"http"),
        "no CDN, no external assets — the file must work offline, stdout was:\n{stdout}"
    );
}

#[test]
fn the_page_offers_a_filter() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("id=\"filter\""),
        "5000 findings need a filter box, stdout was:\n{stdout}"
    );
}

#[test]
fn output_flag_writes_the_file() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let out = temp.path().join("report.html");

    let output = run(temp.path(), &["--output", out.to_str().unwrap()]);
    assert!(output.status.success());
    let content = fs::read_to_string(&out).expect("report.html exists");
    assert!(
        content.contains("HtmlZombie"),
        "the written file carries the findings, content was:\n{content}"
    );
}
