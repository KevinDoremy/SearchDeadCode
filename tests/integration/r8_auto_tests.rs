//! Integration tests for R8 usage.txt auto-discovery: the file lives at
//! a well-known path (build/outputs/mapping/<variant>/usage.txt) — asking
//! for --proguard-usage every run is friction the tool can absorb.

use std::fs;
use std::path::Path;
use std::process::Output;

fn bin(dir: &Path, args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_sources(dir: &Path) {
    fs::write(
        dir.join("DeadHelper.kt"),
        "package sample\n\nclass DeadHelper {\n    fun assist() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

fn write_usage(dir: &Path, variant: &str, classes: &[&str]) {
    let mapping = dir.join("build/outputs/mapping").join(variant);
    fs::create_dir_all(&mapping).unwrap();
    let mut content = String::new();
    for class in classes {
        content.push_str(class);
        content.push('\n');
    }
    fs::write(mapping.join("usage.txt"), content).unwrap();
}

#[test]
fn usage_txt_is_auto_discovered() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());
    write_usage(temp.path(), "release", &["sample.DeadHelper"]);

    let out = bin(temp.path(), &[]);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("usage.txt"),
        "the discovered file is announced, stdout was:\n{stdout}"
    );
}

#[test]
fn an_explicit_flag_wins_over_discovery() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());
    // discovery would find two entries; the explicit file holds one
    write_usage(
        temp.path(),
        "release",
        &["sample.DeadHelper", "sample.Other"],
    );
    let custom = temp.path().join("my-usage.txt");
    fs::write(&custom, "sample.DeadHelper\n").unwrap();

    let out = bin(temp.path(), &["--proguard-usage", custom.to_str().unwrap()]);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("1 unused items"),
        "the explicit file is the one loaded, stdout was:\n{stdout}"
    );
}

#[test]
fn no_usage_txt_means_no_noise() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());

    let out = bin(temp.path(), &[]);
    let stdout = stdout_of(&out);
    assert!(
        !stdout.contains("usage.txt"),
        "nothing discovered, nothing announced, stdout was:\n{stdout}"
    );
}

#[test]
fn the_release_variant_wins_over_debug() {
    let temp = tempfile::tempdir().unwrap();
    write_sources(temp.path());
    write_usage(temp.path(), "debug", &["sample.DeadHelper"]);
    write_usage(
        temp.path(),
        "release",
        &["sample.DeadHelper", "sample.Other"],
    );

    let out = bin(temp.path(), &[]);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("2 unused items"),
        "release is the shrunk build teams care about, stdout was:\n{stdout}"
    );
}

#[test]
fn a_module_level_build_dir_is_found_too() {
    let temp = tempfile::tempdir().unwrap();
    let module = temp.path().join("app");
    fs::create_dir_all(&module).unwrap();
    write_sources(&module);
    write_usage(&module, "release", &["sample.DeadHelper"]);

    let out = bin(temp.path(), &[]);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("usage.txt"),
        "app/build/... is the common layout, stdout was:\n{stdout}"
    );
}
