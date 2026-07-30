// Parser utilities - some reserved for future use
#![allow(dead_code)]

use crate::graph::{Declaration, Location, UnresolvedReference};
use miette::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Result of parsing a source file
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Give every reference back the file's import list.
    ///
    /// `UnresolvedReference::imports` is skipped when serializing, since it is
    /// always a copy of this list and repeating it per reference dominated the
    /// cache file. Resolution reads it from the reference, so a result loaded
    /// from cache has to be rehydrated before use, or every reference resolves
    /// as though the file imported nothing.
    pub fn restore_reference_imports(&mut self) {
        if self.imports.is_empty() {
            return;
        }
        for reference in &mut self.references {
            if reference.imports.is_empty() {
                reference.imports.clone_from(&self.imports);
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Location, ReferenceKind, UnresolvedReference};
    use std::path::PathBuf;

    fn reference(name: &str, imports: Vec<String>) -> UnresolvedReference {
        UnresolvedReference {
            name: name.to_string(),
            qualified_name: None,
            kind: ReferenceKind::Type,
            location: Location::new(PathBuf::from("A.kt"), 1, 1, 0, 1),
            imports,
        }
    }

    fn sample() -> ParseResult {
        let mut r = ParseResult::new();
        r.imports = vec!["a.B".to_string(), "c.D".to_string()];
        r.references = vec![
            reference("B", r.imports.clone()),
            reference("D", r.imports.clone()),
        ];
        r
    }

    /// A cache round trip must not quietly drop the imports resolution needs.
    #[test]
    fn reference_imports_survive_a_cache_round_trip() {
        let original = sample();
        let json = serde_json::to_string(&original).expect("serialize");

        let mut loaded: ParseResult = serde_json::from_str(&json).expect("deserialize");
        assert!(
            loaded.references.iter().all(|r| r.imports.is_empty()),
            "the point of skipping the field is that it is not written"
        );

        loaded.restore_reference_imports();
        for (before, after) in original.references.iter().zip(&loaded.references) {
            assert_eq!(before.imports, after.imports, "reference {}", after.name);
        }
    }

    /// The saving is the whole reason for the change, so it is asserted.
    #[test]
    fn skipping_imports_shrinks_what_is_written() {
        let mut fat = ParseResult::new();
        fat.imports = (0..40).map(|i| format!("some.package.Type{i}")).collect();
        fat.references = (0..200)
            .map(|i| reference(&format!("Type{i}"), fat.imports.clone()))
            .collect();

        let written = serde_json::to_string(&fat).expect("serialize").len();
        let inline: usize = fat
            .references
            .iter()
            .map(|r| {
                serde_json::to_string(&r.imports)
                    .map(|s| s.len())
                    .unwrap_or(0)
            })
            .sum();

        assert!(
            written < inline / 4,
            "expected the file to be far smaller than the inlined copies: \
             wrote {written} bytes, the copies alone would be {inline}"
        );
    }

    /// A file with no imports must not gain any.
    #[test]
    fn a_file_without_imports_restores_to_nothing() {
        let mut r = ParseResult::new();
        r.references = vec![reference("Local", vec![])];
        r.restore_reference_imports();
        assert!(r.references[0].imports.is_empty());
    }
}
