//! Minimal LSP server over the saved reference graph: initialize,
//! shutdown, and dead-code diagnostics published on didOpen. Editors
//! speak Content-Length framing — this is the real protocol, sized to
//! what the graph file can answer without a re-scan.

use crate::report::graph_export::SavedGraph;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

pub fn serve(graph: SavedGraph, graph_path: &Path, project_root: &Path) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = std::io::stdout();
    let mut graph = graph;
    let mut loaded_at = std::fs::metadata(graph_path)
        .and_then(|m| m.modified())
        .ok();
    while let Some(body) = read_frame(&mut reader)? {
        let Ok(message) = serde_json::from_str::<Value>(&body) else {
            continue;
        };
        // a re-exported graph file replaces the snapshot; a corrupt or
        // half-written one must never kill the running server
        if let Ok(modified) = std::fs::metadata(graph_path).and_then(|m| m.modified()) {
            if loaded_at != Some(modified) {
                if let Ok(fresh) = SavedGraph::load(graph_path) {
                    graph = fresh;
                }
                loaded_at = Some(modified);
            }
        }
        match message["method"].as_str() {
            Some("initialize") => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": message["id"],
                    "result": {
                        "capabilities": {
                            "textDocumentSync": 1,
                            "hoverProvider": true,
                            "codeActionProvider": true,
                            "executeCommandProvider": {
                                "commands": ["searchdeadcode.addToBaseline"]
                            }
                        },
                        "serverInfo": {
                            "name": "searchdeadcode",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                });
                write_frame(&mut stdout, &response)?;
            }
            Some("textDocument/didOpen") | Some("textDocument/didSave") => {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("");
                let notification = json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {
                        "uri": uri,
                        "diagnostics": diagnostics_for(&graph, uri)
                    }
                });
                write_frame(&mut stdout, &notification)?;
            }
            Some("textDocument/hover") => {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("");
                let line = message["params"]["position"]["line"].as_u64().unwrap_or(0);
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": message["id"],
                    "result": hover_for(&graph, uri, line)
                });
                write_frame(&mut stdout, &response)?;
            }
            Some("textDocument/codeAction") => {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("");
                let start = message["params"]["range"]["start"]["line"]
                    .as_u64()
                    .unwrap_or(0);
                let end = message["params"]["range"]["end"]["line"]
                    .as_u64()
                    .unwrap_or(start);
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": message["id"],
                    "result": code_actions_for(&graph, uri, start, end)
                });
                write_frame(&mut stdout, &response)?;
            }
            Some("workspace/executeCommand") => {
                let command = message["params"]["command"].as_str().unwrap_or("");
                let args = message["params"]["arguments"].as_array();
                let result = if command == "searchdeadcode.addToBaseline" {
                    let uri = args
                        .and_then(|a| a.first())
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let name = args
                        .and_then(|a| a.get(1))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    add_to_baseline(&graph, project_root, uri, name)
                } else {
                    Value::Null
                };
                let response = json!({ "jsonrpc": "2.0", "id": message["id"], "result": result });
                write_frame(&mut stdout, &response)?;
            }
            Some("shutdown") => {
                let response = json!({ "jsonrpc": "2.0", "id": message["id"], "result": null });
                write_frame(&mut stdout, &response)?;
            }
            Some("exit") => break,
            _ => {} // notifications and unknown methods: nothing owed
        }
    }
    Ok(())
}

fn diagnostics_for(graph: &SavedGraph, uri: &str) -> Vec<Value> {
    // URIs use forward slashes; the exported node paths follow the OS
    let path = uri
        .strip_prefix("file://")
        .unwrap_or(uri)
        .replace('\\', "/");
    graph
        .dead_symbols()
        .into_iter()
        .filter(|node| node.file.replace('\\', "/") == path)
        .map(|node| {
            let line = node.line.saturating_sub(1) as u64;
            json!({
                "range": {
                    "start": { "line": line, "character": 0 },
                    "end": { "line": line, "character": 0 }
                },
                "severity": 2,
                "source": "searchdeadcode",
                "message": format!("{} '{}' has no incoming references", node.kind, node.name)
            })
        })
        .collect()
}

