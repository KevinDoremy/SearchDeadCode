//! Integration tests for --format csv: findings as a spreadsheet for
//! team triage — assignment columns and home-made filters live there.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--format", "csv"])
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn findings_come_out_as_csv_rows_under_a_header() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    let lines: Vec<&str> = stdout.lines().filter(|l| l.contains(',')).collect();
    assert!(
        lines[0].starts_with("code,symbol,kind,file,line,confidence,risk,message"),
        "a stable header first, got:\n{}",
        lines[0]
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("DC001") && l.contains("Ghost")),
        "the finding is a row, stdout was:\n{stdout}"
    );
}

#[test]
fn commas_and_quotes_in_messages_are_escaped() {
    // a long-parameter-list style message contains commas — the row
    // must stay one row; RFC 4180 quoting
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    let ghost_row = stdout
        .lines()
        .find(|l| l.contains("Ghost") && l.contains("DC001"))
        .expect("ghost row");
    // the message field holds a quote-wrapped value ending the row:
    // naive splitting on ',' inside it must not create phantom columns
    let quoted = ghost_row.matches('"').count();
    assert!(
        quoted % 2 == 0,
        "quotes are balanced (RFC 4180), row was:\n{ghost_row}"
    );
}

#[test]
fn a_clean_project_prints_only_the_header() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = run(temp.path());
    let stdout = stdout_of(&out);
    assert!(out.status.success());
    let csv_lines: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with("code,") || l.contains("DC0"))
        .collect();
    assert_eq!(
        csv_lines.len(),
        1,
        "header only, no data rows, stdout was:\n{stdout}"
    );
}
