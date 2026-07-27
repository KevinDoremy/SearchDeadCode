//! Integration tests for getIdentifier() awareness: a codebase that
//! resolves resources by runtime-built names makes "unused resource"
//! findings unsafe to delete blindly — they stay reported, but flagged
//! high-risk with the reason named.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--unused-resources")
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

fn drawable_fixture(root: &Path) {
    write_file(
        root,
        "src/main/res/drawable/zombie_icon.xml",
        "<vector xmlns:android=\"http://schemas.android.com/apk/res/android\"/>",
    );
}

#[test]
fn a_literal_get_identifier_marks_matching_resource_types() {
    let temp = tempfile::tempdir().unwrap();
    drawable_fixture(temp.path());
    write_file(
        temp.path(),
        "src/main/kotlin/Icons.kt",
        concat!(
            "package sample\n\n",
            "fun icon(res: Resources, name: String, pkg: String): Int {\n",
            "    return res.getIdentifier(name, \"drawable\", pkg)\n",
            "}\n\n",
            "fun main() {\n    println(\"alive\")\n}\n",
        ),
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("zombie_icon"),
        "the finding still exists, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("getIdentifier"),
        "the dynamic-resolution risk is named, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unrelated_literal_type_leaves_the_finding_clean() {
    let temp = tempfile::tempdir().unwrap();
    drawable_fixture(temp.path());
    write_file(
        temp.path(),
        "src/main/kotlin/Texts.kt",
        concat!(
            "package sample\n\n",
            "fun text(res: Resources, name: String, pkg: String): Int {\n",
            "    return res.getIdentifier(name, \"string\", pkg)\n",
            "}\n\n",
            "fun main() {\n    println(\"alive\")\n}\n",
        ),
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("zombie_icon") && !stdout.contains("getIdentifier"),
        "a string-typed lookup says nothing about drawables, stdout was:\n{stdout}"
    );
}

#[test]
fn a_non_literal_type_marks_every_resource_finding() {
    let temp = tempfile::tempdir().unwrap();
    drawable_fixture(temp.path());
    write_file(
        temp.path(),
        "src/main/kotlin/Dynamic.kt",
        concat!(
            "package sample\n\n",
            "fun lookup(res: Resources, name: String, type: String, pkg: String): Int {\n",
            "    return res.getIdentifier(name, type, pkg)\n",
            "}\n\n",
            "fun main() {\n    println(\"alive\")\n}\n",
        ),
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("zombie_icon") && stdout.contains("getIdentifier"),
        "an unknowable type puts everything at risk, stdout was:\n{stdout}"
    );
}

#[test]
fn no_get_identifier_means_no_noise() {
    let temp = tempfile::tempdir().unwrap();
    drawable_fixture(temp.path());
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("zombie_icon") && !stdout.contains("getIdentifier"),
        "no dynamic lookups, no caveat, stdout was:\n{stdout}"
    );
}
