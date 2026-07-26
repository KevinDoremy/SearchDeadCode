//! Integration tests for incremental analysis caching (--incremental, --cache-path, --clear-cache)

use std::fs;
use std::path::Path;
use std::process::Output;

fn write_sample_project(dir: &Path) {
    fs::write(
        dir.join("Main.kt"),
        "package sample\n\nfun main() {\n    UsedHelper().greet()\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("UsedHelper.kt"),
        "package sample\n\nclass UsedHelper {\n    fun greet() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("ObsoleteWidget.kt"),
        "package sample\n\nclass ObsoleteWidget {\n    fun render() {}\n}\n",
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

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn analysis_writes_cache_file() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());
    let cache_path = temp.path().join("analysis-cache.json");

    run(temp.path(), &["--cache-path", cache_path.to_str().unwrap()]);

    assert!(
        cache_path.exists(),
        "an incremental run must write the cache file"
    );
    let content = fs::read_to_string(&cache_path).unwrap();
    assert!(
        content.contains("Main.kt"),
        "cache must contain an entry for each analyzed file"
    );
}

#[test]
fn second_run_loads_files_from_cache() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());
    let cache_path = temp.path().join("analysis-cache.json");
    let args = ["--cache-path", cache_path.to_str().unwrap()];

    run(temp.path(), &args);
    let second = run(temp.path(), &args);

    let stderr = stderr_of(&second);
    assert!(
        stderr.contains("3 from cache"),
        "unchanged files must be loaded from cache on the second run, stderr was:\n{stderr}"
    );
}

#[test]
fn cached_run_reports_same_findings() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());
    let cache_path = temp.path().join("analysis-cache.json");
    let args = ["--cache-path", cache_path.to_str().unwrap()];

    let first = run(temp.path(), &args);
    let second = run(temp.path(), &args);

    let first_out = stdout_of(&first);
    let second_out = stdout_of(&second);
    assert!(
        first_out.contains("ObsoleteWidget"),
        "first run must flag the dead class, stdout was:\n{first_out}"
    );
    assert!(
        second_out.contains("ObsoleteWidget"),
        "a run served from cache must report the same findings, stdout was:\n{second_out}"
    );
}

#[test]
fn modified_file_is_reparsed() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());
    let cache_path = temp.path().join("analysis-cache.json");
    let args = ["--cache-path", cache_path.to_str().unwrap()];

    run(temp.path(), &args);
    fs::write(
        temp.path().join("UsedHelper.kt"),
        "package sample\n\nclass UsedHelper {\n    fun greet() {}\n    fun wave() {}\n}\n",
    )
    .unwrap();
    let second = run(temp.path(), &args);

    let stderr = stderr_of(&second);
    assert!(
        stderr.contains("2 from cache"),
        "only the two unchanged files may come from cache after a modification, stderr was:\n{stderr}"
    );
}

#[test]
fn clear_cache_flag_resets_the_cache() {
    let temp = tempfile::tempdir().unwrap();
    write_sample_project(temp.path());
    let cache_path = temp.path().join("analysis-cache.json");

    run(temp.path(), &["--cache-path", cache_path.to_str().unwrap()]);
    let cleared = run(
        temp.path(),
        &[
            "--cache-path",
            cache_path.to_str().unwrap(),
            "--clear-cache",
        ],
    );

    let stderr = stderr_of(&cleared);
    assert!(
        stderr.contains("Cache cleared"),
        "--clear-cache must confirm the reset, stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("0 from cache"),
        "a cleared cache means every file is parsed again, stderr was:\n{stderr}"
    );
}
