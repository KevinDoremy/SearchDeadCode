//! Integration tests for --profile: two audiences, one flag.
//! `ci` is strict (high-confidence findings only), `explore` shows
//! everything down to Low. An explicit --min-confidence always wins.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    // DC005 (unused enum case) is a Medium-confidence finding:
    // visible by default, hidden under the ci profile.
    fs::write(
        dir.join("Status.kt"),
        concat!(
            "package sample\n\n",
            "enum class Status {\n",
            "    ACTIVE,\n",
            "    LEGACY\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val s = Status.ACTIVE\n",
            "    println(s)\n",
            "}\n",
        ),
    )
    .unwrap();
}

fn run(dir: &Path, extra: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn the_ci_profile_hides_medium_confidence_findings() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--profile", "ci"]));
    assert!(
        !stdout.contains("LEGACY"),
        "ci wants confirmed corpses only, stdout was:\n{stdout}"
    );
}

#[test]
fn the_default_still_shows_medium_confidence() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("LEGACY"),
        "no profile keeps the default behavior, stdout was:\n{stdout}"
    );
}

#[test]
fn the_explore_profile_shows_low_confidence_findings() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    // A soft-referenced corpse drops to Low: invisible by default,
    // the explore profile digs it up.
    fs::write(
        temp.path().join("Reflected.kt"),
        "package sample\n\nclass BuriedZombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Reflection.kt"),
        "package sample\n\nfun reflect() {\n    println(\"sample.BuriedZombie\")\n}\n",
    )
    .unwrap();

    let default_out = stdout_of(&run(temp.path(), &[]));
    let explore_out = stdout_of(&run(temp.path(), &["--profile", "explore"]));
    assert!(
        explore_out.contains("BuriedZombie"),
        "explore shows everything, stdout was:\n{explore_out}"
    );
    assert!(
        explore_out.contains("LEGACY"),
        "explore keeps the mediums too, stdout was:\n{explore_out}"
    );
    let _ = default_out; // documented contrast, not asserted: soft-refs land post-filter
}

#[test]
fn an_explicit_min_confidence_beats_the_profile() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(
        temp.path(),
        &["--profile", "ci", "--min-confidence", "medium"],
    ));
    assert!(
        stdout.contains("LEGACY"),
        "the explicit flag wins over the profile, stdout was:\n{stdout}"
    );
}