/// Quickfixes for the dead symbols inside the 0-indexed line range.
fn code_actions_for(graph: &SavedGraph, uri: &str, start: u64, end: u64) -> Vec<Value> {
    let path = uri
        .strip_prefix("file://")
        .unwrap_or(uri)
        .replace('\\', "/");
    graph
        .dead_symbols()
        .into_iter()
        .filter(|node| {
            let line = node.line.saturating_sub(1) as u64;
            node.file.replace('\\', "/") == path && line >= start && line <= end
        })
        .map(|node| {
            json!({
                "title": format!("Add '{}' to the searchdeadcode baseline", node.name),
                "kind": "quickfix",
                "command": {
                    "title": "Add to baseline",
                    "command": "searchdeadcode.addToBaseline",
                    "arguments": [uri, node.name]
                }
            })
        })
        .collect()
}

/// Append the symbol's fingerprint to <root>/.searchdeadcode-baseline.json
/// so the next CLI run with --baseline stays quiet about it. Unknown
/// symbols write nothing — a quickfix must never invent an entry.
fn add_to_baseline(graph: &SavedGraph, project_root: &Path, uri: &str, name: &str) -> Value {
    let path = uri
        .strip_prefix("file://")
        .unwrap_or(uri)
        .replace('\\', "/");
    let Some(node) = graph
        .nodes
        .iter()
        .find(|n| n.name == name && n.file.replace('\\', "/") == path)
    else {
        return Value::Null;
    };
    let baseline_path = project_root.join(".searchdeadcode-baseline.json");
    let mut baseline = if baseline_path.exists() {
        match crate::baseline::Baseline::load(&baseline_path) {
            Ok(b) => b,
            Err(_) => return Value::Null,
        }
    } else {
        crate::baseline::Baseline::from_findings(&[], project_root)
    };
    let relative = Path::new(&node.file)
        .strip_prefix(project_root)
        .unwrap_or(Path::new(&node.file))
        .to_string_lossy()
        .to_string();
    let already = baseline
        .issues
        .iter()
        .any(|i| i.file == relative && i.name == node.name && i.kind == node.kind);
    if !already {
        baseline.issues.push(crate::baseline::IssueFingerprint {
            file: relative,
            name: node.name.clone(),
            kind: node.kind.clone(),
            line: node.line,
            fqn: None,
            rule: None,
        });
        if baseline.save(&baseline_path).is_err() {
            return Value::Null;
        }
    }
    json!({ "baseline": baseline_path.to_string_lossy(), "added": !already })
}

/// Life-or-death verdict for the symbol declared on this 0-indexed
/// line, or null when the line holds none.
fn hover_for(graph: &SavedGraph, uri: &str, line: u64) -> Value {
    let path = uri
        .strip_prefix("file://")
        .unwrap_or(uri)
        .replace('\\', "/");
    let Some(node) = graph
        .nodes
        .iter()
        .find(|n| n.file.replace('\\', "/") == path && n.line == (line + 1) as usize)
    else {
        return Value::Null;
    };
    let text = match graph.why_alive(&node.name) {
        None => format!("`{}` is not in the graph", node.name),
        Some(chain) if chain.is_empty() => {
            format!(
                "`{}` ({}) is **dead** — no entry point reaches it",
                node.name, node.kind
            )
        }
        Some(chain) => {
            let path_names: Vec<&str> = chain.iter().map(|n| n.name.as_str()).collect();
            format!(
                "`{}` ({}) is alive — kept by: {}",
                node.name,
                node.kind,
                path_names.join(" -> ")
            )
        }
    };
    json!({ "contents": { "kind": "markdown", "value": text } })
}

fn read_frame(reader: &mut impl BufRead) -> std::io::Result<Option<String>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None); // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break; // header/body separator
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().ok();
        }
    }
    let Some(length) = content_length else {
        return Ok(Some(String::new()));
    };
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok(Some(String::from_utf8_lossy(&body).to_string()))
}

fn write_frame(writer: &mut impl Write, message: &Value) -> std::io::Result<()> {
    let body = message.to_string();
    write!(writer, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    writer.flush()
}
