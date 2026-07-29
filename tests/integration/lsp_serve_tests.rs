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
    serve_rooted(graph.parent().unwrap(), graph, bodies)
}

/// Same, but with an explicit project root — the directory an editor
/// would pass, and where the baseline quickfix writes.
fn serve_rooted(root: &Path, graph: &Path, bodies: &[String]) -> Vec<serde_json::Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(root)
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
fn initialize_advertises_code_actions() {
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
    assert_eq!(init["result"]["capabilities"]["codeActionProvider"], true);
    assert!(
        init["result"]["capabilities"]["executeCommandProvider"]["commands"]
            .as_array()
            .map(|c| c.iter().any(|v| v == "searchdeadcode.addToBaseline"))
            .unwrap_or(false),
        "the baseline command is advertised, got:\n{init}"
    );
}

#[test]
fn code_action_on_a_dead_line_offers_add_to_baseline() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let ghost_uri = format!("file://{}", project.join("Ghost.kt").display()).replace('\\', "/");

    // Ghost is declared on line 3 (0-indexed: 2)
    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/codeAction","params":{{"textDocument":{{"uri":"{ghost_uri}"}},"range":{{"start":{{"line":2,"character":0}},"end":{{"line":2,"character":10}}}},"context":{{"diagnostics":[]}}}}}}"#
            ),
        ],
    );
    let actions = responses
        .iter()
        .find(|r| r["id"] == 2)
        .expect("codeAction response");
    let list = actions["result"].as_array().expect("an action list");
    assert!(
        list.iter().any(|a| {
            a["title"].as_str().unwrap_or("").contains("Ghost")
                && a["title"]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("baseline")
        }),
        "the quickfix names the symbol and the fix, got:\n{actions}"
    );
}

#[test]
fn code_action_on_a_living_range_is_empty() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let main_uri = format!("file://{}", project.join("Main.kt").display()).replace('\\', "/");

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/codeAction","params":{{"textDocument":{{"uri":"{main_uri}"}},"range":{{"start":{{"line":2,"character":0}},"end":{{"line":2,"character":5}}}},"context":{{"diagnostics":[]}}}}}}"#
            ),
        ],
    );
    let actions = responses
        .iter()
        .find(|r| r["id"] == 2)
        .expect("codeAction response");
    assert_eq!(
        actions["result"].as_array().map(Vec::len),
        Some(0),
        "nothing to baseline on a living line, got:\n{actions}"
    );
}

#[test]
fn execute_command_writes_a_baseline_the_cli_honors() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let ghost_uri = format!("file://{}", project.join("Ghost.kt").display()).replace('\\', "/");

    serve_rooted(
        &project,
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"workspace/executeCommand","params":{{"command":"searchdeadcode.addToBaseline","arguments":["{ghost_uri}","Ghost"]}}}}"#
            ),
        ],
    );

    let baseline = project.join(".searchdeadcode-baseline.json");
    assert!(baseline.exists(), "the baseline file is created");
    let json = fs::read_to_string(&baseline).unwrap();
    assert!(json.contains("Ghost"), "the symbol is in it:\n{json}");

    // the whole point: a CLI run with that baseline no longer reports Ghost
    let report = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(&project)
        .args(["--baseline", baseline.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(
        !stdout.contains("'Ghost'"),
        "the editor quickfix silences the CLI too, stdout was:\n{stdout}"
    );
}

#[test]
fn execute_command_on_an_unknown_symbol_does_not_corrupt_anything() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());

    let responses = serve_rooted(
        &project,
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            r#"{"jsonrpc":"2.0","id":2,"method":"workspace/executeCommand","params":{"command":"searchdeadcode.addToBaseline","arguments":["file:///nowhere.kt","Nobody"]}}"#.to_string(),
        ],
    );
    assert!(
        responses.iter().any(|r| r["id"] == 2),
        "the request still gets a response"
    );
    assert!(
        !project.join(".searchdeadcode-baseline.json").exists(),
        "no baseline is written for a symbol the graph does not know"
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

#[test]
fn did_save_republishes_for_the_saved_file() {
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let ghost_uri = format!("file://{}", project.join("Ghost.kt").display()).replace('\\', "/");

    let responses = serve(
        &graph,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            format!(
                r#"{{"jsonrpc":"2.0","method":"textDocument/didSave","params":{{"textDocument":{{"uri":"{ghost_uri}"}}}}}}"#
            ),
        ],
    );
    let publish = responses
        .iter()
        .find(|r| r["method"] == "textDocument/publishDiagnostics")
        .expect("a save refreshes the diagnostics");
    assert!(
        publish["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"].as_str().unwrap_or("").contains("Ghost")),
        "the corpse is still diagnosed after save, got:\n{publish}"
    );
}

