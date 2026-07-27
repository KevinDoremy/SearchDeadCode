//! Integration tests for --pr-description: the cleanup PR body writes
//! itself — stats, proof of death per symbol, residual risks. The
//! natural complement of --batch-branches.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--pr-description")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn the_body_carries_stats_proof_and_symbols() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("GhostA.kt"),
        "package sample\n\nclass GhostA {\n    fun a() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("GhostB.kt"),
        "package sample\n\nclass GhostB {\n    fun b() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("GhostA") && stdout.contains("GhostB"),
        "every corpse is listed, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("0 incoming references") || stdout.contains("no incoming references"),
        "the proof of death is written out, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("## "),
        "it reads as a PR body (markdown sections), stdout was:\n{stdout}"
    );
}

#[test]
fn high_risk_findings_land_in_a_residual_risks_section() {
    let temp = tempfile::tempdir().unwrap();
    // a class referenced by string: the risk assessor marks it high
    fs::write(
        temp.path().join("Reflected.kt"),
        "package sample\n\nclass Reflected {\n    fun glow() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val name = \"Reflected\"\n",
            "    println(name)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    let risks_at = stdout
        .to_lowercase()
        .find("risk")
        .expect("a risks section exists");
    assert!(
        stdout[risks_at..].contains("Reflected"),
        "the string-referenced symbol is called out as residual risk, stdout was:\n{stdout}"
    );
}

#[test]
fn a_clean_project_says_nothing_to_clean() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(output.status.success());
    assert!(
        stdout.to_lowercase().contains("nothing to clean"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
