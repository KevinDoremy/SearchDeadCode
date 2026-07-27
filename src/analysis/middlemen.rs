//! Middle-man classes: every method is an expression-body forward to
//! the same delegate (`fun x(...) = engine.x(...)`). Callers can talk
//! to the delegate directly — the façade is a post-migration leftover.
//!
//! Two methods minimum (one thin adapter method is not evidence) and
//! one single receiver (routing between delegates is a decision, not
//! forwarding).

use crate::graph::{DeclarationKind, Graph};
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::LazyLock;

#[derive(Debug)]
pub struct Middleman {
    pub class: String,
    pub receiver: String,
    pub methods: usize,
    pub file: PathBuf,
    pub line: usize,
}

/// Signature end, optional return type, then `= receiver.method(`.
static DELEGATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\)\s*(?::\s*[\w<>?.\s]+?)?\s*=\s*(\w+)\s*\.\s*\w+\s*\(").unwrap()
});

pub fn middlemen(graph: &Graph) -> Vec<Middleman> {
    // methods grouped under their parent class
    let mut by_class: HashMap<String, Vec<&crate::graph::Declaration>> = HashMap::new();
    let mut classes: HashMap<String, &crate::graph::Declaration> = HashMap::new();
    for decl in graph.declarations() {
        match decl.kind {
            DeclarationKind::Class => {
                classes.insert(decl.id.to_string(), decl);
            }
            DeclarationKind::Method | DeclarationKind::Function => {
                if let Some(parent) = &decl.parent {
                    by_class.entry(parent.to_string()).or_default().push(decl);
                }
            }
            _ => {}
        }
    }

    let mut file_cache: HashMap<PathBuf, String> = HashMap::new();
    let mut findings: Vec<Middleman> = Vec::new();
    for (parent_key, methods) in by_class {
        let Some(class) = classes.get(&parent_key) else {
            continue;
        };
        if methods.len() < 2 {
            continue;
        }
        let mut receivers: BTreeSet<String> = BTreeSet::new();
        let mut all_delegate = true;
        for method in &methods {
            let content = file_cache
                .entry(method.id.file.clone())
                .or_insert_with(|| std::fs::read_to_string(&method.id.file).unwrap_or_default());
            let text = content
                .get(method.id.start..method.id.end)
                .unwrap_or_default();
            // only the part before the first block brace: an
            // expression body has none, a block body is disqualifying
            let head = text.split('{').next().unwrap_or(text);
            match DELEGATE_RE.captures(head) {
                Some(cap) => {
                    receivers.insert(cap[1].to_string());
                }
                None => {
                    all_delegate = false;
                    break;
                }
            }
        }
        if all_delegate && receivers.len() == 1 {
            let receiver = receivers.into_iter().next().unwrap();
            findings.push(Middleman {
                class: class.name.clone(),
                receiver,
                methods: methods.len(),
                file: class.location.file.clone(),
                line: class.location.line,
            });
        }
    }
    findings.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_body_forward_matches() {
        let caps = DELEGATE_RE
            .captures("fun place(order: String) = engine.place(order)")
            .unwrap();
        assert_eq!(&caps[1], "engine");
    }

    #[test]
    fn a_block_body_does_not_match_before_the_brace() {
        let text = "fun cancelAll(ids: List<Int>) {\n    for (id in ids) engine.cancel(id)\n}";
        let head = text.split('{').next().unwrap();
        assert!(DELEGATE_RE.captures(head).is_none());
    }
}
