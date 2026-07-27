//! Dead DI modules: a @Module whose every @Provides/@Binds produces a
//! type nobody consumes is a whole DI cluster to delete. The @Module
//! annotation retains the class, so the standard report never says so —
//! this view does, reusing the same consumption rule the entry-point
//! detector applies per binding.

use crate::graph::{DeclarationKind, Graph};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub struct DeadDiModule {
    pub name: String,
    pub bindings: usize,
    pub file: PathBuf,
    pub line: usize,
}

pub fn dead_di_modules(graph: &Graph) -> Vec<DeadDiModule> {
    // providers grouped under their parent class
    let mut providers_by_class: HashMap<String, Vec<&crate::graph::Declaration>> = HashMap::new();
    for decl in graph.declarations() {
        if matches!(
            decl.kind,
            DeclarationKind::Method | DeclarationKind::Function
        ) && decl
            .annotations
            .iter()
            .any(|a| a.contains("Provides") || a.contains("Binds"))
        {
            if let Some(parent) = &decl.parent {
                providers_by_class
                    .entry(parent.to_string())
                    .or_default()
                    .push(decl);
            }
        }
    }

    let mut findings: Vec<DeadDiModule> = Vec::new();
    for decl in graph.declarations() {
        if decl.kind != DeclarationKind::Class
            || !decl.annotations.iter().any(|a| a.contains("Module"))
        {
            continue;
        }
        let Some(providers) = providers_by_class.get(&decl.id.to_string()) else {
            continue; // no bindings at all: nothing to judge
        };
        let any_consumed = providers
            .iter()
            .any(|provider| super::entry_points::di_binding_is_consumed(graph, provider));
        if !any_consumed {
            findings.push(DeadDiModule {
                name: decl.name.clone(),
                bindings: providers.len(),
                file: decl.location.file.clone(),
                line: decl.location.line,
            });
        }
    }
    findings.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    findings
}
