//! Minimal LSP server over the saved reference graph: initialize,
//! shutdown, and dead-code diagnostics published on didOpen. Editors
//! speak Content-Length framing — this is the real protocol, sized to
//! what the graph file can answer without a re-scan.

use crate::report::graph_export::SavedGraph;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

pub fn serve(graph: &SavedGraph) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = std::io::stdout();
    while let Some(body) = read_frame(&mut reader)? {
        let Ok(message) = serde_json::from_str::<Value>(&body) else {
            continue;
        };
        match message["method"].as_str() {
            Some("initialize") => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": message["id"],
                    "result": {
                        "capabilities": { "textDocumentSync": 1, "hoverProvider": true },
                        "serverInfo": {
                            "name": "searchdeadcode",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                });
                write_frame(&mut stdout, &response)?;
            }
            Some("textDocument/didOpen") => {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("");
                let notification = json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {
                        "uri": uri,
                        "diagnostics": diagnostics_for(graph, uri)
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
                    "result": hover_for(graph, uri, line)
                });
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
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    graph
        .dead_symbols()
        .into_iter()
        .filter(|node| node.file == path)
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

/// Life-or-death verdict for the symbol declared on this 0-indexed
/// line, or null when the line holds none.
fn hover_for(graph: &SavedGraph, uri: &str, line: u64) -> Value {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let Some(node) = graph
        .nodes
        .iter()
        .find(|n| n.file == path && n.line == (line + 1) as usize)
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
