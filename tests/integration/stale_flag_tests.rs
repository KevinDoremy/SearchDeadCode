//! Integration tests for --stale-flags: the Piranha inventory. Boolean
//! entries in remote_config_defaults.xml are feature flags; each one
//! read by the code is listed with its default and the ready-made
//! --flag command to see what dies under it.

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
            "        <key>welcome_text</key>\n",
            "        <value>Hello</value>\n",
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
            "    if (config.getBoolean(\"promo_enabled\")) {\n",
            "        println(\"promo\")\n",
            "    }\n",
            "    println(config.getString(\"welcome_text\"))\n",
            "}\n",
        ),
    )
    .unwrap();
}

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--stale-flags")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn boolean_flags_are_listed_with_their_default_and_a_next_step() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("promo_enabled") && stdout.contains("true"),
        "the flag and its default appear, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("--flag promo_enabled"),
        "the ready-made command to probe it appears, stdout was:\n{stdout}"
    );
}

#[test]
fn non_boolean_entries_are_not_flags() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("welcome_text"),
        "a string entry is configuration, not a flag, stdout was:\n{stdout}"
    );
}

#[test]
fn no_defaults_file_says_so() {
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
        "no defaults is a clean answer, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no remote_config_defaults"),
        "the user learns why the list is empty, stdout was:\n{stdout}"
    );
}
