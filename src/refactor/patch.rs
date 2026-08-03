//! Unified diff generation for --patch: deletion as a code review
//! artifact. The patch is derived from the SAME rewrite plan `--delete`
//! applies — one byte mask per file — so it promises exactly what the
//! deletion delivers: parameters excluded, overlapping findings merged,
//! shared lines shrunk instead of removed. The previous implementation
//! built one hunk per finding with its own context and line arithmetic;
//! two findings close together produced overlapping hunks `git apply`
//! rejected, and parameters bypassed the hand-edit gate entirely.

use crate::analysis::DeadCode;
use crate::refactor::safe_delete::plan_file_rewrite;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CONTEXT: usize = 3;

/// How one line of the original fares under the rewrite plan.
#[derive(Clone, PartialEq)]
enum LineFate {
    Kept,
    Dropped,
    /// Live code shares the line with a removed span: the line shrinks
    Shrunk(String),
}

/// Build one unified patch covering every finding, one file at a time.
pub fn unified_patch(dead_code: &[DeadCode], base: &Path) -> String {
    let mut per_file: BTreeMap<PathBuf, Vec<&DeadCode>> = BTreeMap::new();
    for dc in dead_code {
        per_file
            .entry(dc.declaration.location.file.clone())
            .or_default()
            .push(dc);
    }

    let mut patch = String::new();
    for (file, items) in per_file {
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        let plan = plan_file_rewrite(&content, &items);
        if plan.deleted.is_empty() {
            continue;
        }

        let fates = line_fates(&content, &plan.mask);
        if fates.iter().all(|f| *f == LineFate::Kept) {
            continue;
        }

        let rel = file.strip_prefix(base).unwrap_or(&file);
        let rel_display = rel.display().to_string().replace('\\', "/");
        patch.push_str(&format!("--- a/{rel_display}\n+++ b/{rel_display}\n"));
        patch.push_str(&hunks(&content, &fates));
    }
    patch
}

/// Classify each line of the original against the byte mask.
fn line_fates(content: &str, mask: &[bool]) -> Vec<LineFate> {
    let bytes = content.as_bytes();
    let mut fates = Vec::new();
    let mut line_start = 0usize;
    while line_start < bytes.len() {
        let line_end = content[line_start..]
            .find('\n')
            .map_or(bytes.len(), |n| line_start + n);
        let masked = |i: usize| mask.get(i).copied().unwrap_or(false);
        let touched = (line_start..line_end).any(masked);
        let fate = if !touched {
            LineFate::Kept
        } else if (line_start..line_end).all(masked) {
            LineFate::Dropped
        } else {
            let survivor: String = (line_start..line_end)
                .filter(|i| !masked(*i))
                .map(|i| bytes[i] as char)
                .collect();
            let survivor = survivor.trim_end_matches('\r').to_string();
            LineFate::Shrunk(survivor)
        };
        fates.push(fate);
        line_start = line_end + 1;
    }
    fates
}

