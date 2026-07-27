//! Integration tests for --module-usage: who actually uses a shared module.
//!
//! For each outermost symbol of the module: unreferenced, internal-only
//! (visibility-narrowing candidate), or used by named consumer directories.
//! Ambiguous simple-name edges never create phantom consumers.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_monorepo(dir: &Path) {
    for sub in ["core", "appa", "appb"] {
        fs::create_dir_all(dir.join(sub)).unwrap();
    }
    fs::write(
        dir.join("core/SharedThing.kt"),
        "package core\n\nclass SharedThing {\n    fun serve() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("core/OnlyForB.kt"),
        "package core\n\nclass OnlyForB {\n    fun niche() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("core/InternalCog.kt"),
        "package core\n\nclass InternalCog {\n    fun turn() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("core/CoreHelper.kt"),
        "package core\n\nclass CoreHelper {\n    fun help() {\n        InternalCog().turn()\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("core/OrphanPart.kt"),
        "package core\n\nclass OrphanPart {\n    fun idle() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("appa/MainA.kt"),
        "package appa\n\nfun main() {\n    SharedThing().serve()\n    CoreHelper().help()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("appb/MainB.kt"),
        "package appb\n\nfun main() {\n    SharedThing().serve()\n    OnlyForB().niche()\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--module-usage", "core"])
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn line_of<'a>(stdout: &'a str, needle: &str) -> &'a str {
    stdout
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("{needle} absent from:\n{stdout}"))
}

#[test]
fn consumers_are_named_per_symbol() {
    let temp = tempfile::tempdir().unwrap();
    write_monorepo(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    let only_b = line_of(&stdout, "OnlyForB");
    assert!(
        only_b.contains("appb") && !only_b.contains("appa"),
        "OnlyForB is attributed to appb alone, line was: {only_b}"
    );
    let shared = line_of(&stdout, "SharedThing");
    assert!(
        shared.contains("appa") && shared.contains("appb"),
        "SharedThing is attributed to both apps, line was: {shared}"
    );
}

#[test]
fn internal_only_symbols_are_visibility_candidates() {
    let temp = tempfile::tempdir().unwrap();
    write_monorepo(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("internal"),
        "the internal-only section exists, stdout was:\n{stdout}"
    );
    let cog = line_of(&stdout, "InternalCog");
    assert!(
        !cog.contains("appa") && !cog.contains("appb"),
        "InternalCog has no external consumer, line was: {cog}"
    );
}

#[test]
fn unreferenced_symbols_are_grouped_apart() {
    let temp = tempfile::tempdir().unwrap();
    write_monorepo(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("Unreferenced") || stdout.contains("unreferenced"),
        "the unreferenced section exists, stdout was:\n{stdout}"
    );
    line_of(&stdout, "OrphanPart");
}

#[test]
fn ambiguous_name_matches_create_no_phantom_consumer() {
    let temp = tempfile::tempdir().unwrap();
    write_monorepo(temp.path());
    // appa's Panel has its own render(); calling it must not attribute
    // core's Widget to appa through the ambiguous simple-name edge.
    fs::write(
        temp.path().join("core/Widget.kt"),
        "package core\n\nclass Widget {\n    fun render() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("appa/Panel.kt"),
        "package appa\n\nclass Panel {\n    fun render() {}\n}\n\nfun usePanel() {\n    Panel().render()\n}\n",
    )
    .unwrap();

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    let widget = line_of(&stdout, "Widget");
    assert!(
        !widget.contains("appa"),
        "an ambiguous render() match must not invent a consumer, line was: {widget}"
    );
}
