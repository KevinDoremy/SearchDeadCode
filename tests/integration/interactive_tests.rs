//! Integration tests for --interactive triage mode.
//!
//! assert_cmd pipes stdin/stdout, so these tests exercise the non-TTY
//! boundary: the mode must never hang or crash outside a real terminal.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    UsedHelper().greet()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("UsedHelper.kt"),
        "package sample\n\nclass UsedHelper {\n    fun greet() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ObsoleteWidget.kt"),
        "package sample\n\nclass ObsoleteWidget {\n    fun render() {}\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path, extra_args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra_args)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn interactive_non_tty_falls_back_to_report() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &["--interactive"]);

    assert!(output.status.success(), "no crash outside a terminal");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires a terminal"),
        "the fallback is explained, stderr was:\n{stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ObsoleteWidget"),
        "the standard report still runs, stdout was:\n{stdout}"
    );
}

#[test]
fn interactive_non_tty_zero_findings_exits_clean() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    UsedHelper().greet()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("UsedHelper.kt"),
        "package sample\n\nclass UsedHelper {\n    fun greet() {}\n}\n",
    )
    .unwrap();

    let output = run(temp.path(), &["--interactive"]);

    assert!(output.status.success(), "clean exit, no hang");
}

#[test]
fn delete_interactive_keeps_confirm_each_semantics() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &["--delete", "--interactive"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("requires a terminal"),
        "--delete --interactive keeps its historical path, stderr was:\n{stderr}"
    );
}
