//! Minimal MCP stdio server over a saved reference graph. An AI agent
//! asks "who references X" / "is X dead" through newline-delimited
//! JSON-RPC without ever re-scanning the repo — the first slice of the
//! MCP roadmap, sized to the graph-file query mode it builds on.

use crate::report::graph_export::{QueryAnswer, SavedGraph};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

pub fn serve(graph: &SavedGraph) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = request["id"].clone();
        if id.is_null() {
            continue; // notification: no response owed
        }
        let response = handle(graph, &request, id);
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}

fn handle(graph: &SavedGraph, request: &Value, id: Value) -> Value {
    match request["method"].as_str() {
        Some("initialize") => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "searchdeadcode",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        }),
        Some("tools/list") => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "refs_of",
                        "description": "Who references this symbol, from the saved reference graph",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "symbol": { "type": "string" } },
                            "required": ["symbol"]
                        }
                    },
                    {
                        "name": "is_dead",
                        "description": "Is this symbol dead (zero incoming references)?",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "symbol": { "type": "string" } },
                            "required": ["symbol"]
                        }
                    }
                ]
            }
        }),
        Some("tools/call") => {
            let tool = request["params"]["name"].as_str().unwrap_or("");
            let symbol = request["params"]["arguments"]["symbol"]
                .as_str()
                .unwrap_or("");
            let text = match tool {
                "refs_of" => refs_of_text(graph, symbol),
                "is_dead" => is_dead_text(graph, symbol),
                other => {
                    return json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32602, "message": format!("unknown tool '{other}'") }
                    })
                }
            };
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "content": [ { "type": "text", "text": text } ] }
            })
        }
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": "method not found" }
        }),
    }
}

fn refs_of_text(graph: &SavedGraph, symbol: &str) -> String {
    match graph.refs_of(symbol) {
        QueryAnswer::UnknownSymbol => format!("'{symbol}' is not in the graph"),
        QueryAnswer::Referencers(refs) if refs.is_empty() => {
            format!("no references to '{symbol}'")
        }
        QueryAnswer::Referencers(refs) => {
            let mut out = format!("references to '{symbol}':\n");
            for node in refs {
                out.push_str(&format!(
                    "- {} ({}) at {}:{}\n",
                    node.name, node.kind, node.file, node.line
                ));
            }
            out
        }
    }
}

fn is_dead_text(graph: &SavedGraph, symbol: &str) -> String {
    match graph.refs_of(symbol) {
        QueryAnswer::UnknownSymbol => format!("'{symbol}' is not in the graph"),
        QueryAnswer::Referencers(refs) if refs.is_empty() => {
            format!("'{symbol}' is dead: 0 references in the graph")
        }
        QueryAnswer::Referencers(refs) => {
            format!("'{symbol}' is alive: referenced {} time(s)", refs.len())
        }
    }
}
