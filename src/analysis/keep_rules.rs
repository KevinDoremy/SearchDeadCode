//! ProGuard/R8 -keep rule retention.
//!
//! A class matched by any -keep* rule survives shrinking: the developer
//! declared it dynamically used. All -keep variants are treated as class
//! retention (conservative — -keepclassmembers technically keeps members
//! only, but flagging its class as dead would still be a false positive).

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// A compiled -keep class pattern
pub struct KeepPattern {
    regex: Regex,
}

impl KeepPattern {
    pub fn matches(&self, fqn: &str) -> bool {
        self.regex.is_match(fqn)
    }
}

/// ProGuard wildcards → anchored regex: `**` crosses package separators,
/// `*` stays within one segment, `?` is a single character.
fn pattern_to_regex(spec: &str) -> Option<Regex> {
    let mut out = String::from("^");
    let mut chars = spec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    out.push_str(".*");
                } else {
                    out.push_str("[^.]*");
                }
            }
            '?' => out.push('.'),
            '.' => out.push_str("\\."),
            '$' => out.push_str("\\$"),
            other if other.is_alphanumeric() || other == '_' => out.push(other),
            _ => return None, // member blocks, annotations: not a class name spec
        }
    }
    out.push('$');
    Regex::new(&out).ok()
}

/// The raw class specs of every -keep rule in a rules file
fn class_specs(text: &str) -> Vec<String> {
    let mut specs = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('#') || !line.starts_with("-keep") {
            continue;
        }
        // -keepclassmembers / -keepclassmembernames keep MEMBERS of matching
        // classes, never the classes themselves. Treating Otto's
        // `-keepclassmembers class ** { @Subscribe public *; }` as a keep-all
        // turned every declaration of a real corpus into a retention root.
        if line.starts_with("-keepclassmember") {
            continue;
        }
        // "-keep[variant] [modifiers] class|interface|enum <spec> ..."
        let mut tokens = line.split_whitespace().peekable();
        let _keep = tokens.next();
        let mut spec = None;
        while let Some(token) = tokens.next() {
            if matches!(token, "class" | "interface" | "enum" | "@interface") {
                spec = tokens.next();
                break;
            }
        }
        let Some(spec) = spec else { continue };
        let spec = spec.trim_end_matches('{').trim();
        if spec.is_empty() || spec.starts_with('@') {
            continue;
        }
        specs.push(spec.to_string());
    }
    specs
}

/// Extract class specs from -keep rules in a rules file
pub fn parse_keep_patterns(text: &str) -> Vec<KeepPattern> {
    class_specs(text)
        .iter()
        .filter_map(|spec| pattern_to_regex(spec))
        .map(|regex| KeepPattern { regex })
        .collect()
}

/// Every *.pro file under the root (shallow walk, build/VCS dirs skipped)
fn pro_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if path.is_dir() {
                if depth < 4
                    && !matches!(
                        name.as_ref(),
                        "build" | ".git" | ".gradle" | ".idea" | "node_modules"
                    )
                {
                    stack.push((path, depth + 1));
                }
            } else if name.ends_with(".pro") {
                files.push(path);
            }
        }
    }
    files
}

/// Raw class specs of every class-keeping rule under the root, for
/// callers that need to distinguish exact names from wildcard blankets.
pub fn collect_keep_specs(root: &Path) -> Vec<String> {
    let mut specs = Vec::new();
    for path in pro_files(root) {
        if let Ok(text) = fs::read_to_string(&path) {
            specs.extend(class_specs(&text));
        }
    }
    specs
}

/// Collect keep patterns from every *.pro file under the root
pub fn collect_keep_patterns(root: &Path) -> Vec<KeepPattern> {
    let mut patterns = Vec::new();
    for path in pro_files(root) {
        if let Ok(text) = fs::read_to_string(&path) {
            patterns.extend(parse_keep_patterns(&text));
        }
    }
    patterns
}

/// A -keep rule naming a project class that no longer exists
#[derive(Debug)]
pub struct DeadKeepRule {
    pub spec: String,
    pub file: PathBuf,
}

/// Exact (wildcard-free) -keep specs pointing into a project package
/// but matching no declaration. Library classes are unverifiable from
/// sources alone: a spec whose package the graph never declares is
/// skipped, as is anything with wildcards.
pub fn dead_keep_rules(root: &Path, graph: &crate::graph::Graph) -> Option<Vec<DeadKeepRule>> {
    let files = pro_files(root);
    if files.is_empty() {
        return None;
    }
    let mut fqns: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut packages: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for decl in graph.declarations() {
        if let Some(fqn) = decl.fully_qualified_name.as_deref() {
            fqns.insert(fqn);
            if let Some((package, _)) = fqn.rsplit_once('.') {
                packages.insert(package);
            }
        }
    }

    let mut dead = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for spec in class_specs(&text) {
            if spec.contains('*') || spec.contains('?') {
                continue;
            }
            let Some((package, _)) = spec.rsplit_once('.') else {
                continue;
            };
            if packages.contains(package) && !fqns.contains(spec.as_str()) {
                dead.push(DeadKeepRule {
                    spec,
                    file: file.clone(),
                });
            }
        }
    }
    dead.sort_by(|a, b| a.spec.cmp(&b.spec));
    Some(dead)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(spec: &str, fqn: &str) -> bool {
        pattern_to_regex(spec).is_some_and(|r| r.is_match(fqn))
    }

    #[test]
    fn exact_names_match_exactly() {
        assert!(matches("com.example.Foo", "com.example.Foo"));
        assert!(!matches("com.example.Foo", "com.example.FooBar"));
    }

    #[test]
    fn double_star_crosses_packages_single_star_does_not() {
        assert!(matches("com.example.**", "com.example.deep.Nested"));
        assert!(matches("com.example.*", "com.example.Top"));
        assert!(!matches("com.example.*", "com.example.deep.Nested"));
    }

    #[test]
    fn question_mark_is_one_character() {
        assert!(matches("com.example.Fo?", "com.example.Foo"));
        assert!(!matches("com.example.Fo?", "com.example.Fooo"));
    }

    #[test]
    fn keep_variants_and_modifiers_are_parsed() {
        // -keepclassmembers keeps MEMBERS of matching classes, never the
        // classes: Otto's `-keepclassmembers class ** { @Subscribe public *; }`
        // parsed as a keep-all turned every declaration of a real corpus
        // into a retention root. Only class-keeping variants yield patterns.
        let patterns = parse_keep_patterns(
            "-keepclassmembers public class com.a.B { *; }\n-keepclassmembers class ** { *; }\n-keepnames class com.c.D\n",
        );
        assert_eq!(patterns.len(), 1);
        assert!(patterns[0].matches("com.c.D"));
    }

    #[test]
    fn annotation_specs_are_skipped_not_mismatched() {
        let patterns =
            parse_keep_patterns("-keep @interface com.a.KeepMe\n-keep class @com.a.Keep *\n");
        // @interface spec is a real class name; annotated-class spec is skipped
        assert_eq!(patterns.len(), 1);
        assert!(patterns[0].matches("com.a.KeepMe"));
    }
}
