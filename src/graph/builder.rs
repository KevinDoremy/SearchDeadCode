use super::{Declaration, DeclarationId, Graph, Reference, ReferenceKind};
use crate::discovery::{FileType, SourceFile};
use crate::parser::{JavaParser, KotlinParser, Parser as SourceParser};
use miette::Result;
use tracing::debug;

/// Builder for constructing the reference graph
pub struct GraphBuilder {
    /// The graph being built
    graph: Graph,

    /// Kotlin parser
    kotlin_parser: KotlinParser,

    /// Java parser
    java_parser: JavaParser,

    /// Unresolved references to be resolved after all files are parsed
    unresolved_references: Vec<UnresolvedRef>,
}

struct UnresolvedRef {
    from: DeclarationId,
    name: String,
    qualified_name: Option<String>,
    kind: ReferenceKind,
    imports: Vec<String>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            kotlin_parser: KotlinParser::new(),
            java_parser: JavaParser::new(),
            unresolved_references: Vec::new(),
        }
    }

    /// Process a source file and add its declarations to the graph
    pub fn process_file(&mut self, file: &SourceFile) -> Result<()> {
        let contents = file.read_contents()?;

        match file.file_type {
            FileType::Kotlin => {
                self.process_kotlin_file(&file.path, &contents)?;
            }
            FileType::Java => {
                self.process_java_file(&file.path, &contents)?;
            }
            FileType::XmlManifest
            | FileType::XmlLayout
            | FileType::XmlNavigation
            | FileType::XmlMenu => {
                // XML files are processed separately for entry point detection
            }
            FileType::XmlOther => {
                // Ignore other XML files
            }
        }

        Ok(())
    }

    fn process_kotlin_file(&mut self, path: &std::path::Path, contents: &str) -> Result<()> {
        debug!("Parsing Kotlin file: {}", path.display());

        let parse_result = self.kotlin_parser.parse(path, contents)?;

        // Add declarations to graph (clone since we need to reference them later)
        let declarations = parse_result.declarations.clone();
        for decl in parse_result.declarations {
            self.graph.add_declaration(decl);
        }

        // Store unresolved references for later resolution
        self.store_unresolved_references(&declarations, parse_result.references);

        Ok(())
    }

    fn process_java_file(&mut self, path: &std::path::Path, contents: &str) -> Result<()> {
        debug!("Parsing Java file: {}", path.display());

        let parse_result = self.java_parser.parse(path, contents)?;

        // Add declarations to graph (clone since we need to reference them later)
        let declarations = parse_result.declarations.clone();
        for decl in parse_result.declarations {
            self.graph.add_declaration(decl);
        }

        // Store unresolved references for later resolution
        self.store_unresolved_references(&declarations, parse_result.references);

        Ok(())
    }

    /// Store unresolved references, attributing each to the correct enclosing declaration
    fn store_unresolved_references(
        &mut self,
        declarations: &[Declaration],
        references: Vec<crate::graph::UnresolvedReference>,
    ) {
        for unresolved in references {
            // Find the declaration that CONTAINS this reference (by byte range)
            // This ensures references are attributed to the correct enclosing declaration
            let ref_byte = unresolved.location.start_byte;

            // First try to find the innermost declaration that contains this reference
            let from_decl = declarations
                .iter()
                .filter(|d| {
                    d.location.file == unresolved.location.file
                        && d.id.start <= ref_byte
                        && d.id.end >= ref_byte
                })
                // Pick the smallest (innermost) containing declaration
                .min_by_key(|d| d.id.end - d.id.start);

            // Fallback: use any declaration from the same file (file-level reference)
            let from_decl = from_decl.or_else(|| {
                declarations
                    .iter()
                    .find(|d| d.location.file == unresolved.location.file)
            });

            if let Some(from_decl) = from_decl {
                self.unresolved_references.push(UnresolvedRef {
                    from: from_decl.id.clone(),
                    name: unresolved.name,
                    qualified_name: unresolved.qualified_name,
                    kind: unresolved.kind,
                    imports: unresolved.imports,
                });
            }
        }
    }

    /// Build the final graph, resolving all references
    pub fn build(mut self) -> Graph {
        self.resolve_references();
        self.graph
    }

    /// Resolve all unresolved references
    fn resolve_references(&mut self) {
        let references = std::mem::take(&mut self.unresolved_references);

        for unresolved in references {
            let resolved_ids = self.resolve_reference(&unresolved);
            for to_id in resolved_ids {
                // Skip self-references (e.g., property referencing itself in initialization)
                // These are artifacts of parsing and don't represent actual code usage
                if unresolved.from == to_id {
                    continue;
                }

                // Skip cross-file same-name references for properties/fields
                // When two files have properties with the same name, simple-name resolution
                // incorrectly creates references between them. This is especially problematic
                // for write-only detection where properties in different classes should be
                // analyzed independently.
                if let Some(from_decl) = self.graph.get_declaration(&unresolved.from) {
                    if let Some(to_decl) = self.graph.get_declaration(&to_id) {
                        // Skip if: same name AND from different files AND target is a property/field
                        if from_decl.name == to_decl.name
                            && from_decl.location.file != to_decl.location.file
                            && matches!(
                                to_decl.kind,
                                super::DeclarationKind::Property | super::DeclarationKind::Field
                            )
                        {
                            continue;
                        }
                    }
                }

                let reference = Reference::new(
                    unresolved.kind,
                    super::Location::new(
                        unresolved.from.file.clone(),
                        0, // Line info not preserved in unresolved ref
                        0,
                        unresolved.from.start,
                        unresolved.from.end,
                    ),
                    unresolved.name.clone(),
                );
                self.graph
                    .add_reference(&unresolved.from, &to_id, reference);
            }
        }
    }

    /// Try to resolve a reference to declarations (may return multiple for overloaded functions)
    fn resolve_reference(&self, unresolved: &UnresolvedRef) -> Vec<DeclarationId> {
        // Try fully qualified name first
        if let Some(fqn) = &unresolved.qualified_name {
            if let Some(decl) = self.graph.find_by_fqn(fqn) {
                return vec![decl.id.clone()];
            }
        }

        // Try to resolve using imports
        for import in &unresolved.imports {
            // Star import
            if import.ends_with(".*") {
                let package = &import[..import.len() - 2];
                let fqn = format!("{}.{}", package, unresolved.name);
                if let Some(decl) = self.graph.find_by_fqn(&fqn) {
                    return vec![decl.id.clone()];
                }
            }
            // Specific import
            else if import.ends_with(&format!(".{}", unresolved.name)) {
                if let Some(decl) = self.graph.find_by_fqn(import) {
                    return vec![decl.id.clone()];
                }
            }
            // Aliased import (Kotlin)
            else if let Some(alias_start) = import.find(" as ") {
                let alias = &import[alias_start + 4..];
                if alias == unresolved.name {
                    let original = &import[..alias_start];
                    if let Some(decl) = self.graph.find_by_fqn(original) {
                        return vec![decl.id.clone()];
                    }
                }
            }
        }

        // Try simple name match - return ALL candidates for overloaded functions
        let candidates = self.graph.find_by_name(&unresolved.name);
        if !candidates.is_empty() {
            // For ambiguous references (overloaded functions), mark all as referenced
            // This is conservative but avoids false positives
            return candidates.iter().map(|c| c.id.clone()).collect();
        }

        Vec::new()
    }
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_builder_creation() {
        let builder = GraphBuilder::new();
        let graph = builder.build();
        assert_eq!(graph.declaration_count(), 0);
    }
}
