//! Integration tests for dead layout detection (ViewBinding-aware).
//!
//! A layout is dead when nothing uses its generated Binding class, nothing
//! inflates it via R.layout.<name>, and no other layout includes it.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    let layouts = dir.join("res/layout");
    fs::create_dir_all(&layouts).unwrap();
    fs::write(
        layouts.join("used_screen.xml"),
        "<LinearLayout xmlns:android=\"http://schemas.android.com/apk/res/android\" />",
    )
    .unwrap();
    fs::write(
        layouts.join("inflated_screen.xml"),
        "<FrameLayout xmlns:android=\"http://schemas.android.com/apk/res/android\" />",
    )
    .unwrap();
    fs::write(
        layouts.join("part_header.xml"),
        "<TextView xmlns:android=\"http://schemas.android.com/apk/res/android\" />",
    )
    .unwrap();
    fs::write(
        layouts.join("host_screen.xml"),
        "<LinearLayout xmlns:android=\"http://schemas.android.com/apk/res/android\">\n    <include layout=\"@layout/part_header\" />\n</LinearLayout>",
    )
    .unwrap();
    fs::write(
        layouts.join("dead_screen.xml"),
        "<LinearLayout xmlns:android=\"http://schemas.android.com/apk/res/android\" />",
    )
    .unwrap();

    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    val binding = UsedScreenBinding.inflate()\n",
            "    render(binding)\n",
            "    inflate(R.layout.inflated_screen)\n",
            "    HostScreenBinding.inflate()\n",
            "}\n\n",
            "fun render(b: Any) {}\n",
            "fun inflate(id: Int) {}\n",
        ),
    )
    .unwrap();
}

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn layout_without_binding_usage_or_inflate_is_dead() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("Unused layouts"),
        "the section names itself, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("dead_screen.xml"),
        "the dead layout is listed, stdout was:\n{stdout}"
    );
}

#[test]
fn layout_used_through_its_binding_survives() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("used_screen.xml"),
        "UsedScreenBinding usage keeps the layout alive, stdout was:\n{stdout}"
    );
}

#[test]
fn layout_inflated_via_r_layout_survives() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("inflated_screen.xml"),
        "R.layout.inflated_screen keeps it alive, stdout was:\n{stdout}"
    );
}

#[test]
fn a_dead_layout_is_not_shadowed_by_a_prefixed_sibling() {
    let temp = tempfile::tempdir().unwrap();
    let layouts = temp.path().join("res/layout");
    fs::create_dir_all(&layouts).unwrap();
    fs::write(
        layouts.join("screen.xml"),
        "<LinearLayout xmlns:android=\"http://schemas.android.com/apk/res/android\" />",
    )
    .unwrap();
    fs::write(
        layouts.join("screen_v2.xml"),
        "<LinearLayout xmlns:android=\"http://schemas.android.com/apk/res/android\" />",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    inflate(R.layout.screen_v2)\n}\n\nfun inflate(id: Int) {}\n",
    )
    .unwrap();

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("screen.xml"),
        "R.layout.screen_v2 must not shadow the dead screen.xml, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("screen_v2.xml"),
        "the inflated sibling stays alive, stdout was:\n{stdout}"
    );
}

#[test]
fn layout_included_by_a_live_layout_survives() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("part_header.xml"),
        "an <include> from a live layout keeps it alive, stdout was:\n{stdout}"
    );
}
