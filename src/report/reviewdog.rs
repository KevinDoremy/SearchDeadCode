//! Reviewdog Diagnostic JSON Lines (rdjsonl): one JSON object per
//! finding, consumable by `reviewdog -f=rdjsonl` in any CI review flow
//! without a dedicated action.

use crate::analysis::{DeadCode, Severity};
use miette::{IntoDiagnostic, Result};
use serde_json::json;
use std::path::PathBuf;

pub struct ReviewdogReporter {
    output_path: Option<PathBuf>,
    base_path: Option<PathBuf>,
}

impl ReviewdogReporter {
    pub fn new(output_path: Option<PathBuf>) -> Self {
        Self {
            output_path,
            base_path: None,
        }
    }

    pub fn with_base_path(mut self, base: Option<PathBuf>) -> Self {
        self.base_path = base;
        self
    }

    pub fn report(&self, dead_code: &[DeadCode]) -> Result<()> {
        let body = self.render(dead_code);
        if let Some(path) = &self.output_path {
            std::fs::write(path, &body).into_diagnostic()?;
            eprintln!("reviewdog diagnostics written to: {}", path.display());
        } else {
            print!("{body}");
        }
        Ok(())
    }

    fn render(&self, dead_code: &[DeadCode]) -> String {
        let mut out = String::new();
        for dc in dead_code {
            let path = match &self.base_path {
                Some(base) => dc
                    .declaration
                    .location
                    .file
                    .strip_prefix(base)
                    .unwrap_or(&dc.declaration.location.file)
                    .to_path_buf(),
                None => dc.declaration.location.file.clone(),
            };
            // reviewdog wants repo-relative, forward-slash paths
            let path = path
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
                .to_string();
            let severity = match dc.severity {
                Severity::Error => "ERROR",
                Severity::Warning => "WARNING",
                Severity::Info => "INFO",
            };
            let diagnostic = json!({
                "message": dc.message,
                "location": {
                    "path": path,
                    "range": {
                        "start": {
                            "line": dc.declaration.location.line.max(1),
                            "column": dc.declaration.location.column.max(1)
                        }
                    }
                },
                "severity": severity,
                "code": { "value": dc.issue.code() }
            });
            out.push_str(&diagnostic.to_string());
            out.push('\n');
        }
        out
    }
}
