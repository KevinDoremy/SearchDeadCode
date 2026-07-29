//! Integration tests for rule-code completion: --expand-rule knows its
//! possible values, so a typo dies at parse time (with the real codes
//! suggested) instead of silently expanding nothing, and shell
//! completions can offer the codes.

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

#[test]
fn a_rule_typo_is_rejected_at_parse_time() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--expand-rule", "DC0001"]);
    assert!(
        !out.status.success(),
        "a typo must die loudly, output was:\n{out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("DC001"),
        "the real codes are suggested, stderr was:\n{stderr}"
    );
}

#[test]
fn a_real_rule_code_still_works() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--expand-rule", "DC001"]);
    assert!(
        out.status.success(),
        "valid codes keep working, output was:\n{out:?}"
    );
}

#[test]
fn zsh_completions_offer_the_rule_codes() {
    let temp = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--completions", "zsh"])
        .output()
        .unwrap();
    let script = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        script.contains("DC001") && script.contains("AP001"),
        "the completion script carries the codes, script had {} chars",
        script.len()
    );
}
