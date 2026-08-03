use crate::analysis::DeadCode;
use crate::refactor::undo::UndoScript;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};
use miette::{IntoDiagnostic, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// What a deletion actually removed. Line-based (brace matching), which can
/// differ from the declaration's byte span — this is the source of truth for
/// shifting the remaining findings of the same file.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Deletion {
    /// 1-based first removed line
    pub start_line: usize,
    /// Number of lines actually removed
    pub removed_lines: usize,
    /// Number of bytes actually removed
    pub removed_bytes: usize,
}

/// Delta-style preview: the exact lines a deletion would remove, prefixed
/// with a red minus. Falls back to a one-line description when the source
/// cannot be read.
pub(crate) fn removal_diff(item: &DeadCode) -> String {
    let id = &item.declaration.id;
    let Ok(content) = std::fs::read_to_string(&id.file) else {
        return format!(
            "  {} {} at {}:{}",
            item.declaration.kind.display_name(),
            item.declaration.name.white(),
            item.declaration.location.file.display(),
            item.declaration.location.line
        );
    };

    let end = id.end.min(content.len());
    let start = id.start.min(end);
    let snippet = &content[start..end];
    let first_line = item.declaration.location.line;

    let mut out = String::new();
    out.push('\n');
    out.push_str(
        &format!(
            "── {}:{} ({} {}) ",
            item.declaration.location.file.display(),
            first_line,
            item.declaration.kind.display_name(),
            item.declaration.name
        )
        .dimmed()
        .to_string(),
    );
    for (offset, line) in snippet.lines().enumerate() {
        out.push('\n');
        out.push_str(&format!(
            "{:>5} {}",
            first_line + offset,
            format!("- {}", line).red()
        ));
    }
    out
}

/// Record the pre-delete state under refs/searchdeadcode/undo. `git
/// stash create` captures modified tracked files without touching the
/// worktree; a clean tree falls back to HEAD (the committed state IS
/// the pre-delete state). No repo, no ref — quietly.
fn record_git_undo(selected: &[&DeadCode]) -> bool {
    let Some(first) = selected.first() else {
        return false;
    };
    let Some(dir) = first.declaration.location.file.parent() else {
        return false;
    };
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
    };

    let Some(created) = git(&["stash", "create", "searchdeadcode undo point"]) else {
        return false;
    };
    if !created.status.success() {
        return false; // not a repo
    }
    let mut sha = String::from_utf8_lossy(&created.stdout).trim().to_string();
    if sha.is_empty() {
        // clean tree: the committed state is the pre-delete state
        let Some(head) = git(&["rev-parse", "HEAD"]) else {
            return false;
        };
        if !head.status.success() {
            return false;
        }
        sha = String::from_utf8_lossy(&head.stdout).trim().to_string();
    }
    matches!(
        git(&["update-ref", "refs/searchdeadcode/undo", &sha]),
        Some(out) if out.status.success()
    )
}

/// The outcome of planning one file's rewrite, before anything is written.
/// `--delete` applies `new_contents`; `--patch` diffs it against the
/// original — both promise exactly what this plan says.
pub(crate) struct RewritePlan<'a> {
    pub deleted: Vec<&'a DeadCode>,
    pub unresolved: Vec<&'a DeadCode>,
    pub parameters_left: Vec<&'a DeadCode>,
    /// One flag per byte of the original contents: true = removed
    pub mask: Vec<bool>,
    pub new_contents: Vec<u8>,
}

