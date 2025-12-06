use crate::analysis::{Confidence, DeadCode, Severity};
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

    #[allow(dead_code)] // Builder pattern method for future use
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
            "{}",
            format!("Found {} dead code issues:", dead_code.len())
                .yellow()
                .bold()
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
            println!("{}", file.display().to_string().cyan().bold());

            for item in items {
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
            "●".green().bold(),
            "Confirmed (runtime)".dimmed(),
            "◉".bright_green(),
            "High".dimmed()
        );
        println!(
            "  {} {} {} {}",
            "○".yellow(),
            "Medium".dimmed(),
            "◌".red(),
            "Low".dimmed()
        );
        println!();
    }

    fn confidence_indicator(&self, item: &DeadCode) -> colored::ColoredString {
        if item.runtime_confirmed {
            "●".green().bold()
        } else {
            match item.confidence {
                Confidence::Confirmed => "●".green().bold(),
                Confidence::High => "◉".bright_green(),
                Confidence::Medium => "○".yellow(),
                Confidence::Low => "◌".red(),
            }
        }
    }

    fn print_item(&self, item: &DeadCode) {
        let severity_str = match item.severity {
            Severity::Error => "error".red().bold(),
            Severity::Warning => "warning".yellow().bold(),
            Severity::Info => "info".blue().bold(),
        };

        let location = format!(
            "{}:{}",
            item.declaration.location.line, item.declaration.location.column
        );

        // Build confidence badge
        let confidence_badge = if self.show_confidence {
            format!("{} ", self.confidence_indicator(item))
        } else {
            String::new()
        };

        // Runtime confirmed badge
        let runtime_badge = if item.runtime_confirmed {
            " [RUNTIME]".green().bold().to_string()
        } else {
            String::new()
        };

        println!(
            "  {}{} {} [{}] {}{}",
            confidence_badge,
            location.dimmed(),
            severity_str,
            item.issue.code().dimmed(),
            item.message,
            runtime_badge
        );

        // Print declaration info
        println!(
            "    {} {} '{}'",
            "→".dimmed(),
            item.declaration.kind.display_name().dimmed(),
            item.declaration.name.white()
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

        println!("{}", "─".repeat(60).dimmed());

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
            if confirmed > 0 || runtime_confirmed_count > 0 {
                println!(
                    "  {} {} ({} runtime-confirmed)",
                    "●".green().bold(),
                    format!("{} confirmed", confirmed).green(),
                    runtime_confirmed_count
                );
            }
            if high > 0 {
                println!(
                    "  {} {}",
                    "◉".bright_green(),
                    format!("{} high confidence", high).bright_green()
                );
            }
            if medium > 0 {
                println!(
                    "  {} {}",
                    "○".yellow(),
                    format!("{} medium confidence", medium).yellow()
                );
            }
            if low > 0 {
                println!(
                    "  {} {}",
                    "◌".red(),
                    format!("{} low confidence", low).red()
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
