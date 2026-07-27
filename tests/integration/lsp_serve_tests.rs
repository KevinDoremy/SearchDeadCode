//! Integration tests for --lsp-serve: a minimal LSP server over the
//! saved graph. Editors get initialize/shutdown plus dead-code
//! diagnostics on didOpen — Content-Length framing, the real protocol.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn saved_graph(temp: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let project = temp.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("Ghost.kt"),
        "package sample\n\nclass Ghost {\n    fun haunt() {}\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("Main.kt"),
        "package sample\n\nfun main() {\n    println(\"alive\")\n}\n",
    )
    .unwrap();
    let graph = temp.join("graph.json");
    let out = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(&project)
        .args(["--export-graph", graph.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "export failed:\n{out:?}");
    (graph, project)
}

fn frame(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

/// Send LSP frames, close stdin, parse every framed response body.
fn serve(graph: &Path, bodies: &[String]) -> Vec<serde_json::Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(graph.parent().unwrap())
        .args(["--graph-file", graph.to_str().unwrap(), "--lsp-serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        for body in bodies {
            stdin.write_all(&frame(body)).unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    let raw = String::from_utf8_lossy(&out.stdout).to_string();
    raw.split("Content-Length:")
        .filter_map(|chunk| {
            let json_start = chunk.find('{')?;
            serde_json::from_str(chunk[json_start..].trim()).ok()
        })
        .collect()
}

#[test]
fn initialize_answers_with_capabilities() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, _) = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string()],
    );
    let init = responses
        .iter()
        .find(|r| r["id"] == 1)
        .expect("initialize response");
    assert_eq!(init["result"]["serverInfo"]["name"], "searchdeadcode");
    assert!(!init["result"]["capabilities"].is_null());
}

#[test]
fn did_open_publishes_diagnostics_for_the_dead() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let ghost_uri = format!("file://{}", project.join("Ghost.kt").display()).replace('\\', "/");

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            format!(
                r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{ghost_uri}"}}}}}}"#
            ),
        ],
    );
    let publish = responses
        .iter()
        .find(|r| r["method"] == "textDocument/publishDiagnostics")
        .expect("diagnostics notification");
    let diagnostics = publish["params"]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["message"].as_str().unwrap_or("").contains("Ghost")),
        "the dead class is diagnosed, got:\n{publish}"
    );
}

#[test]
fn a_living_file_gets_empty_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let main_uri = format!("file://{}", project.join("Main.kt").display()).replace('\\', "/");

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            format!(
                r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{main_uri}"}}}}}}"#
            ),
        ],
    );
    let publish = responses
        .iter()
        .find(|r| r["method"] == "textDocument/publishDiagnostics")
        .expect("diagnostics notification even when clean");
    let diagnostics = publish["params"]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics.is_empty(),
        "main is alive, no noise, got:\n{publish}"
    );
}

#[test]
fn hover_on_a_dead_symbol_says_dead() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let ghost_uri = format!("file://{}", project.join("Ghost.kt").display()).replace('\\', "/");

    // Ghost is declared on line 3 (0-indexed: 2)
    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{{"textDocument":{{"uri":"{ghost_uri}"}},"position":{{"line":2,"character":6}}}}}}"#
            ),
        ],
    );
    let hover = responses
        .iter()
        .find(|r| r["id"] == 2)
        .expect("hover response");
    let text = hover["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        text.contains("Ghost") && text.to_lowercase().contains("dead"),
        "the hover names the corpse, text was:\n{text}"
    );
}

#[test]
fn hover_on_a_living_symbol_shows_the_life_path() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let main_uri = format!("file://{}", project.join("Main.kt").display()).replace('\\', "/");

    // main is declared on line 3 (0-indexed: 2)
    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            format!(
                r#"{{"jsonrpc":"2.0","id":3,"method":"textDocument/hover","params":{{"textDocument":{{"uri":"{main_uri}"}},"position":{{"line":2,"character":4}}}}}}"#
            ),
        ],
    );
    let hover = responses
        .iter()
        .find(|r| r["id"] == 3)
        .expect("hover response");
    let text = hover["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        text.contains("main") && (text.to_lowercase().contains("alive") || text.contains("root")),
        "the hover explains the life, text was:\n{text}"
    );
}

#[test]
fn hover_on_an_empty_line_answers_null() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let main_uri = format!("file://{}", project.join("Main.kt").display()).replace('\\', "/");

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            format!(
                r#"{{"jsonrpc":"2.0","id":4,"method":"textDocument/hover","params":{{"textDocument":{{"uri":"{main_uri}"}},"position":{{"line":1,"character":0}}}}}}"#
            ),
        ],
    );
    let hover = responses
        .iter()
        .find(|r| r["id"] == 4)
        .expect("hover response");
    assert!(
        hover["result"].is_null(),
        "no symbol on that line, null per the LSP spec, got:\n{hover}"
    );
}

#[test]
fn shutdown_answers_and_the_server_exits_on_eof() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, _) = saved_graph(temp.path());

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}"#.to_string(),
        ],
    );
    assert!(
        responses.iter().any(|r| r["id"] == 2),
        "shutdown gets a response — and reaching here proves EOF ends the loop"
    );
}