/// Plan the removal of `items` from `contents` as one byte mask. Every range
/// is resolved against the same snapshot, so deletions cannot shift one
/// another, and overlapping ranges (a member inside its dead class, one
/// symbol flagged by two rules) merge instead of firing twice. The mask works
/// in BYTES, not lines: a live declaration sharing its line with a dead one
/// keeps its half of the line. Parameters are never masked — removing one
/// changes a signature no call site is rewritten for — except implicitly,
/// when their whole function goes.
pub(crate) fn plan_file_rewrite<'a>(contents: &str, items: &[&'a DeadCode]) -> RewritePlan<'a> {
    let bytes = contents.as_bytes();
    let mut mask = vec![false; bytes.len()];
    let mut deleted = Vec::new();
    let mut unresolved = Vec::new();
    let mut parameters = Vec::new();

    for item in items {
        if item.declaration.kind == crate::graph::DeclarationKind::Parameter {
            parameters.push(*item);
            continue;
        }
        match byte_span(contents, &item.declaration) {
            Some((start, end)) => {
                mark_with_annotations(contents, &mut mask, start, end);
                deleted.push(*item);
            }
            None => unresolved.push(*item),
        }
    }

    // Promote to whole-line removal every line whose meaningful content is
    // entirely masked, newline included: removing `fun dead() = 1` must not
    // leave a blank hole. A line the mask never touched keeps its blanks,
    // and a line where live code survives keeps its newline.
    let mut line_start = 0usize;
    while line_start < bytes.len() {
        let line_end = contents[line_start..]
            .find('\n')
            .map_or(bytes.len(), |n| line_start + n);
        let touched = mask[line_start..line_end].iter().any(|m| *m);
        if touched {
            let nothing_meaningful_left =
                (line_start..line_end).all(|i| mask[i] || bytes[i].is_ascii_whitespace());
            if nothing_meaningful_left {
                for flag in &mut mask[line_start..line_end] {
                    *flag = true;
                }
                if line_end < bytes.len() {
                    mask[line_end] = true; // the newline goes with its line
                }
            }
        }
        line_start = line_end + 1;
    }

    // A parameter needs no note when its whole function is going anyway
    let parameters_left = parameters
        .into_iter()
        .filter(|p| {
            let (start, end) = (p.declaration.id.start, p.declaration.id.end);
            !(start < end && end <= mask.len() && mask[start..end].iter().all(|m| *m))
        })
        .collect();

    let new_contents = bytes
        .iter()
        .enumerate()
        .filter(|(i, _)| !mask[*i])
        .map(|(_, b)| *b)
        .collect();

    RewritePlan {
        deleted,
        unresolved,
        parameters_left,
        mask,
        new_contents,
    }
}

/// The byte range a declaration occupies, when its recorded span is still
/// consistent with the file; otherwise whole lines found by brace matching
/// from the recorded line number. None when even that line is gone.
fn byte_span(contents: &str, decl: &crate::graph::Declaration) -> Option<(usize, usize)> {
    let (start, end) = (decl.id.start, decl.id.end);
    if start < end && end <= contents.len() {
        let start_line = contents[..start].matches('\n').count();
        if start_line == decl.location.line.saturating_sub(1) {
            return Some((start, end));
        }
    }

    let lines: Vec<&str> = contents.lines().collect();
    let first = decl.location.line.saturating_sub(1);
    if first >= lines.len() {
        return None;
    }
    let last = find_declaration_end(&lines, first);
    // Line offsets by scanning for newlines: `lines()` strips `\r`, so
    // summing line lengths would drift on CRLF files.
    let mut line_starts = vec![0usize];
    for (i, b) in contents.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }
    let span_start = *line_starts.get(first)?;
    let span_end = line_starts.get(last + 1).copied().unwrap_or(contents.len());
    Some((span_start, span_end.min(contents.len())))
}

