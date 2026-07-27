//! Integration tests for --top-files: files ranked by deletable lines,
//! not alphabetically — the Monday-morning "where do I start".

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    // A big corpse: many dead lines
    let mut big = String::from("package sample\n\nclass BigCorpse {\n");
    for i in 0..30 {
        big.push_str(&format!(
            "    fun rot{i}() {{\n        println({i})\n    }}\n"
        ));
    }
    big.push_str("}\n");
    fs::write(dir.join("BigCorpse.kt"), big).unwrap();

    fs::write(
        dir.join("SmallCorpse.kt"),
        "package sample\n\nclass SmallCorpse {\n    fun tinyRot() {}\n}\n",
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
        .args(extra)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn files_rank_by_deletable_lines_not_name() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--top-files", "10"]));
    let big_pos = stdout.find("BigCorpse.kt");
    let small_pos = stdout.find("SmallCorpse.kt");
    assert!(
        big_pos.is_some() && small_pos.is_some(),
        "both corpses appear, stdout was:\n{stdout}"
    );
    assert!(
        big_pos < small_pos,
        "BigCorpse outweighs SmallCorpse despite the alphabet, stdout was:\n{stdout}"
    );
}

#[test]
fn the_limit_caps_the_list() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--top-files", "1"]));
    assert!(
        stdout.contains("BigCorpse.kt") && !stdout.contains("SmallCorpse.kt"),
        "only the heaviest file with a limit of 1, stdout was:\n{stdout}"
    );
}

#[test]
fn a_healthy_file_never_ranks() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--top-files", "10"]));
    assert!(
        !stdout.contains("Main.kt"),
        "nothing deletable in Main.kt, stdout was:\n{stdout}"
    );
}

#[test]
fn a_zero_limit_still_shows_the_heaviest_file() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path(), &["--top-files", "0"]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success() && stdout.contains("BigCorpse.kt"),
        "zero clamps to one, never panics, stdout was:\n{stdout}"
    );
}

#[test]
fn a_healthy_project_says_so_without_crashing() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path(), &["--top-files", "10"]);
    assert!(
        output.status.success(),
        "no findings is not an error, output was:\n{output:?}"
    );
}
