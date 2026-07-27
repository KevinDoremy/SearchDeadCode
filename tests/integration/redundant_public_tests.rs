//! Integration tests for DC006 (redundant public).
//!
//! A public Kotlin declaration whose references all live in its own
//! module could be `internal`. The verdict needs at least two modules
//! (otherwise `internal` changes nothing) and at least one reference
//! (a declaration nobody uses is dead code, a different report).

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_two_module_project(root: &Path) {
    fs::create_dir_all(root.join("core/src")).unwrap();
    fs::create_dir_all(root.join("app/src")).unwrap();
    fs::write(
        root.join("core/src/Helper.kt"),
        concat!(
            "package core\n\n",
            "class LocalHelper {\n",
            "    fun help() {}\n",
            "}\n\n",
            "class SharedHelper {\n",
            "    fun help() {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("core/src/CoreMain.kt"),
        concat!(
            "package core\n\n",
            "fun coreMain() {\n",
            "    LocalHelper().help()\n",
            "    SharedHelper().help()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("app/src/App.kt"),
        concat!(
            "package app\n\n",
            "import core.SharedHelper\n\n",
            "fun main() {\n",
            "    core.coreMain()\n",
            "    SharedHelper().help()\n",
            "}\n",
        ),
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
fn a_public_class_used_only_in_its_module_could_be_internal() {
    let temp = tempfile::tempdir().unwrap();
    write_two_module_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("DC006") && stdout.contains("LocalHelper"),
        "LocalHelper never leaves module core, stdout was:\n{stdout}"
    );
}

#[test]
fn a_class_referenced_from_another_module_is_rightly_public() {
    let temp = tempfile::tempdir().unwrap();
    write_two_module_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    let dc006_lines: Vec<&str> = stdout.lines().filter(|l| l.contains("DC006")).collect();
    assert!(
        !dc006_lines.iter().any(|l| l.contains("SharedHelper")),
        "SharedHelper is used from app, DC006 lines were:\n{dc006_lines:?}"
    );
}

#[test]
fn a_single_module_project_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Helper.kt"),
        "package solo\n\nclass OnlyHelper {\n    fun help() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package solo\n\nfun main() {\n    OnlyHelper().help()\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DC006"),
        "internal means nothing with one module, stdout was:\n{stdout}"
    );
}

#[test]
fn an_internal_class_is_not_reflagged() {
    let temp = tempfile::tempdir().unwrap();
    write_two_module_project(temp.path());
    fs::write(
        temp.path().join("core/src/Quiet.kt"),
        concat!(
            "package core\n\n",
            "internal class AlreadyQuiet {\n",
            "    fun hush() {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("core/src/QuietUser.kt"),
        "package core\n\nfun useQuiet() {\n    AlreadyQuiet().hush()\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    let dc006_lines: Vec<&str> = stdout.lines().filter(|l| l.contains("DC006")).collect();
    assert!(
        !dc006_lines.iter().any(|l| l.contains("AlreadyQuiet")),
        "already internal, nothing to suggest, DC006 lines were:\n{dc006_lines:?}"
    );
}
