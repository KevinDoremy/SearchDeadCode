mod json;
mod sarif;
mod terminal;

pub use json::JsonReporter;
pub use sarif::SarifReporter;
pub use terminal::TerminalReporter;

use crate::analysis::DeadCode;
use miette::Result;
use std::path::PathBuf;

/// Output format for reports
#[derive(Debug, Clone, Default)]
pub enum ReportFormat {
    #[default]
    Terminal,
    Json,
    Sarif,
}

/// Reporter for outputting dead code analysis results
pub struct Reporter {
    format: ReportFormat,
    output_path: Option<PathBuf>,
}

impl Reporter {
    pub fn new(format: ReportFormat, output_path: Option<PathBuf>) -> Self {
        Self {
            format,
            output_path,
        }
    }

    /// Report the dead code findings
    pub fn report(&self, dead_code: &[DeadCode]) -> Result<()> {
        match &self.format {
            ReportFormat::Terminal => {
                let reporter = TerminalReporter::new();
                reporter.report(dead_code)
            }
            ReportFormat::Json => {
                let reporter = JsonReporter::new(self.output_path.clone());
                reporter.report(dead_code)
            }
            ReportFormat::Sarif => {
                let reporter = SarifReporter::new(self.output_path.clone());
                reporter.report(dead_code)
            }
        }
    }
}
