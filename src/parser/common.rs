// Parser utilities - some reserved for future use
#![allow(dead_code)]

use crate::graph::{Declaration, Location, UnresolvedReference};
use miette::Result;
use std::path::Path;

/// Result of parsing a source file
#[derive(Debug)]
pub struct ParseResult {
    /// Declarations found in the file
    pub declarations: Vec<Declaration>,

    /// Unresolved references that need to be resolved against other files
    pub references: Vec<UnresolvedReference>,

    /// Package/namespace of the file
    pub package: Option<String>,

    /// Import statements
    pub imports: Vec<String>,
}

impl ParseResult {
    pub fn new() -> Self {
        Self {
            declarations: Vec::new(),
            references: Vec::new(),
            package: None,
            imports: Vec::new(),
        }
    }
}

impl Default for ParseResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for language-specific parsers
pub trait Parser {
    /// Parse a source file and extract declarations and references
    fn parse(&self, path: &Path, contents: &str) -> Result<ParseResult>;
}

/// Helper to convert tree-sitter Point to Location
pub fn point_to_location(
    file: &Path,
    start: tree_sitter::Point,
    _end: tree_sitter::Point,
    start_byte: usize,
    end_byte: usize,
) -> Location {
    Location::new(
        file.to_path_buf(),
        start.row + 1,    // tree-sitter uses 0-indexed lines
        start.column + 1, // tree-sitter uses 0-indexed columns
        start_byte,
        end_byte,
    )
}

/// Extract text from a node
pub fn node_text<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> &'a str {
    &source[node.start_byte()..node.end_byte()]
}

/// Find child node by field name
pub fn child_by_field<'a>(
    node: tree_sitter::Node<'a>,
    field: &str,
) -> Option<tree_sitter::Node<'a>> {
    node.child_by_field_name(field)
}

/// Find all children of a specific kind
pub fn children_of_kind<'a>(node: tree_sitter::Node<'a>, kind: &str) -> Vec<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|child| child.kind() == kind)
        .collect()
}

/// Iterator over all descendant nodes
pub fn descendants(node: tree_sitter::Node) -> impl Iterator<Item = tree_sitter::Node> {
    DescendantIterator::new(node)
}

struct DescendantIterator<'a> {
    cursor: tree_sitter::TreeCursor<'a>,
    done: bool,
}

impl<'a> DescendantIterator<'a> {
    fn new(node: tree_sitter::Node<'a>) -> Self {
        Self {
            cursor: node.walk(),
            done: false,
        }
    }
}

impl<'a> Iterator for DescendantIterator<'a> {
    type Item = tree_sitter::Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let node = self.cursor.node();

        // Try to go to first child
        if self.cursor.goto_first_child() {
            return Some(node);
        }

        // Try to go to next sibling
        loop {
            if self.cursor.goto_next_sibling() {
                return Some(node);
            }

            // Go up to parent
            if !self.cursor.goto_parent() {
                self.done = true;
                return Some(node);
            }
        }
    }
}
