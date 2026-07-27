//! Integration tests for --deprecated: a @Deprecated symbol with no
//! reference left has finished its job — delete it. One still
//! referenced is an unfinished migration. Both lists in one view.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Api.kt"),
        concat!(
            "package sample\n\n",
            "@Deprecated(\"use NewHelper\")\n",
            "class DoneHelper {\n    fun help() {}\n}\n\n",
            "@Deprecated(\"use NewApi\")\n",
            "class LingeringApi {\n    fun call() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    LingeringApi().call()\n",
            "}\n",
        ),
    )
    .unwrap();
}

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--deprecated")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn an_unreferenced_deprecated_symbol_is_ready_to_delete() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    let ready = stdout.find("ready to delete");
    let done = stdout.find("DoneHelper");
    assert!(
        ready.is_some() && done.is_some() && ready < done,
        "DoneHelper sits in the ready-to-delete section, stdout was:\n{stdout}"
    );
}

#[test]
fn a_referenced_deprecated_symbol_is_an_unfinished_migration() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    let still = stdout.find("still referenced");
    let lingering = stdout.find("LingeringApi");
    assert!(
        still.is_some() && lingering.is_some() && still < lingering,
        "LingeringApi sits in the still-referenced section, stdout was:\n{stdout}"
    );
}

#[test]
fn no_deprecated_symbols_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "no deprecations is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no deprecated"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
