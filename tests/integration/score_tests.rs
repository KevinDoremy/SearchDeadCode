//! Integration tests for --score: one sortable number per finding —
//! impact (cluster LOC) × confidence ÷ risk. "Delete in this order."

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    let mut big = String::from("package sample\n\nclass HeavyCorpse {\n");
    for i in 0..25 {
        big.push_str(&format!(
            "    fun rot{i}() {{\n        println({i})\n    }}\n"
        ));
    }
    big.push_str("}\n");
    fs::write(dir.join("HeavyCorpse.kt"), big).unwrap();

    fs::write(
        dir.join("LightCorpse.kt"),
        "package sample\n\nclass LightCorpse {\n    fun tinyRot() {}\n}\n",
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
fn heavier_low_risk_corpses_score_first() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--score"]));
    let heavy = stdout.find("HeavyCorpse");
    let light = stdout.find("LightCorpse");
    assert!(
        heavy.is_some() && light.is_some(),
        "both corpses are ranked, stdout was:\n{stdout}"
    );
    assert!(
        heavy < light,
        "more deletable lines at equal confidence wins, stdout was:\n{stdout}"
    );
}

#[test]
fn a_soft_referenced_corpse_drops_behind_a_clean_equal() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("CleanTwin.kt"),
        "package sample\n\nclass CleanTwin {\n    fun rot() {\n        println(1)\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("RiskyTwin.kt"),
        "package sample\n\nclass RiskyTwin {\n    fun rot() {\n        println(2)\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"sample.RiskyTwin\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--score", "--min-confidence", "low"]));
    let clean = stdout.find("CleanTwin");
    let risky = stdout.find("RiskyTwin");
    assert!(
        clean.is_some() && risky.is_some(),
        "both twins are ranked, stdout was:\n{stdout}"
    );
    assert!(
        clean < risky,
        "same size, but the string-referenced twin is riskier, stdout was:\n{stdout}"
    );
}

#[test]
fn a_healthy_project_exits_clean() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path(), &["--score"]);
    assert!(
        output.status.success(),
        "no findings, no drama, output was:\n{output:?}"
    );
}
