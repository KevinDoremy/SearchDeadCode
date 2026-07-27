//! Integration tests for the soft-reference safety net: a dead symbol
//! whose name appears in a string literal (Class.forName, JSON keys,
//! FQN navigation) is downgraded and labeled — the SCARF-style guard
//! that trims false positives across every detection at once.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_main(dir: &Path, extra_line: &str) {
    fs::write(
        dir.join("Main.kt"),
        format!("package sample\n\nfun main() {{\n    {extra_line}\n}}\n"),
    )
    .unwrap();
}

#[test]
fn a_name_inside_a_string_literal_is_flagged_as_soft_referenced() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Zombie.kt"),
        "package sample\n\nclass ReflectedZombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();
    write_main(
        temp.path(),
        "println(Class.forName(\"sample.ReflectedZombie\"))",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("string literal"),
        "the finding warns about its soft reference, stdout was:\n{stdout}"
    );
}

#[test]
fn a_short_name_inside_a_longer_word_is_not_a_soft_reference() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Log.kt"),
        "package sample\n\nclass Log {\n    fun write() {}\n}\n",
    )
    .unwrap();
    write_main(temp.path(), "println(\"Dialog opened for Catalogue\")");

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("Log"),
        "the corpse is still reported, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("string literal"),
        "'Log' inside 'Dialog'/'Catalogue' is not a reference, stdout was:\n{stdout}"
    );
}

#[test]
fn a_clean_corpse_carries_no_soft_reference_warning() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Zombie.kt"),
        "package sample\n\nclass PlainZombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();
    write_main(temp.path(), "println(\"nothing to see\")");

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("PlainZombie") && !stdout.contains("string literal"),
        "no literal names it, no warning, stdout was:\n{stdout}"
    );
}
