//! Integration tests for --quick-wins: the findings you can delete blind.
//!
//! A quick win is a finding whose entire connected cluster is dead AND
//! whose every clustered finding carries low risk. One risky member
//! poisons its whole cluster: deleting the root would drag the risky
//! member down with it.

use std::fs;
use std::path::Path;
use std::process::Output;

/// Live: Main -> SharedFormatter. Safe dead cluster: OldScreen -> OldPrinter.
fn write_safe_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    SharedFormatter().format()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("SharedFormatter.kt"),
        "package sample\n\nclass SharedFormatter {\n    fun format() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("OldScreen.kt"),
        "package sample\n\nclass OldScreen {\n    fun show() {\n        OldPrinter().dump()\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("OldPrinter.kt"),
        "package sample\n\nclass OldPrinter {\n    fun dump() {}\n}\n",
    )
    .unwrap();
}

fn run_quick_wins(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--quick-wins")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn quick_wins_lists_safe_closed_clusters() {
    let temp = tempfile::tempdir().unwrap();
    write_safe_project(temp.path());

    let output = run_quick_wins(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("quick win"),
        "the view names itself, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("OldScreen"),
        "the safe dead cluster is listed, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("SharedFormatter"),
        "live code stays out, stdout was:\n{stdout}"
    );
}

#[test]
fn a_risky_member_poisons_its_whole_cluster() {
    let temp = tempfile::tempdir().unwrap();
    write_safe_project(temp.path());
    // RiskyGadget is dead but its name lives in a string literal (reflection
    // risk); ToxicRoot's only dependent is RiskyGadget.
    fs::write(
        temp.path().join("ToxicRoot.kt"),
        "package sample\n\nclass ToxicRoot {\n    fun boot() {\n        RiskyGadget().arm()\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("RiskyGadget.kt"),
        "package sample\n\nclass RiskyGadget {\n    fun arm() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Config.kt"),
        "package sample\n\nclass Config {\n    val target = \"RiskyGadget\"\n}\n",
    )
    .unwrap();

    let output = run_quick_wins(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("OldScreen"),
        "the safe cluster is still a quick win, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("RiskyGadget") && !stdout.contains("ToxicRoot"),
        "one risky member disqualifies the whole cluster, stdout was:\n{stdout}"
    );
}

#[test]
fn no_quick_wins_says_so_plainly() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    SharedFormatter().format()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("SharedFormatter.kt"),
        "package sample\n\nclass SharedFormatter {\n    fun format() {}\n}\n",
    )
    .unwrap();

    let output = run_quick_wins(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("No quick wins"),
        "the empty case is stated plainly, stdout was:\n{stdout}"
    );
}
