//! Integration tests for per-module config: a module-level .deadcode.yml
//! merges its excludes into the root config, scoped to that module.
//! In a many-module monorepo one root config never fits everybody.

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

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn a_module_exclude_applies_to_that_module_only() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "core/.deadcode.yml",
        "exclude:\n  - \"legacy/**\"\n",
    );
    write_file(
        temp.path(),
        "core/legacy/OldCore.kt",
        "package sample\n\nclass OldCore {\n    fun rot() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/legacy/OldApp.kt",
        "package sample\n\nclass OldApp {\n    fun rot() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("OldCore"),
        "core's own exclude silences core/legacy, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("OldApp"),
        "the exclude must not leak into other modules, stdout was:\n{stdout}"
    );
}

#[test]
fn the_root_config_still_applies_everywhere() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        ".deadcode.yml",
        "exclude:\n  - \"**/generated/**\"\n",
    );
    write_file(
        temp.path(),
        "core/.deadcode.yml",
        "exclude:\n  - \"legacy/**\"\n",
    );
    write_file(
        temp.path(),
        "core/generated/Gen.kt",
        "package sample\n\nclass Gen {\n    fun g() {}\n}\n",
    );
    write_file(
        temp.path(),
        "core/legacy/OldCore.kt",
        "package sample\n\nclass OldCore {\n    fun rot() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("Gen") && !stdout.contains("OldCore"),
        "root and module excludes stack, stdout was:\n{stdout}"
    );
}

#[test]
fn a_corrupt_module_config_is_skipped_not_fatal() {
    let temp = tempfile::tempdir().unwrap();
    write_file(temp.path(), "core/.deadcode.yml", "{{{{ not yaml");
    write_file(
        temp.path(),
        "core/Thing.kt",
        "package sample\n\nclass Thing {\n    fun t() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let output = run(temp.path());
    assert!(
        output.status.success(),
        "a broken module config must not kill the run, output was:\n{output:?}"
    );
}

#[test]
fn without_module_configs_nothing_changes() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "core/legacy/OldCore.kt",
        "package sample\n\nclass OldCore {\n    fun rot() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("OldCore"),
        "no module config, no magic exclusion, stdout was:\n{stdout}"
    );
}
