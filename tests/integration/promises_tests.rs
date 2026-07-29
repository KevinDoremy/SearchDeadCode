//! Integration tests for --promises: TODO remove / FIXME delete
//! comments are written deletion promises — crossing them with the
//! actual death of the nearby symbol surfaces the most consensual
//! cleanups in the backlog.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--promises")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_promise_on_a_dead_symbol_is_ready() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("OldGate.kt"),
        concat!(
            "package sample\n\n",
            "// TODO remove after the v2 rollout\n",
            "class OldGate {\n    fun check() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("OldGate") && stdout.to_lowercase().contains("ready"),
        "a dead symbol under a promise is ready to honor, stdout was:\n{stdout}"
    );
}

#[test]
fn a_promise_on_a_living_symbol_is_not_ready_yet() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Bridge.kt"),
        concat!(
            "package sample\n\n",
            "// FIXME delete once the migration lands\n",
            "class Bridge {\n    fun cross() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    Bridge().cross()\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("Bridge") && stdout.to_lowercase().contains("still referenced"),
        "a living symbol under a promise is a stalled migration, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unrelated_todo_is_not_a_promise() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Notes.kt"),
        concat!(
            "package sample\n\n",
            "// TODO add better logging here\n",
            "class Notes {\n    fun jot() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("Notes"),
        "only removal promises count, stdout was:\n{stdout}"
    );
}

#[test]
fn no_promises_is_a_clean_answer() {
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
        "no promises is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no deletion promises"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
