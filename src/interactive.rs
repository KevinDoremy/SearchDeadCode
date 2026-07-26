//! Interactive triage mode (fzf-style): filter findings by typing, act on
//! them from the keyboard — explain, kill-list, delete with a diff preview.
//!
//! The dialoguer prompts are a thin shell; every decision lives in pure,
//! unit-tested helpers below.

use crate::analysis::DeadCode;
use crate::graph::{DeclarationId, Graph};
use crate::refactor::safe_delete::Deletion;
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
fn format_row(dc: &DeadCode, base: &Path, approx_lines: usize, dependent: bool) -> String {
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
    let marker = if dependent { " ↯" } else { "" };
    format!(
        "{} {:<30} {:<10} {}:{}  ~{}L{}{}",
        symbol,
        dc.declaration.name,
        dc.declaration.kind.display_name(),
        path.display(),
        dc.declaration.location.line,
        approx_lines,
        risk,
        marker
    )
}

/// Exclusive dependents of a deleted symbol: everything that only stayed
/// alive through it (the target itself excluded)
#[allow(dead_code)] // consumed by the dialoguer loop, landing next
fn dependents_of(
    graph: &Graph,
    entry_points: &HashSet<DeclarationId>,
    deleted: &DeclarationId,
) -> HashSet<DeclarationId> {
    let targets: HashSet<DeclarationId> = std::iter::once(deleted.clone()).collect();
    crate::analysis::kill_list::kill_list(graph, entry_points, &targets)
        .into_iter()
        .filter(|id| id != deleted)
        .collect()
}

/// After a deletion, update the remaining findings of that file: drop those
/// whose line fell inside the removed range (nested members), shift the line
/// and byte offsets of everything below. Other files are untouched.
#[allow(dead_code)] // consumed by the dialoguer loop, landing next
fn apply_deletion(findings: &mut Vec<DeadCode>, file: &Path, del: &Deletion) {
    let removed_end = del.start_line + del.removed_lines; // exclusive
    findings.retain(|dc| {
        dc.declaration.location.file != file
            || dc.declaration.location.line < del.start_line
            || dc.declaration.location.line >= removed_end
    });
    for dc in findings.iter_mut() {
        if dc.declaration.location.file == file && dc.declaration.location.line >= removed_end {
            dc.declaration.location.line -= del.removed_lines;
            dc.declaration.id.start = dc.declaration.id.start.saturating_sub(del.removed_bytes);
            dc.declaration.id.end = dc.declaration.id.end.saturating_sub(del.removed_bytes);
        }
    }
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

        let row = format_row(&dc, Path::new("/proj"), 12, false);

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

        let low_row = format_row(&low, Path::new("/p"), 1, false);
        let high_row = format_row(&high, Path::new("/p"), 1, false);

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

        let row = format_row(&dc, Path::new("/p"), 5, false);

        assert!(
            !row.contains('\u{1b}'),
            "fuzzy matching needs plain text, row was: {row:?}"
        );
    }

    fn deletion(start_line: usize, removed_lines: usize, removed_bytes: usize) -> Deletion {
        Deletion {
            start_line,
            removed_lines,
            removed_bytes,
        }
    }

    #[test]
    fn apply_deletion_drops_findings_inside_the_removed_range() {
        let file = Path::new("/p/Code.kt");
        let mut findings = vec![
            finding(
                "Removed",
                "/p/Code.kt",
                5,
                Severity::Warning,
                RiskLevel::Low,
            ),
            finding("Nested", "/p/Code.kt", 6, Severity::Warning, RiskLevel::Low),
        ];

        apply_deletion(&mut findings, file, &deletion(5, 3, 60));

        assert!(findings.is_empty(), "both fall inside lines 5..8");
    }

    #[test]
    fn apply_deletion_shifts_later_findings_of_the_same_file() {
        let file = Path::new("/p/Code.kt");
        let mut below = finding("Below", "/p/Code.kt", 20, Severity::Warning, RiskLevel::Low);
        below.declaration.id.start = 500;
        below.declaration.id.end = 700;
        let mut findings = vec![below];

        apply_deletion(&mut findings, file, &deletion(5, 3, 60));

        assert_eq!(findings[0].declaration.location.line, 17);
        assert_eq!(findings[0].declaration.id.start, 440);
        assert_eq!(findings[0].declaration.id.end, 640);
    }

    #[test]
    fn apply_deletion_leaves_earlier_findings_untouched() {
        let file = Path::new("/p/Code.kt");
        let mut findings = vec![finding(
            "Above",
            "/p/Code.kt",
            2,
            Severity::Warning,
            RiskLevel::Low,
        )];

        apply_deletion(&mut findings, file, &deletion(5, 3, 60));

        assert_eq!(findings[0].declaration.location.line, 2);
        assert_eq!(findings[0].declaration.id.start, 100);
    }

    #[test]
    fn apply_deletion_ignores_other_files() {
        let mut findings = vec![finding(
            "Elsewhere",
            "/p/Other.kt",
            10,
            Severity::Warning,
            RiskLevel::Low,
        )];

        apply_deletion(&mut findings, Path::new("/p/Code.kt"), &deletion(5, 3, 60));

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].declaration.location.line, 10);
    }

    #[test]
    fn row_marks_dependents_of_a_deleted_symbol() {
        let dc = finding("Orphan", "/p/O.kt", 1, Severity::Warning, RiskLevel::Low);

        let plain = format_row(&dc, Path::new("/p"), 2, false);
        let marked = format_row(&dc, Path::new("/p"), 2, true);

        assert!(!plain.contains('↯'), "plain row: {plain}");
        assert!(marked.ends_with(" ↯"), "marked row: {marked}");
    }

    #[test]
    fn dependents_of_returns_exclusive_dependents_without_the_target() {
        use crate::discovery::{FileType, SourceFile};
        use crate::graph::GraphBuilder;

        let temp = tempfile::tempdir().unwrap();
        let root_path = temp.path().join("Root.kt");
        std::fs::write(
            &root_path,
            "package s\n\nclass Root {\n    fun go() {\n        Leaf().poke()\n    }\n}\n",
        )
        .unwrap();
        let leaf_path = temp.path().join("Leaf.kt");
        std::fs::write(
            &leaf_path,
            "package s\n\nclass Leaf {\n    fun poke() {}\n}\n",
        )
        .unwrap();

        let mut builder = GraphBuilder::new();
        builder
            .process_file(&SourceFile::new(root_path.clone(), FileType::Kotlin))
            .unwrap();
        builder
            .process_file(&SourceFile::new(leaf_path, FileType::Kotlin))
            .unwrap();
        let graph = builder.build();

        let root_id = graph
            .find_by_name("Root")
            .first()
            .map(|d| d.id.clone())
            .expect("Root parsed");
        let leaf_id = graph
            .find_by_name("Leaf")
            .first()
            .map(|d| d.id.clone())
            .expect("Leaf parsed");

        let dependents = dependents_of(&graph, &HashSet::new(), &root_id);

        assert!(!dependents.contains(&root_id), "the target itself is out");
        assert!(
            dependents.contains(&leaf_id),
            "Leaf only lives through Root"
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
