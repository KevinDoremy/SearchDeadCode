//! Integration tests for the opt-in --style trio.
//!
//! DC014 redundant `this.` (no shadowing forces it), DC015 doubled
//! parentheses around a condition, DC016 size/length compared to zero.
//! All three stay silent without --style: they are style, not deadness.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_style_project(dir: &Path) {
    fs::write(
        dir.join("Styles.kt"),
        concat!(
            "package sample\n\n",
            "class Account {\n",
            "    private var balance: Int = 0\n\n",
            "    fun deposit(amount: Int) {\n",
            "        this.balance = amount\n",
            "    }\n\n",
            "    fun reset(balance: Int) {\n",
            "        this.balance = balance\n",
            "    }\n\n",
            "    fun audit(entries: List<String>) {\n",
            "        val quiet = entries.isEmpty()\n",
            "        if ((quiet)) {\n",
            "            return\n",
            "        }\n",
            "        if (entries.size == 0) {\n",
            "            return\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val account = Account()\n",
            "    account.deposit(5)\n",
            "    account.reset(0)\n",
            "    account.audit(listOf())\n",
            "}\n",
        ),
    )
    .unwrap();
}

fn run_with_style(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--style")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_this_without_shadowing_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_style_project(temp.path());

    let stdout = stdout_of(&run_with_style(temp.path()));
    assert!(
        stdout.contains("DC014"),
        "this.balance = amount needs no this, stdout was:\n{stdout}"
    );
}

#[test]
fn a_this_disambiguating_a_parameter_is_kept() {
    let temp = tempfile::tempdir().unwrap();
    write_style_project(temp.path());

    let stdout = stdout_of(&run_with_style(temp.path()));
    let dc014_lines: Vec<&str> = stdout.lines().filter(|l| l.contains("[DC014]")).collect();
    assert!(
        dc014_lines.len() == 1,
        "reset(balance) shadows the field, only deposit() should fire, DC014 lines:\n{dc014_lines:?}"
    );
}

#[test]
fn doubled_parentheses_are_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_style_project(temp.path());

    let stdout = stdout_of(&run_with_style(temp.path()));
    assert!(
        stdout.contains("DC015"),
        "if ((...)) has one pair too many, stdout was:\n{stdout}"
    );
}

#[test]
fn size_compared_to_zero_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_style_project(temp.path());

    let stdout = stdout_of(&run_with_style(temp.path()));
    assert!(
        stdout.contains("DC016"),
        "entries.size == 0 should suggest isEmpty(), stdout was:\n{stdout}"
    );
}

#[test]
fn without_the_flag_style_stays_out_of_the_report() {
    let temp = tempfile::tempdir().unwrap();
    write_style_project(temp.path());

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .output()
        .unwrap();

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("DC014") && !stdout.contains("DC015") && !stdout.contains("DC016"),
        "style is opt-in, stdout was:\n{stdout}"
    );
}

#[test]
fn legitimate_comparisons_are_not_style_findings() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Fine.kt"),
        concat!(
            "package sample\n\n",
            "fun fine(entries: List<String>) {\n",
            "    if (entries.size == 3) return\n",
            "    if (entries.size >= 0) return\n",
            "    if ((entries.size == 3) && entries.isEmpty()) return\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run_with_style(temp.path()));
    assert!(
        !stdout.contains("DC015") && !stdout.contains("DC016"),
        "specific sizes and balanced parens are fine, stdout was:\n{stdout}"
    );
}
