//! --fix: zero-risk automatic cleanup, v1 scope = unused imports.
//!
//! An import is removable when its bound name (last segment, or the alias
//! after `as`) never appears outside the import lines. Conservative on
//! purpose: a mention in a comment or a string keeps the import; star
//! imports are never touched.

use regex::Regex;

/// One removable import: line index (0-based) and the line text
#[derive(Debug, PartialEq)]
pub struct UnusedImport {
    pub line_index: usize,
    pub line: String,
}

/// The name an import binds in this file
fn bound_name(import_line: &str) -> Option<String> {
    let rest = import_line.trim().strip_prefix("import ")?.trim();
    let rest = rest.strip_prefix("static ").unwrap_or(rest);
    let rest = rest.trim_end_matches(';').trim();
    if rest.ends_with(".*") {
        return None; // star import: usage cannot be proven textually
    }
    if let Some((_, alias)) = rest.split_once(" as ") {
        return Some(alias.trim().to_string());
    }
    rest.rsplit('.').next().map(|s| s.to_string())
}

/// Find the removable imports of a source file
pub fn unused_imports(content: &str) -> Vec<UnusedImport> {
    let lines: Vec<&str> = content.lines().collect();
    let import_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with("import "))
        .map(|(i, _)| i)
        .collect();

    let body: String = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| !import_indices.contains(i))
        .map(|(_, l)| *l)
        .collect::<Vec<_>>()
        .join("\n");

    import_indices
        .into_iter()
        .filter_map(|i| {
            let name = bound_name(lines[i])?;
            let used = Regex::new(&format!(r"\b{}\b", regex::escape(&name)))
                .ok()?
                .is_match(&body);
            if used {
                None
            } else {
                Some(UnusedImport {
                    line_index: i,
                    line: lines[i].to_string(),
                })
            }
        })
        .collect()
}

/// Remove the given import lines; returns the new content
pub fn remove_imports(content: &str, unused: &[UnusedImport]) -> String {
    let doomed: std::collections::HashSet<usize> = unused.iter().map(|u| u.line_index).collect();
    let kept: Vec<&str> = content
        .lines()
        .enumerate()
        .filter(|(i, _)| !doomed.contains(i))
        .map(|(_, l)| l)
        .collect();
    let mut out = kept.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_and_aliased_unused_imports_are_found() {
        let content = "import a.b.Ghost\nimport a.b.Phantom as Ph\nimport a.b.Used\n\nfun main() { Used() }\n";
        let unused = unused_imports(content);
        let names: Vec<&str> = unused.iter().map(|u| u.line.trim()).collect();
        assert_eq!(names, vec!["import a.b.Ghost", "import a.b.Phantom as Ph"]);
    }

    #[test]
    fn star_imports_are_never_reported() {
        let content = "import a.b.*\n\nfun main() {}\n";
        assert!(unused_imports(content).is_empty());
    }

    #[test]
    fn an_alias_in_use_keeps_its_import() {
        let content = "import a.b.Phantom as Ph\n\nfun main() { Ph() }\n";
        assert!(unused_imports(content).is_empty());
    }

    #[test]
    fn java_static_and_semicolon_forms_are_handled() {
        let content = "import static a.b.C.dead;\nimport a.b.Alive;\n\nclass X { Alive a; }\n";
        let unused = unused_imports(content);
        assert_eq!(unused.len(), 1);
        assert!(unused[0].line.contains("dead"));
    }

    #[test]
    fn removal_only_drops_the_doomed_lines() {
        let content = "import a.Ghost\nimport a.Used\n\nfun main() { Used() }\n";
        let unused = unused_imports(content);
        let out = remove_imports(content, &unused);
        assert_eq!(out, "import a.Used\n\nfun main() { Used() }\n");
    }
}
