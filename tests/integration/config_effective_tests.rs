//! Integration tests for item: config.detection.* and config.report.*
//! must actually do something. The whole detection block used to be
//! deserialized and then ignored — users were configuring into the void.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_dead_class_project(dir: &Path) {
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
fn disabling_unused_class_hides_the_zombie() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_class_project(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        "detection:\n  unused_class: false\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("Zombie"),
        "unused_class: false must silence dead classes, stdout was:\n{stdout}"
    );
}

#[test]
fn disabling_dead_branch_hides_dc007() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_class_project(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        "detection:\n  dead_branch: false\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("DC007"),
        "dead_branch: false must silence DC007, stdout was:\n{stdout}"
    );
}

#[test]
fn defaults_report_everything() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_class_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("Zombie") && stdout.contains("DC007"),
        "without config every detection stays on, stdout was:\n{stdout}"
    );
}

#[test]
fn one_switch_does_not_drag_the_others() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_class_project(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        "detection:\n  unused_class: false\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("DC007"),
        "dead_branch stays on when only unused_class is off, stdout was:\n{stdout}"
    );
}

#[test]
fn report_format_from_config_is_used() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_class_project(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        "report:\n  format: json\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.trim_start().starts_with('{'),
        "report.format: json must produce JSON without any flag, stdout was:\n{stdout}"
    );
}

#[test]
fn an_explicit_cli_format_beats_the_config() {
    let temp = tempfile::tempdir().unwrap();
    write_dead_class_project(temp.path());
    fs::write(
        temp.path().join(".deadcode.yml"),
        "report:\n  format: json\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--format", "terminal"]));
    assert!(
        !stdout.trim_start().starts_with('{'),
        "the flag on the command line wins over the file, stdout was:\n{stdout}"
    );
}
