//! Integration tests for --twins: Xxx/XxxV2, Xxx/XxxLegacy pairs
//! presented side by side with their reference counts — half of a
//! migration is usually one of these pairs waiting to die.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Parsers.kt"),
        concat!(
            "package sample\n\n",
            "class PayloadParser {\n    fun parse() {}\n}\n\n",
            "class PayloadParserV2 {\n    fun parse() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    PayloadParserV2().parse()\n",
            "}\n",
        ),
    )
    .unwrap();
}

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--twins")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_v2_pair_is_shown_side_by_side() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("PayloadParser") && stdout.contains("PayloadParserV2"),
        "both halves of the pair appear, stdout was:\n{stdout}"
    );
}

#[test]
fn the_dead_half_is_called_out() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.to_lowercase().contains("unreferenced"),
        "the side nobody calls is named as such, stdout was:\n{stdout}"
    );
}

#[test]
fn a_legacy_suffix_pairs_too() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Engines.kt"),
        concat!(
            "package sample\n\n",
            "class RenderEngine {\n    fun draw() {}\n}\n\n",
            "class RenderEngineLegacy {\n    fun draw() {}\n}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    RenderEngine().draw()\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("RenderEngineLegacy"),
        "the Legacy suffix pairs as well, stdout was:\n{stdout}"
    );
}

#[test]
fn no_pairs_is_a_clean_answer() {
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
        "no twins is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no version twins"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
