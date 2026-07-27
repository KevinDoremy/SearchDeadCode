//! Integration tests: out-of-graph findings must reach JSON and SARIF.
//!
//! Unused resources and dead layouts used to print as ad-hoc terminal
//! sections and vanish from --format json/sarif and from baselines —
//! invisible to CI. They now flow through the standard report as DC017
//! and DC018.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_android_project(dir: &Path) {
    fs::create_dir_all(dir.join("res/values")).unwrap();
    fs::create_dir_all(dir.join("res/layout")).unwrap();
    fs::write(
        dir.join("res/values/strings.xml"),
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n",
            "<resources>\n",
            "    <string name=\"app_name\">Sample</string>\n",
            "    <string name=\"forgotten_toast\">Bye</string>\n",
            "</resources>\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("res/layout/ghost_screen.xml"),
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n",
            "<FrameLayout xmlns:android=\"http://schemas.android.com/apk/res/android\"\n",
            "    android:layout_width=\"match_parent\"\n",
            "    android:layout_height=\"match_parent\" />\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    println(R.string.app_name)\n",
            "}\n\n",
            "object R { object string { val app_name = 1 } }\n",
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
fn an_unused_resource_reaches_json() {
    let temp = tempfile::tempdir().unwrap();
    write_android_project(temp.path());

    let stdout = stdout_of(&run(
        temp.path(),
        &["--unused-resources", "--format", "json"],
    ));
    assert!(
        stdout.contains("forgotten_toast") && stdout.contains("DC017"),
        "the dead string must be a JSON finding, stdout was:\n{stdout}"
    );
}

#[test]
fn a_used_resource_stays_out_of_json() {
    let temp = tempfile::tempdir().unwrap();
    write_android_project(temp.path());

    let stdout = stdout_of(&run(
        temp.path(),
        &["--unused-resources", "--format", "json"],
    ));
    assert!(
        !stdout.contains("app_name"),
        "app_name is referenced, stdout was:\n{stdout}"
    );
}

#[test]
fn a_dead_layout_reaches_sarif() {
    let temp = tempfile::tempdir().unwrap();
    write_android_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--format", "sarif"]));
    assert!(
        stdout.contains("ghost_screen") && stdout.contains("DC018"),
        "the orphan layout must be a SARIF result, stdout was:\n{stdout}"
    );
}

#[test]
fn terminal_mode_still_reports_them() {
    let temp = tempfile::tempdir().unwrap();
    write_android_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--unused-resources"]));
    assert!(
        stdout.contains("forgotten_toast") && stdout.contains("ghost_screen"),
        "terminal keeps showing both findings, stdout was:\n{stdout}"
    );
}
