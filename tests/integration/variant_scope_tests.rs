//! Integration tests for --debug-only: a src/main symbol whose every
//! reference lives in another source set (debug, a flavor, tests) is
//! alive in that variant only — in the release build it is dead weight
//! the standard report cannot see.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--debug-only")
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
fn a_main_symbol_used_only_from_debug_is_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/Tracker.kt",
        "package sample\n\nclass Tracker {\n    fun track() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/debug/kotlin/DebugHook.kt",
        "package sample\n\nfun hook() {\n    Tracker().track()\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("Tracker") && stdout.contains("debug"),
        "the debug-only lifeline is named, stdout was:\n{stdout}"
    );
}

#[test]
fn a_main_symbol_used_from_main_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/Tracker.kt",
        "package sample\n\nclass Tracker {\n    fun track() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    Tracker().track()\n}\n",
    );
    write_file(
        temp.path(),
        "src/debug/kotlin/DebugHook.kt",
        "package sample\n\nfun hook() {\n    Tracker().track()\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("Tracker"),
        "a main-side reference makes it alive everywhere, stdout was:\n{stdout}"
    );
}

#[test]
fn a_debug_symbol_used_from_debug_is_not_flagged() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/debug/kotlin/DebugUtil.kt",
        "package sample\n\nclass DebugUtil {\n    fun dump() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/debug/kotlin/DebugHook.kt",
        "package sample\n\nfun hook() {\n    DebugUtil().dump()\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("DebugUtil"),
        "debug helpers living in debug are healthy, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unreferenced_main_symbol_is_not_repeated_here() {
    // zero references = plain dead code, the standard report owns it
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/GhostMain.kt",
        "package sample\n\nclass GhostMain {\n    fun haunt() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("GhostMain"),
        "plain dead code belongs to the standard report, stdout was:\n{stdout}"
    );
}

#[test]
fn multiple_lifeline_sets_are_all_named() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "src/main/kotlin/Telemetry.kt",
        "package sample\n\nclass Telemetry {\n    fun ping() {}\n}\n",
    );
    write_file(
        temp.path(),
        "src/debug/kotlin/DebugHook.kt",
        "package sample\n\nfun hookDebug() {\n    Telemetry().ping()\n}\n",
    );
    write_file(
        temp.path(),
        "src/demo/kotlin/DemoHook.kt",
        "package sample\n\nfun hookDemo() {\n    Telemetry().ping()\n}\n",
    );
    write_file(
        temp.path(),
        "src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("Telemetry") && stdout.contains("debug") && stdout.contains("demo"),
        "every lifeline set is listed, stdout was:\n{stdout}"
    );
}

#[test]
fn no_source_sets_is_a_clean_answer() {
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
        "a flat project is fine, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no debug-only"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
