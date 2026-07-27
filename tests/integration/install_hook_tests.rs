//! Integration tests for --install-hook: a packaged pre-commit hook
//! that runs the fast diff mode before each commit. Installing by hand
//! is exactly the friction that keeps hooks uninstalled.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn bin(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

fn init_repo(dir: &Path) {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["init", "--quiet"])
        .output()
        .unwrap();
}

#[test]
fn install_hook_writes_an_executable_pre_commit() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();
    init_repo(temp.path());

    let out = bin(temp.path(), &["--install-hook"]);
    assert!(out.status.success(), "install failed:\n{out:?}");

    let hook = temp.path().join(".git/hooks/pre-commit");
    let content = fs::read_to_string(&hook).expect("hook file exists");
    assert!(
        content.contains("searchdeadcode"),
        "the hook invokes the tool, content was:\n{content}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&hook).unwrap().permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "the hook is executable, mode was {mode:o}"
        );
    }
}

#[test]
fn install_hook_refuses_to_overwrite_a_foreign_hook() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();
    init_repo(temp.path());
    let hook = temp.path().join(".git/hooks/pre-commit");
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(&hook, "#!/bin/sh\necho custom hook\n").unwrap();

    let out = bin(temp.path(), &["--install-hook"]);
    assert!(
        !out.status.success(),
        "a foreign hook must not be clobbered, output was:\n{out:?}"
    );
    let content = fs::read_to_string(&hook).unwrap();
    assert!(
        content.contains("custom hook"),
        "the existing hook survives, content was:\n{content}"
    );
}

#[test]
fn reinstalling_our_own_hook_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();
    init_repo(temp.path());

    let first = bin(temp.path(), &["--install-hook"]);
    assert!(first.status.success(), "first install failed:\n{first:?}");
    let second = bin(temp.path(), &["--install-hook"]);
    assert!(
        second.status.success(),
        "reinstalling our own hook is fine, output was:\n{second:?}"
    );
}

#[test]
fn install_hook_outside_a_git_repo_fails_cleanly() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--install-hook"]);
    assert!(
        !out.status.success(),
        "no repo, no hook, output was:\n{out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("git"),
        "the error names the missing repo, stderr was:\n{stderr}"
    );
}
