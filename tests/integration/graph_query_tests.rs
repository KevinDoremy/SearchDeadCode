//! Integration tests for graph queries: --graph-file + --refs-of
//! answers "who references X" from a saved --export-graph JSON without
//! re-scanning anything — the base for the future MCP/LSP servers.

use std::fs;
use std::path::Path;
use std::process::Output;

fn bin(dir: &Path, args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Export a graph from a real project, return the JSON path.
fn saved_graph(temp: &Path) -> std::path::PathBuf {
    let project = temp.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("Engine.kt"),
        "package sample\n\nclass Engine {\n    fun run() {}\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("Main.kt"),
        "package sample\n\nfun main() {\n    Engine().run()\n}\n",
    )
    .unwrap();
    let graph = temp.join("graph.json");
    let out = bin(&project, &["--export-graph", graph.to_str().unwrap()]);
    assert!(out.status.success(), "export failed:\n{out:?}");
    graph
}

#[test]
fn refs_of_answers_from_the_saved_graph_without_scanning() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());
    // an EMPTY directory: any answer must come from the file alone
    let empty = temp.path().join("empty");
    fs::create_dir_all(&empty).unwrap();

    let out = bin(
        &empty,
        &[
            "--graph-file",
            graph.to_str().unwrap(),
            "--refs-of",
            "Engine",
        ],
    );
    let stdout = stdout_of(&out);
    assert!(out.status.success(), "query failed:\n{out:?}");
    assert!(
        stdout.contains("main"),
        "the referencer is named, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unreferenced_symbol_gets_an_explicit_zero() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());
    let empty = temp.path().join("empty");
    fs::create_dir_all(&empty).unwrap();

    // 'run' is called through Engine().run() — query something dead-end
    let out = bin(
        &empty,
        &["--graph-file", graph.to_str().unwrap(), "--refs-of", "main"],
    );
    let stdout = stdout_of(&out);
    assert!(out.status.success(), "query failed:\n{out:?}");
    assert!(
        stdout.to_lowercase().contains("no references"),
        "zero refs is an explicit answer, stdout was:\n{stdout}"
    );
}

#[test]
fn an_unknown_symbol_is_named_as_missing() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());
    let empty = temp.path().join("empty");
    fs::create_dir_all(&empty).unwrap();

    let out = bin(
        &empty,
        &["--graph-file", graph.to_str().unwrap(), "--refs-of", "Nope"],
    );
    let stdout = stdout_of(&out);
    assert!(
        stdout.to_lowercase().contains("not in the graph"),
        "an unknown symbol is a distinct answer from zero refs, stdout was:\n{stdout}"
    );
}

#[test]
fn a_missing_graph_file_is_a_clean_error() {
    let temp = tempfile::tempdir().unwrap();

    let out = bin(
        temp.path(),
        &["--graph-file", "/nonexistent/graph.json", "--refs-of", "X"],
    );
    assert!(
        !out.status.success(),
        "missing file is an error, output was:\n{out:?}"
    );
}

#[test]
fn refs_of_without_graph_file_errors_out() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Main.kt"),
        "package sample\n\nfun main() {}\n",
    )
    .unwrap();

    let out = bin(temp.path(), &["--refs-of", "X"]);
    assert!(
        !out.status.success(),
        "--refs-of needs --graph-file, output was:\n{out:?}"
    );
}
