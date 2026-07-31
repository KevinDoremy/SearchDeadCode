//! Integration tests for the DC010 unprovability guard.
//!
//! A preference wrapper with parameterized keys reads keys no scan can
//! enumerate: every write-only verdict becomes a guess. The detector says
//! so instead of guessing, and constant keys unify with their literals.

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

#[test]
fn a_parameterized_wrapper_makes_write_only_unprovable() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("PreferenceService.kt"),
        concat!(
            "package sample\n\n",
            "class PreferenceService(private val prefs: SharedPreferences) {\n",
            "    fun save() {\n",
            "        prefs.edit().putString(\"maybe_orphan\", \"v\").apply()\n",
            "    }\n\n",
            "    fun read(key: String): String? {\n",
            "        return prefs.getString(key, null)\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"app\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("\"maybe_orphan\" is written but never read"),
        "un wrapper à clés paramétrées rend le verdict non prouvable, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("unprovable"),
        "le silence est expliqué, pas muet, stdout was:\n{stdout}"
    );
}

#[test]
fn a_constant_write_and_literal_read_are_the_same_key() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Auth.kt"),
        concat!(
            "package sample\n\n",
            "const val KEY_TOKEN = \"auth_token\"\n\n",
            "fun save(prefs: SharedPreferences, t: String) {\n",
            "    prefs.edit().putString(KEY_TOKEN, t).apply()\n",
            "}\n\n",
            "fun load(prefs: SharedPreferences): String? {\n",
            "    return prefs.getString(\"auth_token\", null)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"app\")\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("written but never read"),
        "la constante résolue rejoint sa lecture littérale, stdout was:\n{stdout}"
    );
}