/// Mask `start..end`, plus the annotation lines stacked directly above the
/// declaration and its attached doc comment: a `@Deprecated` or a KDoc left
/// behind would silently re-attach to the next declaration. `@file:` targets
/// the file, never the declaration below.
fn mark_with_annotations(contents: &str, mask: &mut [bool], start: usize, end: usize) {
    let end = end.min(mask.len());
    for flag in &mut mask[start..end] {
        *flag = true;
    }

    let line_start = contents[..start].rfind('\n').map_or(0, |n| n + 1);
    // An annotation on the SAME line, ahead of the declaration
    // (`@Deprecated fun dead()…`), goes too — but only when the whole
    // prefix is annotations, not when live code opens the line.
    let prefix = contents[line_start..start].trim();
    if prefix.starts_with('@') && !prefix.starts_with("@file") {
        for flag in &mut mask[line_start..start] {
            *flag = true;
        }
    }

    // Line ranges (start, end-without-newline) once, for the upward walks
    let mut line_ranges: Vec<(usize, usize)> = Vec::new();
    let mut cursor = 0usize;
    for segment in contents.split_inclusive('\n') {
        let seg_end = cursor + segment.len();
        let text_end = if segment.ends_with('\n') {
            seg_end - 1
        } else {
            seg_end
        };
        line_ranges.push((cursor, text_end));
        cursor = seg_end;
    }
    let Some(decl_line) = line_ranges.iter().position(|(s, _)| *s == line_start) else {
        return;
    };

    let trimmed = |i: usize| {
        let (s, e) = line_ranges[i];
        contents[s..e].trim()
    };
    let paren_debt =
        |line: &str| line.matches(')').count() as isize - line.matches('(').count() as isize;
    let is_annotation_head = |line: &str| line.starts_with('@') && !line.starts_with("@file");

    // Annotations stacked above, each possibly MULTI-line: reading upward, a
    // line with more `)` than `(` is the tail of a block that must close on
    // a line starting with `@`; anything else abandons the block unmasked.
    let mut idx = decl_line;
    while idx > 0 {
        let below = idx - 1;
        let line = trimmed(below);
        let debt = paren_debt(line);
        if is_annotation_head(line) && debt <= 0 {
            let (block_start, _) = line_ranges[below];
            for flag in &mut mask[block_start..line_ranges[idx].0] {
                *flag = true;
            }
            idx = below;
            continue;
        }
        if debt > 0 {
            let mut needed = debt;
            let mut walker = below;
            let mut head = None;
            // Bounded: an unbalanced `)` inside a raw string above the
            // declaration must not drag the walk across half the file.
            while walker > 0 && below - walker < 12 {
                walker -= 1;
                let candidate = trimmed(walker);
                needed += paren_debt(candidate);
                if needed <= 0 {
                    if is_annotation_head(candidate) {
                        head = Some(walker);
                    }
                    break;
                }
            }
            if let Some(head) = head {
                let (block_start, _) = line_ranges[head];
                for flag in &mut mask[block_start..line_ranges[idx].0] {
                    *flag = true;
                }
                idx = head;
                continue;
            }
        }
        break;
    }

    // The doc comment attached above the annotations: a `*/` directly against
    // the block walks up to its opener. Line comments stay — a `//` above a
    // declaration is as often a section header as a doc line.
    if idx > 0 && trimmed(idx - 1).ends_with("*/") {
        let mut walker = idx - 1;
        loop {
            if trimmed(walker).starts_with("/*") {
                let (block_start, _) = line_ranges[walker];
                for flag in &mut mask[block_start..line_ranges[idx].0] {
                    *flag = true;
                }
                break;
            }
            if walker == 0 || idx - walker > 200 {
                break;
            }
            walker -= 1;
        }
    }
}

/// Find the end line of a declaration by brace matching from its first line.
pub(crate) fn find_declaration_end(lines: &[&str], start_line: usize) -> usize {
    let mut brace_count = 0;
    let mut found_open = false;

    for (i, line) in lines.iter().enumerate().skip(start_line) {
        for ch in line.chars() {
            match ch {
                '{' => {
                    brace_count += 1;
                    found_open = true;
                }
                '}' => {
                    brace_count -= 1;
                    if found_open && brace_count == 0 {
                        return i;
                    }
                }
                _ => {}
            }
        }

        // If no braces found on this line and we haven't found any yet,
        // it might be a one-liner
        if i == start_line && !found_open && !line.contains('{') {
            return i;
        }
    }

    start_line
}

/// Safe delete functionality with user confirmation
pub struct SafeDeleter {
    interactive: bool,
    dry_run: bool,
    undo_script_path: Option<PathBuf>,
    assume_yes: bool,
}

impl SafeDeleter {
    pub fn new(interactive: bool, dry_run: bool, undo_script_path: Option<PathBuf>) -> Self {
        Self {
            interactive,
            dry_run,
            undo_script_path,
            assume_yes: false,
        }
    }

    /// Skip confirmation prompts — the CI path (pair with --verify-cmd)
    pub fn with_assume_yes(mut self, yes: bool) -> Self {
        self.assume_yes = yes;
        self
    }

