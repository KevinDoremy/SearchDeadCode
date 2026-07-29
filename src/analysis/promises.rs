//! Written deletion promises: a `TODO remove` / `FIXME delete` comment
//! above a declaration is the most consensual cleanup in the backlog.
//! Crossing the promise with the symbol's actual reference count says
//! whether it can be honored today or the migration is still stalled.

use crate::graph::Graph;
use regex::Regex;
use std::path::PathBuf;
use std::sync::LazyLock;

#[derive(Debug, PartialEq, Eq)]
pub enum PromiseState {
    /// Zero references — the promise can be honored now
    Ready,
    /// Still referenced — the migration behind the promise stalled
    StillReferenced(usize),
}

#[derive(Debug)]
pub struct Promise {
    pub symbol: String,
    pub comment: String,
    pub state: PromiseState,
    pub file: PathBuf,
    pub line: usize,
}

static PROMISE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*(?://|\*)\s*.*\b(?:TODO|FIXME)\b[^\n]*\b(?:remove|delete)\b[^\n]*")
        .unwrap()
});

/// How far below the comment the promised declaration may sit.
const MAX_GAP_LINES: usize = 3;

pub fn deletion_promises(graph: &Graph) -> Vec<Promise> {
    let mut files: Vec<PathBuf> = graph
        .declarations()
        .map(|d| d.location.file.clone())
        .collect();
    files.sort();
    files.dedup();

    let mut promises: Vec<Promise> = Vec::new();
    for file in files {
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        for found in PROMISE_RE.find_iter(&content) {
            let comment_line = content[..found.start()].matches('\n').count() + 1;
            // the promised symbol: nearest declaration just below
            let Some(decl) = graph
                .declarations()
                .filter(|d| {
                    d.location.file == file
                        && d.location.line > comment_line
                        && d.location.line <= comment_line + MAX_GAP_LINES
                })
                .min_by_key(|d| d.location.line)
            else {
                continue;
            };
            let refs = graph.get_references_to(&decl.id).len();
            promises.push(Promise {
                symbol: decl.name.clone(),
                comment: found.as_str().trim().to_string(),
                state: if refs == 0 {
                    PromiseState::Ready
                } else {
                    PromiseState::StillReferenced(refs)
                },
                file: file.clone(),
                line: comment_line,
            });
        }
    }
    promises.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    promises
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removal_wording_matches_and_plain_todos_do_not() {
        assert!(PROMISE_RE.is_match("// TODO remove after v2"));
        assert!(PROMISE_RE.is_match("// FIXME delete once migrated"));
        assert!(!PROMISE_RE.is_match("// TODO add better logging"));
    }
}
