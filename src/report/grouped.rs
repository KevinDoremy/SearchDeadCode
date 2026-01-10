//! Grouped reporter - organize issues by rule or category
//!
//! Helps identify patterns across the codebase

use crate::analysis::DeadCode;
use crate::report::aggregator::{Aggregator, IssueGroup};
use crate::report::colors::{BoxChars, ConfidenceIndicator, SeveritySymbol, StructureColors};
use colored::Colorize;
use std::path::{Path, PathBuf};

/// How to group issues
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    /// Group by rule/detector (e.g., AP001, DC001)
    Rule,
    /// Group by category (Architecture, Kotlin, Performance, etc.)
    Category,
    /// Group by severity (Error, Warning, Info)
    Severity,
    /// Group by file (default behavior)
    File,
}

impl std::str::FromStr for GroupBy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rule" | "detector" => Ok(GroupBy::Rule),
            "category" | "cat" => Ok(GroupBy::Category),
            "severity" | "sev" => Ok(GroupBy::Severity),
            "file" => Ok(GroupBy::File),
            _ => Err(format!("Unknown grouping: {}. Use: rule, category, severity, file", s)),
        }
    }
}

/// Grouped reporter for organizing issues
pub struct GroupedReporter {
    /// How to group issues
    group_by: GroupBy,
    /// Base path to strip from file paths
    base_path: Option<PathBuf>,
    /// Maximum items to show per group before collapsing
    max_per_group: usize,
    /// Show all items (no collapsing)
    expand_all: bool,
    /// Specific rule to expand
    expand_rule: Option<String>,
}

impl GroupedReporter {
    pub fn new(group_by: GroupBy) -> Self {
        Self {
            group_by,
            base_path: None,
            max_per_group: 5,
            expand_all: false,
            expand_rule: None,
        }
    }

    pub fn with_base_path(mut self, path: PathBuf) -> Self {
        self.base_path = Some(path);
        self
    }

    pub fn with_max_per_group(mut self, max: usize) -> Self {
        self.max_per_group = max;
        self
    }

    pub fn expand_all(mut self) -> Self {
        self.expand_all = true;
        self
    }

    pub fn expand_rule(mut self, rule: String) -> Self {
        self.expand_rule = Some(rule);
        self
    }

    /// Format a path relative to base path if set
    fn format_path(&self, path: &Path) -> String {
        if let Some(base) = &self.base_path {
            path.strip_prefix(base)
                .unwrap_or(path)
                .display()
                .to_string()
        } else {
            path.display().to_string()
        }
    }

    pub fn report(&self, dead_code: Vec<DeadCode>) {
        if dead_code.is_empty() {
            println!("{}", "No issues found!".green().bold());
            return;
        }

        let aggregator = Aggregator::new();
        let results = aggregator.aggregate(dead_code);

        match self.group_by {
            GroupBy::Rule => self.report_by_rule(&results.by_rule),
            GroupBy::Category => self.report_by_category(&results.by_category, &results.by_rule),
            GroupBy::Severity => self.report_by_severity(&results.by_rule),
            GroupBy::File => self.report_by_file_grouped(&results.by_rule),
        }

        // Print total
        println!();
        println!(
            "{} {} total issues",
            BoxChars::heavy_line(40).dimmed(),
            StructureColors::count(&results.total.to_string())
        );
    }

    fn report_by_rule(&self, groups: &[IssueGroup]) {
        println!();
        println!("{}", "Issues Grouped by Rule".cyan().bold());
        println!("{}", BoxChars::heavy_line(50).dimmed());
        println!();

        for group in groups {
            self.print_rule_group(group);
            println!();
        }
    }

    fn report_by_category(
        &self,
        by_category: &std::collections::HashMap<String, Vec<IssueGroup>>,
        _all_groups: &[IssueGroup],
    ) {
        println!();
        println!("{}", "Issues Grouped by Category".cyan().bold());
        println!("{}", BoxChars::heavy_line(50).dimmed());
        println!();

        // Sort categories
        let mut categories: Vec<_> = by_category.keys().collect();
        categories.sort();

        for category in categories {
            let groups = &by_category[category];
            let total: usize = groups.iter().map(|g| g.count()).sum();

            println!(
                "{} ({} issues)",
                StructureColors::category(category),
                StructureColors::count(&total.to_string())
            );
            println!("{}", BoxChars::light_line(40).dimmed());

            for group in groups {
                let count = group.count();
                let rule = group.issue.code();
                let desc = &group.description;
                println!(
                    "  {}  {:>4}  {}",
                    StructureColors::rule_code(rule),
                    count.to_string().dimmed(),
                    desc
                );
            }
            println!();
        }
    }

