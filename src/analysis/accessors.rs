//! Dead JavaBean accessor groups. A field is always "used" by its own
//! getter and setter, so per-symbol reports can never say the useful
//! thing: nobody CALLS the getter, so the property is never read.
//! Setter still called → the whole write pipeline runs for nothing.
//! Neither called → field plus both accessors can go together.

use crate::graph::{DeclarationKind, Graph, Language};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub enum AccessorVerdict {
    /// Setter called, getter never — written but never read
    WriteOnly,
    /// Neither accessor called — the property group is dead
    Dead,
}

#[derive(Debug)]
pub struct AccessorFinding {
    pub field: String,
    pub class: String,
    pub verdict: AccessorVerdict,
    pub file: PathBuf,
    pub line: usize,
}

fn capitalized(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn dead_accessors(graph: &Graph) -> Vec<AccessorFinding> {
    // methods indexed by (parent class, name)
    let mut methods: HashMap<(String, String), &crate::graph::Declaration> = HashMap::new();
    for decl in graph.declarations() {
        if decl.kind == DeclarationKind::Method {
            if let Some(parent) = &decl.parent {
                methods.insert((parent.to_string(), decl.name.clone()), decl);
            }
        }
    }

    let mut findings: Vec<AccessorFinding> = Vec::new();
    for field in graph.declarations() {
        if field.kind != DeclarationKind::Field || field.language != Language::Java {
            continue;
        }
        let Some(parent) = &field.parent else {
            continue;
        };
        let parent_key = parent.to_string();
        let cap = capitalized(&field.name);
        let getter = methods
            .get(&(parent_key.clone(), format!("get{cap}")))
            .or_else(|| methods.get(&(parent_key.clone(), format!("is{cap}"))));
        let Some(getter) = getter else {
            continue;
        };
        if !graph.get_references_to(&getter.id).is_empty() {
            continue; // somebody reads it
        }
        // a direct field access from another file is a read/write we
        // cannot classify — stay conservative
        let externally_touched = graph
            .get_references_to(&field.id)
            .iter()
            .any(|(referencer, _)| referencer.location.file != field.location.file);
        if externally_touched {
            continue;
        }
        let setter = methods.get(&(parent_key.clone(), format!("set{cap}")));
        let setter_called = setter
            .map(|s| !graph.get_references_to(&s.id).is_empty())
            .unwrap_or(false);
        let class_name = graph
            .get_declaration(parent)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        findings.push(AccessorFinding {
            field: field.name.clone(),
            class: class_name,
            verdict: if setter_called {
                AccessorVerdict::WriteOnly
            } else {
                AccessorVerdict::Dead
            },
            file: field.location.file.clone(),
            line: field.location.line,
        });
    }
    findings.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalization_matches_bean_conventions() {
        assert_eq!(capitalized("nickname"), "Nickname");
        assert_eq!(capitalized("x"), "X");
        assert_eq!(capitalized(""), "");
    }
}
