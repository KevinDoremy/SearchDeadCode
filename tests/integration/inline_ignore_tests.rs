//! Integration tests for inline ignores: `// deadcode:ignore(reason)`
//! above (or on) a declaration silences its findings. The reason is
//! MANDATORY — an ignore without one is not honored, because "shut up"
//! with no why rots into mystery suppressions.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_main(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

#[test]
fn an_ignored_symbol_disappears_and_its_reason_shows_in_stats() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("Zombie.kt"),
        concat!(
            "package sample\n\n",
            "// deadcode:ignore(kept for QA tooling)\n",
            "class Zombie {\n",
            "    fun groan() {}\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("[DC001]") || !stdout.contains("'Zombie'"),
        "the ignored class is not a finding, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("kept for QA tooling"),
        "the reason is visible in the stats, stdout was:\n{stdout}"
    );
}

#[test]
fn without_a_comment_the_zombie_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("Zombie.kt"),
        "package sample\n\nclass Zombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("Zombie"),
        "no directive, normal report, stdout was:\n{stdout}"
    );
}

#[test]
fn an_ignore_without_a_reason_is_not_honored() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("Zombie.kt"),
        concat!(
            "package sample\n\n",
            "// deadcode:ignore\n",
            "class Zombie {\n",
            "    fun groan() {}\n",
            "}\n",
        ),
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        stdout.contains("Zombie"),
        "no reason, no silence, stdout was:\n{stdout}"
    );
    assert!(
        stderr.contains("reason") || stdout.contains("reason"),
        "the user learns why the directive was refused, stderr was:\n{stderr}"
    );
}

#[test]
fn an_empty_reason_is_no_reason() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("Zombie.kt"),
        concat!(
            "package sample\n\n",
            "// deadcode:ignore(  )\n",
            "class Zombie {\n",
            "    fun groan() {}\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("Zombie"),
        "blank parens are not a reason, stdout was:\n{stdout}"
    );
}

#[test]
fn a_same_line_directive_works_too() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("Zombie.kt"),
        concat!(
            "package sample\n\n",
            "class Zombie { // deadcode:ignore(legacy bridge, remove with v3)\n",
            "    fun groan() {}\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("legacy bridge"),
        "trailing directives count, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("[DC001]") || !stdout.contains("'Zombie'"),
        "the class itself is silenced, stdout was:\n{stdout}"
    );
}
