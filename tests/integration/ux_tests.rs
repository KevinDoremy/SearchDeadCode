//! Integration tests for the CLI user experience: clean output streams,
//! first-contact guidance, and contextual next steps.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    UsedHelper().greet()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("UsedHelper.kt"),
        "package sample\n\nclass UsedHelper {\n    fun greet() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ObsoleteWidget.kt"),
        "package sample\n\nclass ObsoleteWidget {\n    fun render() {}\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path, extra_args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra_args)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn missing_config_suggests_init() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &[]);

    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("--init"),
        "a project without config gets pointed at --init, stderr was:\n{stderr}"
    );
}

#[test]
fn empty_project_explains_what_was_searched() {
    let temp = tempfile::tempdir().unwrap();

    let output = run(temp.path(), &[]);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("No Kotlin or Java files found"),
        "the empty case is stated plainly, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains(temp.path().to_str().unwrap()),
        "the searched path is shown so the user can spot a typo, stdout was:\n{stdout}"
    );
}

#[test]
fn report_footer_suggests_next_steps() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &[]);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("--explain") && stdout.contains("--clusters"),
        "a report with findings guides the user to the next move, stdout was:\n{stdout}"
    );
}

#[test]
fn default_report_annotates_findings_with_source() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &[]);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("class ObsoleteWidget"),
        "the offending source line is shown, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("^^^"),
        "the symbol is underlined rustc-style, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("= help: searchdeadcode --explain"),
        "each finding carries its own next step, stdout was:\n{stdout}"
    );
}

#[test]
fn compact_report_stays_one_line_per_finding() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &["--compact"]);

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("^^^"),
        "--compact keeps the dense view, stdout was:\n{stdout}"
    );
}

#[test]
fn big_reports_skip_annotations_to_stay_readable() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();
    let mut many = String::from("package sample\n\n");
    for i in 0..25 {
        many.push_str(&format!(
            "class DeadThing{i} {{\n    fun poke() {{}}\n}}\n\n"
        ));
    }
    fs::write(temp.path().join("ManyDead.kt"), many).unwrap();

    let output = run(temp.path(), &[]);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("DeadThing0"),
        "findings are still listed, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("^^^"),
        "less is more: big reports keep one line per finding, stdout was:\n{stdout}"
    );
}

#[test]
fn report_paths_are_relative_to_the_project() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &[]);

    let stdout = stdout_of(&output);
    let header = stdout
        .lines()
        .find(|l| l.trim_end().ends_with("ObsoleteWidget.kt"))
        .expect("a file header names ObsoleteWidget.kt");
    assert!(
        !header.contains(temp.path().to_str().unwrap()),
        "file headers are relative to the analyzed root, header was:\n{header}"
    );
}

#[test]
fn progress_renders_as_aligned_phase_lines() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &[]);

    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("✓ parsed"),
        "phases render as checked lines, stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("✓ analysis"),
        "the analysis phase is a checked line too, stderr was:\n{stderr}"
    );
    assert!(
        !stderr.contains("Deep mode: aggressive"),
        "the old banner style is gone, stderr was:\n{stderr}"
    );
}

#[test]
fn healthy_project_gets_one_quiet_line() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    UsedHelper().greet()\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("UsedHelper.kt"),
        "package sample\n\nclass UsedHelper {\n    fun greet() {}\n}\n",
    )
    .unwrap();

    let output = run(temp.path(), &[]);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("No dead code found"),
        "the happy case is stated, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("Analysis Summary"),
        "a healthy project needs no summary block, stdout was:\n{stdout}"
    );
    assert!(
        stdout.lines().filter(|l| !l.trim().is_empty()).count() <= 3,
        "less is more: a clean run fits in three lines, stdout was:\n{stdout}"
    );
}

#[test]
fn json_on_stdout_is_pure_json() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());

    let output = run(temp.path(), &["--format", "json"]);

    let stdout = stdout_of(&output);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        parsed.is_ok(),
        "stdout must be pipeable JSON, logs belong on stderr; stdout was:\n{stdout}"
    );
}
