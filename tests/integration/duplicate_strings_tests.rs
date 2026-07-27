//! Integration tests for --duplicate-strings: the same string VALUE
//! declared in several modules is a centralization candidate — each
//! copy drifts (and gets translated) on its own.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--duplicate-strings")
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

fn strings_xml(entries: &[(&str, &str)]) -> String {
    let mut out = String::from("<resources>\n");
    for (name, value) in entries {
        out.push_str(&format!("    <string name=\"{name}\">{value}</string>\n"));
    }
    out.push_str("</resources>\n");
    out
}

#[test]
fn the_same_value_in_two_modules_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "core/src/main/res/values/strings.xml",
        &strings_xml(&[("retry_label", "Retry now")]),
    );
    write_file(
        temp.path(),
        "app/src/main/res/values/strings.xml",
        &strings_xml(&[("try_again", "Retry now")]),
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("Retry now") && stdout.contains("core") && stdout.contains("app"),
        "the shared value and both modules are named, stdout was:\n{stdout}"
    );
}

#[test]
fn a_value_duplicated_within_one_module_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "app/src/main/res/values/strings.xml",
        &strings_xml(&[("a", "Same words"), ("b", "Same words")]),
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("Same words"),
        "intra-module duplication is another problem, stdout was:\n{stdout}"
    );
}

#[test]
fn different_values_are_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "core/src/main/res/values/strings.xml",
        &strings_xml(&[("ok", "OK")]),
    );
    write_file(
        temp.path(),
        "app/src/main/res/values/strings.xml",
        &strings_xml(&[("cancel", "Cancel")]),
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {}\n",
    );

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        stdout.to_lowercase().contains("no duplicate strings"),
        "nothing shared, clean verdict, stdout was:\n{stdout}"
    );
    assert!(output.status.success());
}

#[test]
fn locale_folders_are_not_compared_with_the_base() {
    // values-fr of one module accidentally matching values/ of another
    // is translation noise, not a centralization candidate
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "core/src/main/res/values/strings.xml",
        &strings_xml(&[("greeting", "Impossible")]),
    );
    write_file(
        temp.path(),
        "app/src/main/res/values-fr/strings.xml",
        &strings_xml(&[("salutation", "Impossible")]),
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("Impossible"),
        "locale folders stay out of the comparison, stdout was:\n{stdout}"
    );
}

#[test]
fn no_strings_files_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    write_file(temp.path(), "Main.kt", "package sample\n\nfun main() {}\n");

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "no strings.xml is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no string resources"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
