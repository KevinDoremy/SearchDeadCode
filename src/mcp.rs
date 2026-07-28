//! Minimal MCP stdio server over a saved reference graph. An AI agent
//! asks "who references X" / "is X dead" through newline-delimited
//! JSON-RPC without ever re-scanning the repo — the first slice of the
//! MCP roadmap, sized to the graph-file query mode it builds on.

use crate::report::graph_export::{QueryAnswer, SavedGraph};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

pub fn serve(graph: &SavedGraph, project_root: &Path) -> std::io::Result<()> {
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
        let response = handle(graph, project_root, &request, id);
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}

fn handle(graph: &SavedGraph, project_root: &Path, request: &Value, id: Value) -> Value {
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
                        "name": "dead_list",
                        "description": "Symbols with zero incoming references, 50 per page (offset to continue)",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "offset": { "type": "integer" } }
                        }
                    },
                    {
                        "name": "why_alive",
                        "description": "The shortest entry-point path keeping this symbol alive",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "symbol": { "type": "string" } },
                            "required": ["symbol"]
                        }
                    },
                    {
                        "name": "search",
                        "description": "Find symbols by case-insensitive substring, 50 per page (offset to continue)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" },
                                "offset": { "type": "integer" }
                            },
                            "required": ["query"]
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
                    },
                    {
                        "name": "health",
                        "description": "A-F dead-code grade per module, worst first — where to clean up next",
                        "inputSchema": { "type": "object", "properties": {} }
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
                "dead_list" => {
                    let offset = request["params"]["arguments"]["offset"]
                        .as_u64()
                        .unwrap_or(0) as usize;
                    dead_list_text(graph, offset)
                }
                "health" => health_text(graph, project_root),
                "why_alive" => why_alive_text(graph, symbol),
                "search" => {
                    let query = request["params"]["arguments"]["query"]
                        .as_str()
                        .unwrap_or("");
                    let offset = request["params"]["arguments"]["offset"]
                        .as_u64()
                        .unwrap_or(0) as usize;
                    search_text(graph, query, offset)
                }
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

/// An agent context is finite: 50 rows per page, deterministic order.
const DEAD_LIST_PAGE: usize = 50;

fn dead_list_text(graph: &SavedGraph, offset: usize) -> String {
    let mut dead = graph.dead_symbols();
    if dead.is_empty() {
        return "no unreferenced symbols in the graph".to_string();
    }
    dead.sort_by(|a, b| a.name.cmp(&b.name).then(a.file.cmp(&b.file)));
    let total = dead.len();
    if offset >= total {
        return format!(
            "no symbols at offset {offset} — the graph has {total} unreferenced symbol(s)"
        );
    }
    let page: Vec<_> = dead.into_iter().skip(offset).take(DEAD_LIST_PAGE).collect();
    let last = offset + page.len();
    let mut out = format!("unreferenced symbols {}-{last} of {total}:\n", offset + 1);
    for node in &page {
        out.push_str(&format!(
            "- {} ({}) at {}:{}\n",
            node.name, node.kind, node.file, node.line
        ));
    }
    if last < total {
        out.push_str(&format!("pass offset={last} for the next page\n"));
    }
    out
}

/// A-F grade per module, worst first — same thresholds as --health.
fn health_text(graph: &SavedGraph, project_root: &Path) -> String {
    use std::collections::{HashMap, HashSet};
    let mut totals: HashMap<String, usize> = HashMap::new();
    for node in &graph.nodes {
        let module = crate::analysis::strings_dup::module_of(project_root, Path::new(&node.file));
        *totals.entry(module).or_default() += 1;
    }
    let mut dead: HashMap<String, HashSet<String>> = HashMap::new();
    for node in graph.dead_symbols() {
        let module = crate::analysis::strings_dup::module_of(project_root, Path::new(&node.file));
        dead.entry(module)
            .or_default()
            .insert(format!("{}:{}", node.file, node.line));
    }
    let mut rows: Vec<(String, usize, usize)> = totals
        .into_iter()
        .map(|(module, total)| {
            let corpses = dead.get(&module).map(HashSet::len).unwrap_or(0);
            (module, corpses, total)
        })
        .collect();
    rows.sort_by(|a, b| {
        let ratio_a = a.1 as f64 / a.2.max(1) as f64;
        let ratio_b = b.1 as f64 / b.2.max(1) as f64;
        ratio_b.partial_cmp(&ratio_a).unwrap().then(a.0.cmp(&b.0))
    });
    let mut out = String::from("module health (dead/total declarations), worst first:\n");
    for (module, corpses, total) in rows {
        let percent = corpses as f64 * 100.0 / total.max(1) as f64;
        let grade = match percent {
            p if p <= 1.0 => "A",
            p if p <= 3.0 => "B",
            p if p <= 8.0 => "C",
            p if p <= 15.0 => "D",
            _ => "F",
        };
        out.push_str(&format!(
            "- {grade} {module}: {corpses}/{total} dead ({percent:.1}%)\n"
        ));
    }
    out
}

fn why_alive_text(graph: &SavedGraph, symbol: &str) -> String {
    if graph.roots.is_empty() {
        return "this graph has no roots recorded — re-export it with a current build".to_string();
    }
    match graph.why_alive(symbol) {
        None => format!("'{symbol}' is not in the graph"),
        Some(path) if path.is_empty() => {
            format!("'{symbol}' is not reachable from any entry point — dead")
        }
        Some(path) => {
            let mut out = format!("life path for '{symbol}':\n");
            for (i, node) in path.iter().enumerate() {
                let arrow = if i == 0 { "root" } else { "  ->" };
                out.push_str(&format!("{arrow} {} ({})\n", node.name, node.kind));
            }
            out
        }
    }
}

fn search_text(graph: &SavedGraph, query: &str, offset: usize) -> String {
    let needle = query.to_lowercase();
    let mut hits: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| !needle.is_empty() && n.name.to_lowercase().contains(&needle))
        .collect();
    hits.sort_by(|a, b| a.name.cmp(&b.name).then(a.file.cmp(&b.file)));
    let total = hits.len();
    if total == 0 {
        return format!("no symbol matches '{query}'");
    }
    if offset >= total {
        return format!("no matches at offset {offset} — {total} match(es) for '{query}'");
    }
    let page: Vec<_> = hits.into_iter().skip(offset).take(DEAD_LIST_PAGE).collect();
    let last = offset + page.len();
    let mut out = if total > DEAD_LIST_PAGE {
        format!("matches {}-{last} of {total} for '{query}':\n", offset + 1)
    } else {
        format!("{total} match(es) for '{query}':\n")
    };
    for node in &page {
        out.push_str(&format!(
            "- {} ({}) at {}:{}\n",
            node.name, node.kind, node.file, node.line
        ));
    }
    if last < total {
        out.push_str(&format!("pass offset={last} for the next page\n"));
    }
    out
}
