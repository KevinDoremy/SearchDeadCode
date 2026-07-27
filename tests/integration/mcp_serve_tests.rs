//! Integration tests for --mcp-serve: a minimal MCP stdio server over
//! a saved graph. An AI agent asks "who references X" / "is X dead"
//! through JSON-RPC without ever re-scanning the repo.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn saved_graph(temp: &Path) -> std::path::PathBuf {
    let project = temp.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("Engine.kt"),
        "package sample\n\nclass Engine {\n    fun run() {}\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("Main.kt"),
        "package sample\n\nfun main() {\n    Engine().run()\n}\n",
    )
    .unwrap();
    let graph = temp.join("graph.json");
    let out = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(&project)
        .args(["--export-graph", graph.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "export failed:\n{out:?}");
    graph
}

/// Send newline-delimited JSON-RPC requests, collect stdout lines.
fn serve(graph: &Path, requests: &[&str]) -> Vec<serde_json::Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(graph.parent().unwrap())
        .args(["--graph-file", graph.to_str().unwrap(), "--mcp-serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        for request in requests {
            writeln!(stdin, "{request}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.trim_start().starts_with('{'))
        .map(|l| serde_json::from_str(l).expect("valid json response"))
        .collect()
}

#[test]
fn initialize_answers_with_server_info() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#],
    );
    assert_eq!(responses.len(), 1, "one response per request");
    let result = &responses[0]["result"];
    assert_eq!(result["serverInfo"]["name"], "searchdeadcode");
    assert!(result["protocolVersion"].is_string());
}

#[test]
fn tools_list_names_the_graph_tools() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#],
    );
    let tools = responses[0]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(
        names.contains(&"refs_of") && names.contains(&"is_dead"),
        "both graph tools are advertised, got: {names:?}"
    );
}

#[test]
fn refs_of_answers_from_the_graph() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"refs_of","arguments":{"symbol":"Engine"}}}"#,
        ],
    );
    let text = responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("main"),
        "the referencer is named, text was:\n{text}"
    );
}

#[test]
fn is_dead_says_yes_for_the_ghost() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"is_dead","arguments":{"symbol":"Ghost"}}}"#,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"is_dead","arguments":{"symbol":"Engine"}}}"#,
        ],
    );
    let ghost = responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let engine = responses[1]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        ghost.to_lowercase().contains("dead") || ghost.contains("0 reference"),
        "Ghost has no referencers, text was:\n{ghost}"
    );
    assert!(
        engine.to_lowercase().contains("alive") || engine.contains("referenced"),
        "Engine is referenced, text was:\n{engine}"
    );
}

#[test]
fn dead_list_names_the_unreferenced_symbols() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"dead_list","arguments":{}}}"#,
        ],
    );
    let text = responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("Ghost"),
        "the unreferenced class is listed, text was:\n{text}"
    );
    assert!(
        !text.contains("Engine"),
        "a referenced class is not dead, text was:\n{text}"
    );
}

#[test]
fn the_export_records_the_entry_point_roots() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&graph).unwrap()).unwrap();
    let roots = json["roots"].as_array().expect("roots array present");
    assert!(!roots.is_empty(), "main is a root, json was:\n{json}");
}

#[test]
fn why_alive_walks_from_a_root_to_the_symbol() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"why_alive","arguments":{"symbol":"Engine"}}}"#,
        ],
    );
    let text = responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("main") && text.contains("Engine"),
        "the life path names the root and the symbol, text was:\n{text}"
    );
}

#[test]
fn why_alive_says_dead_for_the_ghost() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"why_alive","arguments":{"symbol":"Ghost"}}}"#,
        ],
    );
    let text = responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        text.to_lowercase().contains("not reachable") || text.to_lowercase().contains("dead"),
        "no root reaches the ghost, text was:\n{text}"
    );
}

#[test]
fn search_finds_symbols_by_case_insensitive_substring() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"search","arguments":{"query":"eng"}}}"#,
            r#"{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"search","arguments":{"query":"zzz"}}}"#,
        ],
    );
    let hit = responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        hit.contains("Engine") && hit.contains("class"),
        "substring match with kind, text was:\n{hit}"
    );
    let miss = responses[1]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        miss.to_lowercase().contains("no symbol"),
        "an empty result is explicit, text was:\n{miss}"
    );
}

#[test]
fn an_unknown_method_gets_a_json_rpc_error() {
    let temp = tempfile::tempdir().unwrap();
    let graph = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[r#"{"jsonrpc":"2.0","id":6,"method":"nope/nothing","params":{}}"#],
    );
    assert_eq!(
        responses[0]["error"]["code"], -32601,
        "method-not-found per JSON-RPC, response was:\n{}",
        responses[0]
    );
}
