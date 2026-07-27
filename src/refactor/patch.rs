//! Unified diff generation for --patch: deletion as a code review
//! artifact. The output is a plain `git apply`-compatible patch built
//! from the same byte spans --delete would use.

use crate::analysis::DeadCode;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CONTEXT: usize = 3;

/// Build one unified patch covering every finding's whole-line span.
pub fn unified_patch(dead_code: &[DeadCode], base: &Path) -> String {
    // file → deleted 0-based line ranges (inclusive)
    let mut per_file: BTreeMap<PathBuf, Vec<(usize, usize)>> = BTreeMap::new();
    for dc in dead_code {
        let file = &dc.declaration.location.file;
        let Ok(content) = std::fs::read_to_string(file) else {
            continue;
        };
        let (start, end) = (dc.declaration.id.start, dc.declaration.id.end);
        let range = if start < end && end <= content.len() {
            let start_line = content[..start].matches('\n').count();
            let end_line = content[..end].matches('\n').count();
            (start_line, end_line)
        } else {
            let line = dc.declaration.location.line.saturating_sub(1);
            (line, line)
        };
        per_file.entry(file.clone()).or_default().push(range);
    }

    let mut patch = String::new();
    for (file, mut ranges) in per_file {
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        let lines: Vec<&str> = content.lines().collect();

        // merge overlapping/adjacent ranges
        ranges.sort();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (start, end) in ranges {
            match merged.last_mut() {
                Some((_, last_end)) if start <= *last_end + 1 => {
                    *last_end = (*last_end).max(end);
                }
                _ => merged.push((start, end)),
            }
        }

        let rel = file.strip_prefix(base).unwrap_or(&file);
        let rel_display = rel.display().to_string().replace('\\', "/");
        patch.push_str(&format!("--- a/{rel_display}\n+++ b/{rel_display}\n"));

        let mut removed_so_far = 0usize;
        for (start, end) in merged {
            let end = end.min(lines.len().saturating_sub(1));
            let ctx_start = start.saturating_sub(CONTEXT);
            let ctx_end = (end + CONTEXT).min(lines.len().saturating_sub(1));
            let ctx_before = start - ctx_start;
            let ctx_after = ctx_end - end;
            let deleted = end - start + 1;

            let old_start = ctx_start + 1;
            let old_count = ctx_before + deleted + ctx_after;
            let new_count = ctx_before + ctx_after;
            let new_start = if new_count == 0 {
                ctx_start.saturating_sub(removed_so_far)
            } else {
                old_start - removed_so_far
            };

            patch.push_str(&format!(
                "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
            ));
            for line in lines.iter().take(start).skip(ctx_start) {
                patch.push_str(&format!(" {line}\n"));
            }
            for line in lines.iter().take(end + 1).skip(start) {
                patch.push_str(&format!("-{line}\n"));
            }
            for line in lines.iter().take(ctx_end + 1).skip(end + 1) {
                patch.push_str(&format!(" {line}\n"));
            }
            removed_so_far += deleted;
        }
    }
    patch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::DeadCodeIssue;
    use crate::graph::{Declaration, DeclarationId, DeclarationKind, Language, Location};

    #[test]
    fn a_single_block_becomes_one_clean_hunk() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Code.kt");
        let source = "line1\nclass Dead {\n    fun x() {}\n}\nline5\n";
        std::fs::write(&file, source).unwrap();
        let start = source.find("class").unwrap();
        let end = source.find("}\nline5").unwrap() + 1;
        let decl = Declaration::new(
            DeclarationId::new(file.clone(), start, end),
            "Dead".to_string(),
            DeclarationKind::Class,
            Location::new(file.clone(), 2, 1, start, end),
            Language::Kotlin,
        );
        let dc = DeadCode::new(decl, DeadCodeIssue::Unreferenced);

        let patch = unified_patch(&[dc], temp.path());

        assert!(patch.contains("--- a/Code.kt"), "patch was:\n{patch}");
        assert!(patch.contains("-class Dead {"), "patch was:\n{patch}");
        assert!(
            patch.contains(" line1"),
            "context kept, patch was:\n{patch}"
        );
    }
}
