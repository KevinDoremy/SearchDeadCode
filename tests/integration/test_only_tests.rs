//! Integration tests for --test-only: a src/main symbol whose every
//! reference lives in test source sets is production code only the
//! tests keep alive — delete both together. Reference-based, not the
//! naming heuristic deep.rs already applies.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--test-only")
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

#[test]
fn a_main_symbol_kept_only_by_tests_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/FixtureFactory.kt",
        "package sample\n\nclass FixtureFactory {\n    fun build() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/test/kotlin/SomeTest.kt",
        concat!(
            "package sample\n\n",
            "class SomeTest {\n",
            "    fun scenario() {\n",
            "        FixtureFactory().build()\n",
            "    }\n",
            "}\n",
        ),
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("FixtureFactory"),
        "only the tests keep it — production ships it for nothing, stdout was:\n{stdout}"
    );
}

#[test]
fn a_symbol_also_used_by_production_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/Mapper.kt",
        "package sample\n\nclass Mapper {\n    fun map() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    Mapper().map()\n",
            "}\n",
        ),
    );
    write_file(
        temp.path(),
        "src/test/kotlin/MapperTest.kt",
        concat!(
            "package sample\n\n",
            "class MapperTest {\n",
            "    fun check() {\n",
            "        Mapper().map()\n",
            "    }\n",
            "}\n",
        ),
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout
            .lines()
            .any(|l| l.contains("Mapper") && !l.contains("Test")),
        "tested AND used production code is healthy, stdout was:\n{stdout}"
    );
}

#[test]
fn a_debug_lifeline_is_not_mistaken_for_a_test_one() {
    // kept by src/debug — that is --debug-only territory, not test-only
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/Tracer.kt",
        "package sample\n\nclass Tracer {\n    fun trace() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/debug/kotlin/DebugHook.kt",
        "package sample\n\nfun hook() {\n    Tracer().trace()\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("Tracer"),
        "a debug lifeline is a different diagnosis, stdout was:\n{stdout}"
    );
}

#[test]
fn no_test_only_symbols_is_a_clean_answer() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(output.status.success());
    assert!(
        stdout.to_lowercase().contains("no test-only"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
