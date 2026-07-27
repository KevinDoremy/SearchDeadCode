//! Integration tests for dead Remote Config keys: entries declared in
//! remote_config_defaults.xml that no source literal ever reads are
//! configuration deadness nobody ever sees.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::create_dir_all(dir.join("res/xml")).unwrap();
    fs::write(
        dir.join("res/xml/remote_config_defaults.xml"),
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n",
            "<defaultsMap>\n",
            "    <entry>\n",
            "        <key>promo_enabled</key>\n",
            "        <value>true</value>\n",
            "    </entry>\n",
            "    <entry>\n",
            "        <key>forgotten_toggle</key>\n",
            "        <value>false</value>\n",
            "    </entry>\n",
            "</defaultsMap>\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main(config: Config) {\n",
            "    println(config.getBoolean(\"promo_enabled\"))\n",
            "}\n",
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
fn an_unread_key_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        stdout.contains("forgotten_toggle"),
        "no source literal ever reads it, stdout was:\n{stdout}"
    );
}

#[test]
fn a_read_key_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("promo_enabled"),
        "getBoolean(\"promo_enabled\") is a read, stdout was:\n{stdout}"
    );
}

#[test]
fn a_key_read_through_a_constant_stays_silent() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(
        temp.path().join("Keys.kt"),
        concat!(
            "package sample\n\n",
            "object Keys {\n",
            "    const val OLD = \"forgotten_toggle\"\n",
            "}\n\n",
            "fun readOld(config: Config) {\n",
            "    println(config.getBoolean(Keys.OLD))\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path(), &[]));
    assert!(
        !stdout.contains("forgotten_toggle"),
        "the literal lives in a constant — that is a read, stdout was:\n{stdout}"
    );
}

#[test]
fn no_defaults_file_reports_nothing() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path(), &[]);
    assert!(
        output.status.success(),
        "no defaults, nothing to check, output was:\n{output:?}"
    );
}

#[test]
fn dead_keys_reach_json_as_findings() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path(), &["--format", "json"]));
    assert!(
        stdout.contains("forgotten_toggle") && stdout.contains("DC020"),
        "configuration deadness is CI-visible, stdout was:\n{stdout}"
    );
}