    /// A parameter is never auto-deleted: removing one changes the function's
    /// signature, and no call site is rewritten to match. The finding stays
    /// visible; the fix is a hand edit.
    fn is_hand_edit_only(item: &DeadCode) -> bool {
        item.declaration.kind == crate::graph::DeclarationKind::Parameter
    }

    /// Delete dead code with user confirmation
    pub fn delete(&self, dead_code: &[DeadCode]) -> Result<()> {
        if dead_code.is_empty() {
            println!("{}", "No dead code to delete.".green());
            return Ok(());
        }

        // In dry-run mode, skip selection and preview the removal by running
        // the SAME per-file plans the real run applies: parameters excluded
        // (minus those whose whole function goes — no phantom note), one
        // preview per distinct symbol. The preview must not promise more, or
        // other, than the deletion delivers.
        if self.dry_run {
            let mut by_file: HashMap<PathBuf, Vec<&DeadCode>> = HashMap::new();
            for item in dead_code {
                by_file
                    .entry(item.declaration.location.file.clone())
                    .or_default()
                    .push(item);
            }
            let mut files: Vec<_> = by_file.into_iter().collect();
            files.sort_by(|a, b| a.0.cmp(&b.0));

            println!();
            println!(
                "{}",
                "Dry run — the diff --delete would apply:".yellow().bold()
            );
            let mut seen = std::collections::HashSet::new();
            let mut total = 0usize;
            let mut notes: Vec<String> = Vec::new();
            for (file, items) in files {
                let Ok(contents) = std::fs::read_to_string(&file) else {
                    continue;
                };
                let plan = plan_file_rewrite(&contents, &items);
                for item in plan.deleted {
                    if !seen.insert(item.declaration.id.clone()) {
                        continue;
                    }
                    total += 1;
                    self.print_removal_diff(item);
                }
                for item in plan.unresolved {
                    notes.push(format!(
                        "  would skip '{}': its recorded position no longer matches the file",
                        item.declaration.name
                    ));
                }
                for item in plan.parameters_left {
                    notes.push(format!(
                        "  parameter '{}' left in place — deleting it would change a signature no call site is rewritten for",
                        item.declaration.name
                    ));
                }
            }
            println!();
            println!(
                "{}",
                format!("Total: {total} items would be deleted").dimmed()
            );
            for note in notes {
                println!("{}", note.yellow());
            }
            return Ok(());
        }

        // Get user selection (only in non-dry-run mode)
        let selected = if self.assume_yes {
            dead_code.iter().collect()
        } else if self.interactive {
            self.interactive_select(dead_code)?
        } else {
            self.batch_confirm(dead_code)?
        };

        if selected.is_empty() {
            println!("{}", "No items selected for deletion.".yellow());
            return Ok(());
        }

        // Generate undo script if requested
        let mut undo_script = if self.undo_script_path.is_some() {
            Some(UndoScript::new())
        } else {
            None
        };

        // Git-aware undo: record the pre-delete tree without touching it
        let git_undo_recorded = record_git_undo(&selected);

        // One pass per FILE, not per item. Deleting item by item shifted every
        // remaining finding of the same file: the second deletion checked its
        // now-stale span, fell back to its old line number, and removed
        // whatever sat there — while still printing a checkmark. All ranges
        // are resolved against a single read, so nothing can shift, and a
        // symbol reported by two rules collapses into one removal.
        let mut by_file: HashMap<PathBuf, Vec<&DeadCode>> = HashMap::new();
        for item in selected {
            by_file
                .entry(item.declaration.location.file.clone())
                .or_default()
                .push(item);
        }

        // Perform deletions
        println!();
        println!("{}", "Deleting dead code...".cyan().bold());

        let mut files: Vec<_> = by_file.into_iter().collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        for (file, items) in files {
            match self.delete_file_batch(&file, &items, &mut undo_script) {
                Ok((deleted, unresolved, parameters_left)) => {
                    for item in deleted {
                        println!(
                            "  {} Deleted {} '{}'",
                            "✓".green(),
                            item.declaration.kind.display_name(),
                            item.declaration.name
                        );
                    }
                    for item in unresolved {
                        println!(
                            "  {} Skipped '{}': its recorded position no longer matches the file",
                            "✗".red(),
                            item.declaration.name
                        );
                    }
                    for item in parameters_left {
                        println!(
                            "  {} parameter '{}' left in place — deleting it would change a signature no call site is rewritten for",
                            "→".yellow(),
                            item.declaration.name
                        );
                    }
                }
                Err(e) => {
                    println!("  {} Failed in {}: {}", "✗".red(), file.display(), e);
                }
            }
        }

        // Write undo script
        if let (Some(script), Some(path)) = (undo_script, &self.undo_script_path) {
            script.write(path)?;
            println!();
            println!("{} Undo script saved to: {}", "→".dimmed(), path.display());
        }

        if git_undo_recorded {
            println!(
                "{} Undo anytime: git restore --source refs/searchdeadcode/undo -- .",
                "→".dimmed()
            );
        }

        Ok(())
    }

