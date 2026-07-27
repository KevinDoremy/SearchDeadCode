//! Integration tests for --dead-keep-rules: a -keep rule naming a class
//! that no longer exists keeps nothing — the rules file rots too.
//! Only exact, project-package rules are flagged: wildcards and
//! library-class rules are unverifiable from sources alone.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--dead-keep-rules")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_sources(dir: &Path) {
    fs::write(
        dir.join("Anchor.kt"),
        "package sample.app\n\nclass Anchor {\n    fun hold() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample.app\n\nfun main() {\n    Anchor().hold()\n}\n",
    )
    .unwrap();
}

#[test]
fn a_keep_rule_for_a_vanished_class_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "-keep class sample.app.GoneClass\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("sample.app.GoneClass"),
        "the rule keeps nothing, stdout was:\n{stdout}"
    );
}

#[test]
fn a_keep_rule_for_an_existing_class_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "-keep class sample.app.Anchor\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("sample.app.Anchor"),
        "the class exists, the rule is alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_library_keep_rule_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "-keep class com.google.gson.Gson\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("com.google.gson.Gson"),
        "library classes are not in our graph — unverifiable, stdout was:\n{stdout}"
    );
}

#[test]
fn wildcard_rules_are_left_alone() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "-keep class sample.gone.** { *; }\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("sample.gone"),
        "wildcards may target generated or library classes, stdout was:\n{stdout}"
    );
}

#[test]
fn a_commented_out_rule_is_not_a_rule() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        concat!(
            "# -keep class sample.app.GoneClass\n",
            "-keepclassmembers class sample.app.Anchor { <init>(...); }\n",
        ),
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("GoneClass"),
        "a commented rule keeps nothing and flags nothing, stdout was:\n{stdout}"
    );
    assert!(
        stdout.to_lowercase().contains("every verifiable"),
        "the member-variant rule on a living class is fine, stdout was:\n{stdout}"
    );
}

#[test]
fn no_pro_files_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "no rules files is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no proguard rules"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
