//! Interactive triage mode (fzf-style): filter findings by typing, act on
//! them from the keyboard — explain, kill-list, delete with a diff preview.
//!
//! The dialoguer prompts are a thin shell; every decision lives in pure,
//! unit-tested helpers below.

use crate::analysis::DeadCode;
use crate::graph::{DeclarationId, Graph};
use miette::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Run the triage loop. The caller guarantees a real terminal.
pub fn run_triage(
    _graph: &Graph,
    _entry_points: &HashSet<DeclarationId>,
    _reachable: &HashSet<DeclarationId>,
    findings: Vec<DeadCode>,
    _base_path: &Path,
    _undo_script_path: Option<PathBuf>,
) -> Result<()> {
    if findings.is_empty() {
        println!("Nothing to triage — no dead code found.");
        return Ok(());
    }

    // The dialoguer loop lands in a later step of the plan.
    println!("Interactive triage: {} findings.", findings.len());
    Ok(())
}

/// One plain-text row per finding for the fuzzy list. No ANSI: the fuzzy
/// matcher works on the raw string, escape codes would break filtering.
#[allow(dead_code)] // consumed by the dialoguer loop, landing next
fn format_row(dc: &DeadCode, base: &Path, approx_lines: usize) -> String {
    use crate::analysis::{RiskLevel, Severity};

    let symbol = match dc.severity {
        Severity::Error => "✖",
        Severity::Warning => "▲",
        Severity::Info => "·",
    };
    let path = dc
        .declaration
        .location
        .file
        .strip_prefix(base)
        .unwrap_or(&dc.declaration.location.file);
    let risk = match dc.risk {
        RiskLevel::High => "  [risk:high]",
        RiskLevel::Medium => "  [risk:med]",
        RiskLevel::Low => "",
    };
    format!(
        "{} {:<30} {:<10} {}:{}  ~{}L{}",
        symbol,
        dc.declaration.name,
        dc.declaration.kind.display_name(),
        path.display(),
        dc.declaration.location.line,
        approx_lines,
        risk
    )
}

/// Line count of the declaration's byte span, as a size estimate
#[allow(dead_code)] // consumed by the dialoguer loop, landing next
fn approx_lines_in(content: &str, dc: &DeadCode) -> usize {
    let end = dc.declaration.id.end.min(content.len());
    let start = dc.declaration.id.start.min(end);
    let span = content[start..end].trim_end_matches('\n');
    span.bytes().filter(|b| *b == b'\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{DeadCodeIssue, RiskLevel, Severity};
    use crate::graph::{Declaration, DeclarationId, DeclarationKind, Language, Location};

    fn finding(
        name: &str,
        file: &str,
        line: usize,
        severity: Severity,
        risk: RiskLevel,
    ) -> DeadCode {
        let id = DeclarationId::new(PathBuf::from(file), 100, 300);
        let location = Location::new(PathBuf::from(file), line, 1, 100, 300);
        let decl = Declaration::new(
            id,
            name.to_string(),
            DeclarationKind::Class,
            location,
            Language::Kotlin,
        );
        let mut dc = DeadCode::new(decl, DeadCodeIssue::Unreferenced);
        dc.severity = severity;
        dc.risk = risk;
        dc
    }

    #[test]
    fn row_shows_severity_symbol_name_kind_and_location() {
        let dc = finding(
            "ObsoleteWidget",
            "/proj/legacy/Old.kt",
            3,
            Severity::Warning,
            RiskLevel::Low,
        );

        let row = format_row(&dc, Path::new("/proj"), 12);

        assert!(
            row.starts_with("▲ "),
            "warning symbol first, row was: {row}"
        );
        assert!(row.contains("ObsoleteWidget"), "row was: {row}");
        assert!(row.contains("class"), "row was: {row}");
        assert!(
            row.contains("legacy/Old.kt:3"),
            "relative path, row was: {row}"
        );
        assert!(row.contains("~12L"), "size estimate, row was: {row}");
    }

    #[test]
    fn row_tags_medium_and_high_risk_only() {
        let low = finding("A", "/p/A.kt", 1, Severity::Info, RiskLevel::Low);
        let high = finding("B", "/p/B.kt", 1, Severity::Error, RiskLevel::High);

        let low_row = format_row(&low, Path::new("/p"), 1);
        let high_row = format_row(&high, Path::new("/p"), 1);

        assert!(!low_row.contains("risk"), "low risk stays quiet: {low_row}");
        assert!(
            high_row.contains("[risk:high]"),
            "high risk is tagged: {high_row}"
        );
        assert!(high_row.starts_with("✖ "), "error symbol: {high_row}");
    }

    #[test]
    fn rows_contain_no_ansi_escapes() {
        let dc = finding("X", "/p/X.kt", 1, Severity::Warning, RiskLevel::High);

        let row = format_row(&dc, Path::new("/p"), 5);

        assert!(
            !row.contains('\u{1b}'),
            "fuzzy matching needs plain text, row was: {row:?}"
        );
    }

    #[test]
    fn approx_lines_counts_newlines_in_the_declaration_span() {
        let content = "0123456789\nline\nline\nline\n";
        let mut dc = finding("X", "/p/X.kt", 1, Severity::Warning, RiskLevel::Low);
        dc.declaration.id.start = 0;
        dc.declaration.id.end = content.len();

        assert_eq!(approx_lines_in(content, &dc), 4);
    }
}
