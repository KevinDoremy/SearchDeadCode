//! Integration tests for --patch: a unified diff of what --delete
//! would remove, reviewable and applicable with git apply — deletion
//! as a code review artifact instead of a leap of faith.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Zombie.kt"),
        "package sample\n\nclass Zombie {\n    fun groan() {}\n}\n",
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

#[test]
fn the_patch_file_describes_the_deletion() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let patch = temp.path().join("out.diff");

    let output = run(
        temp.path(),
        &["--delete", "--dry-run", "--patch", patch.to_str().unwrap()],
    );
    assert!(output.status.success(), "output was:\n{output:?}");
    let content = fs::read_to_string(&patch).expect("patch written");
    assert!(
        content.contains("--- a/Zombie.kt") && content.contains("-class Zombie {"),
        "the doomed lines appear as removals, patch was:\n{content}"
    );
    assert!(
        !content.contains("Main.kt"),
        "healthy files stay out of the patch, patch was:\n{content}"
    );
}

#[test]
fn git_apply_accepts_the_patch() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let patch = temp.path().join("out.diff");

    let output = run(
        temp.path(),
        &["--delete", "--dry-run", "--patch", patch.to_str().unwrap()],
    );
    assert!(output.status.success());

    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(args)
            .output()
            .unwrap()
    };
    git(&["init", "--quiet"]);
    let applied = git(&["apply", patch.to_str().unwrap()]);
    assert!(
        applied.status.success(),
        "git apply must accept the patch, stderr was:\n{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let after = fs::read_to_string(temp.path().join("Zombie.kt")).unwrap();
    assert!(
        !after.contains("class Zombie"),
        "applying the patch buries the corpse, file was:\n{after}"
    );
}

#[test]
fn patch_without_dry_run_is_a_config_error() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let patch = temp.path().join("out.diff");

    let output = run(temp.path(), &["--patch", patch.to_str().unwrap()]);
    assert!(
        !output.status.success(),
        "--patch describes a dry-run, it needs one, output was:\n{output:?}"
    );
}
