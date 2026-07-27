//! Integration tests for dead-code tests: a test file that still
//! references a dead symbol should be deleted together with it — the
//! finding says so. Real case: disabled tests keeping a corpse company.

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
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_corpse_still_referenced_by_a_test_says_so() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::create_dir_all(temp.path().join("src/test/kotlin")).unwrap();
    fs::write(
        temp.path().join("src/test/kotlin/ZombieTest.kt"),
        concat!(
            "package sample\n\n",
            "// @Disabled since the redesign\n",
            "class ZombieTest {\n",
            "    fun stillHere() {\n",
            "        Zombie().groan()\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("referenced by tests"),
        "the finding tells you to delete the test too, stdout was:\n{stdout}"
    );
}

#[test]
fn a_corpse_with_no_test_gets_no_test_note() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("Zombie") && !stdout.contains("referenced by tests"),
        "no test names it, stdout was:\n{stdout}"
    );
}

#[test]
fn a_longer_test_identifier_is_not_a_reference() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::create_dir_all(temp.path().join("src/test/kotlin")).unwrap();
    fs::write(
        temp.path().join("src/test/kotlin/OtherTest.kt"),
        concat!(
            "package sample\n\n",
            "class OtherTest {\n",
            "    fun checks() {\n",
            "        println(\"MegaZombieHelper unrelated\")\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("referenced by tests"),
        "'Zombie' inside 'MegaZombieHelper' is not a reference, stdout was:\n{stdout}"
    );
}
