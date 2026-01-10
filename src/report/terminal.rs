//! Terminal reporter with improved colored output
//!
//! Based on Rust compiler diagnostic design (RFC 1644)

use crate::analysis::{Confidence, DeadCode, Severity};
use crate::report::colors::{BoxChars, ChartChars, ConfidenceIndicator, SeveritySymbol, StructureColors};
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

        // Print summary
        self.print_summary(dead_code);

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

    fn print_summary(&self, dead_code: &[DeadCode]) {
        // Severity counts
        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;

        // Confidence counts
        let mut confirmed = 0;
        let mut high = 0;
        let mut medium = 0;
        let mut low = 0;
        let mut runtime_confirmed_count = 0;

        for item in dead_code {
            match item.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Info => infos += 1,
            }
            match item.confidence {
                Confidence::Confirmed => confirmed += 1,
                Confidence::High => high += 1,
                Confidence::Medium => medium += 1,
                Confidence::Low => low += 1,
            }
            if item.runtime_confirmed {
                runtime_confirmed_count += 1;
            }
        }

        println!("{}", BoxChars::heavy_line(60).dimmed());

        // Severity summary
        let mut severity_parts = Vec::new();
        if errors > 0 {
            severity_parts.push(format!("{} errors", errors).red().to_string());
        }
        if warnings > 0 {
            severity_parts.push(format!("{} warnings", warnings).yellow().to_string());
        }
        if infos > 0 {
            severity_parts.push(format!("{} info", infos).blue().to_string());
        }
        println!("Summary: {}", severity_parts.join(", "));

        // Confidence summary (if showing confidence)
        if self.show_confidence {
            println!();
            println!("{}", "By Confidence:".dimmed());

            let total = dead_code.len() as f64;

            if confirmed > 0 || runtime_confirmed_count > 0 {
                let pct = (confirmed as f64 / total) * 100.0;
                let bar = ChartChars::bar(pct, 15);
                println!(
                    "  {} {} {:>5} │{}│ ({} runtime)",
                    "✓".green().bold(),
                    "confirmed".green(),
                    confirmed,
                    bar.green(),
                    runtime_confirmed_count
                );
            }
            if high > 0 {
                let pct = (high as f64 / total) * 100.0;
                let bar = ChartChars::bar(pct, 15);
                println!(
                    "  {} {} {:>5} │{}│",
                    "!".yellow().bold(),
                    "high     ".yellow(),
                    high,
                    bar.yellow()
                );
            }
            if medium > 0 {
                let pct = (medium as f64 / total) * 100.0;
                let bar = ChartChars::bar(pct, 15);
                println!(
                    "  {} {} {:>5} │{}│",
                    "?".dimmed(),
                    "medium   ".dimmed(),
                    medium,
                    bar.dimmed()
                );
            }
            if low > 0 {
                let pct = (low as f64 / total) * 100.0;
                let bar = ChartChars::bar(pct, 15);
                println!(
                    "  {} {} {:>5} │{}│",
                    "~".dimmed(),
                    "low      ".dimmed(),
                    low,
                    bar.dimmed()
                );
            }
        }

        println!();

        // Tips based on results
        if runtime_confirmed_count > 0 {
            println!(
                "{}",
                format!(
                    "✓ {} items confirmed by runtime coverage - safe to delete",
                    runtime_confirmed_count
                )
                .green()
            );
        }
        if low > 0 {
            println!(
                "{}",
                "⚠ Low confidence items may be false positives (reflection, dynamic calls)"
                    .yellow()
            );
        }
        println!(
            "{}",
            "Tip: Run with --delete to safely remove dead code".dimmed()
        );
        println!(
            "{}",
            "Tip: Use --min-confidence high to filter low confidence results".dimmed()
        );
    }
}

impl Default for TerminalReporter {
    fn default() -> Self {
        Self::new()
    }
}