    fn report_by_severity(&self, groups: &[IssueGroup]) {
        use crate::analysis::Severity;

        println!();
        println!("{}", "Issues Grouped by Severity".cyan().bold());
        println!("{}", BoxChars::heavy_line(50).dimmed());
        println!();

        // Group by severity
        let mut errors: Vec<&IssueGroup> = vec![];
        let mut warnings: Vec<&IssueGroup> = vec![];
        let mut infos: Vec<&IssueGroup> = vec![];

        for group in groups {
            match group.severity {
                Severity::Error => errors.push(group),
                Severity::Warning => warnings.push(group),
                Severity::Info => infos.push(group),
            }
        }

        if !errors.is_empty() {
            let total: usize = errors.iter().map(|g| g.count()).sum();
            println!("{} ({} issues)", "Errors".red().bold(), total);
            println!("{}", BoxChars::light_line(40).dimmed());
            for group in &errors {
                println!(
                    "  {}  {:>4}  {}",
                    StructureColors::rule_code(group.issue.code()),
                    group.count().to_string().dimmed(),
                    &group.description
                );
            }
            println!();
        }

        if !warnings.is_empty() {
            let total: usize = warnings.iter().map(|g| g.count()).sum();
            println!("{} ({} issues)", "Warnings".yellow().bold(), total);
            println!("{}", BoxChars::light_line(40).dimmed());
            for group in &warnings {
                println!(
                    "  {}  {:>4}  {}",
                    StructureColors::rule_code(group.issue.code()),
                    group.count().to_string().dimmed(),
                    &group.description
                );
            }
            println!();
        }

        if !infos.is_empty() {
            let total: usize = infos.iter().map(|g| g.count()).sum();
            println!("{} ({} issues)", "Info".blue().bold(), total);
            println!("{}", BoxChars::light_line(40).dimmed());
            for group in &infos {
                println!(
                    "  {}  {:>4}  {}",
                    StructureColors::rule_code(group.issue.code()),
                    group.count().to_string().dimmed(),
                    &group.description
                );
            }
            println!();
        }
    }

    fn report_by_file_grouped(&self, groups: &[IssueGroup]) {
        // Collect all items and group by file
        let mut by_file: std::collections::HashMap<PathBuf, Vec<&DeadCode>> =
            std::collections::HashMap::new();

        for group in groups {
            for item in &group.items {
                by_file
                    .entry(item.declaration.location.file.clone())
                    .or_default()
                    .push(item);
            }
        }

        // Sort files
        let mut files: Vec<_> = by_file.keys().collect();
        files.sort();

        println!();
        println!("{}", "Issues Grouped by File".cyan().bold());
        println!("{}", BoxChars::heavy_line(50).dimmed());
        println!();

        for file in files {
            let items = by_file.get(file).unwrap();
            let path_str = self.format_path(file);

            println!(
                "{} ({} issues)",
                StructureColors::file_path(&path_str),
                items.len()
            );

            // Sort by line
            let mut sorted: Vec<_> = items.iter().collect();
            sorted.sort_by_key(|i| i.declaration.location.line);

            // Show limited items or all
            let show_count = if self.expand_all {
                sorted.len()
            } else {
                self.max_per_group.min(sorted.len())
            };

            for item in sorted.iter().take(show_count) {
                let loc = format!(
                    "{:>5}:{:<3}",
                    item.declaration.location.line, item.declaration.location.column
                );
                let symbol = SeveritySymbol::colored(&item.severity);
                let rule = StructureColors::rule_code(item.issue.code());
                let name = StructureColors::symbol_name(&item.declaration.name);

                println!("  {}  {}  {}  '{}'", loc.dimmed(), symbol, rule, name);
            }

            let remaining = sorted.len().saturating_sub(show_count);
            if remaining > 0 {
                println!("  {} ... and {} more", "".dimmed(), remaining.to_string().yellow());
            }

            println!();
        }
    }

    fn print_rule_group(&self, group: &IssueGroup) {
        let rule = group.issue.code();
        let count = group.count();
        let severity_symbol = SeveritySymbol::colored(&group.severity);

        // Header
        println!(
            "{} {} - {} ({} issues)",
            severity_symbol,
            StructureColors::rule_code(rule),
            &group.description,
            StructureColors::count(&count.to_string())
        );
        println!("{}", BoxChars::light_line(50).dimmed());

        // Check if we should expand this rule
        let should_expand = self.expand_all
            || self
                .expand_rule
                .as_ref()
                .map(|r| r == rule)
                .unwrap_or(false);

        // Group items by file for cleaner display
        let by_file = group.by_file();
        let mut files: Vec<_> = by_file.keys().collect();
        files.sort();

        let max_files = if should_expand { files.len() } else { 3 };
        let mut shown_items = 0;
        let max_items = if should_expand {
            usize::MAX
        } else {
            self.max_per_group
        };

        for file in files.iter().take(max_files) {
            let items = by_file.get(*file).unwrap();
            let path_str = self.format_path(file);

            // Show file with item count if multiple
            if items.len() > 1 {
                println!("  {} ({})", path_str.dimmed(), items.len());
            } else {
                println!("  {}", path_str.dimmed());
            }

            // Sort items by line
            let mut sorted: Vec<_> = items.iter().collect();
            sorted.sort_by_key(|i| i.declaration.location.line);

            for item in sorted {
                if shown_items >= max_items {
                    break;
                }

                let loc = format!(":{}", item.declaration.location.line);
                let confidence = ConfidenceIndicator::for_level(&item.confidence, item.runtime_confirmed);
                let name = StructureColors::symbol_name(&item.declaration.name);

                println!("    {} {}  '{}'", loc.dimmed(), confidence, name);
                shown_items += 1;
            }

            if shown_items >= max_items {
                break;
            }
        }

        // Show remaining count
        let remaining_files = files.len().saturating_sub(max_files);
        let remaining_items = count.saturating_sub(shown_items);

        if remaining_items > 0 {
            println!();
            println!(
                "  {} ... {} more in {} files",
                "→".dimmed(),
                remaining_items.to_string().yellow(),
                if remaining_files > 0 {
                    format!("{}", remaining_files)
                } else {
                    "this".to_string()
                }
            );
            println!(
                "    Run with {} to see all",
                format!("--expand {}", rule).cyan()
            );
        }
    }
}

impl Default for GroupedReporter {
    fn default() -> Self {
        Self::new(GroupBy::Rule)
    }
}
