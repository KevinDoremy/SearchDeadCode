//! CSV report: findings as a spreadsheet for team triage. RFC 4180
//! quoting — a message full of commas stays one row.

use crate::analysis::DeadCode;
use miette::{IntoDiagnostic, Result};
use std::path::PathBuf;

pub struct CsvReporter {
    output_path: Option<PathBuf>,
    base_path: Option<PathBuf>,
}

impl CsvReporter {
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
            eprintln!("CSV report written to: {}", path.display());
        } else {
            print!("{body}");
        }
        Ok(())
    }

    fn render(&self, dead_code: &[DeadCode]) -> String {
        let mut out = String::from("code,symbol,kind,file,line,confidence,risk,message\n");
        for dc in dead_code {
            let file = match &self.base_path {
                Some(base) => dc
                    .declaration
                    .location
                    .file
                    .strip_prefix(base)
                    .unwrap_or(&dc.declaration.location.file)
                    .display()
                    .to_string(),
                None => dc.declaration.location.file.display().to_string(),
            };
            let fields = [
                dc.issue.code().to_string(),
                dc.declaration.name.clone(),
                dc.declaration.kind.display_name().to_string(),
                file,
                dc.declaration.location.line.to_string(),
                dc.confidence.to_string(),
                dc.risk.to_string(),
                dc.message.clone(),
            ];
            let row: Vec<String> = fields.iter().map(|f| escape(f)).collect();
            out.push_str(&row.join(","));
            out.push('\n');
        }
        out
    }
}

/// RFC 4180: quote when the field holds a comma, a quote or a newline;
/// double the inner quotes.
fn escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_fields_pass_through_and_dirty_ones_get_quoted() {
        assert_eq!(escape("DC001"), "DC001");
        assert_eq!(escape("a,b"), "\"a,b\"");
        assert_eq!(escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
