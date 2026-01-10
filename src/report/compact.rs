//! Compact terminal reporter - minimal output format
//!
//! One line per issue, optimized for scanning large result sets

use crate::analysis::DeadCode;
use crate::report::colors::{BoxChars, ConfidenceIndicator, SeveritySymbol, StructureColors};
use colored::Colorize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Compact reporter for minimal, scannable output
pub struct CompactReporter {
    /// Base path to strip from file paths for shorter display
    base_path: Option<PathBuf>,
    /// Show confidence indicators
    show_confidence: bool,
    /// Maximum width for file paths (truncate if longer)
    max_path_width: usize,
}

impl CompactReporter {
    pub fn new() -> Self {
        Self {
            base_path: None,
            show_confidence: true,
            max_path_width: 60,
        }
    }

    pub fn with_base_path(mut self, path: PathBuf) -> Self {
        self.base_path = Some(path);
        self
    }

    pub fn with_confidence(mut self, show: bool) -> Self {
        self.show_confidence = show;
        self
    }

    /// Format a path relative to base path if set
    fn format_path(&self, path: &Path) -> String {
        let display = if let Some(base) = &self.base_path {
            path.strip_prefix(base)
                .unwrap_or(path)
                .display()
                .to_string()
        } else {
            path.display().to_string()
        };

        // Truncate if too long
        if display.len() > self.max_path_width {
            format!("...{}", &display[display.len() - self.max_path_width + 3..])
        } else {
            display
        }
    }

    pub fn report(&self, dead_code: &[DeadCode]) {
        if dead_code.is_empty() {
            println!("{}", "No issues found!".green().bold());
            return;
        }

        // Group by file
        let mut by_file: HashMap<PathBuf, Vec<&DeadCode>> = HashMap::new();
        for item in dead_code {
            by_file
                .entry(item.declaration.location.file.clone())
                .or_default()
                .push(item);
        }

        // Sort files
        let mut files: Vec<_> = by_file.keys().collect();
        files.sort();

        // Print each file's issues
        for file in files {
            let items = &by_file[file];
            let path_str = self.format_path(file);
            println!("{}", StructureColors::file_path(&path_str));

            // Sort items by line number
            let mut sorted_items: Vec<_> = items.iter().collect();
            sorted_items.sort_by_key(|i| i.declaration.location.line);

            for item in sorted_items {
                self.print_item(item);
            }
            println!();
        }

        // Print compact summary
        self.print_summary(dead_code);
    }

    fn print_item(&self, item: &DeadCode) {
        let location = format!(
            "{:>5}:{:<3}",
            item.declaration.location.line, item.declaration.location.column
        );

        let severity_symbol = SeveritySymbol::colored(&item.severity);
        let rule_code = StructureColors::rule_code(item.issue.code());

        // Build the message - extract key info
        let short_message = self.shorten_message(&item.message, &item.declaration.name);

        // Confidence indicator
        let confidence = if self.show_confidence {
            format!(
                "{} ",
                ConfidenceIndicator::for_level(&item.confidence, item.runtime_confirmed)
            )
        } else {
            String::new()
        };

        println!(
            "  {}{}  {}  {}  {}",
            confidence,
            StructureColors::location(&location),
            severity_symbol,
            rule_code,
            short_message
        );
    }

    /// Shorten message to essential info
    fn shorten_message(&self, message: &str, name: &str) -> String {
        // If message contains the name, try to extract the key part
        if message.len() > 60 {
            // Find key patterns and shorten
            if let Some(pos) = message.find(". Consider") {
                return format!(
                    "{} '{}'",
                    &message[..pos.min(40)],
                    StructureColors::symbol_name(name)
                );
            }
            if let Some(pos) = message.find(". Use") {
                return format!(
                    "{} '{}'",
                    &message[..pos.min(40)],
                    StructureColors::symbol_name(name)
                );
            }
            // Default: truncate and add name
            return format!(
                "{}... '{}'",
                &message[..40],
                StructureColors::symbol_name(name)
            );
        }

        // Already short enough, just highlight the name if present
        if message.contains(name) {
            message.replace(&format!("'{}'", name), &format!("'{}'", StructureColors::symbol_name(name)))
        } else {
            format!("{} '{}'", message, StructureColors::symbol_name(name))
        }
    }

    fn print_summary(&self, dead_code: &[DeadCode]) {
        use crate::analysis::Severity;

        let errors = dead_code.iter().filter(|d| matches!(d.severity, Severity::Error)).count();
        let warnings = dead_code.iter().filter(|d| matches!(d.severity, Severity::Warning)).count();
        let infos = dead_code.iter().filter(|d| matches!(d.severity, Severity::Info)).count();

        println!("{}", BoxChars::heavy_line(50).dimmed());

        let mut parts = Vec::new();
        if errors > 0 {
            parts.push(format!("{} {}", errors, "errors".red()));
        }
        if warnings > 0 {
            parts.push(format!("{} {}", warnings, "warnings".yellow()));
        }
        if infos > 0 {
            parts.push(format!("{} {}", infos, "info".blue()));
        }

        println!(
            "  {} {} ({})",
            StructureColors::count(&dead_code.len().to_string()),
            "issues".bold(),
            parts.join(", ")
        );
    }
}

impl Default for CompactReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_truncation() {
        let reporter = CompactReporter::new();
        let long_path = Path::new("/very/long/path/that/exceeds/the/maximum/width/setting/for/display/purposes/file.kt");
        let formatted = reporter.format_path(long_path);
        assert!(formatted.len() <= 60);
        assert!(formatted.starts_with("..."));
    }
}
