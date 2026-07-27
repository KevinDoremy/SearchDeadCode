//! Near-duplicate function bodies across files: the copy-paste a
//! v1→v2 migration leaves behind. Same simple name, different files,
//! bodies whose normalized lines overlap at 80%+ (Jaccard) — once v1
//! dies, this shows what stayed duplicated in v2.

use crate::graph::{DeclarationKind, Graph};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

#[derive(Debug)]
pub struct NearTwin {
    pub name: String,
    pub left: (PathBuf, usize),
    pub right: (PathBuf, usize),
    pub similarity: f64,
}

const MIN_SIMILARITY: f64 = 0.8;
/// A one-liner matching another one-liner proves nothing.
const MIN_LINES: usize = 3;

fn normalized_lines(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "{" && *l != "}")
        .map(str::to_string)
        .collect()
}

fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

pub fn near_twins(graph: &Graph) -> Vec<NearTwin> {
    let mut by_name: HashMap<&str, Vec<&crate::graph::Declaration>> = HashMap::new();
    for decl in graph.declarations() {
        if matches!(
            decl.kind,
            DeclarationKind::Function | DeclarationKind::Method
        ) {
            by_name.entry(decl.name.as_str()).or_default().push(decl);
        }
    }

    let mut file_cache: HashMap<PathBuf, String> = HashMap::new();
    let mut findings: Vec<NearTwin> = Vec::new();
    for (name, decls) in by_name {
        if decls.len() < 2 {
            continue;
        }
        for i in 0..decls.len() {
            for j in (i + 1)..decls.len() {
                let (a, b) = (decls[i], decls[j]);
                if a.id.file == b.id.file {
                    continue; // overloads and local helpers repeat legitimately
                }
                let body = |d: &crate::graph::Declaration,
                            cache: &mut HashMap<PathBuf, String>|
                 -> BTreeSet<String> {
                    let content = cache
                        .entry(d.id.file.clone())
                        .or_insert_with(|| std::fs::read_to_string(&d.id.file).unwrap_or_default());
                    normalized_lines(content.get(d.id.start..d.id.end).unwrap_or_default())
                };
                let lines_a = body(a, &mut file_cache);
                let lines_b = body(b, &mut file_cache);
                if lines_a.len() < MIN_LINES || lines_b.len() < MIN_LINES {
                    continue;
                }
                let similarity = jaccard(&lines_a, &lines_b);
                if similarity >= MIN_SIMILARITY {
                    findings.push(NearTwin {
                        name: name.to_string(),
                        left: (a.location.file.clone(), a.location.line),
                        right: (b.location.file.clone(), b.location.line),
                        similarity,
                    });
                }
            }
        }
    }
    findings.sort_by(|a, b| a.name.cmp(&b.name).then(a.left.0.cmp(&b.left.0)));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_bodies_score_one_and_disjoint_score_zero() {
        let a = normalized_lines("  x = 1\n  y = 2\n  return y\n");
        let b = normalized_lines("x = 1\ny = 2\nreturn y");
        assert_eq!(jaccard(&a, &b), 1.0, "normalization ignores indentation");
        let c = normalized_lines("val q = 9\nprintln(q)\nq += 1");
        assert!(jaccard(&a, &c) < 0.2);
    }
}
