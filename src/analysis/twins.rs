//! Version twins: Xxx paired with XxxV2 / XxxLegacy / XxxOld…
//!
//! Half of a migration is usually one of these pairs waiting to die.
//! The pair is presented side by side with reference counts so the
//! dominated half is obvious.

use crate::graph::{DeclarationId, DeclarationKind, Graph};
use std::collections::HashMap;

const VERSION_SUFFIXES: &[&str] = &["V2", "V3", "V4", "Legacy", "Old", "New", "Deprecated"];

pub struct TwinSide {
    pub name: String,
    pub refs: usize,
    pub id: DeclarationId,
}

pub struct TwinPair {
    pub base: TwinSide,
    pub variant: TwinSide,
}

pub fn version_twins(graph: &Graph) -> Vec<TwinPair> {
    let mut types: HashMap<&str, &DeclarationId> = HashMap::new();
    for decl in graph.declarations() {
        if decl.parent.is_none()
            && matches!(
                decl.kind,
                DeclarationKind::Class
                    | DeclarationKind::Interface
                    | DeclarationKind::Object
                    | DeclarationKind::Enum
            )
        {
            types.insert(decl.name.as_str(), &decl.id);
        }
    }

    let mut pairs = Vec::new();
    for (name, id) in &types {
        for suffix in VERSION_SUFFIXES {
            let variant_name = format!("{name}{suffix}");
            if let Some(variant_id) = types.get(variant_name.as_str()) {
                pairs.push(TwinPair {
                    base: TwinSide {
                        name: (*name).to_string(),
                        refs: graph.get_references_to(id).len(),
                        id: (*id).clone(),
                    },
                    variant: TwinSide {
                        name: variant_name,
                        refs: graph.get_references_to(variant_id).len(),
                        id: (*variant_id).clone(),
                    },
                });
            }
        }
    }
    pairs.sort_by(|a, b| a.base.name.cmp(&b.base.name));
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Declaration, Language, Location};
    use std::path::PathBuf;

    fn class_decl(name: &str, line: usize) -> Declaration {
        let path = PathBuf::from("Types.kt");
        Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 10),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 10),
            Language::Kotlin,
        )
    }

    #[test]
    fn v2_and_legacy_suffixes_pair_up() {
        let mut graph = Graph::new();
        graph.add_declaration(class_decl("Parser", 1));
        graph.add_declaration(class_decl("ParserV2", 5));
        graph.add_declaration(class_decl("Engine", 9));
        graph.add_declaration(class_decl("EngineLegacy", 13));
        graph.add_declaration(class_decl("Loner", 17));

        let pairs = version_twins(&graph);

        let names: Vec<String> = pairs.iter().map(|p| p.variant.name.clone()).collect();
        assert!(names.contains(&"ParserV2".to_string()));
        assert!(names.contains(&"EngineLegacy".to_string()));
        assert_eq!(pairs.len(), 2, "Loner pairs with nobody");
    }

    #[test]
    fn methods_do_not_pair() {
        let mut graph = Graph::new();
        let mut method = class_decl("computeV2", 1);
        method.kind = DeclarationKind::Function;
        graph.add_declaration(method);
        let mut base = class_decl("compute", 5);
        base.kind = DeclarationKind::Function;
        graph.add_declaration(base);

        assert!(version_twins(&graph).is_empty(), "types only in v1");
    }
}
