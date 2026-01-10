//! Summary reporter - statistics and overview only
//!
//! High-level view of analysis results with ASCII charts

use crate::analysis::DeadCode;
use crate::report::aggregator::ResultStats;
use crate::report::colors::{BoxChars, ChartChars, StructureColors};
use colored::Colorize;

/// Summary-only reporter with statistics and charts
pub struct SummaryReporter {
    /// Width of bar charts
    bar_width: usize,
    /// Number of top issues to show
    top_n: usize,
    /// Show files analyzed count
    show_files_count: Option<usize>,
    /// Show declarations count
    show_declarations_count: Option<usize>,
    /// Whether this is a final summary appended to another report
    is_final_summary: bool,
}

impl SummaryReporter {
    pub fn new() -> Self {
        Self {
            bar_width: 20,
            top_n: 10,
            show_files_count: None,
            show_declarations_count: None,
            is_final_summary: false,
        }
    }

    pub fn with_top_n(mut self, n: usize) -> Self {
        self.top_n = n;
        self
    }

    pub fn with_files_count(mut self, count: usize) -> Self {
        self.show_files_count = Some(count);
        self
    }

    pub fn with_declarations_count(mut self, count: usize) -> Self {
        self.show_declarations_count = Some(count);
        self
    }

    /// Mark this as a final summary appended to another report (different footer)
    pub fn as_final_summary(mut self) -> Self {
        self.is_final_summary = true;
        self
    }

    pub fn report(&self, dead_code: &[DeadCode]) {
        println!();
        println!("{}", "SearchDeadCode Analysis Summary".cyan().bold());
        println!("{}", BoxChars::heavy_line(50));
        println!();

        if dead_code.is_empty() {
            println!("{}", "No issues found!".green().bold());
            return;
        }

        let stats = ResultStats::from_dead_code(dead_code);

        // Basic stats
        self.print_basic_stats(&stats);
        println!();

        // Severity breakdown
        self.print_severity_breakdown(&stats);
        println!();

        // Category breakdown with charts
        self.print_category_breakdown(&stats);
        println!();

        // Top issues
        self.print_top_issues(&stats);
        println!();

        // Confidence breakdown
        self.print_confidence_breakdown(&stats);
        println!();

        // Footer
        self.print_footer();
    }

    fn print_basic_stats(&self, stats: &ResultStats) {
        let label_width = 20;

        if let Some(files) = self.show_files_count {
            println!(
                "{:>width$}  {}",
                "Files analyzed:".dimmed(),
                StructureColors::count(&Self::format_number(files)),
                width = label_width
            );
        }

        if let Some(decls) = self.show_declarations_count {
            println!(
                "{:>width$}  {}",
                "Declarations:".dimmed(),
                StructureColors::count(&Self::format_number(decls)),
                width = label_width
            );
        }

        println!(
            "{:>width$}  {}",
            "Files affected:".dimmed(),
            StructureColors::count(&Self::format_number(stats.files_affected)),
            width = label_width
        );

        println!(
            "{:>width$}  {}",
            "Issues found:".dimmed(),
            StructureColors::count(&Self::format_number(stats.total_issues)),
            width = label_width
        );
    }

    /// Format a number with thousands separators
    fn format_number(n: usize) -> String {
        let s = n.to_string();
        let mut result = String::new();
        for (i, c) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        result.chars().rev().collect()
    }

    fn print_severity_breakdown(&self, stats: &ResultStats) {
        println!("{}", "By Severity:".white().bold());

        let total = stats.total_issues as f64;
        if total == 0.0 {
            return;
        }

        if stats.errors > 0 {
            let pct = (stats.errors as f64 / total) * 100.0;
            println!(
                "  {} {:>6} ({:>5.1}%)",
                "Errors".red(),
                stats.errors,
                pct
            );
        }

        if stats.warnings > 0 {
            let pct = (stats.warnings as f64 / total) * 100.0;
            println!(
                "  {} {:>6} ({:>5.1}%)",
                "Warnings".yellow(),
                stats.warnings,
                pct
            );
        }

        if stats.infos > 0 {
            let pct = (stats.infos as f64 / total) * 100.0;
            println!(
                "  {} {:>6} ({:>5.1}%)",
                "Info".blue(),
                stats.infos,
                pct
            );
        }
    }

    fn print_category_breakdown(&self, stats: &ResultStats) {
        println!("{}", "By Category:".white().bold());

        let total = stats.total_issues as f64;
        if total == 0.0 {
            return;
        }

        // Sort categories by count
        let mut categories: Vec<_> = stats.by_category.iter().collect();
        categories.sort_by(|a, b| b.1.cmp(a.1));

        let max_name_len = categories
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(10);

        for (name, count) in categories {
            let pct = (*count as f64 / total) * 100.0;
            let bar = ChartChars::bar(pct, self.bar_width);

            let colored_bar = if name == "Dead Code" {
                bar.red()
            } else if name == "Architecture" {
                bar.magenta()
            } else if name == "Kotlin" {
                bar.blue()
            } else if name == "Performance" {
                bar.yellow()
            } else if name == "Android" {
                bar.green()
            } else if name == "Compose" {
                bar.cyan()
            } else {
                bar.white()
            };

            println!(
                "  {:width$} │{}│ {:>4} ({:>5.1}%)",
                name,
                colored_bar,
                count,
                pct,
                width = max_name_len
            );
        }
    }

