//! Integration tests for --library-mode: a published library's public
//! API is alive by definition (its consumers live outside the repo).
//! Only internal deadness is worth reporting — without this mode the
//! tool is unusable on a published module.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_library(dir: &Path) {
    fs::write(
        dir.join("Api.kt"),
        concat!(
            "package lib\n\n",
            "class PublicEntryPoint {\n",
            "    fun consume() {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Internal.kt"),
        concat!(
            "package lib\n\n",
            "internal class DeadInternalHelper {\n",
            "    fun help() {}\n",
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
fn public_api_is_alive_by_definition() {
    let temp = tempfile::tempdir().unwrap();
    write_library(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--library-mode"]));
    assert!(
        !stdout.contains("PublicEntryPoint"),
        "consumers live outside the repo, stdout was:\n{stdout}"
    );
}

#[test]
fn internal_deadness_is_still_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_library(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--library-mode"]));
    assert!(
        stdout.contains("DeadInternalHelper"),
        "internal cannot be reached from outside, stdout was:\n{stdout}"
    );
}

#[test]
fn without_the_flag_public_dead_code_still_reports() {
    let temp = tempfile::tempdir().unwrap();
    write_library(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("PublicEntryPoint"),
        "an app analysis keeps flagging unreferenced public classes, stdout was:\n{stdout}"
    );
}

#[test]
fn a_private_top_level_helper_is_still_dead_in_library_mode() {
    let temp = tempfile::tempdir().unwrap();
    write_library(temp.path());
    fs::write(
        temp.path().join("PrivateHelper.kt"),
        "package lib\n\nprivate fun deadPrivateHelper() {}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--library-mode"]));
    assert!(
        stdout.contains("deadPrivateHelper"),
        "private symbols have no external consumers, stdout was:\n{stdout}"
    );
}
