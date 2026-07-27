//! Integration tests for --by-module: findings aggregated per Gradle
//! module (count + top rule) — the view a lead of a 49-module repo
//! actually looks at.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--by-module")
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
fn findings_are_grouped_per_module_with_counts() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "app/src/main/kotlin/DeadA.kt",
        "package sample\n\nclass DeadA {\n    fun a() {}\n}\n",
    );
    write_file(
        temp.path(),
        "core/src/main/kotlin/DeadB.kt",
        "package sample\n\nclass DeadB {\n    fun b() {}\n}\n",
    );
    write_file(
        temp.path(),
        "core/src/main/kotlin/DeadC.kt",
        "package sample\n\nclass DeadC {\n    fun c() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("app") && stdout.contains("core"),
        "both modules appear, stdout was:\n{stdout}"
    );
    let core_line = stdout
        .lines()
        .find(|l| l.contains("core"))
        .expect("core line");
    let app_line = stdout
        .lines()
        .find(|l| l.contains("app"))
        .expect("app line");
    // core has more findings than app, so it ranks first
    let core_pos = stdout.find(core_line).unwrap();
    let app_pos = stdout.find(app_line).unwrap();
    assert!(
        core_pos < app_pos,
        "modules rank by finding count, stdout was:\n{stdout}"
    );
}

#[test]
fn the_top_rule_is_named_per_module() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        temp.path(),
        "app/src/main/kotlin/DeadA.kt",
        "package sample\n\nclass DeadA {\n    fun a() {}\n}\n",
    );
    write_file(
        temp.path(),
        "app/src/main/kotlin/Main.kt",
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    );

    let stdout = stdout_of(&run(temp.path()));
    assert!(
        stdout.contains("DC"),
        "the dominant rule code shows per module, stdout was:\n{stdout}"
    );
}

#[test]
fn a_flat_project_groups_under_dot() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun g() {}\n}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();

    let output = run(temp.path());
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "flat repos work too, output was:\n{output:?}"
    );
    assert!(
        stdout.lines().any(|l| l.trim_start().starts_with('.')),
        "the rootless module is '.', stdout was:\n{stdout}"
    );
}

#[test]
fn a_clean_project_says_so() {
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
        stdout.to_lowercase().contains("no findings"),
        "the verdict is explicit, stdout was:\n{stdout}"
    );
}
