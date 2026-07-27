//! Integration tests for primary-constructor annotations: in Kotlin,
//! `class Foo @Inject constructor()` puts the annotation on the
//! constructor node — the class escaped DI retention and read as dead.
//! Dangerous false positives for --delete.

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
fn an_inject_constructor_class_is_retained() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("Service.kt"),
        concat!(
            "package sample\n\n",
            "class InjectedWidget @Inject constructor() {\n",
            "    fun serve() {}\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'InjectedWidget'"),
        "DI instantiates it — the class is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unannotated_constructor_class_still_reads_dead() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("Plain.kt"),
        concat!(
            "package sample\n\n",
            "class PlainWidget constructor() {\n",
            "    fun serve() {}\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("'PlainWidget'"),
        "no annotation, no retention, stdout was:\n{stdout}"
    );
}

#[test]
fn a_non_retaining_constructor_annotation_does_not_retain() {
    let temp = tempfile::tempdir().unwrap();
    write_main(temp.path());
    fs::write(
        temp.path().join("Marked.kt"),
        concat!(
            "package sample\n\n",
            "class MarkedWidget @JvmOverloads constructor(val x: Int = 0) {\n",
            "    fun serve() {}\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("'MarkedWidget'"),
        "JvmOverloads is not a retention annotation, stdout was:\n{stdout}"
    );
}
