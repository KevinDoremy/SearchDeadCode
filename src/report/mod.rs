mod aggregator;
mod colors;
mod compact;
mod grouped;
mod json;
mod sarif;
mod summary;
mod terminal;

pub use compact::CompactReporter;
pub use grouped::{GroupBy, GroupedReporter};
pub use json::JsonReporter;
pub use sarif::SarifReporter;
pub use summary::SummaryReporter;
pub use terminal::TerminalReporter;

use crate::analysis::DeadCode;
use miette::Result;
use std::path::PathBuf;

/// Output format for reports
#[derive(Debug, Clone, Default)]
pub enum ReportFormat {
    /// Default terminal output (improved colors)
    #[default]
    Terminal,
    /// Compact one-line-per-issue format
    Compact,
    /// Grouped by rule/category/severity
    Grouped(GroupBy),
    /// Summary statistics only
    Summary,
    /// JSON machine-readable format
    Json,
    /// SARIF format for IDE integration
    Sarif,
}

/// Options for report generation
#[derive(Debug, Clone, Default)]
pub struct ReportOptions {
    /// Output file path (for JSON/SARIF)
    pub output_path: Option<PathBuf>,
    /// Base path to strip from file paths for shorter display
    pub base_path: Option<PathBuf>,
    /// Expand all collapsed groups
    pub expand_all: bool,
    /// Expand a specific rule
    pub expand_rule: Option<String>,
    /// Show confidence indicators
    pub show_confidence: bool,
    /// Maximum items per group before collapsing
    pub max_per_group: usize,
    /// Number of top issues to show in summary
    pub top_n: usize,
    /// Files analyzed count (for summary)
    pub files_count: Option<usize>,
    /// Declarations count (for summary)
    pub declarations_count: Option<usize>,
}

impl ReportOptions {
    pub fn new() -> Self {
        Self {
            output_path: None,
            base_path: None,
            expand_all: false,
            expand_rule: None,
            show_confidence: true,
            max_per_group: 5,
            top_n: 10,
            files_count: None,
            declarations_count: None,
        }
    }

}

/// Reporter for outputting dead code analysis results
pub struct Reporter {
    format: ReportFormat,
    options: ReportOptions,
}

impl Reporter {
    pub fn new(format: ReportFormat, output_path: Option<PathBuf>) -> Self {
        Self {
            format,
            options: ReportOptions {
                output_path,
                ..Default::default()
            },
        }
    }

    pub fn with_options(format: ReportFormat, options: ReportOptions) -> Self {
        Self { format, options }
    }

    /// Report the dead code findings
    pub fn report(&self, dead_code: &[DeadCode]) -> Result<()> {
        match &self.format {
            ReportFormat::Terminal => {
                let reporter = TerminalReporter::new()
                    .with_confidence(self.options.show_confidence);
                reporter.report(dead_code)?;
                // Always show full summary at the end
                self.print_final_summary(dead_code);
                Ok(())
            }
            ReportFormat::Compact => {
                let mut reporter = CompactReporter::new()
                    .with_confidence(self.options.show_confidence);
                if let Some(base) = &self.options.base_path {
                    reporter = reporter.with_base_path(base.clone());
                }
                reporter.report(dead_code);
                // Always show full summary at the end
                self.print_final_summary(dead_code);
                Ok(())
            }
            ReportFormat::Grouped(group_by) => {
                let mut reporter = GroupedReporter::new(*group_by)
                    .with_max_per_group(self.options.max_per_group);
                if let Some(base) = &self.options.base_path {
                    reporter = reporter.with_base_path(base.clone());
                }
                if self.options.expand_all {
                    reporter = reporter.expand_all();
                }
                if let Some(rule) = &self.options.expand_rule {
                    reporter = reporter.expand_rule(rule.clone());
                }
                reporter.report(dead_code.to_vec());
                // Always show full summary at the end
                self.print_final_summary(dead_code);
                Ok(())
            }
            ReportFormat::Summary => {
                let mut reporter = SummaryReporter::new().with_top_n(self.options.top_n);
                if let Some(files) = self.options.files_count {
                    reporter = reporter.with_files_count(files);
                }
                if let Some(decls) = self.options.declarations_count {
                    reporter = reporter.with_declarations_count(decls);
                }
                reporter.report(dead_code);
                Ok(())
            }
            ReportFormat::Json => {
                let reporter = JsonReporter::new(self.options.output_path.clone());
                reporter.report(dead_code)
            }
            ReportFormat::Sarif => {
                let reporter = SarifReporter::new(self.options.output_path.clone());
                reporter.report(dead_code)
            }
        }
    }

    /// Print the full summary at the end of any report
    fn print_final_summary(&self, dead_code: &[DeadCode]) {
        let mut reporter = SummaryReporter::new()
            .with_top_n(self.options.top_n)
            .as_final_summary();
        if let Some(files) = self.options.files_count {
            reporter = reporter.with_files_count(files);
        }
        if let Some(decls) = self.options.declarations_count {
            reporter = reporter.with_declarations_count(decls);
        }
        reporter.report(dead_code);
    }
}
