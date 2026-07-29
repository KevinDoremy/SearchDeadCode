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

#[test]
fn an_unknown_config_key_warns_with_a_suggestion() {
    // serde silently swallows unknown keys: a typo like 'exclud' means
    // the user's guard simply stops applying, with no signal at all
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join(".deadcode.yml"),
        "exclud:\n  - \"**/gen/**\"\n",
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "a typo warns, it does not break the run:\n{out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("exclud"),
        "the unknown key is named, stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("exclude"),
        "the closest real key is suggested, stderr was:\n{stderr}"
    );
}

#[test]
fn a_valid_config_warns_about_nothing() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join(".deadcode.yml"),
        "exclude:\n  - \"**/gen/**\"\n",
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.to_lowercase().contains("unknown"),
        "a clean config stays silent, stderr was:\n{stderr}"
    );
}
