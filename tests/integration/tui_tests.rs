//! Integration test for --tui: assert_cmd runs are never a TTY, so the
//! flag must fall back to the standard report instead of hanging on a
//! terminal that is not there. The full-screen loop itself is covered
//! by the unit tests against ratatui's TestBackend.

use std::fs;

#[test]
fn non_tty_falls_back_to_the_report() {
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

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .arg("--tui")
        .output()
        .unwrap();
    assert!(out.status.success(), "fallback run failed:\n{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("requires a terminal"),
        "the fallback is explained, stderr was:\n{stderr}"
    );
    assert!(
        stdout.contains("'Ghost'"),
        "the standard report still comes out, stdout was:\n{stdout}"
    );
}
