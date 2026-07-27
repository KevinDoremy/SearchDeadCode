//! Reference-graph export: JSON for machine consumers (and the future
//! query mode), DOT for graphviz/Gephi. Format follows the extension.

// consumed by the binary's --export-graph wedge only, invisible to lib users
#![allow(dead_code)]

use crate::graph::Graph;
use serde::Serialize;
use std::io::Write;
use std::path::Path;

#[derive(Serialize)]
struct NodeOut {
    id: String,
    name: String,
    kind: &'static str,
    file: String,
    line: usize,
}

#[derive(Serialize)]
struct EdgeOut {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct GraphOut {
    nodes: Vec<NodeOut>,
    edges: Vec<EdgeOut>,
}

fn collect(graph: &Graph) -> GraphOut {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for decl in graph.declarations() {
        nodes.push(NodeOut {
            id: decl.id.to_string(),
            name: decl.name.clone(),
            kind: decl.kind.display_name(),
            file: decl.location.file.display().to_string(),
            line: decl.location.line,
        });
        for (referencer, _) in graph.get_references_to(&decl.id) {
            edges.push(EdgeOut {
                from: referencer.id.to_string(),
                to: decl.id.to_string(),
            });
        }
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    edges.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
    GraphOut { nodes, edges }
}

pub fn export_json(graph: &Graph, path: &Path) -> std::io::Result<()> {
    let out = collect(graph);
    let file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(std::io::BufWriter::new(file), &out).map_err(std::io::Error::other)
}

pub fn export_dot(graph: &Graph, path: &Path) -> std::io::Result<()> {
    let out = collect(graph);
    let mut writer = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(writer, "digraph references {{")?;
    for node in &out.nodes {
        writeln!(
            writer,
            "    \"{}\" [label=\"{}\"];",
            node.id.replace('"', "'"),
            node.name.replace('"', "'")
        )?;
    }
    for edge in &out.edges {
        writeln!(
            writer,
            "    \"{}\" -> \"{}\";",
            edge.from.replace('"', "'"),
            edge.to.replace('"', "'")
        )?;
    }
    writeln!(writer, "}}")?;
    Ok(())
}
