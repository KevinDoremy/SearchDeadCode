// Parallel graph builder using rayon

use super::{Declaration, DeclarationId, Graph, Location, Reference, ReferenceKind};
use crate::discovery::{FileType, SourceFile};
use crate::parser::{JavaParser, KotlinParser, Parser as SourceParser};
use miette::Result;
use rayon::prelude::*;
use tracing::{debug, info};

/// Parsed file result
struct ParsedFile {
    declarations: Vec<Declaration>,
    unresolved_refs: Vec<UnresolvedRef>,
}

struct UnresolvedRef {
    from: DeclarationId,
    name: String,
    qualified_name: Option<String>,
    kind: ReferenceKind,
    imports: Vec<String>,
}

/// Parallel graph builder for faster processing
pub struct ParallelGraphBuilder;

impl ParallelGraphBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Build graph from source files using parallel processing
    pub fn build_from_files(&self, files: &[SourceFile]) -> Result<Graph> {
        info!("Parsing {} files in parallel...", files.len());

        // Parse files in parallel
        let results: Vec<Result<ParsedFile>> =
            files.par_iter().map(|file| self.parse_file(file)).collect();

        // Collect results
        let mut all_declarations = Vec::new();
        let mut all_unresolved = Vec::new();

        for result in results {
            match result {
                Ok(parsed) => {
                    all_declarations.extend(parsed.declarations);
                    all_unresolved.extend(parsed.unresolved_refs);
                }
                Err(e) => {
                    debug!("Parse error (continuing): {}", e);
                }
            }
        }

        info!(
            "Parsed {} declarations, {} unresolved references",
            all_declarations.len(),
            all_unresolved.len()
        );

        // Build graph
        let mut graph = Graph::new();
        for decl in all_declarations {
            graph.add_declaration(decl);
        }

        // Resolve references
        info!("Resolving references...");
        self.resolve_references(&mut graph, all_unresolved);

        Ok(graph)
    }

    /// Parse a single file
    fn parse_file(&self, file: &SourceFile) -> Result<ParsedFile> {
        let contents = file.read_contents()?;

        match file.file_type {
            FileType::Kotlin => self.parse_kotlin_file(&file.path, &contents),
            FileType::Java => self.parse_java_file(&file.path, &contents),
            _ => Ok(ParsedFile {
                declarations: Vec::new(),
                unresolved_refs: Vec::new(),
            }),
        }
    }

    fn parse_kotlin_file(&self, path: &std::path::Path, contents: &str) -> Result<ParsedFile> {
        let parser = KotlinParser::new();
        let result = parser.parse(path, contents)?;

        let declarations = result.declarations.clone();
        let unresolved = self.extract_unresolved(&declarations, result.references);

        Ok(ParsedFile {
            declarations: result.declarations,
            unresolved_refs: unresolved,
        })
    }

    fn parse_java_file(&self, path: &std::path::Path, contents: &str) -> Result<ParsedFile> {
        let parser = JavaParser::new();
        let result = parser.parse(path, contents)?;

        let declarations = result.declarations.clone();
        let unresolved = self.extract_unresolved(&declarations, result.references);

        Ok(ParsedFile {
            declarations: result.declarations,
            unresolved_refs: unresolved,
        })
    }

    fn extract_unresolved(
        &self,
        declarations: &[Declaration],
        references: Vec<crate::graph::UnresolvedReference>,
    ) -> Vec<UnresolvedRef> {
        let mut result = Vec::new();

        for unresolved in references {
            let ref_byte = unresolved.location.start_byte;

            // Find innermost containing declaration
            let from_decl = declarations
                .iter()
                .filter(|d| {
                    d.location.file == unresolved.location.file
                        && d.id.start <= ref_byte
                        && d.id.end >= ref_byte
                })
                .min_by_key(|d| d.id.end - d.id.start);

            let from_decl = from_decl.or_else(|| {
                declarations
                    .iter()
                    .find(|d| d.location.file == unresolved.location.file)
            });

            if let Some(from_decl) = from_decl {
                result.push(UnresolvedRef {
                    from: from_decl.id.clone(),
                    name: unresolved.name,
                    qualified_name: unresolved.qualified_name,
                    kind: unresolved.kind,
                    imports: unresolved.imports,
                });
            }
        }

        result
    }

    fn resolve_references(&self, graph: &mut Graph, unresolved: Vec<UnresolvedRef>) {
        for unresolved in unresolved {
            let resolved_ids = self.resolve_reference(graph, &unresolved);
            for to_id in resolved_ids {
                let reference = Reference::new(
                    unresolved.kind,
                    Location::new(
                        unresolved.from.file.clone(),
                        0,
                        0,
                        unresolved.from.start,
                        unresolved.from.end,
                    ),
                    unresolved.name.clone(),
                );
                graph.add_reference(&unresolved.from, &to_id, reference);
            }
        }
    }

    fn resolve_reference(&self, graph: &Graph, unresolved: &UnresolvedRef) -> Vec<DeclarationId> {
        // Try fully qualified name first
        if let Some(fqn) = &unresolved.qualified_name {
            if let Some(decl) = graph.find_by_fqn(fqn) {
                return vec![decl.id.clone()];
            }
        }

        // Try imports
        for import in &unresolved.imports {
            if import.ends_with(".*") {
                let package = &import[..import.len() - 2];
                let fqn = format!("{}.{}", package, unresolved.name);
                if let Some(decl) = graph.find_by_fqn(&fqn) {
                    return vec![decl.id.clone()];
                }
            } else if import.ends_with(&format!(".{}", unresolved.name)) {
                if let Some(decl) = graph.find_by_fqn(import) {
                    return vec![decl.id.clone()];
                }
            } else if let Some(alias_start) = import.find(" as ") {
                let alias = &import[alias_start + 4..];
                if alias == unresolved.name {
                    let original = &import[..alias_start];
                    if let Some(decl) = graph.find_by_fqn(original) {
                        return vec![decl.id.clone()];
                    }
                }
            }
        }

        // Try simple name match
        let candidates = graph.find_by_name(&unresolved.name);
        if !candidates.is_empty() {
            return candidates.iter().map(|c| c.id.clone()).collect();
        }

        Vec::new()
    }
}

impl Default for ParallelGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}