#[test]
fn did_save_picks_up_a_regenerated_graph() {
    use std::io::Write as _;
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let ghost_uri = format!("file://{}", project.join("Ghost.kt").display()).replace('\\', "/");

    let mut child = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(&project)
        .args(["--graph-file", graph.to_str().unwrap(), "--lsp-serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdin = child.stdin.as_mut().unwrap();
    stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        ))
        .unwrap();
    stdin
        .write_all(&frame(&format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{ghost_uri}"}}}}}}"#
        )))
        .unwrap();
    stdin.flush().unwrap();

    // the world changes: Main now uses Ghost, the graph is re-exported
    fs::write(
        project.join("Main.kt"),
        "package sample\n\nfun main() {\n    Ghost().haunt()\n}\n",
    )
    .unwrap();
    let export = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(&project)
        .args(["--export-graph", graph.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(export.status.success());

    let stdin = child.stdin.as_mut().unwrap();
    stdin
        .write_all(&frame(&format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didSave","params":{{"textDocument":{{"uri":"{ghost_uri}"}}}}}}"#
        )))
        .unwrap();
    drop(child.stdin.take());

    let out = child.wait_with_output().unwrap();
    let raw = String::from_utf8_lossy(&out.stdout).to_string();
    let publishes: Vec<serde_json::Value> = raw
        .split("Content-Length:")
        .filter_map(|chunk| {
            let start = chunk.find('{')?;
            serde_json::from_str(chunk[start..].trim()).ok()
        })
        .filter(|r: &serde_json::Value| r["method"] == "textDocument/publishDiagnostics")
        .collect();
    assert!(
        publishes.len() >= 2,
        "open + save both publish, got:\n{raw}"
    );
    let last = publishes.last().unwrap();
    assert!(
        last["params"]["diagnostics"].as_array().unwrap().is_empty(),
        "the reloaded graph knows Ghost is alive now, got:\n{last}"
    );
}

#[test]
fn a_corrupt_regenerated_graph_keeps_the_old_one() {
    use std::io::Write as _;
    let temp = tempfile::tempdir().unwrap();
    let (graph, project) = saved_graph(temp.path());
    let ghost_uri = format!("file://{}", project.join("Ghost.kt").display()).replace('\\', "/");

    let mut child = Command::new(env!("CARGO_BIN_EXE_searchdeadcode"))
        .arg(&project)
        .args(["--graph-file", graph.to_str().unwrap(), "--lsp-serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdin = child.stdin.as_mut().unwrap();
    stdin
        .write_all(&frame(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        ))
        .unwrap();
    stdin.flush().unwrap();

    // wait for the initialize response: the initial graph load is done,
    // so the corruption below can only hit the RE-load path
    let mut stdout = child.stdout.take().unwrap();
    let mut first = [0u8; 1];
    use std::io::Read as _;
    stdout.read_exact(&mut first).unwrap();

    fs::write(&graph, "{ not json at all").unwrap();

    let stdin = child.stdin.as_mut().unwrap();
    stdin
        .write_all(&frame(&format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didSave","params":{{"textDocument":{{"uri":"{ghost_uri}"}}}}}}"#
        )))
        .unwrap();
    drop(child.stdin.take());

    let mut rest = Vec::new();
    stdout.read_to_end(&mut rest).unwrap();
    let status = child.wait().unwrap();
    assert!(
        status.success(),
        "a corrupt reload must not kill the server"
    );
    let raw = String::from_utf8_lossy(&rest).to_string();
    assert!(
        raw.contains("Ghost"),
        "the old graph still answers, got:\n{raw}"
    );
}
