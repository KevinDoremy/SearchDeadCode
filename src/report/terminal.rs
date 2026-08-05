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
    /// Base path used to relativize file headers
    base_path: Option<PathBuf>,
}

impl TerminalReporter {
    pub fn new() -> Self {
        Self {
            show_confidence: true,
            base_path: None,
        }
    }

    pub fn with_confidence(mut self, show: bool) -> Self {
        self.show_confidence = show;
        self
    }

    pub fn with_base_path(mut self, base: PathBuf) -> Self {
        self.base_path = Some(base);
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

        // Less is more: source annotations earn their lines on digestible
        // reports; past that, one line per finding keeps the report scannable
        let annotate = dead_code.len() <= 20;

        // Print by file
        let mut files: Vec<_> = by_file.keys().collect();
        files.sort();

        for file in files {
            let items = &by_file[file];

            // File header, relative to the analyzed root when possible
            let display_path = self
                .base_path
                .as_ref()
                .and_then(|base| file.strip_prefix(base).ok())
                .unwrap_or(file);
            println!(
                "{}",
                StructureColors::file_path(&display_path.display().to_string())
            );

            // Source lines for rustc-style annotations, read once per file
            let source_lines: Option<Vec<String>> = if annotate {
                std::fs::read_to_string(file)
                    .ok()
                    .map(|content| content.lines().map(String::from).collect())
            } else {
                None
            };

            // Sort items by line number — ordre total, deux trouvailles sur
            // la même ligne (deux paramètres, deux règles) doivent tomber
            // toujours dans le même sens
            let mut sorted_items: Vec<_> = items.iter().collect();
            sorted_items.sort_by(|a, b| crate::analysis::report_order(a, b));

            for item in sorted_items {
                self.print_item(item);
                if annotate {
                    self.print_annotation(item, source_lines.as_deref());
                }
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

        // Risk badge: only medium/high, low stays quiet
        let risk_badge = match item.risk {
            crate::analysis::RiskLevel::High => " [risk: high]".red().bold().to_string(),
            crate::analysis::RiskLevel::Medium => " [risk: medium]".yellow().to_string(),
            crate::analysis::RiskLevel::Low => String::new(),
        };

        // Issue code
        let issue_code = StructureColors::rule_code(item.issue.code());

        println!(
            "  {}{} {} [{}] {}{}{}",
            confidence_indicator,
            StructureColors::location(&location),
            severity_symbol,
            issue_code,
            item.message,
            runtime_badge,
            risk_badge
        );
    }

    /// Rustc-style annotation: the offending source line, an underline and a
    /// per-finding next step. Falls back to a plain declaration line when the
    /// source is unreadable.
    fn print_annotation(&self, item: &DeadCode, source_lines: Option<&[String]>) {
        let line_no = item.declaration.location.line;
        let src = source_lines.and_then(|lines| line_no.checked_sub(1).and_then(|i| lines.get(i)));

        let Some(src) = src else {
            println!(
                "    {} {} '{}'",
                "→".dimmed(),
                item.declaration.kind.display_name().dimmed(),
                StructureColors::symbol_name(&item.declaration.name)
            );
            return;
        };

        let src = src.trim_end();
        let name = &item.declaration.name;
        let (pad, caret_len) = match src.find(name.as_str()) {
            Some(byte_pos) => (src[..byte_pos].chars().count(), name.chars().count().max(1)),
            None => (
                item.declaration.location.column.saturating_sub(1),
                src.chars().count().max(1),
            ),
        };

        println!("      {}", "|".dimmed());
        println!("{:>5} {} {}", line_no, "|".dimmed(), src);
        println!(
            "      {} {}{} {}",
            "|".dimmed(),
            " ".repeat(pad),
            "^".repeat(caret_len).yellow().bold(),
            "declared here".dimmed()
        );
        println!(
            "      {} {}",
            "=".dimmed(),
            format!("help: searchdeadcode --explain {}", name).dimmed()
        );
    }
}

impl Default for TerminalReporter {
    fn default() -> Self {
        Self::new()
    }
}
