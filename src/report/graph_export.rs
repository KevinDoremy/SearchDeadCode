//! Reference-graph export and query: JSON for machine consumers, DOT
//! for graphviz/Gephi, and instant "who references X" answers from a
//! saved JSON without re-scanning anything.

// consumed by the binary's graph wedges only, invisible to lib users
#![allow(dead_code)]

use crate::graph::Graph;
use serde::{Deserialize, Serialize};
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
    /// Entry-point declaration ids — the reasons anything is alive
    roots: Vec<String>,
}

/// Owned mirror of the export format, for reading a saved graph back.
#[derive(Deserialize)]
pub struct SavedNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
}

#[derive(Deserialize)]
pub struct SavedEdge {
    pub from: String,
    pub to: String,
}

#[derive(Deserialize)]
pub struct SavedGraph {
    pub nodes: Vec<SavedNode>,
    pub edges: Vec<SavedEdge>,
    /// Absent in old exports — why_alive then has nothing to walk from
    #[serde(default)]
    pub roots: Vec<String>,
}

pub enum QueryAnswer<'a> {
    /// The symbol exists; referencers listed (possibly empty)
    Referencers(Vec<&'a SavedNode>),
    /// No node carries that name at all
    UnknownSymbol,
}

impl SavedGraph {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(std::io::Error::other)
    }

    /// Nodes with zero incoming edges that are not entry-point roots —
    /// a root has no referencer by design, it is the reason things live.
    pub fn dead_symbols(&self) -> Vec<&SavedNode> {
        let referenced: std::collections::HashSet<&str> =
            self.edges.iter().map(|e| e.to.as_str()).collect();
        let roots: std::collections::HashSet<&str> =
            self.roots.iter().map(String::as_str).collect();
        let mut dead: Vec<&SavedNode> = self
            .nodes
            .iter()
            .filter(|n| {
                !referenced.contains(n.id.as_str())
                    && !roots.contains(n.id.as_str())
                    && matches!(
                        n.kind.as_str(),
                        "class" | "interface" | "object" | "enum" | "function" | "method"
                    )
            })
            .collect();
        dead.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        dead
    }

    /// Shortest root-to-symbol path, if any root reaches it.
    pub fn why_alive(&self, symbol: &str) -> Option<Vec<&SavedNode>> {
        use std::collections::{HashMap, VecDeque};
        let by_id: HashMap<&str, &SavedNode> =
            self.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let targets: std::collections::HashSet<&str> = self
            .nodes
            .iter()
            .filter(|n| n.name == symbol)
            .map(|n| n.id.as_str())
            .collect();
        if targets.is_empty() {
            return None;
        }
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            adjacency
                .entry(edge.from.as_str())
                .or_default()
                .push(edge.to.as_str());
        }
        let mut predecessor: HashMap<&str, &str> = HashMap::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        for root in &self.roots {
            if by_id.contains_key(root.as_str()) && !predecessor.contains_key(root.as_str()) {
                predecessor.insert(root.as_str(), "");
                queue.push_back(root.as_str());
            }
        }
        while let Some(current) = queue.pop_front() {
            if targets.contains(current) {
                let mut path = vec![current];
                let mut cursor = current;
                while let Some(&prev) = predecessor.get(cursor) {
                    if prev.is_empty() {
                        break;
                    }
                    path.push(prev);
                    cursor = prev;
                }
                path.reverse();
                return Some(
                    path.into_iter()
                        .filter_map(|id| by_id.get(id).copied())
                        .collect(),
                );
            }
            for next in adjacency.get(current).into_iter().flatten() {
                if !predecessor.contains_key(next) {
                    predecessor.insert(next, current);
                    queue.push_back(next);
                }
            }
        }
        Some(vec![]) // known symbol, but no root reaches it
    }

    /// Who references any declaration named `symbol`?
    pub fn refs_of(&self, symbol: &str) -> QueryAnswer<'_> {
        let targets: std::collections::HashSet<&str> = self
            .nodes
            .iter()
            .filter(|n| n.name == symbol)
            .map(|n| n.id.as_str())
            .collect();
        if targets.is_empty() {
            return QueryAnswer::UnknownSymbol;
        }
        let by_id: std::collections::HashMap<&str, &SavedNode> =
            self.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let mut referencers: Vec<&SavedNode> = self
            .edges
            .iter()
            .filter(|e| targets.contains(e.to.as_str()))
            .filter_map(|e| by_id.get(e.from.as_str()).copied())
            .collect();
        referencers.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        referencers.dedup_by(|a, b| a.id == b.id);
        QueryAnswer::Referencers(referencers)
    }
}

fn collect(
    graph: &Graph,
    entry_points: &std::collections::HashSet<crate::graph::DeclarationId>,
) -> GraphOut {
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
    let mut roots: Vec<String> = entry_points.iter().map(|id| id.to_string()).collect();
    roots.sort();
    GraphOut {
        nodes,
        edges,
        roots,
    }
}

pub fn export_json(
    graph: &Graph,
    entry_points: &std::collections::HashSet<crate::graph::DeclarationId>,
    path: &Path,
) -> std::io::Result<()> {
    let out = collect(graph, entry_points);
    // land via rename: the LSP/MCP reload path watches this file and
    // must never see a half-written export; a failed write must not
    // destroy the previous one either
    let staging = path.with_extension("json.staging");
    {
        let file = std::fs::File::create(&staging)?;
        serde_json::to_writer_pretty(std::io::BufWriter::new(file), &out)
            .map_err(std::io::Error::other)?;
    }
    if std::fs::rename(&staging, path).is_err() {
        // Windows cannot rename over an existing file
        std::fs::remove_file(path)?;
        std::fs::rename(&staging, path)?;
    }
    Ok(())
}

pub fn export_dot(
    graph: &Graph,
    entry_points: &std::collections::HashSet<crate::graph::DeclarationId>,
    path: &Path,
) -> std::io::Result<()> {
    let out = collect(graph, entry_points);
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
