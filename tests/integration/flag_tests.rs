//! Integration tests for --flag/--behavior: what dies when a feature flag
//! is settled? The losing branch of every gate on that flag, plus the
//! symbols only that branch used.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    HomeRouter().open()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("HomeRouter.kt"),
        concat!(
            "package sample\n\n",
            "class HomeRouter {\n",
            "    fun open() {\n",
            "        if (isFlagEnabled(\"new_home\")) {\n",
            "            NewHomeScreen().show()\n",
            "            SharedBanner().show()\n",
            "        } else {\n",
            "            OldHomeScreen().show()\n",
            "            SharedBanner().show()\n",
            "        }\n",
            "    }\n\n",
            "    private fun isFlagEnabled(name: String): Boolean = true\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("NewHomeScreen.kt"),
        "package sample\n\nclass NewHomeScreen {\n    fun show() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("OldHomeScreen.kt"),
        "package sample\n\nclass OldHomeScreen {\n    fun show() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("SharedBanner.kt"),
        "package sample\n\nclass SharedBanner {\n    fun show() {}\n}\n",
    )
    .unwrap();
}

fn run_flag(dir: &Path, flag: &str, behavior: &str) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(["--flag", flag, "--behavior", behavior])
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn enabled_flag_kills_the_losing_branch_symbols() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_flag(temp.path(), "new_home", "enabled");

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("OldHomeScreen"),
        "the else-branch symbol dies when the flag is enabled, stdout was:\n{stdout}"
    );
}

#[test]
fn enabled_flag_spares_the_winning_branch() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_flag(temp.path(), "new_home", "enabled");

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("NewHomeScreen"),
        "the winning branch survives, stdout was:\n{stdout}"
    );
}

#[test]
fn symbols_used_by_both_branches_survive() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_flag(temp.path(), "new_home", "enabled");

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("SharedBanner"),
        "a symbol used by both branches is not dead, stdout was:\n{stdout}"
    );
}

#[test]
fn disabled_behavior_inverts_the_verdict() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run_flag(temp.path(), "new_home", "disabled");

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("NewHomeScreen") && !stdout.contains("OldHomeScreen"),
        "disabled kills the then-branch instead, stdout was:\n{stdout}"
    );
}

#[test]
fn boolean_flags_accept_their_bare_form() {
    // --deep alone errored with "a value is required" — hostile for a
    // flag whose only intent when typed bare is "turn it on"
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    for flag in ["--deep", "--parallel", "--incremental"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
            .arg(temp.path())
            .arg(flag)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "bare {flag} must mean 'on', stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn boolean_flags_keep_their_explicit_forms() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    for args in [
        ["--deep", "true"],
        ["--deep", "false"],
        ["--parallel", "false"],
        ["--incremental", "false"],
    ] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
            .arg(temp.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "explicit {args:?} stays valid, stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
