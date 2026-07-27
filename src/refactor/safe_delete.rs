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

    /// Delete dead code with user confirmation
    pub fn delete(&self, dead_code: &[DeadCode]) -> Result<()> {
        if dead_code.is_empty() {
            println!("{}", "No dead code to delete.".green());
            return Ok(());
        }

        // Group by file for batch operations
        let mut by_file: HashMap<PathBuf, Vec<&DeadCode>> = HashMap::new();
        for item in dead_code {
            by_file
                .entry(item.declaration.location.file.clone())
                .or_default()
                .push(item);
        }

        // In dry-run mode, skip selection and preview the removal as a diff
        if self.dry_run {
            println!();
            println!(
                "{}",
                "Dry run — the diff --delete would apply:".yellow().bold()
            );
            for item in dead_code {
                self.print_removal_diff(item);
            }
            println!();
            println!(
                "{}",
                format!("Total: {} items would be deleted", dead_code.len()).dimmed()
            );
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

        // Perform deletions
        println!();
        println!("{}", "Deleting dead code...".cyan().bold());

        for item in &selected {
            // Perform deletion (undo state is recorded inside)
            match self.delete_one(item, &mut undo_script) {
                Ok(_) => {
                    println!(
                        "  {} Deleted {} '{}'",
                        "✓".green(),
                        item.declaration.kind.display_name(),
                        item.declaration.name
                    );
                }
                Err(e) => {
                    println!(
                        "  {} Failed to delete '{}': {}",
                        "✗".red(),
                        item.declaration.name,
                        e
                    );
                }
            }
        }

        // Write undo script
        if let (Some(script), Some(path)) = (undo_script, &self.undo_script_path) {
            script.write(path)?;
            println!();
            println!("{} Undo script saved to: {}", "→".dimmed(), path.display());
        }

        Ok(())
    }

    fn print_removal_diff(&self, item: &DeadCode) {
        println!("{}", removal_diff(item));
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
        let start_idx = item.declaration.location.line.saturating_sub(1);
        let end_idx = self.find_declaration_end(&lines, start_idx);

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
    pub(crate) fn find_declaration_end(&self, lines: &[&str], start_line: usize) -> usize {
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
