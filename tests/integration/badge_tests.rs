//! Integration tests for --badge: the dead-code percentage as a
//! shields-style SVG for the README — generated locally, no service.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path, out: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--badge")
        .arg(out)
        .output()
        .unwrap()
}

#[test]
fn the_badge_carries_the_dead_percentage() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("DeadThing.kt"),
        "package sample\n\nclass DeadThing {\n    fun rot() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let out = temp.path().join("badge.svg");

    let output = run(temp.path(), &out);
    assert!(output.status.success(), "badge failed:\n{output:?}");

    let svg = fs::read_to_string(&out).unwrap();
    assert!(svg.starts_with("<svg"), "a real SVG file:\n{svg}");
    assert!(
        svg.contains("dead code") && svg.contains('%'),
        "label and percentage present:\n{svg}"
    );
    assert!(
        !svg.contains(">0%<"),
        "a project with corpses is not at zero:\n{svg}"
    );
}

#[test]
fn a_clean_project_gets_a_green_zero() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let out = temp.path().join("badge.svg");

    let output = run(temp.path(), &out);
    assert!(output.status.success(), "badge failed:\n{output:?}");

    let svg = fs::read_to_string(&out).unwrap();
    assert!(svg.contains("0%"), "clean repo shows zero:\n{svg}");
    assert!(svg.contains("#4c1"), "zero is shields-green:\n{svg}");
}

#[test]
fn an_unwritable_path_is_a_clean_error() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let output = run(temp.path(), Path::new("/nonexistent/dir/badge.svg"));
    assert!(
        !output.status.success(),
        "unwritable path fails loudly, output was:\n{output:?}"
    );
}
