//! Integration tests for --baseline-stats: baseline entries are
//! findings someone judged not-actionable — counting them per rule
//! shows where the tool cries wolf the most.

use std::fs;
use std::path::Path;
use std::process::Output;

fn bin(dir: &Path, args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn seeded(temp: &Path) -> std::path::PathBuf {
    fs::write(
        temp.join("DeadA.kt"),
        "package sample\n\nclass DeadA {\n    fun a() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.join("DeadB.kt"),
        "package sample\n\nclass DeadB {\n    fun b() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.join("baseline.json");
    let out = bin(temp, &["--generate-baseline", baseline.to_str().unwrap()]);
    assert!(out.status.success(), "seed failed:\n{out:?}");
    baseline
}

#[test]
fn generated_baselines_record_the_rule_per_entry() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded(temp.path());

    let json = fs::read_to_string(&baseline).unwrap();
    assert!(
        json.contains("\"rule\"") && json.contains("DC001"),
        "each entry knows which rule produced it, json was:\n{json}"
    );
}

#[test]
fn baseline_stats_ranks_rules_by_count() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded(temp.path());

    let out = bin(
        temp.path(),
        &["--baseline", baseline.to_str().unwrap(), "--baseline-stats"],
    );
    let stdout = stdout_of(&out);
    assert!(out.status.success(), "stats failed:\n{out:?}");
    assert!(
        stdout.contains("DC001"),
        "the dominant rule is listed, stdout was:\n{stdout}"
    );
    assert!(
        stdout.chars().filter(|c| c.is_ascii_digit()).count() >= 1,
        "counts ride along, stdout was:\n{stdout}"
    );
}

#[test]
fn an_old_baseline_without_rules_still_works() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded(temp.path());
    // strip the rule fields to simulate a pre-upgrade baseline
    let json = fs::read_to_string(&baseline).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    for issue in value["issues"].as_array_mut().unwrap() {
        issue.as_object_mut().unwrap().remove("rule");
    }
    fs::write(&baseline, serde_json::to_string_pretty(&value).unwrap()).unwrap();

    let out = bin(
        temp.path(),
        &["--baseline", baseline.to_str().unwrap(), "--baseline-stats"],
    );
    let stdout = stdout_of(&out);
    assert!(
        out.status.success(),
        "old baselines must keep loading, output was:\n{out:?}"
    );
    assert!(
        stdout.to_lowercase().contains("unknown"),
        "rule-less entries group under unknown, stdout was:\n{stdout}"
    );
}

#[test]
fn stats_without_baseline_errors_out() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--baseline-stats"]);
    assert!(
        !out.status.success(),
        "--baseline-stats needs --baseline, output was:\n{out:?}"
    );
}
