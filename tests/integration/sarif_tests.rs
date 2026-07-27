//! Integration tests for complete SARIF output: stable fingerprints
//! (without them GitHub Code Scanning re-opens every alert when a line
//! moves), every emitted ruleId declared in driver.rules, and repo-
//! relative URIs.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Zombie.kt"),
        "package sample\n\nclass Zombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    if (false) {\n",
            "        println(\"never\")\n",
            "    }\n",
            "    println(\"alive\")\n",
            "}\n",
        ),
    )
    .unwrap();
}

fn sarif(dir: &Path) -> serde_json::Value {
    let output: Output = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--format", "sarif", "--quiet"])
        .output()
        .unwrap();
    serde_json::from_slice(&output.stdout).expect("SARIF output must be valid JSON")
}

#[test]
fn every_result_carries_a_fingerprint() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let doc = sarif(temp.path());
    let results = doc["runs"][0]["results"].as_array().unwrap();
    assert!(!results.is_empty(), "the corpse produces results");
    for result in results {
        let fp = &result["partialFingerprints"]["searchdeadcode/v1"];
        assert!(
            fp.is_string() && !fp.as_str().unwrap().is_empty(),
            "every result needs a stable fingerprint, result was:\n{result}"
        );
    }
}

#[test]
fn the_fingerprint_survives_a_line_shift() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let before = sarif(temp.path());
    let fp_of = |doc: &serde_json::Value| -> Option<String> {
        doc["runs"][0]["results"]
            .as_array()?
            .iter()
            .find(|r| {
                r["message"]["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Zombie")
            })
            .and_then(|r| r["partialFingerprints"]["searchdeadcode/v1"].as_str())
            .map(String::from)
    };
    let fp_before = fp_of(&before).expect("Zombie has a fingerprint");

    // Push the class down a few lines: the alert must not re-open
    let shifted =
        "package sample\n\n// pad\n// pad\n// pad\n\nclass Zombie {\n    fun groan() {}\n}\n";
    fs::write(temp.path().join("Zombie.kt"), shifted).unwrap();

    let after = sarif(temp.path());
    let fp_after = fp_of(&after).expect("Zombie still has a fingerprint");
    assert_eq!(
        fp_before, fp_after,
        "a line shift must not change the fingerprint"
    );
}

#[test]
fn every_emitted_rule_id_is_declared() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let doc = sarif(temp.path());
    let declared: std::collections::HashSet<&str> = doc["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["id"].as_str())
        .collect();
    for result in doc["runs"][0]["results"].as_array().unwrap() {
        let rule_id = result["ruleId"].as_str().unwrap();
        assert!(
            declared.contains(rule_id),
            "ruleId {rule_id} is emitted but not declared in driver.rules"
        );
    }
}

#[test]
fn uris_are_relative_to_the_analyzed_root() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let doc = sarif(temp.path());
    let root = temp.path().to_string_lossy().to_string();
    for result in doc["runs"][0]["results"].as_array().unwrap() {
        let uri = result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
            .as_str()
            .unwrap();
        assert!(
            !uri.contains(&root),
            "URIs must be repo-relative for Code Scanning, got {uri}"
        );
    }
}
