//! Integration tests for configurable thresholds (item: LongMethod=50,
//! LargeClass=500, params=6 were hardcoded — every team has its own
//! conventions). Also proves the anti_patterns config groups activate
//! detectors without any CLI flag: they were CLI-only until now.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun chatty() {\n",
            "    println(\"one\")\n",
            "    println(\"two\")\n",
            "    println(\"three\")\n",
            "    println(\"four\")\n",
            "    println(\"five\")\n",
            "}\n\n",
            "fun main() {\n",
            "    chatty()\n",
            "}\n",
        ),
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
fn a_lowered_threshold_flags_a_small_method() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        concat!(
            "detection:\n",
            "  anti_patterns:\n",
            "    performance: true\n",
            "  thresholds:\n",
            "    long_method_lines: 3\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("AP012") || stdout.to_lowercase().contains("long method"),
        "chatty() has 5 lines, the configured limit is 3, stdout was:\n{stdout}"
    );
}

#[test]
fn the_default_threshold_leaves_small_methods_alone() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        "detection:\n  anti_patterns:\n    performance: true\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("AP012"),
        "5 lines is far below the default 50, stdout was:\n{stdout}"
    );
}

#[test]
fn config_groups_activate_without_any_cli_flag() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        concat!(
            "detection:\n",
            "  anti_patterns:\n",
            "    kotlin: true\n",
            "  thresholds:\n",
            "    long_parameter_list: 1\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Wide.kt"),
        concat!(
            "package sample\n\n",
            "fun wide(a: Int, b: Int, c: Int) {\n",
            "    println(a + b + c)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("AP021") || stdout.to_lowercase().contains("parameter"),
        "3 params over a configured limit of 1, no CLI flag needed, stdout was:\n{stdout}"
    );
}

#[test]
fn cli_flags_still_work_without_config() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path(), &["--performance-patterns"]);
    assert!(
        output.status.success(),
        "the CLI path must not regress: {output:?}"
    );
}
