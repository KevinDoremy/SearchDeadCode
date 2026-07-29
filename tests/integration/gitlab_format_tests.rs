//! Integration tests for --format gitlab: the Code Quality JSON that
//! GitLab renders as an MR widget — third CI platform after
//! GitHub (SARIF) and reviewdog.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--format", "gitlab"])
        .output()
        .unwrap()
}

fn parsed(dir: &Path) -> serde_json::Value {
    let out = run(dir);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let json_start = stdout.find('[').expect("a JSON array");
    serde_json::from_str(stdout[json_start..].trim()).expect("valid Code Quality JSON")
}

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

#[test]
fn findings_follow_the_code_quality_shape() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let doc = parsed(temp.path());
    let issues = doc.as_array().unwrap();
    let ghost = issues
        .iter()
        .find(|i| i["description"].as_str().unwrap_or("").contains("Ghost"))
        .expect("the dead class is an issue");
    assert_eq!(ghost["check_name"], "DC001");
    assert!(
        ghost["fingerprint"]
            .as_str()
            .map(|f| !f.is_empty())
            .unwrap_or(false),
        "a stable fingerprint, issue was:\n{ghost}"
    );
    assert!(
        ghost["location"]["lines"]["begin"].as_u64().unwrap() >= 1,
        "a 1-indexed line, issue was:\n{ghost}"
    );
    let severity = ghost["severity"].as_str().unwrap();
    assert!(
        ["info", "minor", "major", "critical", "blocker"].contains(&severity),
        "a GitLab severity, got {severity}"
    );
}

#[test]
fn paths_are_repo_relative() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let doc = parsed(temp.path());
    for issue in doc.as_array().unwrap() {
        let path = issue["location"]["path"].as_str().unwrap();
        assert!(
            !path.starts_with('/'),
            "the MR widget wants repo-relative paths, got {path}"
        );
    }
}

#[test]
fn fingerprints_survive_a_line_shift() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let before = parsed(temp.path());
    let ghost_fp = |doc: &serde_json::Value| -> String {
        doc.as_array()
            .unwrap()
            .iter()
            .find(|i| i["description"].as_str().unwrap_or("").contains("Ghost"))
            .unwrap()["fingerprint"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let fp_before = ghost_fp(&before);

    // push the class down two lines — the widget must not reopen it
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\n\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    let after = parsed(temp.path());
    assert_eq!(
        fp_before,
        ghost_fp(&after),
        "line shifts must not change the fingerprint"
    );
}

#[test]
fn a_clean_project_is_an_empty_array() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let doc = parsed(temp.path());
    assert_eq!(
        doc.as_array().unwrap().len(),
        0,
        "no findings, empty array, got:\n{doc}"
    );
}
