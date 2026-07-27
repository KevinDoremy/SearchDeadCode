//! Integration tests for --fix: zero-risk automatic cleanup (unused imports).

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_project(dir: &Path) {
    fs::create_dir_all(dir.join("util")).unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "import sample.util.UsedTool\n",
            "import sample.util.GhostTool\n",
            "import sample.util.SpecterTool as Spec\n",
            "import sample.util.*\n\n",
            "fun main() {\n",
            "    UsedTool().work()\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("util/UsedTool.kt"),
        "package sample.util\n\nclass UsedTool {\n    fun work() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("util/GhostTool.kt"),
        "package sample.util\n\nclass GhostTool {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("util/SpecterTool.kt"),
        "package sample.util\n\nclass SpecterTool {\n    fun float() {}\n}\n",
    )
    .unwrap();
}

fn run(dir: &Path, extra_args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(extra_args)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn fix_removes_unused_and_aliased_unused_imports() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path(), &["--fix"]);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("2 unused import"),
        "both dead imports are counted, stdout was:\n{stdout}"
    );
    let main = fs::read_to_string(temp.path().join("Main.kt")).unwrap();
    assert!(!main.contains("GhostTool"), "plain unused import removed");
    assert!(
        !main.contains("SpecterTool"),
        "aliased import with unused alias removed"
    );
}

#[test]
fn fix_keeps_used_and_star_imports() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    run(temp.path(), &["--fix"]);

    let main = fs::read_to_string(temp.path().join("Main.kt")).unwrap();
    assert!(
        main.contains("import sample.util.UsedTool"),
        "used import survives"
    );
    assert!(
        main.contains("import sample.util.*"),
        "star imports are never touched: their usage cannot be proven textually"
    );
}

#[test]
fn fix_touches_nothing_but_import_lines() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let before = fs::read_to_string(temp.path().join("Main.kt")).unwrap();

    run(temp.path(), &["--fix"]);

    let after = fs::read_to_string(temp.path().join("Main.kt")).unwrap();
    let expected: String = before
        .lines()
        .filter(|l| !l.contains("GhostTool") && !l.contains("SpecterTool"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        after.trim_end(),
        expected.trim_end(),
        "only the two dead import lines may differ"
    );
}

#[test]
fn fix_writes_an_undo_script() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());

    let output = run(temp.path(), &["--fix"]);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("undo"),
        "the undo script is announced, stdout was:\n{stdout}"
    );
    assert!(
        temp.path().join(".searchdeadcode-undo.sh").exists(),
        "the undo script exists on disk"
    );
}

#[test]
fn fix_with_dry_run_touches_nothing() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let before = fs::read_to_string(temp.path().join("Main.kt")).unwrap();

    let output = run(temp.path(), &["--fix", "--dry-run"]);

    let after = fs::read_to_string(temp.path().join("Main.kt")).unwrap();
    assert_eq!(before, after, "dry run must not modify files");
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("GhostTool"),
        "the dry run still lists what would go, stdout was:\n{stdout}"
    );
}

#[test]
fn a_name_used_only_in_a_string_still_counts_as_used() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("util")).unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        concat!(
            "package sample\n\n",
            "import sample.util.ReflectedTool\n\n",
            "fun main() {\n",
            "    val target = \"ReflectedTool\"\n",
            "    println(target)\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("util/ReflectedTool.kt"),
        "package sample.util\n\nclass ReflectedTool {\n    fun reflect() {}\n}\n",
    )
    .unwrap();

    run(temp.path(), &["--fix"]);

    let main = fs::read_to_string(temp.path().join("Main.kt")).unwrap();
    assert!(
        main.contains("import sample.util.ReflectedTool"),
        "conservative: a name mentioned anywhere in the body keeps its import"
    );
}
