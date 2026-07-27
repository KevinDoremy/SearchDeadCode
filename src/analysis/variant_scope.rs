//! src/main symbols alive only through another source set: every
//! incoming reference comes from src/debug, a flavor or tests. In the
//! release build the symbol is dead weight, which the standard report
//! cannot see because the reference graph has no variant dimension.

use crate::graph::Graph;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct VariantFinding {
    pub name: String,
    /// The non-main source sets keeping the symbol alive
    pub sets: Vec<String>,
    pub file: PathBuf,
    pub line: usize,
}

/// The Gradle source set a file belongs to: the path segment right
/// after `src`, if any.
fn source_set_of(path: &Path) -> Option<String> {
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        if component.as_os_str() == "src" {
            if let Some(next) = components.peek() {
                return Some(next.as_os_str().to_string_lossy().to_string());
            }
        }
    }
    None
}

pub fn debug_only_symbols(graph: &Graph) -> Vec<VariantFinding> {
    let mut findings: Vec<VariantFinding> = Vec::new();
    for decl in graph.declarations() {
        // members ride with their type; reporting both is noise
        if decl.parent.is_some() {
            continue;
        }
        match source_set_of(&decl.location.file) {
            Some(set) if set == "main" => {}
            _ => continue,
        }
        let refs = graph.get_references_to(&decl.id);
        if refs.is_empty() {
            // zero references = plain dead code, the standard report owns it
            continue;
        }
        let mut sets: BTreeSet<String> = BTreeSet::new();
        let mut main_side = false;
        for (referencer, _) in refs {
            match source_set_of(&referencer.location.file).as_deref() {
                Some("main") | None => {
                    main_side = true;
                    break;
                }
                Some(other) => {
                    sets.insert(other.to_string());
                }
            }
        }
        if main_side {
            continue;
        }
        findings.push(VariantFinding {
            name: decl.name.clone(),
            sets: sets.into_iter().collect(),
            file: decl.location.file.clone(),
            line: decl.location.line,
        });
    }
    findings.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_set_is_the_segment_after_src() {
        assert_eq!(
            source_set_of(Path::new("/repo/app/src/debug/kotlin/A.kt")).as_deref(),
            Some("debug")
        );
        assert_eq!(
            source_set_of(Path::new("/repo/app/src/main/kotlin/A.kt")).as_deref(),
            Some("main")
        );
    }

    #[test]
    fn a_flat_path_has_no_source_set() {
        assert_eq!(source_set_of(Path::new("/repo/A.kt")), None);
    }
}
