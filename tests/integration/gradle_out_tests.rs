//! Integration tests: Gradle build scripts stay out of the code graph.
//! build.gradle.kts parsed as ordinary Kotlin pollutes the graph with
//! build-DSL declarations that look dead but are not application code.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::write(
        dir.join("build.gradle.kts"),
        concat!(
            "plugins {\n",
            "    kotlin(\"jvm\") version \"1.9.0\"\n",
            "}\n\n",
            "fun buildDslHelper() {\n",
            "    println(\"never called from app code\")\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("settings.gradle.kts"),
        "rootProject.name = \"sample\"\n\nfun settingsHelper() {}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Zombie.kt"),
        "package sample\n\nclass RealZombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
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
fn build_script_declarations_are_not_findings() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains("buildDslHelper") && !stdout.contains("settingsHelper"),
        "build DSL helpers are not application dead code, stdout was:\n{stdout}"
    );
}

#[test]
fn real_code_is_still_analyzed() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("RealZombie"),
        "the application corpse still reports, stdout was:\n{stdout}"
    );
}

#[test]
fn a_kotlin_file_merely_named_gradle_is_still_code() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    fs::write(
        temp.path().join("GradleZombie.kt"),
        "package sample\n\nclass GradleZombie {\n    fun groan() {}\n}\n",
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("GradleZombie"),
        "only *.gradle.kts is a build script, stdout was:\n{stdout}"
    );
}
