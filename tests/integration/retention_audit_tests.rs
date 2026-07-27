//! Integration tests for --retention-audit: which annotations keep how
//! many declarations alive. The legacy usually survives through @Inject
//! and @Subscribe — the audit shows where retention is broad enough to
//! deserve refining.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Screens.kt"),
        concat!(
            "package sample\n\n",
            "@Preview\nfun ScreenA() {}\n\n",
            "@Preview\nfun ScreenB() {}\n\n",
            "@Preview\nfun ScreenC() {}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Bus.kt"),
        concat!(
            "package sample\n\n",
            "class Listener {\n",
            "    @Subscribe\n",
            "    fun onEvent(e: Any) {}\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--retention-audit")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn retained_annotations_are_counted_and_sorted() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    let preview = stdout.find("Preview");
    let subscribe = stdout.find("Subscribe");
    assert!(
        preview.is_some() && subscribe.is_some(),
        "both retainers appear, stdout was:\n{stdout}"
    );
    assert!(
        preview < subscribe,
        "Preview retains more, it ranks first, stdout was:\n{stdout}"
    );
}

#[test]
fn a_broad_retainer_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("broad"),
        "Preview holds a large share of declarations, stdout was:\n{stdout}"
    );
}

#[test]
fn a_project_without_retained_annotations_says_so() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "nothing retained is a fine answer, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no annotation"),
        "the audit says nothing is annotation-retained, stdout was:\n{stdout}"
    );
}
