//! GitLab Code Quality report: the JSON array the MR widget renders —
//! third CI platform after GitHub (SARIF) and reviewdog. Fingerprints
//! reuse the line-free stable hash so a shifted declaration never
//! reopens the finding.

use crate::analysis::{DeadCode, Severity};
use miette::{IntoDiagnostic, Result};
use serde_json::json;
use std::path::PathBuf;

pub struct GitlabReporter {
    output_path: Option<PathBuf>,
    base_path: Option<PathBuf>,
}

impl GitlabReporter {
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
        let issues: Vec<serde_json::Value> = dead_code
            .iter()
            .map(|dc| {
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
                let path = path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .trim_start_matches('/')
                    .to_string();
                let severity = match dc.severity {
                    Severity::Error => "major",
                    Severity::Warning => "minor",
                    Severity::Info => "info",
                };
                json!({
                    "description": dc.message,
                    "check_name": dc.issue.code(),
                    "fingerprint": super::sarif::stable_hash(&format!(
                        "{path}|{}|{}",
                        dc.declaration.name,
                        dc.issue.code()
                    )),
                    "severity": severity,
                    "location": {
                        "path": path,
                        "lines": { "begin": dc.declaration.location.line.max(1) }
                    }
                })
            })
            .collect();
        let body = serde_json::to_string_pretty(&issues).into_diagnostic()?;
        if let Some(out) = &self.output_path {
            std::fs::write(out, &body).into_diagnostic()?;
            eprintln!("GitLab Code Quality report written to: {}", out.display());
        } else {
            println!("{body}");
        }
        Ok(())
    }
}
