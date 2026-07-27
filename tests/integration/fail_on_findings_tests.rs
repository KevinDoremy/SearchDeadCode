//! Integration tests for --fail-on-findings: granular exit codes make
//! the tool scriptable without parsing output. 0 = clean, 1 = findings
//! (this flag), 2 = config error, 3 = ratchet/necromancy gate.

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

#[test]
fn findings_exit_one_with_the_flag() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--fail-on-findings"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "findings + flag = exit 1, output was:\n{out:?}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        stdout.contains("'Ghost'"),
        "the report still prints before failing, stdout was:\n{stdout}"
    );
}

#[test]
fn a_clean_project_exits_zero_with_the_flag() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--fail-on-findings"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "clean + flag = exit 0, output was:\n{out:?}"
    );
}

#[test]
fn without_the_flag_findings_still_exit_zero() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "the default stays non-breaking, output was:\n{out:?}"
    );
}

#[test]
fn baselined_findings_do_not_fail_the_gate() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let baseline = temp.path().join("baseline.json");
    let seed = bin(
        temp.path(),
        &["--generate-baseline", baseline.to_str().unwrap()],
    );
    assert!(seed.status.success());

    let out = bin(
        temp.path(),
        &[
            "--baseline",
            baseline.to_str().unwrap(),
            "--fail-on-findings",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "the gate judges NEW findings only, output was:\n{out:?}"
    );
}
