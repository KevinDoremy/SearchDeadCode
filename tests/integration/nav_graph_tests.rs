//! End-to-end hardening for navigation graphs as retention roots:
//! fragments referenced by FQN strings, dot-relative names, nested
//! <navigation> blocks and Parcelable argTypes must all stay alive —
//! and a fragment in no graph at all must stay dead.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_base(dir: &Path) {
    fs::create_dir_all(dir.join("res/navigation")).unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package com.app\n\nfun main() {\n    println(\"alive\")\n}\n",
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
fn a_fragment_referenced_by_fqn_stays_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_base(temp.path());
    fs::write(
        temp.path().join("HomeFragment.kt"),
        "package com.app.ui\n\nclass HomeFragment {\n    fun bind() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("res/navigation/nav_main.xml"),
        concat!(
            "<navigation xmlns:android=\"http://schemas.android.com/apk/res/android\">\n",
            "    <fragment android:id=\"@+id/home\" android:name=\"com.app.ui.HomeFragment\" />\n",
            "</navigation>\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'HomeFragment'"),
        "the nav graph is a root, stdout was:\n{stdout}"
    );
}

#[test]
fn a_dot_relative_name_stays_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_base(temp.path());
    fs::write(
        temp.path().join("ProfileFragment.kt"),
        "package com.app.ui\n\nclass ProfileFragment {\n    fun bind() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("res/navigation/nav_profile.xml"),
        concat!(
            "<navigation xmlns:android=\"http://schemas.android.com/apk/res/android\">\n",
            "    <fragment android:id=\"@+id/profile\" android:name=\".ui.ProfileFragment\" />\n",
            "</navigation>\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'ProfileFragment'"),
        "dot-relative names are how real graphs are written, stdout was:\n{stdout}"
    );
}

#[test]
fn a_fragment_in_a_nested_navigation_stays_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_base(temp.path());
    fs::write(
        temp.path().join("DeepFragment.kt"),
        "package com.app.deep\n\nclass DeepFragment {\n    fun bind() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("res/navigation/nav_nested.xml"),
        concat!(
            "<navigation xmlns:android=\"http://schemas.android.com/apk/res/android\">\n",
            "    <navigation android:id=\"@+id/inner_graph\">\n",
            "        <fragment android:id=\"@+id/deep\" android:name=\"com.app.deep.DeepFragment\" />\n",
            "    </navigation>\n",
            "</navigation>\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'DeepFragment'"),
        "nesting must not hide destinations, stdout was:\n{stdout}"
    );
}

#[test]
fn a_parcelable_arg_type_stays_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_base(temp.path());
    fs::write(
        temp.path().join("UserArg.kt"),
        "package com.app.model\n\nclass UserArg {\n    fun payload() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("res/navigation/nav_args.xml"),
        concat!(
            "<navigation xmlns:android=\"http://schemas.android.com/apk/res/android\"\n",
            "    xmlns:app=\"http://schemas.android.com/apk/res-auto\">\n",
            "    <fragment android:id=\"@+id/detail\" android:name=\"com.app.Missing\">\n",
            "        <argument android:name=\"user\" app:argType=\"com.app.model.UserArg\" />\n",
            "    </fragment>\n",
            "</navigation>\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'UserArg'"),
        "argTypes travel through bundles, stdout was:\n{stdout}"
    );
}

#[test]
fn a_plain_class_in_no_graph_stays_dead() {
    let temp = tempfile::tempdir().unwrap();
    write_base(temp.path());
    fs::write(
        temp.path().join("OrphanScreen.kt"),
        "package com.app.ui\n\nclass OrphanScreen {\n    fun bind() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("res/navigation/nav_other.xml"),
        concat!(
            "<navigation xmlns:android=\"http://schemas.android.com/apk/res/android\">\n",
            "    <fragment android:id=\"@+id/x\" android:name=\"com.app.Other\" />\n",
            "</navigation>\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("'OrphanScreen'"),
        "no graph names it and no framework suffix protects it, stdout was:\n{stdout}"
    );
}
