//! Integration tests for --necromancy: mid-migration, someone adding a
//! reference to a symbol already judged dead (baselined) is resurrecting
//! legacy instead of using the new world. The guard names the corpse
//! and its necromancer, and fails the run for CI gating.

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

/// Dead class + baseline recording it as dead.
fn seeded(temp: &Path) -> std::path::PathBuf {
    fs::write(
        temp.join("DeadThing.kt"),
        "package sample\n\nclass DeadThing {\n    fun rot() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.join("baseline.json");
    let out = bin(temp, &["--generate-baseline", baseline.to_str().unwrap()]);
    assert!(out.status.success(), "seed failed:\n{out:?}");
    baseline
}

#[test]
fn a_new_reference_to_a_dead_symbol_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded(temp.path());

    // someone resurrects the corpse
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    DeadThing().rot()\n",
            "}\n",
        ),
    )
    .unwrap();

    let out = bin(
        temp.path(),
        &["--baseline", baseline.to_str().unwrap(), "--necromancy"],
    );
    let stdout = stdout_of(&out);
    assert!(
        !out.status.success(),
        "necromancy must gate the CI, output was:\n{out:?}"
    );
    assert!(
        stdout.contains("DeadThing") && stdout.contains("Main"),
        "corpse and necromancer are both named, stdout was:\n{stdout}"
    );
}

#[test]
fn untouched_dead_symbols_stay_quiet() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded(temp.path());

    let out = bin(
        temp.path(),
        &["--baseline", baseline.to_str().unwrap(), "--necromancy"],
    );
    let stdout = stdout_of(&out);
    assert!(
        out.status.success(),
        "still-dead code is not necromancy, output was:\n{out:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no resurrection"),
        "the clean verdict is explicit, stdout was:\n{stdout}"
    );
}

#[test]
fn a_vanished_baseline_entry_is_fine() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded(temp.path());
    // the corpse got properly buried since the baseline
    fs::remove_file(temp.path().join("DeadThing.kt")).unwrap();

    let out = bin(
        temp.path(),
        &["--baseline", baseline.to_str().unwrap(), "--necromancy"],
    );
    assert!(
        out.status.success(),
        "a deleted symbol cannot be resurrected, output was:\n{out:?}"
    );
}

#[test]
fn necromancy_without_baseline_errors_out() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--necromancy"]);
    assert!(
        !out.status.success(),
        "--necromancy needs --baseline, output was:\n{out:?}"
    );
}