    fn print_removal_diff(&self, item: &DeadCode) {
        println!("{}", removal_diff(item));
    }

    /// Remove every item of one file in a single read-plan-write pass. The
    /// plan works in BYTES against one snapshot, so deletions cannot shift
    /// one another. Returns the removed items, the items whose recorded
    /// position no longer matched the file, and the parameters left in
    /// place (minus those whose whole function went anyway).
    fn delete_file_batch<'a>(
        &self,
        file: &std::path::Path,
        items: &[&'a DeadCode],
        undo: &mut Option<UndoScript>,
    ) -> Result<(Vec<&'a DeadCode>, Vec<&'a DeadCode>, Vec<&'a DeadCode>)> {
        let contents = std::fs::read_to_string(file).into_diagnostic()?;
        let plan = plan_file_rewrite(&contents, items);

        if !plan.deleted.is_empty() {
            if let Some(script) = undo {
                script.record_file_state(file, &contents);
            }
            std::fs::write(file, &plan.new_contents).into_diagnostic()?;
        }

        Ok((plan.deleted, plan.unresolved, plan.parameters_left))
    }

    /// Delete one item: record the pre-delete state for undo, then remove the
    /// declaration. Returns what was actually removed.
    pub(crate) fn delete_one(
        &self,
        item: &DeadCode,
        undo: &mut Option<UndoScript>,
    ) -> Result<Deletion> {
        let file_path = &item.declaration.location.file;
        let contents = std::fs::read_to_string(file_path).into_diagnostic()?;

        if let Some(script) = undo {
            script.record_file_state(file_path, &contents);
        }

        let lines: Vec<&str> = contents.lines().collect();
        // The parser recorded the exact byte span of the declaration —
        // braces inside strings or comments cannot fool it. Character
        // counting is only the fallback for stale offsets (file edited
        // since the analysis).
        let (start_idx, end_idx) = match Self::span_line_range(&contents, &item.declaration) {
            Some(range) => range,
            None => {
                let start = item.declaration.location.line.saturating_sub(1);
                (start, find_declaration_end(&lines, start))
            }
        };

        let new_lines: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < start_idx || *i > end_idx)
            .map(|(_, line)| *line)
            .collect();
        let new_contents = new_lines.join("\n");
        std::fs::write(file_path, &new_contents).into_diagnostic()?;