/// Emit hunks with shared context: change blocks whose context windows touch
/// merge into one hunk, so no two hunks ever overlap.
fn hunks(content: &str, fates: &[LineFate]) -> String {
    let lines: Vec<&str> = content.lines().collect();

    // Contiguous runs of changed lines, as (start, end) inclusive
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    for (i, fate) in fates.iter().enumerate() {
        if *fate == LineFate::Kept {
            continue;
        }
        match blocks.last_mut() {
            Some((_, end)) if i <= *end + 2 * CONTEXT + 1 => *end = i,
            _ => blocks.push((i, i)),
        }
    }

    let mut out = String::new();
    let mut removed_so_far = 0usize;
    for (block_start, block_end) in blocks {
        let ctx_start = block_start.saturating_sub(CONTEXT);
        let ctx_end = (block_end + CONTEXT).min(lines.len().saturating_sub(1));

        let mut old_count = 0usize;
        let mut new_count = 0usize;
        let mut body = String::new();
        for i in ctx_start..=ctx_end {
            match &fates[i] {
                LineFate::Kept => {
                    body.push_str(&format!(" {}\n", lines[i]));
                    old_count += 1;
                    new_count += 1;
                }
                LineFate::Dropped => {
                    body.push_str(&format!("-{}\n", lines[i]));
                    old_count += 1;
                }
                LineFate::Shrunk(survivor) => {
                    body.push_str(&format!("-{}\n", lines[i]));
                    body.push_str(&format!("+{survivor}\n"));
                    old_count += 1;
                    new_count += 1;
                }
            }
        }

        let old_start = ctx_start + 1;
        let new_start = if new_count == 0 {
            ctx_start.saturating_sub(removed_so_far)
        } else {
            old_start - removed_so_far
        };
        out.push_str(&format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
        ));
        out.push_str(&body);
        removed_so_far += old_count - new_count;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::DeadCodeIssue;
    use crate::graph::{Declaration, DeclarationId, DeclarationKind, Language, Location};
    use std::process::Command;

    fn finding(
        file: &std::path::Path,
        source: &str,
        name: &str,
        needle: &str,
        end_needle: &str,
    ) -> DeadCode {
        let start = source.find(needle).unwrap();
        let end = source.find(end_needle).unwrap() + end_needle.len();
        let line = source[..start].matches('\n').count() + 1;
        let decl = Declaration::new(
            DeclarationId::new(file.to_path_buf(), start, end),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(file.to_path_buf(), line, 1, start, end),
            Language::Kotlin,
        );
        DeadCode::new(decl, DeadCodeIssue::Unreferenced)
    }

    #[test]
    fn a_single_block_becomes_one_clean_hunk() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Code.kt");
        let source = "line1\nclass Dead {\n    fun x() {}\n}\nline5\n";
        std::fs::write(&file, source).unwrap();
        let dc = finding(&file, source, "Dead", "class", "}\n");

        let patch = unified_patch(&[dc], temp.path());

        assert!(patch.contains("--- a/Code.kt"), "patch was:\n{patch}");
        assert!(patch.contains("-class Dead {"), "patch was:\n{patch}");
        assert!(
            patch.contains(" line1"),
            "context kept, patch was:\n{patch}"
        );
    }

    #[test]
    fn two_close_findings_produce_a_patch_git_can_apply() {
        // Two findings four lines apart once produced two hunks whose
        // context windows overlapped: `git apply` rejected the whole patch.
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        let file = repo.join("Code.kt");
        let source = concat!(
            "package s\n\n",
            "fun deadOne(): Int = 1\n\n",
            "fun liveOne(): Int = 42\n\n",
            "fun deadTwo(): Int = 2\n\n",
            "fun liveTwo(): Int = 7\n",
        );
        std::fs::write(&file, source).unwrap();
        let one = finding(&file, source, "deadOne", "fun deadOne", "= 1");
        let two = finding(&file, source, "deadTwo", "fun deadTwo", "= 2");

        let patch = unified_patch(&[one, two], repo);
        let patch_file = repo.join("changes.patch");
        std::fs::write(&patch_file, &patch).unwrap();

        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args)
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        let applied = git(&["apply", "--check", "changes.patch"]);
        assert!(
            applied.status.success(),
            "git apply rejected the patch:\n{}\npatch was:\n{patch}",
            String::from_utf8_lossy(&applied.stderr)
        );
    }

    #[test]
    fn a_parameter_finding_produces_no_patch() {
        // The patch path once bypassed the hand-edit gate and emitted a diff
        // removing a live function's signature line.
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Code.kt");
        let source = "package s\n\nfun compute(used: Int, neverRead: String): Int {\n    return used * 2\n}\n";
        std::fs::write(&file, source).unwrap();
        let start = source.find("neverRead").unwrap();
        let end = start + "neverRead: String".len();
        let decl = Declaration::new(
            DeclarationId::new(file.clone(), start, end),
            "neverRead".to_string(),
            DeclarationKind::Parameter,
            Location::new(file.clone(), 3, 24, start, end),
            Language::Kotlin,
        );
        let dc = DeadCode::new(decl, DeadCodeIssue::UnusedParameter);

        let patch = unified_patch(&[dc], temp.path());

        assert!(
            patch.is_empty(),
            "a parameter is a hand edit, patch was:\n{patch}"
        );
    }
}
