use crate::analysis::{Confidence, DeadCode, Severity};
use miette::{IntoDiagnostic, Result};
use serde::Serialize;
use std::path::PathBuf;

/// JSON reporter for programmatic output
pub struct JsonReporter {
    output_path: Option<PathBuf>,
}

impl JsonReporter {
    pub fn new(output_path: Option<PathBuf>) -> Self {
        Self { output_path }
    }

    pub fn report(&self, dead_code: &[DeadCode]) -> Result<()> {
        let report = JsonReport::from_dead_code(dead_code);
        let json = serde_json::to_string_pretty(&report).into_diagnostic()?;

        if let Some(path) = &self.output_path {
            std::fs::write(path, &json).into_diagnostic()?;
            println!("Report written to: {}", path.display());
        } else {
            println!("{}", json);
        }

        Ok(())
    }
}

#[derive(Serialize)]
struct JsonReport {
    version: &'static str,
    total_issues: usize,
    issues: Vec<JsonIssue>,
    summary: JsonSummary,
}

#[derive(Serialize)]
struct JsonIssue {
    code: &'static str,
    severity: &'static str,
    confidence: &'static str,
    confidence_score: f64,
    runtime_confirmed: bool,
    risk: String,
    message: String,
    file: String,
    line: usize,
    column: usize,
    declaration: JsonDeclaration,
}

#[derive(Serialize)]
struct JsonDeclaration {
    name: String,
    kind: &'static str,
    fully_qualified_name: Option<String>,
}

#[derive(Serialize)]
struct JsonSummary {
    errors: usize,
    warnings: usize,
    infos: usize,
    by_confidence: JsonConfidenceSummary,
    runtime_confirmed_count: usize,
}

#[derive(Serialize)]
struct JsonConfidenceSummary {
    confirmed: usize,
    high: usize,
    medium: usize,
    low: usize,
}

impl JsonReport {
    fn from_dead_code(dead_code: &[DeadCode]) -> Self {
        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;
        let mut confirmed = 0;
        let mut high = 0;
        let mut medium = 0;
        let mut low = 0;
        let mut runtime_confirmed_count = 0;

        let issues: Vec<JsonIssue> = dead_code
            .iter()
            .map(|dc| {
                match dc.severity {
                    Severity::Error => errors += 1,
                    Severity::Warning => warnings += 1,
                    Severity::Info => infos += 1,
                }
                match dc.confidence {
                    Confidence::Confirmed => confirmed += 1,
                    Confidence::High => high += 1,
                    Confidence::Medium => medium += 1,
                    Confidence::Low => low += 1,
                }
                if dc.runtime_confirmed {
                    runtime_confirmed_count += 1;
                }

                JsonIssue {
                    code: dc.issue.code(),
                    severity: dc.severity.as_str(),
                    confidence: dc.confidence.as_str(),
                    confidence_score: dc.confidence.score(),
                    runtime_confirmed: dc.runtime_confirmed,
                    risk: dc.risk.to_string(),
                    message: dc.message.clone(),
                    file: dc.declaration.location.file.to_string_lossy().to_string(),
                    line: dc.declaration.location.line,
                    column: dc.declaration.location.column,
                    declaration: JsonDeclaration {
                        name: dc.declaration.name.clone(),
                        kind: dc.declaration.kind.display_name(),
                        fully_qualified_name: dc.declaration.fully_qualified_name.clone(),
                    },
                }
            })
            .collect();

        Self {
            version: "1.1",
            total_issues: dead_code.len(),
            issues,
            summary: JsonSummary {
                errors,
                warnings,
                infos,
                by_confidence: JsonConfidenceSummary {
                    confirmed,
                    high,
                    medium,
                    low,
                },
                runtime_confirmed_count,
            },
        }
    }
}
