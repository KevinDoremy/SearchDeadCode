//! Integration tests for --health: an A-F grade per module from the
//! dead/total declaration ratio — the light gamification that lands in
//! a team review where percentages do not.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--health")
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
fn a_clean_module_gets_an_a_and_a_rotten_one_does_not() {
    let temp = tempfile::tempdir().unwrap();
    // clean module: everything referenced
    write_file(
        temp.path(),
        "clean/src/main/kotlin/Engine.kt",
        "package sample\n\nclass Engine {\n    fun run() {}\n}\n",
    );
    write_file(
        temp.path(),
        "clean/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    Engine().run()\n}\n",
    );
    // rotten module: everything dead
    write_file(
        temp.path(),
        "rotten/src/main/kotlin/GhostA.kt",
        "package sample\n\nclass GhostA {\n    fun a() {}\n}\n",
    );
    write_file(
        temp.path(),
        "rotten/src/main/kotlin/GhostB.kt",
        "package sample\n\nclass GhostB {\n    fun b() {}\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    let clean_line = stdout
        .lines()
        .find(|l| l.contains("clean"))
        .expect("clean module line");
    let rotten_line = stdout
        .lines()
        .find(|l| l.contains("rotten"))
        .expect("rotten module line");
    assert!(
        clean_line.contains(" A ") || clean_line.trim_end().ends_with(" A"),
        "no corpses earns an A, line was:\n{clean_line}"
    );
    assert!(
        rotten_line.contains(" F ") || rotten_line.trim_end().ends_with(" F"),
        "all-dead earns an F, line was:\n{rotten_line}"
    );
}

#[test]
fn grades_come_with_the_ratio_that_earned_them() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "app/src/main/kotlin/Ghost.kt",
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains('%'),
        "the percentage rides with the grade, stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("dead"),
        "the metric is named, stdout was:\n{stdout}"
    );
}

#[test]
fn a_clean_project_is_all_a() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(output.status.success());
    assert!(
        stdout.contains(" A") && !stdout.contains(" F"),
        "nothing dead, everything A, stdout was:\n{stdout}"
    );
}

#[test]
fn health_json_is_machine_readable() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "app/src/main/kotlin/Ghost.kt",
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--health", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("valid JSON on stdout");
    let modules = json["modules"].as_array().expect("a modules array");
    let app = modules
        .iter()
        .find(|m| m["module"].as_str().unwrap_or("").contains("app"))
        .expect("the app module is graded");
    assert!(app["grade"].is_string());
    assert!(app["dead"].is_u64() && app["total"].is_u64());
    assert!(
        app["dead"].as_u64().unwrap() > 0,
        "Ghost counts as dead, got:\n{app}"
    );
}

#[test]
fn health_json_carries_no_ansi_noise() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--health", "--format", "json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains('\u{1b}'),
        "a dashboard parses this — no escape codes, stdout was:\n{stdout}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(stdout.trim()).is_ok(),
        "nothing but JSON on stdout"
    );
}

#[test]
fn min_grade_fails_the_run_when_a_module_rots_below_it() {
    let temp = tempfile::tempdir().unwrap();
    // one corpse out of two declarations: 50% dead, grade F
    write_file(
        temp.path(),
        "app/src/main/kotlin/Ghost.kt",
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--health", "--min-grade", "C"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(3),
        "an F module under a C floor is a CI failure:\n{out:?}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("min-grade") || stdout.contains("below"),
        "the verdict says WHY it failed, stdout was:\n{stdout}"
    );
}

#[test]
fn min_grade_passes_a_clean_project() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "app/src/main/kotlin/Engine.kt",
        "package sample\n\nclass Engine {\n    fun run() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    Engine().run()\n}\n",
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--health", "--min-grade", "C"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "all-A modules pass any floor:\n{out:?}"
    );
}

#[test]
fn min_grade_rejects_nonsense_letters() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(temp.path())
        .args(["--health", "--min-grade", "Z"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "Z is not a grade");
}
