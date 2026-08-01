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
            // Plusieurs candidats = résolution par nom simple, une devinette :
            // le flag permet aux analyses (kill-list, compare) de ne pas
            // s'appuyer dessus. Le builder série le posait déjà.
            let ambiguous = resolved_ids.len() > 1;

            for to_id in resolved_ids {
                // Skip self-references
                if unresolved.from == to_id {
                    continue;
                }

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
                )
                .with_ambiguous(ambiguous);
                graph.add_reference(&unresolved.from, &to_id, reference);
            }
        }
    }

    fn resolve_reference(&self, graph: &Graph, unresolved: &UnresolvedRef) -> Vec<DeclarationId> {
        // Tous les porteurs d'un FQN, cibles d'appel développées, dédupliquées
        // (miroir du builder série : deux surcharges partagent un FQN).
        let expand = |decls: Vec<&Declaration>| -> Vec<DeclarationId> {
            let mut ids: Vec<DeclarationId> = Vec::new();
            for decl in decls {
                let targets = if matches!(
                    unresolved.kind,
                    ReferenceKind::Call | ReferenceKind::Instantiation
                ) {
                    graph.expand_call_target(&decl.id)
                } else {
                    vec![decl.id.clone()]
                };
                for id in targets {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
            ids
        };

        // Try fully qualified name first
        if let Some(fqn) = &unresolved.qualified_name {
            let decls = graph.find_all_by_fqn(fqn);
            if !decls.is_empty() {
                return expand(decls);
            }
        }

        // Try imports
        for import in &unresolved.imports {
            if import.ends_with(".*") {
                let package = &import[..import.len() - 2];
                let fqn = format!("{}.{}", package, unresolved.name);
                let decls = graph.find_all_by_fqn(&fqn);
                if !decls.is_empty() {
                    return expand(decls);
                }
            } else if import.ends_with(&format!(".{}", unresolved.name)) {
                let decls = graph.find_all_by_fqn(import);
                if !decls.is_empty() {
                    return expand(decls);
                }
            } else if let Some(alias_start) = import.find(" as ") {
                let alias = &import[alias_start + 4..];
                if alias == unresolved.name {
                    let original = &import[..alias_start];
                    let decls = graph.find_all_by_fqn(original);
                    if !decls.is_empty() {
                        return expand(decls);
                    }
                }
            }
        }

        // Try simple name match
        let candidates = graph.find_by_name(&unresolved.name);
        if !candidates.is_empty() {
            // Union, pas repli : un homonyme Java quelconque masquait la
            // propriété Kotlin derrière le nom d'accesseur (miroir du
            // builder série).
            let mut ids: Vec<DeclarationId> = candidates.iter().map(|c| c.id.clone()).collect();
            for id in accessor_property_targets(graph, unresolved) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            return ids;
        }

        // Repli accesseur JVM → propriété Kotlin (appel depuis Java)
        let props = accessor_property_targets(graph, unresolved);
        if !props.is_empty() {
            return props;
        }

        Vec::new()
    }
}

/// Cibles propriété/field Kotlin derrière un nom d'accesseur JVM
/// (`getLabel()` appelé depuis Java → `val label`). Kotlin seulement :
/// un champ Java n'engendre aucun accesseur généré, et le rattacher ici
/// ferait passer un `setNickname()` pour une lecture directe du champ.
fn accessor_property_targets(graph: &Graph, unresolved: &UnresolvedRef) -> Vec<DeclarationId> {
    if unresolved.kind != ReferenceKind::Call {
        return Vec::new();
    }
    let Some(prop) = kotlin_property_behind_accessor(&unresolved.name) else {
        return Vec::new();
    };
    graph
        .find_by_name(&prop)
        .iter()
        .filter(|d| {
            d.language == crate::graph::Language::Kotlin
                && matches!(
                    d.kind,
                    crate::graph::DeclarationKind::Property | crate::graph::DeclarationKind::Field
                )
        })
        .map(|d| d.id.clone())
        .collect()
}

impl Default for ParallelGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Nom de propriété Kotlin derrière un accesseur JVM appelé depuis Java :
/// `getMAX_ITEMS` → `MAX_ITEMS`, `getLabel` → `label`, `setLabel` → `label`,
/// `isReady` → `isReady` (Kotlin garde le nom pour les booléens `is*`).
/// Sans ce repli, tout ce que du code Java lit d'un fichier Kotlin passe
/// pour mort : le nom appelé ne correspond à aucune déclaration.
fn kotlin_property_behind_accessor(name: &str) -> Option<String> {
    let rest = name
        .strip_prefix("get")
        .or_else(|| name.strip_prefix("set"))?;
    let mut chars = rest.chars();
    let first = chars.next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    // ALL_CAPS conservé tel quel (convention des constantes), sinon
    // première lettre en minuscule comme le fait le compilateur
    if rest
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
    {
        Some(rest.to_string())
    } else {
        Some(format!("{}{}", first.to_ascii_lowercase(), chars.as_str()))
    }
}
