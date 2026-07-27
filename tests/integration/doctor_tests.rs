//! Integration tests for --doctor: validate the config against the
//! repo's reality. A glob matching nothing or an entry point unknown
//! to the graph silently skews every run — the doctor says it loudly.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_code(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

fn run_doctor(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--doctor")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn an_exclusion_matching_nothing_is_called_out() {
    let temp = tempfile::tempdir().unwrap();
    write_code(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        "exclude:\n  - \"**/legacy_v1/**\"\n",
    )
    .unwrap();

    let output = run_doctor(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("legacy_v1"),
        "the dead glob is named, stdout was:\n{stdout}"
    );
    assert!(
        !output.status.success(),
        "a skewed config is a failed checkup, output was:\n{output:?}"
    );
}

#[test]
fn an_unknown_entry_point_is_called_out() {
    let temp = tempfile::tempdir().unwrap();
    write_code(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        "entry_points:\n  - \"GhostActivity\"\n",
    )
    .unwrap();

    let output = run_doctor(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("GhostActivity"),
        "the phantom entry point is named, stdout was:\n{stdout}"
    );
    assert!(!output.status.success());
}

#[test]
fn a_healthy_config_passes_the_checkup() {
    let temp = tempfile::tempdir().unwrap();
    write_code(temp.path());
    fs::create_dir_all(temp.path().join("legacy")).unwrap();
    fs::write(temp.path().join("legacy/Old.kt"), "package legacy\n").unwrap();
    fs::write(
        temp.path().join(".deadcode.yml"),
        "exclude:\n  - \"**/legacy/**\"\nentry_points:\n  - \"main\"\n",
    )
    .unwrap();

    let output = run_doctor(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "everything checks out, output was:\n{output:?}\n{stdout}"
    );
}

#[test]
fn no_config_is_a_gentle_pass() {
    let temp = tempfile::tempdir().unwrap();
    write_code(temp.path());

    let output = run_doctor(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "defaults in use is not an illness, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no .deadcode.yml") || stdout.contains("defaults"),
        "the doctor says defaults are in use, stdout was:\n{stdout}"
    );
}