        Ok(Deletion {
            start_line: start_idx + 1,
            removed_lines: end_idx - start_idx + 1,
            removed_bytes: contents.len().saturating_sub(new_contents.len()),
        })
    }

    /// Interactive selection mode - confirm each item
    fn interactive_select<'a>(&self, dead_code: &'a [DeadCode]) -> Result<Vec<&'a DeadCode>> {
        let mut selected = Vec::new();

        println!();
        println!(
            "{}",
            "Interactive mode - confirm each deletion:".cyan().bold()
        );
        println!();

        for item in dead_code {
            let prompt = format!(
                "Delete {} '{}' at {}:{}?",
                item.declaration.kind.display_name(),
                item.declaration.name,
                item.declaration.location.file.display(),
                item.declaration.location.line
            );

            if Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(&prompt)
                .default(false)
                .interact()
                .into_diagnostic()?
            {
                selected.push(item);
            }
        }

        Ok(selected)
    }

    /// Batch confirmation - select multiple at once
    fn batch_confirm<'a>(&self, dead_code: &'a [DeadCode]) -> Result<Vec<&'a DeadCode>> {
        let items: Vec<String> = dead_code
            .iter()
            .map(|dc| {
                format!(
                    "{} '{}' at {}:{}",
                    dc.declaration.kind.display_name(),
                    dc.declaration.name,
                    dc.declaration.location.file.display(),
                    dc.declaration.location.line
                )
            })
            .collect();

        println!();
        println!("{}", "Select items to delete:".cyan().bold());
        println!("{}", "(Space to toggle, Enter to confirm)".dimmed());
        println!();

        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
            .items(&items)
            .interact()
            .into_diagnostic()?;

        let selected: Vec<&DeadCode> = selections.into_iter().map(|i| &dead_code[i]).collect();

        // Confirm final selection
        if !selected.is_empty() {
            println!();
            let confirm = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Delete {} items?", selected.len()))
                .default(false)
                .interact()
                .into_diagnostic()?;

            if !confirm {
                return Ok(Vec::new());
            }
        }

        Ok(selected)
    }

    /// Find the end line of a declaration (simple brace matching)
    /// Zero-based line range covered by the declaration's tree-sitter
    /// byte span, when that span is still consistent with the file.
    fn span_line_range(contents: &str, decl: &crate::graph::Declaration) -> Option<(usize, usize)> {
        let (start, end) = (decl.id.start, decl.id.end);
        if start >= end || end > contents.len() {
            return None;
        }
        let start_line = contents[..start].matches('\n').count();
        // A stale span whose start no longer sits on the recorded line
        // is not trusted
        if start_line != decl.location.line.saturating_sub(1) {
            return None;
        }
        let end_line = contents[..end].matches('\n').count();
        Some((start_line, end_line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::DeadCodeIssue;
    use crate::graph::{Declaration, DeclarationId, DeclarationKind, Language, Location};
    use std::path::Path;

    /// Two small classes; byte offsets computed from the literal below.
    const TWO_CLASSES: &str =
        "class Alpha {\n    fun a() {}\n}\n\nclass Beta {\n    fun b() {}\n}\n";

    fn finding_at(file: &Path, name: &str, line: usize, start: usize, end: usize) -> DeadCode {
        let id = DeclarationId::new(file.to_path_buf(), start, end);
        let location = Location::new(file.to_path_buf(), line, 1, start, end);
        let decl = Declaration::new(
            id,
            name.to_string(),
            DeclarationKind::Class,
            location,
            Language::Kotlin,
        );
        DeadCode::new(decl, DeadCodeIssue::Unreferenced)
    }

    #[test]
    fn removal_diff_prefixes_the_doomed_lines() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Alpha.kt");
        std::fs::write(&file, TWO_CLASSES).unwrap();
        let alpha_end = TWO_CLASSES.find("}\n").unwrap() + 1;
        let item = finding_at(&file, "Alpha", 1, 0, alpha_end);

        let diff = removal_diff(&item);

        assert!(diff.contains("- class Alpha {"), "diff was:\n{diff}");
        assert!(
            !diff.contains("Beta"),
            "only the doomed span, diff was:\n{diff}"
        );
    }

    #[test]
    fn removal_diff_falls_back_when_source_is_unreadable() {
        let item = finding_at(Path::new("/nonexistent/X.kt"), "Ghost", 3, 0, 10);

        let diff = removal_diff(&item);

        assert!(diff.contains("Ghost"), "diff was:\n{diff}");
    }

    #[test]
    fn delete_one_reports_what_was_removed_and_records_undo() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Code.kt");
        std::fs::write(&file, TWO_CLASSES).unwrap();
        let item = finding_at(&file, "Alpha", 1, 0, 30);
        let deleter = SafeDeleter::new(false, false, None);
        let mut undo = Some(UndoScript::new());

        let deletion = deleter.delete_one(&item, &mut undo).unwrap();

        assert_eq!(deletion.start_line, 1);
        assert_eq!(deletion.removed_lines, 3, "class block spans three lines");
        assert!(deletion.removed_bytes > 0);
        let remaining = std::fs::read_to_string(&file).unwrap();
        assert!(!remaining.contains("Alpha"));
        assert!(remaining.contains("Beta"), "the neighbor survives");
        assert_eq!(undo.as_ref().unwrap().file_count(), 1);
    }

    #[test]
    fn a_brace_inside_a_string_does_not_truncate_the_deletion() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Tricky.kt");
        let source = "class Tricky {\n    val stray = \"}\"\n    fun logic() {\n        println(stray)\n    }\n}\n\nclass Neighbor {\n    fun ok() {}\n}\n";
        std::fs::write(&file, source).unwrap();
        let tricky_end = source.find("\n\nclass Neighbor").unwrap();
        let item = finding_at(&file, "Tricky", 1, 0, tricky_end);
        let deleter = SafeDeleter::new(false, false, None);
        let mut undo = None;

        deleter.delete_one(&item, &mut undo).unwrap();

        let remaining = std::fs::read_to_string(&file).unwrap();
        assert!(
            !remaining.contains("stray") && !remaining.contains("logic"),
            "the string brace must not truncate the block, remaining:\n{remaining}"
        );
        assert!(
            remaining.contains("Neighbor"),
            "the neighbor survives, remaining:\n{remaining}"
        );
    }

    #[test]
    fn a_brace_inside_a_comment_does_not_truncate_the_deletion() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Commented.kt");
        let source = "class Commented {\n    // legacy brace } left in a comment\n    fun logic() {\n        println(1)\n    }\n}\n\nclass Neighbor {\n    fun ok() {}\n}\n";
        std::fs::write(&file, source).unwrap();
        let end = source.find("\n\nclass Neighbor").unwrap();
        let item = finding_at(&file, "Commented", 1, 0, end);
        let deleter = SafeDeleter::new(false, false, None);
        let mut undo = None;

        deleter.delete_one(&item, &mut undo).unwrap();

        let remaining = std::fs::read_to_string(&file).unwrap();
        assert!(
            !remaining.contains("logic"),
            "the commented brace must not truncate the block, remaining:\n{remaining}"
        );
        assert!(remaining.contains("Neighbor"));
    }

    #[test]
    fn inconsistent_offsets_fall_back_to_brace_matching() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Stale.kt");
        std::fs::write(&file, TWO_CLASSES).unwrap();
        // Offsets from a stale analysis: end beyond the file
        let item = finding_at(&file, "Alpha", 1, 0, 10_000);
        let deleter = SafeDeleter::new(false, false, None);
        let mut undo = None;

        deleter.delete_one(&item, &mut undo).unwrap();

        let remaining = std::fs::read_to_string(&file).unwrap();
        assert!(!remaining.contains("Alpha"), "remaining:\n{remaining}");
        assert!(remaining.contains("Beta"), "remaining:\n{remaining}");
    }

    #[test]
    fn two_successive_deletes_in_the_same_file_hit_the_right_blocks() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Code.kt");
        std::fs::write(&file, TWO_CLASSES).unwrap();
        let deleter = SafeDeleter::new(false, false, None);
        let mut undo = None;

        let alpha = finding_at(&file, "Alpha", 1, 0, 30);
        let first = deleter.delete_one(&alpha, &mut undo).unwrap();

        // Beta was declared at line 5; after the first deletion it shifted up
        let shifted_line = 5 - first.removed_lines - 1; // one blank line also gone? no: lines strictly inside the block
        let beta_line = 5 - first.removed_lines;
        let _ = shifted_line;
        let beta = finding_at(&file, "Beta", beta_line, 0, 0);
        deleter.delete_one(&beta, &mut undo).unwrap();

        let remaining = std::fs::read_to_string(&file).unwrap();
        assert!(!remaining.contains("Alpha"), "remaining:\n{remaining}");
        assert!(!remaining.contains("Beta"), "remaining:\n{remaining}");
        assert!(
            !remaining.contains("fun"),
            "both bodies gone, remaining:\n{remaining}"
        );
    }
}
