//! Inline ignore directives: `// deadcode:ignore(reason)`.
//!
//! A directive on the declaration line or the line above silences every
//! finding for that declaration. The reason is MANDATORY — "shut up"
//! with no why rots into mystery suppressions, so a bare
//! `deadcode:ignore` is refused and reported.

use crate::analysis::DeadCode;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

static IGNORE_DIRECTIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"deadcode:ignore(?:\(([^)]*)\))?").expect("Invalid ignore regex"));

#[derive(Debug, Default)]
pub struct IgnoreOutcome {
    /// (symbol name, reason) pairs that were silenced
    pub ignored: Vec<(String, String)>,
    /// Directives refused because they carried no reason
    pub missing_reason: usize,
}

/// What a line says: no directive, a directive with a reason, or a
/// directive missing its mandatory reason.
enum Directive {
    None,
    WithReason(String),
    MissingReason,
}

fn directive_on(line: &str) -> Directive {
    let Some(captures) = IGNORE_DIRECTIVE.captures(line) else {
        return Directive::None;
    };
    match captures.get(1).map(|m| m.as_str().trim()) {
        Some(reason) if !reason.is_empty() => Directive::WithReason(reason.to_string()),
        _ => Directive::MissingReason,
    }
}

pub fn apply(dead_code: &mut Vec<DeadCode>) -> IgnoreOutcome {
    let mut cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let mut outcome = IgnoreOutcome::default();

    dead_code.retain(|dc| {
        let file = &dc.declaration.location.file;
        let lines = cache.entry(file.clone()).or_insert_with(|| {
            std::fs::read_to_string(file)
                .map(|c| c.lines().map(String::from).collect())
                .unwrap_or_default()
        });
        let decl_idx = dc.declaration.location.line.saturating_sub(1);
        let above_idx = decl_idx.checked_sub(1);
        let candidates = [Some(decl_idx), above_idx];
        for idx in candidates.into_iter().flatten() {
            match lines.get(idx).map(|l| directive_on(l)) {
                Some(Directive::WithReason(reason)) => {
                    outcome.ignored.push((dc.declaration.name.clone(), reason));
                    return false;
                }
                Some(Directive::MissingReason) => {
                    outcome.missing_reason += 1;
                    return true; // refused: the finding stays visible
                }
                _ => {}
            }
        }
        true
    });

    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason_of(line: &str) -> Option<String> {
        match directive_on(line) {
            Directive::WithReason(r) => Some(r),
            _ => None,
        }
    }

    #[test]
    fn a_reasoned_directive_is_parsed() {
        assert_eq!(
            reason_of("// deadcode:ignore(kept for QA)"),
            Some("kept for QA".to_string())
        );
    }

    #[test]
    fn a_bare_directive_is_missing_its_reason() {
        assert!(matches!(
            directive_on("// deadcode:ignore"),
            Directive::MissingReason
        ));
        assert!(matches!(
            directive_on("// deadcode:ignore(   )"),
            Directive::MissingReason
        ));
    }

    #[test]
    fn ordinary_lines_carry_no_directive() {
        assert!(matches!(directive_on("class Zombie {"), Directive::None));
        assert!(matches!(
            directive_on("// this code is dead, ignore it later"),
            Directive::None
        ));
    }

    #[test]
    fn a_trailing_directive_counts() {
        assert_eq!(
            reason_of("class Zombie { // deadcode:ignore(legacy bridge)"),
            Some("legacy bridge".to_string())
        );
    }
}
