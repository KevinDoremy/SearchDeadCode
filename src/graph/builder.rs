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
        self.add_parse_result(parse_result);

        Ok(())
    }

    fn process_java_file(&mut self, path: &std::path::Path, contents: &str) -> Result<()> {
        debug!("Parsing Java file: {}", path.display());

        let parse_result = self.java_parser.parse(path, contents)?;
        self.add_parse_result(parse_result);

        Ok(())
    }

    /// Parse a Kotlin or Java file without adding it to the graph.
    /// Returns None for file types handled elsewhere (XML).
    pub fn parse_source(
        &mut self,
        file: &SourceFile,
    ) -> Result<Option<crate::parser::ParseResult>> {
        let contents = file.read_contents()?;
        match file.file_type {
            FileType::Kotlin => Ok(Some(self.kotlin_parser.parse(&file.path, &contents)?)),
            FileType::Java => Ok(Some(self.java_parser.parse(&file.path, &contents)?)),
            _ => Ok(None),
        }
    }

    /// Add a parse result (fresh or loaded from cache) to the graph
    pub fn add_parse_result(&mut self, parse_result: crate::parser::ParseResult) {
        let declarations = parse_result.declarations.clone();
        for decl in parse_result.declarations {
            self.graph.add_declaration(decl);
        }
        self.store_unresolved_references(&declarations, parse_result.references);
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
            let ambiguous = resolved_ids.len() > 1;
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
                )
                .with_ambiguous(ambiguous);
                self.graph
                    .add_reference(&unresolved.from, &to_id, reference);
            }
        }
    }

    /// Try to resolve a reference to declarations (may return multiple for overloaded functions)
    fn resolve_reference(&self, unresolved: &UnresolvedRef) -> Vec<DeclarationId> {
        // Tous les porteurs d'un FQN, cibles d'appel développées, dédupliquées.
        // Deux surcharges partagent un FQN : ne lier que le vainqueur de
        // collision privait la surcharge publique de ses appels cross-module.
        let expand = |decls: Vec<&Declaration>| -> Vec<DeclarationId> {
            let mut ids: Vec<DeclarationId> = Vec::new();
            for decl in decls {
                let targets = if matches!(
                    unresolved.kind,
                    ReferenceKind::Call | ReferenceKind::Instantiation
                ) {
                    self.graph.expand_call_target(&decl.id)
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
            let decls = self.graph.find_all_by_fqn(fqn);
            if !decls.is_empty() {
                return expand(decls);
            }
        }

        // Try to resolve using imports
        for import in &unresolved.imports {
            // Star import
            if import.ends_with(".*") {
                let package = &import[..import.len() - 2];
                let fqn = format!("{}.{}", package, unresolved.name);
                let decls = self.graph.find_all_by_fqn(&fqn);
                if !decls.is_empty() {
                    return expand(decls);
                }
            }
            // Specific import
            else if import.ends_with(&format!(".{}", unresolved.name)) {
                let decls = self.graph.find_all_by_fqn(import);
                if !decls.is_empty() {
                    return expand(decls);
                }
            }
            // Aliased import (Kotlin)
            else if let Some(alias_start) = import.find(" as ") {
                let alias = &import[alias_start + 4..];
                if alias == unresolved.name {
                    let original = &import[..alias_start];
                    let decls = self.graph.find_all_by_fqn(original);
                    if !decls.is_empty() {
                        return expand(decls);
                    }
                }
            }
        }

        // Try simple name match - return ALL candidates for overloaded functions
        let candidates = self.graph.find_by_name(&unresolved.name);
        if !candidates.is_empty() {
            // For ambiguous references (overloaded functions), mark all as referenced
            // This is conservative but avoids false positives
            let mut ids: Vec<DeclarationId> = candidates.iter().map(|c| c.id.clone()).collect();
            // Union, pas repli : un homonyme Java quelconque (n'importe quel
            // getX() sans rapport) masquait la propriété Kotlin derrière le
            // nom d'accesseur et la laissait sans référence. Plusieurs cibles
            // sont déjà marquées ambiguës par l'appelant.
            for id in self.accessor_property_targets(unresolved) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            return ids;
        }

        // Repli accesseur JVM → propriété Kotlin (appel depuis Java)
        let props = self.accessor_property_targets(unresolved);
        if !props.is_empty() {
            return props;
        }

        Vec::new()
    }

    /// Cibles propriété/field Kotlin derrière un nom d'accesseur JVM
    /// (`getLabel()` appelé depuis Java → `val label`). Kotlin seulement :
    /// un champ Java n'engendre aucun accesseur généré, et le rattacher ici
    /// ferait passer un `setNickname()` pour une lecture directe du champ.
    fn accessor_property_targets(&self, unresolved: &UnresolvedRef) -> Vec<DeclarationId> {
        if unresolved.kind != ReferenceKind::Call {
            return Vec::new();
        }
        let Some(prop) = kotlin_property_behind_accessor(&unresolved.name) else {
            return Vec::new();
        };
        self.graph
            .find_by_name(&prop)
            .iter()
            .filter(|d| {
                d.language == crate::graph::Language::Kotlin
                    && matches!(
                        d.kind,
                        crate::graph::DeclarationKind::Property
                            | crate::graph::DeclarationKind::Field
                    )
            })
            .map(|d| d.id.clone())
            .collect()
    }
}

impl Default for GraphBuilder {
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

/// Noms d'accesseurs Java derrière un accès propriété Kotlin — le miroir de
/// `kotlin_property_behind_accessor`. `interactionCount` lu depuis Kotlin →
/// `getInteractionCount`, écrit → `setInteractionCount` ; `isReady` garde son
/// nom (Kotlin mappe `isX()` sur la propriété `isX`). Le genre de référence
/// décide du sens : une écriture ne prouve pas que le getter est appelé,
/// et l'inverse non plus — sinon un champ écrit une seule fois ressusciterait
/// son getter mort et tuerait la détection write-only.
pub fn java_accessors_behind_property(name: &str, kind: ReferenceKind) -> Vec<String> {
    let mut first = name.chars();
    let Some(head) = first.next() else {
        return Vec::new();
    };
    // Un nom d'accesseur n'est jamais lui-même une propriété synthétique.
    if !head.is_ascii_lowercase() || name.starts_with("get") || name.starts_with("set") {
        return Vec::new();
    }
    let capitalized = format!("{}{}", head.to_ascii_uppercase(), first.as_str());
    match kind {
        ReferenceKind::Write => {
            // `obj.isReady = x` compile vers `setReady()`, pas `setIsReady()`.
            if let Some(rest) = name.strip_prefix("is") {
                let mut c = rest.chars();
                if let Some(head) = c.next() {
                    if head.is_ascii_uppercase() {
                        return vec![
                            format!("set{head}{}", c.as_str()),
                            format!("set{capitalized}"),
                        ];
                    }
                }
            }
            vec![format!("set{capitalized}")]
        }
        ReferenceKind::Read => {
            // `isReady` en Kotlin appelle `isReady()`, pas `getIsReady()`.
            if name.starts_with("is") && name.len() > 2 {
                vec![name.to_string(), format!("get{capitalized}")]
            } else {
                vec![format!("get{capitalized}")]
            }
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_accessors_behind_property() {
        assert_eq!(
            java_accessors_behind_property("interactionCount", ReferenceKind::Read),
            vec!["getInteractionCount"]
        );
        assert_eq!(
            java_accessors_behind_property("interactionCount", ReferenceKind::Write),
            vec!["setInteractionCount"]
        );
        assert_eq!(
            java_accessors_behind_property("isReady", ReferenceKind::Read),
            vec!["isReady", "getIsReady"]
        );
        // Un appel n'est pas un accès propriété : pas de pont.
        assert!(java_accessors_behind_property("count", ReferenceKind::Call).is_empty());
        // Un nom d'accesseur ou un type ne sont pas des propriétés synthétiques.
        assert!(java_accessors_behind_property("getCount", ReferenceKind::Read).is_empty());
        assert!(java_accessors_behind_property("Button", ReferenceKind::Read).is_empty());
    }

    #[test]
    fn test_graph_builder_creation() {
        let builder = GraphBuilder::new();
        let graph = builder.build();
        assert_eq!(graph.declaration_count(), 0);
    }
}
