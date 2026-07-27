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

#[test]
fn unreferenced_results_carry_a_deletion_fix() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let doc = sarif(temp.path());
    let dc001 = doc["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["ruleId"] == "DC001")
        .expect("an unreferenced finding exists");
    let fix = &dc001["fixes"][0];
    assert!(
        fix["description"]["text"]
            .as_str()
            .unwrap()
            .contains("delete"),
        "the fix says what it does, fix was:\n{fix}"
    );
    let replacement = &fix["artifactChanges"][0]["replacements"][0];
    let start = replacement["deletedRegion"]["startLine"].as_u64().unwrap();
    let end = replacement["deletedRegion"]["endLine"].as_u64().unwrap();
    assert!(
        start >= 1 && end >= start,
        "a real line range to delete, got {start}..{end}"
    );
    assert_eq!(
        replacement["insertedContent"]["text"], "",
        "a pure deletion inserts nothing"
    );
}

#[test]
fn synthetic_findings_offer_no_fix() {
    // resource findings carry synthetic offsets — a deletedRegion built
    // from them would point at the wrong lines
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let res_dir = temp.path().join("src/main/res/drawable");
    std::fs::create_dir_all(&res_dir).unwrap();
    std::fs::write(
        res_dir.join("zombie_icon.xml"),
        "<vector xmlns:android=\"http://schemas.android.com/apk/res/android\"/>",
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--unused-resources", "--format", "sarif"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let json_start = stdout.find('{').expect("sarif json");
    let doc: serde_json::Value = serde_json::from_str(stdout[json_start..].trim()).unwrap();
    let dc017 = doc["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["ruleId"] == "DC017")
        .expect("the resource finding exists");
    assert!(
        dc017["fixes"].is_null(),
        "no fix on synthetic spans, result was:\n{dc017}"
    );
}
