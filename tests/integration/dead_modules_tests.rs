//! Integration tests for --dead-modules: a module included in settings
//! with no incoming project() dependency and no application plugin is a
//! whole-module deletion candidate — the biggest LOC wins there are.

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_multi_module(dir: &Path) {
    fs::write(
        dir.join("settings.gradle.kts"),
        concat!(
            "rootProject.name = \"sample\"\n",
            "include(\":app\")\n",
            "include(\":core\")\n",
            "include(\":orphan\")\n",
        ),
    )
    .unwrap();
    fs::create_dir_all(dir.join("app")).unwrap();
    fs::write(
        dir.join("app/build.gradle.kts"),
        concat!(
            "plugins {\n",
            "    id(\"com.android.application\")\n",
            "}\n",
            "dependencies {\n",
            "    implementation(project(\":core\"))\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::create_dir_all(dir.join("core")).unwrap();
    fs::write(
        dir.join("core/build.gradle.kts"),
        "plugins {\n    id(\"com.android.library\")\n}\n",
    )
    .unwrap();
    fs::create_dir_all(dir.join("orphan")).unwrap();
    fs::write(
        dir.join("orphan/build.gradle.kts"),
        "plugins {\n    id(\"com.android.library\")\n}\n",
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
        .arg("--dead-modules")
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_module_nobody_depends_on_is_a_candidate() {
    let temp = tempfile::tempdir().unwrap();
    write_multi_module(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains(":orphan"),
        "no build file references it, stdout was:\n{stdout}"
    );
}

#[test]
fn a_depended_on_module_is_not_a_candidate() {
    let temp = tempfile::tempdir().unwrap();
    write_multi_module(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains(":core"),
        "app depends on core, stdout was:\n{stdout}"
    );
}

#[test]
fn an_application_module_is_never_a_candidate() {
    let temp = tempfile::tempdir().unwrap();
    write_multi_module(temp.path());

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains(":app"),
        "application modules are roots by definition, stdout was:\n{stdout}"
    );
}

#[test]
fn a_repo_without_settings_says_so() {
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
        "no settings file is not an error, output was:\n{output:?}"
    );
    assert!(
        stdout.to_lowercase().contains("no settings.gradle"),
        "the user learns why nothing was checked, stdout was:\n{stdout}"
    );
}

#[test]
fn typesafe_accessor_references_count() {
    let temp = tempfile::tempdir().unwrap();
    write_multi_module(temp.path());
    // app also uses the typesafe accessor style for orphan
    fs::write(
        temp.path().join("app/build.gradle.kts"),
        concat!(
            "plugins {\n",
            "    id(\"com.android.application\")\n",
            "}\n",
            "dependencies {\n",
            "    implementation(project(\":core\"))\n",
            "    implementation(projects.orphan)\n",
            "}\n",
        ),
    )
    .unwrap();

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        !stdout.contains(":orphan"),
        "projects.orphan is a dependency too, stdout was:\n{stdout}"
    );
}
