//! Integration tests for res/xml roots: preference screens and
//! shortcuts reference classes by fully qualified name (custom
//! preference tags, android:targetClass, app:fragment). A class whose
//! only reference lives there is alive — the graph just cannot see XML.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn main_kt(root: &Path) {
    write_file(
        root,
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );
}

#[test]
fn a_custom_preference_tag_keeps_the_class_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/ColorPicker.kt",
        "package sample.widgets\n\nclass ColorPicker {\n    fun pick() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/res/xml/preferences.xml",
        concat!(
            "<PreferenceScreen xmlns:android=\"http://schemas.android.com/apk/res/android\">\n",
            "    <sample.widgets.ColorPicker android:key=\"accent\"/>\n",
            "</PreferenceScreen>\n",
        ),
    );
    main_kt(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'ColorPicker'"),
        "a custom preference tag is a reference, stdout was:\n{stdout}"
    );
}

#[test]
fn a_target_class_attribute_keeps_the_class_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/JumpTarget.kt",
        "package sample.nav\n\nclass JumpTarget {\n    fun land() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/res/xml/shortcuts.xml",
        concat!(
            "<shortcuts xmlns:android=\"http://schemas.android.com/apk/res/android\">\n",
            "    <shortcut android:shortcutId=\"jump\">\n",
            "        <intent android:targetClass=\"sample.nav.JumpTarget\"/>\n",
            "    </shortcut>\n",
            "</shortcuts>\n",
        ),
    );
    main_kt(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'JumpTarget'"),
        "targetClass is a reference, stdout was:\n{stdout}"
    );
}

#[test]
fn an_app_fragment_attribute_keeps_the_class_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/PrefsScreen.kt",
        "package sample.ui\n\nclass PrefsScreen {\n    fun render() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/res/xml/root_preferences.xml",
        concat!(
            "<PreferenceScreen xmlns:app=\"http://schemas.android.com/apk/res-auto\">\n",
            "    <Preference app:fragment=\"sample.ui.PrefsScreen\"/>\n",
            "</PreferenceScreen>\n",
        ),
    );
    main_kt(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("'PrefsScreen'"),
        "app:fragment is a reference, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unreferenced_class_still_dies() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/GhostWidget.kt",
        "package sample.widgets\n\nclass GhostWidget {\n    fun haunt() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/res/xml/preferences.xml",
        concat!(
            "<PreferenceScreen xmlns:android=\"http://schemas.android.com/apk/res/android\">\n",
            "    <SwitchPreference android:key=\"dark\"/>\n",
            "</PreferenceScreen>\n",
        ),
    );
    main_kt(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("'GhostWidget'"),
        "standard tags retain nothing by accident, stdout was:\n{stdout}"
    );
}

#[test]
fn a_dangling_fqn_in_xml_is_harmless() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/res/xml/shortcuts.xml",
        concat!(
            "<shortcuts xmlns:android=\"http://schemas.android.com/apk/res/android\">\n",
            "    <intent android:targetClass=\"sample.gone.NeverExisted\"/>\n",
            "</shortcuts>\n",
        ),
    );
    main_kt(temp.path());

    let output = run(temp.path());
    assert!(
        output.status.success(),
        "an FQN pointing nowhere must not break the run, output was:\n{output:?}"
    );
}
