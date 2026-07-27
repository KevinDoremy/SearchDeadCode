//! Integration tests for ProGuard -keep rule retention.
//!
//! A class matched by a -keep rule survives shrinking at build time — the
//! developer explicitly said "this is used dynamically". Reporting it dead
//! is a false positive by definition.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::create_dir_all(dir.join("api/sub")).unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    UsedTool().work()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("UsedTool.kt"),
        "package sample\n\nclass UsedTool {\n    fun work() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("KeptGadget.kt"),
        "package sample\n\nclass KeptGadget {\n    fun poke() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("DoomedGadget.kt"),
        "package sample\n\nclass DoomedGadget {\n    fun poke() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("api/sub/DeepApi.kt"),
        "package sample.api.sub\n\nclass DeepApi {\n    fun call() {}\n}\n",
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
fn a_kept_class_is_never_reported_dead() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "-keep class sample.KeptGadget { *; }\n",
    )
    .unwrap();

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("'KeptGadget'"),
        "-keep retains the class, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("'DoomedGadget'"),
        "unkept dead code is still reported, stdout was:\n{stdout}"
    );
}

#[test]
fn double_star_wildcards_cross_packages() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "-keep class sample.api.** { *; }\n",
    )
    .unwrap();

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("'DeepApi'"),
        "** crosses package levels, stdout was:\n{stdout}"
    );
}

#[test]
fn single_star_stays_in_one_package_level() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "-keep class sample.* { *; }\n",
    )
    .unwrap();

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("'KeptGadget'"),
        "sample.* retains top-level classes, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("'DeepApi'"),
        "a single * must not cross into sample.api.sub, stdout was:\n{stdout}"
    );
}

#[test]
fn commented_keep_rules_are_ignored() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(
        temp.path().join("proguard-rules.pro"),
        "# -keep class sample.DoomedGadget { *; }\n",
    )
    .unwrap();

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("'DoomedGadget'"),
        "a commented rule retains nothing, stdout was:\n{stdout}"
    );
}
