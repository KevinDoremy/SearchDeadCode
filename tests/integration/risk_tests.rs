//! Integration tests for per-finding risk levels.
//!
//! A dead symbol whose name appears in a string literal (reflection, JSON
//! keys, FQN strings) or that lives next to reflection/event-bus code is
//! riskier to delete than a plain unreferenced class.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    val target = \"DeadReflected\"\n    println(target)\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("DeadReflected.kt"),
        "package sample\n\nclass DeadReflected {\n    fun poke() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("DeadPlain.kt"),
        "package sample\n\nclass DeadPlain {\n    fun poke() {}\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path, extra_args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra_args)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn string_referenced_symbol_is_high_risk_in_terminal() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &[]);

    let stdout = stdout_of(&output);
    let reflected_line = stdout
        .lines()
        .find(|l| l.contains("DeadReflected") && l.contains("[DC"))
        .expect("DeadReflected is reported dead");
    assert!(
        reflected_line.contains("risk: high"),
        "a name found in a string literal is high risk, line was:\n{reflected_line}"
    );
}

#[test]
fn plain_dead_symbol_carries_no_risk_tag() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &[]);

    let stdout = stdout_of(&output);
    let plain_line = stdout
        .lines()
        .find(|l| l.contains("DeadPlain") && l.contains("[DC"))
        .expect("DeadPlain is reported dead");
    assert!(
        !plain_line.contains("risk"),
        "low risk stays untagged to keep the report quiet, line was:\n{plain_line}"
    );
}

#[test]
fn json_output_carries_risk_levels() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let report_path = temp.path().join("report.json");
    run(
        temp.path(),
        &[
            "--format",
            "json",
            "--output",
            report_path.to_str().unwrap(),
        ],
    );

    let raw = fs::read_to_string(&report_path).expect("JSON report written");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON output");
    let issues = json["issues"].as_array().expect("issues array");

    let risk_of = |name: &str| -> String {
        issues
            .iter()
            .find(|i| i["declaration"]["name"] == name)
            .unwrap_or_else(|| panic!("{name} present in JSON"))["risk"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };

    assert_eq!(risk_of("DeadReflected"), "high");
    assert_eq!(risk_of("DeadPlain"), "low");
}