    fn print_top_issues(&self, stats: &ResultStats) {
        println!("{}", "Top Issues:".white().bold());

        // Sort by count
        let mut rules: Vec<_> = stats.by_rule.iter().collect();
        rules.sort_by(|a, b| b.1.cmp(a.1));

        for (i, (rule, count)) in rules.iter().take(self.top_n).enumerate() {
            let desc = self.rule_short_description(rule);
            println!(
                "  {:>2}. {}  {:>5}  {}",
                i + 1,
                StructureColors::rule_code(rule),
                count.to_string().white().bold(),
                desc.dimmed()
            );
        }

        let remaining = rules.len().saturating_sub(self.top_n);
        if remaining > 0 {
            println!(
                "      ... and {} more rule types",
                remaining.to_string().dimmed()
            );
        }
    }

    fn print_confidence_breakdown(&self, stats: &ResultStats) {
        println!("{}", "By Confidence:".white().bold());

        let total = stats.total_issues as f64;
        if total == 0.0 {
            return;
        }

        if stats.confirmed > 0 {
            let pct = (stats.confirmed as f64 / total) * 100.0;
            println!(
                "  {} {} {:>6} ({:>5.1}%)",
                "✓".green().bold(),
                "Confirmed".green(),
                stats.confirmed,
                pct
            );
        }

        if stats.high > 0 {
            let pct = (stats.high as f64 / total) * 100.0;
            println!(
                "  {} {} {:>6} ({:>5.1}%)",
                "!".yellow().bold(),
                "High".yellow(),
                stats.high,
                pct
            );
        }

        if stats.medium > 0 {
            let pct = (stats.medium as f64 / total) * 100.0;
            println!(
                "  {} {} {:>6} ({:>5.1}%)",
                "?".dimmed(),
                "Medium".dimmed(),
                stats.medium,
                pct
            );
        }

        if stats.low > 0 {
            let pct = (stats.low as f64 / total) * 100.0;
            println!(
                "  {} {} {:>6} ({:>5.1}%)",
                "~".dimmed(),
                "Low".dimmed(),
                stats.low,
                pct
            );
        }
    }

    fn print_footer(&self) {
        println!("{}", BoxChars::light_line(50).dimmed());
        if self.is_final_summary {
            // Tips for final summary appended to other reports
            println!(
                "{}",
                "Tip: Run with --delete to safely remove dead code".dimmed()
            );
            println!(
                "{}",
                "Tip: Use --min-confidence high to filter low confidence".dimmed()
            );
        } else {
            // Tips for standalone summary mode
            println!(
                "{}",
                "Run without --summary for full details".dimmed()
            );
            println!(
                "{}",
                "Use --group-by rule to see issues grouped by detector".dimmed()
            );
            println!(
                "{}",
                "Use --min-confidence high to filter low confidence".dimmed()
            );
        }
    }

    fn rule_short_description(&self, rule: &str) -> &'static str {
        match rule {
            "DC001" => "Unused declarations",
            "DC002" => "Unused imports",
            "DC003" => "Unused parameters",
            "DC004" => "Assign-only variables",
            "DC005" => "Unreachable code",
            "DC010" => "Redundant overrides",
            "DC011" => "Unused Intent extras",
            "DC016" => "Redundant public",
            "AP001" => "Global mutable state",
            "AP002" => "Deep inheritance",
            "AP003" => "Single-impl interface",
            "AP004" => "EventBus usage",
            "AP007" => "Heavy ViewModel",
            "AP008" => "GlobalScope usage",
            "AP009" => "Lateinit abuse",
            "AP010" => "Scope chaining",
            "AP011" => "Memory leak risk",
            "AP012" => "Long method",
            "AP013" => "Large class",
            "AP014" => "Collection no sequence",
            "AP015" => "Allocation in loop",
            "AP016" => "Exposed mutable state",
            "AP017" => "View in ViewModel",
            "AP018" => "Missing UseCase",
            "AP019" => "Nested callbacks",
            "AP020" => "Hardcoded Dispatcher",
            "AP021" => "Nullability overload",
            "AP022" => "Reflection overuse",
            "AP023" => "Long param list",
            "AP024" => "Complex condition",
            "AP025" => "String duplication",
            "AP026" => "Unclosed resource",
            "AP027" => "Main thread DB",
            "AP028" => "WakeLock issue",
            "AP029" => "AsyncTask usage",
            "AP030" => "onDraw allocation",
            "AP031" => "State no remember",
            "AP032" => "LaunchedEffect no key",
            "AP033" => "Logic in Composable",
            "AP034" => "NavController passing",
            _ => "Unknown rule",
        }
    }
}

impl Default for SummaryReporter {
    fn default() -> Self {
        Self::new()
    }
}
