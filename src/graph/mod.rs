// Graph module - some methods reserved for future use
#![allow(dead_code)]

mod builder;
mod declaration;
mod parallel_builder;
pub mod reference;

pub use builder::GraphBuilder;
pub use declaration::{
    Declaration, DeclarationId, DeclarationKind, Language, Location, Visibility,
};
pub use parallel_builder::ParallelGraphBuilder;
pub use reference::{Reference, ReferenceKind, UnresolvedReference};

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;

/// The reference graph containing all declarations and their relationships
#[derive(Debug)]
pub struct Graph {
    /// The underlying directed graph
    /// Nodes are DeclarationIds, edges are References
    inner: DiGraph<DeclarationId, Reference>,

    /// Map from DeclarationId to node index
    node_map: HashMap<DeclarationId, NodeIndex>,

    /// Map from DeclarationId to Declaration details
    declarations: HashMap<DeclarationId, Declaration>,

    /// Map from simple name to possible declarations (for resolution)
    name_index: HashMap<String, Vec<DeclarationId>>,

    /// Map from fully qualified name to declaration
    fqn_index: HashMap<String, DeclarationId>,

    /// Map from parent to children (for fast member lookup)
    children_index: HashMap<DeclarationId, Vec<DeclarationId>>,
}

impl Graph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            inner: DiGraph::new(),
            node_map: HashMap::new(),
            declarations: HashMap::new(),
            name_index: HashMap::new(),
            fqn_index: HashMap::new(),
            children_index: HashMap::new(),
        }
    }

    /// Add a declaration to the graph
    pub fn add_declaration(&mut self, decl: Declaration) -> DeclarationId {
        let id = decl.id.clone();

        // Add to graph
        let node_idx = self.inner.add_node(id.clone());
        self.node_map.insert(id.clone(), node_idx);

        // Index by simple name
        self.name_index
            .entry(decl.name.clone())
            .or_default()
            .push(id.clone());

        // Index by fully qualified name
        if let Some(fqn) = &decl.fully_qualified_name {
            self.fqn_index.insert(fqn.clone(), id.clone());
        }

        // Index by parent (for fast children lookup)
        if let Some(parent_id) = &decl.parent {
            self.children_index
                .entry(parent_id.clone())
                .or_default()
                .push(id.clone());
        }

        // Store declaration details
        self.declarations.insert(id.clone(), decl);

        id
    }

    /// Add a reference between two declarations
    pub fn add_reference(
        &mut self,
        from: &DeclarationId,
        to: &DeclarationId,
        reference: Reference,
    ) {
        if let (Some(&from_idx), Some(&to_idx)) = (self.node_map.get(from), self.node_map.get(to)) {
            self.inner.add_edge(from_idx, to_idx, reference);
        }
    }

    /// Get a declaration by ID
    pub fn get_declaration(&self, id: &DeclarationId) -> Option<&Declaration> {
        self.declarations.get(id)
    }

    /// Get all declarations
    pub fn declarations(&self) -> impl Iterator<Item = &Declaration> {
        self.declarations.values()
    }

    /// Get declaration IDs
    pub fn declaration_ids(&self) -> impl Iterator<Item = &DeclarationId> {
        self.declarations.keys()
    }

    /// Find declarations by simple name
    pub fn find_by_name(&self, name: &str) -> Vec<&Declaration> {
        self.name_index
            .get(name)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.declarations.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find declaration by fully qualified name
    pub fn find_by_fqn(&self, fqn: &str) -> Option<&Declaration> {
        self.fqn_index
            .get(fqn)
            .and_then(|id| self.declarations.get(id))
    }

    /// Get all declarations that reference the given declaration
    pub fn get_references_to(&self, id: &DeclarationId) -> Vec<(&Declaration, &Reference)> {
        let Some(&node_idx) = self.node_map.get(id) else {
            return Vec::new();
        };

        self.inner
            .edges_directed(node_idx, petgraph::Direction::Incoming)
            .filter_map(|edge| {
                let source_id = self.inner.node_weight(edge.source())?;
                let decl = self.declarations.get(source_id)?;
                Some((decl, edge.weight()))
            })
            .collect()
    }

    /// Get all declarations that this declaration references
    pub fn get_references_from(&self, id: &DeclarationId) -> Vec<(&Declaration, &Reference)> {
        let Some(&node_idx) = self.node_map.get(id) else {
            return Vec::new();
        };

        self.inner
            .edges_directed(node_idx, petgraph::Direction::Outgoing)
            .filter_map(|edge| {
                let target_id = self.inner.node_weight(edge.target())?;
                let decl = self.declarations.get(target_id)?;
                Some((decl, edge.weight()))
            })
            .collect()
    }

    /// Check if a declaration is referenced by anything
    pub fn is_referenced(&self, id: &DeclarationId) -> bool {
        let Some(&node_idx) = self.node_map.get(id) else {
            return false;
        };

        self.inner
            .edges_directed(node_idx, petgraph::Direction::Incoming)
            .next()
            .is_some()
    }

    /// Get children of a declaration (members of a class, etc.)
    pub fn get_children(&self, id: &DeclarationId) -> Vec<&DeclarationId> {
        self.children_index
            .get(id)
            .map(|children| children.iter().collect())
            .unwrap_or_default()
    }

    /// Get the number of declarations
    pub fn declaration_count(&self) -> usize {
        self.declarations.len()
    }

    /// Get all references to a declaration, filtered by kind
    pub fn get_references_by_kind(
        &self,
        id: &DeclarationId,
        kind: ReferenceKind,
    ) -> Vec<(&Declaration, &Reference)> {
        let Some(&node_idx) = self.node_map.get(id) else {
            return Vec::new();
        };

        self.inner
            .edges_directed(node_idx, petgraph::Direction::Incoming)
            .filter_map(|edge| {
                let ref_kind = edge.weight();
                if ref_kind.kind == kind {
                    let source_id = self.inner.node_weight(edge.source())?;
                    let decl = self.declarations.get(source_id)?;
                    Some((decl, edge.weight()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Count read references to a declaration (excluding writes)
    pub fn count_reads(&self, id: &DeclarationId) -> usize {
        let Some(&node_idx) = self.node_map.get(id) else {
            return 0;
        };

        self.inner
            .edges_directed(node_idx, petgraph::Direction::Incoming)
            .filter(|edge| edge.weight().kind.is_read())
            .count()
    }

    /// Count write references to a declaration
    pub fn count_writes(&self, id: &DeclarationId) -> usize {
        let Some(&node_idx) = self.node_map.get(id) else {
            return 0;
        };

        self.inner
            .edges_directed(node_idx, petgraph::Direction::Incoming)
            .filter(|edge| edge.weight().kind.is_write())
            .count()
    }

    /// Get the number of references
    pub fn reference_count(&self) -> usize {
        self.inner.edge_count()
    }

    /// Get the underlying petgraph for advanced operations
    pub fn inner(&self) -> &DiGraph<DeclarationId, Reference> {
        &self.inner
    }

    /// Get node index for a declaration ID
    pub fn node_index(&self, id: &DeclarationId) -> Option<NodeIndex> {
        self.node_map.get(id).copied()
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}
