//! Terminal reporter with improved colored output
//!
//! Based on Rust compiler diagnostic design (RFC 1644)

use crate::analysis::DeadCode;
use crate::report::colors::{ConfidenceIndicator, SeveritySymbol, StructureColors};
use colored::Colorize;
use miette::Result;
use std::collections::HashMap;
use std::path::PathBuf;

/// Terminal reporter with colored output
pub struct TerminalReporter {
    /// Show confidence levels in output
    show_confidence: bool,
}

impl TerminalReporter {
    pub fn new() -> Self {
        Self {
            show_confidence: true,
        }
    }

    pub fn with_confidence(mut self, show: bool) -> Self {
        self.show_confidence = show;
        self
    }

    pub fn report(&self, dead_code: &[DeadCode]) -> Result<()> {
        if dead_code.is_empty() {
            println!("{}", "No dead code found!".green().bold());
            return Ok(());
        }

        // Group by file
        let mut by_file: HashMap<PathBuf, Vec<&DeadCode>> = HashMap::new();
        for item in dead_code {
            by_file
                .entry(item.declaration.location.file.clone())
                .or_default()
                .push(item);
        }

        // Print header
        println!();
        println!(
            "Found {} dead code issues:",
            StructureColors::count(&dead_code.len().to_string())
        );
        println!();

        // Print legend if showing confidence
        if self.show_confidence {
            self.print_legend();
        }

        // Print by file
        let mut files: Vec<_> = by_file.keys().collect();
        files.sort();

        for file in files {
            let items = &by_file[file];

            // File header
            println!("{}", StructureColors::file_path(&file.display().to_string()));

            // Sort items by line number
            let mut sorted_items: Vec<_> = items.iter().collect();
            sorted_items.sort_by_key(|i| i.declaration.location.line);

            for item in sorted_items {
                self.print_item(item);
            }

            println!();
        }

        // Summary is now printed by Reporter (full summary at the end)
        Ok(())
    }

    fn print_legend(&self) {
        println!("{}", "Confidence Legend:".dimmed());
        println!(
            "  {} {} {} {}",
            "✓".green().bold(),
            "Confirmed (runtime)".dimmed(),
            "!".yellow().bold(),
            "High".dimmed()
        );
        println!(
            "  {} {} {} {}",
            "?".dimmed(),
            "Medium".dimmed(),
            "~".dimmed().italic(),
            "Low".dimmed()
        );
        println!();
    }

    fn print_item(&self, item: &DeadCode) {
        let severity_symbol = SeveritySymbol::colored(&item.severity);

        let location = format!(
            "{:>5}:{:<3}",
            item.declaration.location.line, item.declaration.location.column
        );

        // Build confidence indicator
        let confidence_indicator = if self.show_confidence {
            format!(
                "{} ",
                ConfidenceIndicator::for_level(&item.confidence, item.runtime_confirmed)
            )
        } else {
            String::new()
        };

        // Runtime confirmed badge
        let runtime_badge = if item.runtime_confirmed {
            " [RUNTIME]".green().bold().to_string()
        } else {
            String::new()
        };

        // Issue code
        let issue_code = StructureColors::rule_code(item.issue.code());

        println!(
            "  {}{} {} [{}] {}{}",
            confidence_indicator,
            StructureColors::location(&location),
            severity_symbol,
            issue_code,
            item.message,
            runtime_badge
        );

        // Print declaration info
        println!(
            "    {} {} '{}'",
            "→".dimmed(),
            item.declaration.kind.display_name().dimmed(),
            StructureColors::symbol_name(&item.declaration.name)
        );
    }

}

impl Default for TerminalReporter {
    fn default() -> Self {
        Self::new()
    }
}
