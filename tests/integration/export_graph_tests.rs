//! Integration tests for --export-graph: the reference graph as JSON
//! (machine consumers, future query mode) or DOT (Gephi, graphviz).
//! Format follows the file extension.

use std::fs;
use std::path::Path;
use std::process::Output;

fn run(dir: &Path, out: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(dir)
        .arg("--export-graph")
        .arg(out)
        .output()
        .unwrap()
}

fn write_project(dir: &Path) {
    fs::write(
        dir.join("Engine.kt"),
        "package sample\n\nclass Engine {\n    fun run() {}\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("Main.kt"),
        concat!(
            "package sample\n\n",
            "fun main() {\n",
            "    Engine().run()\n",
            "}\n",
        ),
    )
    .unwrap();
}

#[test]
fn json_export_holds_nodes_and_edges() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let out = temp.path().join("graph.json");

    let output = run(temp.path(), &out);
    assert!(output.status.success(), "export failed:\n{output:?}");

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&out).unwrap()).expect("valid json");
    let nodes = json["nodes"].as_array().expect("nodes array");
    assert!(
        nodes.iter().any(|n| n["name"] == "Engine"),
        "Engine is a node, json was:\n{json}"
    );
    let edges = json["edges"].as_array().expect("edges array");
    assert!(!edges.is_empty(), "the Engine usage is an edge:\n{json}");
}

#[test]
fn dot_export_renders_a_digraph() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let out = temp.path().join("graph.dot");

    let output = run(temp.path(), &out);
    assert!(output.status.success(), "export failed:\n{output:?}");

    let dot = fs::read_to_string(&out).unwrap();
    assert!(
        dot.starts_with("digraph") && dot.contains("Engine"),
        "a graphviz digraph with the class in it, dot was:\n{dot}"
    );
}

#[test]
fn the_gv_extension_is_a_dot_alias() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let out = temp.path().join("graph.gv");

    let output = run(temp.path(), &out);
    assert!(output.status.success(), "gv export failed:\n{output:?}");
    let dot = fs::read_to_string(&out).unwrap();
    assert!(dot.starts_with("digraph"), "gv is graphviz too:\n{dot}");
}

#[test]
fn an_unknown_extension_is_a_clean_error() {
    let temp = tempfile::tempdir().unwrap();
    write_project(temp.path());
    let out = temp.path().join("graph.xlsx");

    let output = run(temp.path(), &out);
    assert!(
        !output.status.success(),
        "unknown format must fail loudly, output was:\n{output:?}"
    );
    assert!(!out.exists(), "no file written on error");
}

#[cfg(unix)]
#[test]
fn export_replaces_a_read_only_previous_export() {
    // the LSP/MCP reload path watches this file: the export must land
    // via rename (directory permission), never by truncating in place —
    // truncation both blocks on a read-only target and shows readers a
    // half-written file
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let graph = temp.path().join("graph.json");
    std::fs::write(&graph, "{\"old\": true}").unwrap();
    std::fs::set_permissions(&graph, std::fs::Permissions::from_mode(0o444)).unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(&project)
        .args(["--export-graph", graph.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "a rename-based export replaces even a read-only file:\n{out:?}"
    );
    let content = std::fs::read_to_string(&graph).unwrap();
    assert!(
        content.contains("nodes"),
        "the new export landed, content was:\n{content}"
    );
}

#[test]
fn export_leaves_no_temporary_file_behind() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let graph = temp.path().join("graph.json");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(&project)
        .args(["--export-graph", graph.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());

    let leftovers: Vec<String> = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n != "graph.json" && n != "project")
        .collect();
    assert!(
        leftovers.is_empty(),
        "the staging file is renamed away, found: {leftovers:?}"
    );
}
