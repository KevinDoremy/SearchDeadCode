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

/// Extract class specs from -keep rules in a rules file
pub fn parse_keep_patterns(text: &str) -> Vec<KeepPattern> {
    let mut patterns = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('#') || !line.starts_with("-keep") {
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
        if let Some(regex) = pattern_to_regex(spec) {
            patterns.push(KeepPattern { regex });
        }
    }
    patterns
}

/// Collect keep patterns from every *.pro file under the root (shallow walk,
/// build/VCS dirs skipped)
pub fn collect_keep_patterns(root: &Path) -> Vec<KeepPattern> {
    let mut patterns = Vec::new();
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
                if let Ok(text) = fs::read_to_string(&path) {
                    patterns.extend(parse_keep_patterns(&text));
                }
            }
        }
    }
    patterns
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
        let patterns = parse_keep_patterns(
            "-keepclassmembers public class com.a.B { *; }\n-keepnames class com.c.D\n",
        );
        assert_eq!(patterns.len(), 2);
        assert!(patterns[0].matches("com.a.B"));
        assert!(patterns[1].matches("com.c.D"));
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
