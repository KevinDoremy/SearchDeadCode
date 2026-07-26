//! Integration tests for --explain: why is a symbol considered dead (or alive)?

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    UsedHelper().greet()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("UsedHelper.kt"),
        "package sample\n\nclass UsedHelper {\n    fun greet() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ObsoleteWidget.kt"),
        "package sample\n\nclass ObsoleteWidget {\n    fun render() {}\n}\n",
    )
    .unwrap();
}

fn run_explain(dir: &Path, symbol: &str) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--explain", symbol])
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn explains_a_dead_symbol() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_explain(temp.path(), "ObsoleteWidget");

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("ObsoleteWidget"),
        "explain output names the symbol, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("Incoming references: 0"),
        "a dead symbol has zero incoming references, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("DEAD"),
        "the verdict for an unreferenced symbol is DEAD, stdout was:\n{stdout}"
    );
    assert!(
        stdout.to_lowercase().contains("manifest"),
        "the root sources that were checked are listed, stdout was:\n{stdout}"
    );
}

#[test]
fn explains_an_alive_symbol_with_its_referencers() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_explain(temp.path(), "UsedHelper");

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("ALIVE"),
        "the verdict for a referenced symbol is ALIVE, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("Main.kt"),
        "the referencing file is listed, stdout was:\n{stdout}"
    );
}

#[test]
fn explains_an_unknown_symbol() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_explain(temp.path(), "NoSuchThing");

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("not found"),
        "an unknown symbol yields a clear message, stdout was:\n{stdout}"
    );
}
