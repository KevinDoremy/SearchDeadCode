//! Integration tests for orphan translations: a key living in a locale
//! folder (values-fr) after being removed from the base values/ is
//! localization deadness — it can never be resolved.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::create_dir_all(dir.join("res/values")).unwrap();
    fs::create_dir_all(dir.join("res/values-fr")).unwrap();
    fs::write(
        dir.join("res/values/strings.xml"),
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n",
            "<resources>\n",
            "    <string name=\"app_name\">Sample</string>\n",
            "</resources>\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("res/values-fr/strings.xml"),
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n",
            "<resources>\n",
            "    <string name=\"app_name\">Exemple</string>\n",
            "    <string name=\"vieille_promo\">Promo retirée</string>\n",
            "</resources>\n",
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
fn a_locale_key_missing_from_base_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("vieille_promo"),
        "the base removed it, the locale kept it, stdout was:\n{stdout}"
    );
}

#[test]
fn a_key_present_in_both_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("'app_name'"),
        "translated and alive, stdout was:\n{stdout}"
    );
}

#[test]
fn orphan_translations_reach_json_as_dc022() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--format", "json"]));
    assert!(
        stdout.contains("vieille_promo") && stdout.contains("DC022"),
        "localization deadness is CI-visible, stdout was:\n{stdout}"
    );
}

#[test]
fn a_project_without_locale_folders_reports_nothing() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("res/values")).unwrap();
    fs::write(
        temp.path().join("res/values/strings.xml"),
        "<resources>\n    <string name=\"app_name\">Sample</string>\n</resources>\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(R.string.app_name)\n}\n\nobject R { object string { val app_name = 1 } }\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &["--format", "json"]));
    assert!(
        !stdout.contains("DC022"),
        "no locales, no orphans, stdout was:\n{stdout}"
    );
}
