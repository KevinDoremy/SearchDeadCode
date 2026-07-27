//! Integration tests for baseline management: show/rm/prune from the
//! CLI instead of hand-editing the JSON. The baseline is a daily tool,
//! not a write-once artifact.

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

/// A project with two dead classes, baseline generated for both.
fn seeded_project(temp: &Path) -> std::path::PathBuf {
    fs::write(
        temp.join("DeadThing.kt"),
        "package sample\n\nclass DeadThing {\n    fun rot() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.join("DeadStar.kt"),
        "package sample\n\nclass DeadStar {\n    fun shine() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.join("baseline.json");
    let out = bin(temp, &["--generate-baseline", baseline.to_str().unwrap()]);
    assert!(out.status.success(), "seed run failed:\n{out:?}");
    baseline
}

#[test]
fn show_lists_the_baselined_findings() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded_project(temp.path());

    let out = bin(
        temp.path(),
        &["--baseline", baseline.to_str().unwrap(), "--baseline-show"],
    );
    let stdout = stdout_of(&out);
    assert!(out.status.success(), "show failed:\n{out:?}");
    assert!(
        stdout.contains("DeadThing") && stdout.contains("DeadStar"),
        "every baselined entry is listed, stdout was:\n{stdout}"
    );
}

#[test]
fn rm_removes_a_named_entry_and_rewrites_the_file() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded_project(temp.path());

    let out = bin(
        temp.path(),
        &[
            "--baseline",
            baseline.to_str().unwrap(),
            "--baseline-rm",
            "DeadThing",
        ],
    );
    assert!(out.status.success(), "rm failed:\n{out:?}");

    let json = fs::read_to_string(&baseline).unwrap();
    assert!(
        !json.contains("DeadThing"),
        "the entry is gone from the file, json was:\n{json}"
    );
    assert!(
        json.contains("DeadStar"),
        "the other entry survives, json was:\n{json}"
    );
}

#[test]
fn rm_with_an_unknown_name_leaves_the_file_intact() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded_project(temp.path());
    let before = fs::read_to_string(&baseline).unwrap();

    let out = bin(
        temp.path(),
        &[
            "--baseline",
            baseline.to_str().unwrap(),
            "--baseline-rm",
            "NeverExisted",
        ],
    );
    let stdout = stdout_of(&out);
    assert!(
        stdout.to_lowercase().contains("no entry"),
        "the miss is explicit, stdout was:\n{stdout}"
    );
    let after = fs::read_to_string(&baseline).unwrap();
    assert_eq!(before, after, "a miss must not rewrite the file");
}

#[test]
fn prune_drops_entries_whose_finding_is_resolved() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded_project(temp.path());

    // DeadThing gets resurrected: its baseline entry is now stale
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
        &["--baseline", baseline.to_str().unwrap(), "--baseline-prune"],
    );
    let stdout = stdout_of(&out);
    assert!(out.status.success(), "prune failed:\n{out:?}");
    assert!(
        stdout.to_lowercase().contains("pruned"),
        "prune reports what it dropped, stdout was:\n{stdout}"
    );

    let json = fs::read_to_string(&baseline).unwrap();
    assert!(
        !json.contains("DeadThing"),
        "the resolved entry is pruned, json was:\n{json}"
    );
    assert!(
        json.contains("DeadStar"),
        "the still-dead entry survives, json was:\n{json}"
    );
}

#[test]
fn rm_accepts_a_fully_qualified_name() {
    let temp = tempfile::tempdir().unwrap();
    let baseline = seeded_project(temp.path());

    let out = bin(
        temp.path(),
        &[
            "--baseline",
            baseline.to_str().unwrap(),
            "--baseline-rm",
            "sample.DeadStar",
        ],
    );
    assert!(out.status.success(), "rm by fqn failed:\n{out:?}");

    let json = fs::read_to_string(&baseline).unwrap();
    assert!(
        !json.contains("DeadStar"),
        "the fqn-addressed entry is gone, json was:\n{json}"
    );
    assert!(
        json.contains("DeadThing"),
        "the other entry survives, json was:\n{json}"
    );
}

#[test]
fn management_flags_without_baseline_error_out() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    for flag in ["--baseline-show", "--baseline-prune"] {
        let out = bin(temp.path(), &[flag]);
        assert!(
            !out.status.success(),
            "{flag} without --baseline must fail, output was:\n{out:?}"
        );
    }
}

#[test]
fn show_on_a_corrupt_baseline_fails_cleanly() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");
    fs::write(&baseline, "{ not json").unwrap();

    let out = bin(
        temp.path(),
        &["--baseline", baseline.to_str().unwrap(), "--baseline-show"],
    );
    assert!(
        !out.status.success(),
        "corrupt baseline is an error, not a shrug, output was:\n{out:?}"
    );
}
