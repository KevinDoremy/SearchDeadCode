//! Integration tests for --ratchet: the baseline only accepts decrease.
//! New issues fail the run (exit 3); progress rewrites the baseline to
//! the smaller set automatically — the ceiling tightens with zero
//! discipline required from the team.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn write_two_corpses(dir: &Path) {
    fs::write(
        dir.join("ZombieOne.kt"),
        "package sample\n\nclass ZombieOne {\n    fun groan() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ZombieTwo.kt"),
        "package sample\n\nclass ZombieTwo {\n    fun moan() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap()
}

fn baseline_count(path: &Path) -> usize {
    let content = fs::read_to_string(path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    json["issues"].as_array().unwrap().len()
}

#[test]
fn progress_tightens_the_baseline() {
    let temp = tempfile::tempdir().unwrap();
    write_two_corpses(temp.path());
    let baseline = temp.path().join("baseline.json");
    let baseline_arg = baseline.to_string_lossy().to_string();

    let generated = run(temp.path(), &["--generate-baseline", &baseline_arg]);
    assert!(generated.status.success());
    let before = baseline_count(&baseline);
    assert!(before >= 2, "both zombies should be baselined");

    // One corpse gets buried
    fs::remove_file(temp.path().join("ZombieTwo.kt")).unwrap();

    let output = run(temp.path(), &["--baseline", &baseline_arg, "--ratchet"]);
    assert!(
        output.status.success(),
        "fewer issues than the ceiling must pass, output was:\n{output:?}"
    );
    let after = baseline_count(&baseline);
    assert!(
        after < before,
        "the ratchet must rewrite the baseline downward ({before} -> {after})"
    );
}

#[test]
fn a_new_corpse_fails_the_run() {
    let temp = tempfile::tempdir().unwrap();
    write_two_corpses(temp.path());
    let baseline = temp.path().join("baseline.json");
    let baseline_arg = baseline.to_string_lossy().to_string();

    run(temp.path(), &["--generate-baseline", &baseline_arg]);
    let before = baseline_count(&baseline);

    fs::write(
        temp.path().join("ZombieThree.kt"),
        "package sample\n\nclass ZombieThree {\n    fun wail() {}\n}\n",
    )
    .unwrap();

    let output = run(temp.path(), &["--baseline", &baseline_arg, "--ratchet"]);
    assert!(
        !output.status.success(),
        "a new issue over the ceiling must fail, output was:\n{output:?}"
    );
    assert_eq!(
        baseline_count(&baseline),
        before,
        "a failing run must never rewrite the baseline"
    );
}

#[test]
fn an_unchanged_project_passes_and_keeps_the_baseline() {
    let temp = tempfile::tempdir().unwrap();
    write_two_corpses(temp.path());
    let baseline = temp.path().join("baseline.json");
    let baseline_arg = baseline.to_string_lossy().to_string();

    run(temp.path(), &["--generate-baseline", &baseline_arg]);
    let before = fs::read_to_string(&baseline).unwrap();

    let output = run(temp.path(), &["--baseline", &baseline_arg, "--ratchet"]);
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(&baseline).unwrap(),
        before,
        "no progress, no rewrite"
    );
}

#[test]
fn a_corrupt_baseline_cannot_be_a_ceiling() {
    let temp = tempfile::tempdir().unwrap();
    write_two_corpses(temp.path());
    let baseline = temp.path().join("baseline.json");
    fs::write(&baseline, "{ not json at all").unwrap();
    let baseline_arg = baseline.to_string_lossy().to_string();

    let output = run(temp.path(), &["--baseline", &baseline_arg, "--ratchet"]);
    assert!(
        !output.status.success(),
        "an unreadable ceiling guards nothing — fail loudly, output was:\n{output:?}"
    );
}

#[test]
fn ratchet_without_a_baseline_is_an_error() {
    let temp = tempfile::tempdir().unwrap();
    write_two_corpses(temp.path());

    let output = run(temp.path(), &["--ratchet"]);
    assert!(
        !output.status.success(),
        "the ratchet needs a ceiling to guard, output was:\n{output:?}"
    );
}
