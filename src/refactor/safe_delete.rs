use crate::analysis::DeadCode;
use crate::refactor::undo::UndoScript;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};
use miette::{IntoDiagnostic, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Safe delete functionality with user confirmation
pub struct SafeDeleter {
    interactive: bool,
    dry_run: bool,
    undo_script_path: Option<PathBuf>,
}

impl SafeDeleter {
    pub fn new(interactive: bool, dry_run: bool, undo_script_path: Option<PathBuf>) -> Self {
        Self {
            interactive,
            dry_run,
            undo_script_path,
        }
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

        // In dry-run mode, skip selection and show all candidates
        if self.dry_run {
            println!();
            println!("{}", "Dry run - would delete:".yellow().bold());
            for item in dead_code {
                println!(
                    "  {} {} at {}:{}",
                    item.declaration.kind.display_name(),
                    item.declaration.name.white(),
                    item.declaration.location.file.display(),
                    item.declaration.location.line
                );
            }
            println!();
            println!(
                "{}",
                format!("Total: {} items would be deleted", dead_code.len()).dimmed()
            );
            return Ok(());
        }

        // Get user selection (only in non-dry-run mode)
        let selected = if self.interactive {
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
            if let Some(ref mut script) = undo_script {
                // Record for undo
                if let Ok(contents) = std::fs::read_to_string(&item.declaration.location.file) {
                    script.record_file_state(&item.declaration.location.file, &contents);
                }
            }

            // Perform deletion
            match self.delete_declaration(item) {
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

    /// Delete a single declaration from its file
    fn delete_declaration(&self, dead_code: &DeadCode) -> Result<()> {
        let file_path = &dead_code.declaration.location.file;
        let contents = std::fs::read_to_string(file_path).into_diagnostic()?;

        let lines: Vec<&str> = contents.lines().collect();
        let start_line = dead_code.declaration.location.line.saturating_sub(1);

        // Find the end of the declaration (simple heuristic)
        let end_line = self.find_declaration_end(&lines, start_line);

        // Remove the lines
        let mut new_lines: Vec<&str> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i < start_line || i > end_line {
                new_lines.push(line);
            }
        }

        // Write back
        let new_contents = new_lines.join("\n");
        std::fs::write(file_path, new_contents).into_diagnostic()?;

        Ok(())
    }

    /// Find the end line of a declaration (simple brace matching)
    fn find_declaration_end(&self, lines: &[&str], start_line: usize) -> usize {
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
