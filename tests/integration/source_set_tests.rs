//! Integration tests for phantom source set detection.
//!
//! A "phantom" source set is a directory under src/ that is neither a
//! conventional source set (main, test, androidTest, buildTypes, ...) nor
//! declared in the module's build file. Code in there is never compiled,
//! so it must not keep other code alive.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .output()
        .unwrap()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Project with a build file, a main source set, and an undeclared
/// src/savedTests/ directory whose file references a main-source class.
fn write_project_with_phantom(dir: &Path) {
    fs::write(
        dir.join("build.gradle.kts"),
        "plugins { kotlin(\"jvm\") }\n",
    )
    .unwrap();
    let main = dir.join("src/main/kotlin");
    fs::create_dir_all(&main).unwrap();
    fs::write(
        main.join("Main.kt"),
        "package app\n\nfun main() {\n    ActiveService().ping()\n}\n",
    )
    .unwrap();
    fs::write(
        main.join("ActiveService.kt"),
        "package app\n\nclass ActiveService {\n    fun ping() {}\n}\n",
    )
    .unwrap();
    fs::write(
        main.join("LegacyEngine.kt"),
        "package app\n\nclass LegacyEngine {\n    fun start() {}\n}\n",
    )
    .unwrap();
    let phantom = dir.join("src/savedTests/kotlin");
    fs::create_dir_all(&phantom).unwrap();
    fs::write(
        phantom.join("SavedRef.kt"),
        "package app\n\nclass SavedRef {\n    fun keepAlive() {\n        LegacyEngine().start()\n    }\n}\n",
    )
    .unwrap();
}

#[test]
fn phantom_source_set_is_reported() {
    let temp = tempfile::tempdir().unwrap();
    write_project_with_phantom(temp.path());

    let output = run(temp.path());

    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("savedTests"),
        "an undeclared source set must be reported as phantom, stderr was:\n{stderr}"
    );
}

#[test]
fn phantom_files_do_not_keep_code_alive() {
    let temp = tempfile::tempdir().unwrap();
    write_project_with_phantom(temp.path());

    let output = run(temp.path());

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("LegacyEngine"),
        "a class only referenced from a phantom source set is dead, stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("SavedRef"),
        "files inside a phantom source set are excluded from analysis, stdout was:\n{stdout}"
    );
}

#[test]
fn declared_source_set_is_not_phantom() {
    let temp = tempfile::tempdir().unwrap();
    write_project_with_phantom(temp.path());
    fs::write(
        temp.path().join("build.gradle.kts"),
        "plugins { kotlin(\"jvm\") }\n\nsourceSets {\n    create(\"savedTests\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());

    let stderr = stderr_of(&output);
    assert!(
        !stderr.contains("Phantom"),
        "a source set declared in the build file is not phantom, stderr was:\n{stderr}"
    );
}

#[test]
fn conventional_source_sets_are_not_phantom() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("build.gradle.kts"),
        "plugins { kotlin(\"jvm\") }\n",
    )
    .unwrap();
    for set in ["main", "test", "androidTest", "debug", "release"] {
        let d = temp.path().join(format!("src/{set}/kotlin"));
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("A.kt"),
            format!("package app\n\nclass A{set} {{ fun f() {{}} }}\n"),
        )
        .unwrap();
    }

    let output = run(temp.path());

    let stderr = stderr_of(&output);
    assert!(
        !stderr.contains("Phantom"),
        "conventional source sets are never phantom, stderr was:\n{stderr}"
    );
}

#[test]
fn without_build_file_no_phantom_detection() {
    let temp = tempfile::tempdir().unwrap();
    let odd = temp.path().join("src/whatever/kotlin");
    fs::create_dir_all(&odd).unwrap();
    fs::write(odd.join("B.kt"), "package app\n\nclass B { fun f() {} }\n").unwrap();

    let output = run(temp.path());

    let stderr = stderr_of(&output);
    assert!(
        !stderr.contains("Phantom"),
        "without a build file there is no ground truth, no phantom warning, stderr was:\n{stderr}"
    );
}
