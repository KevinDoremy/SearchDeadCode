//! Markdown report: findings as a paste-ready table for PRs and
//! tickets. Pipes in cell content are escaped so a message cannot
//! break the table.

use crate::analysis::DeadCode;
use miette::{IntoDiagnostic, Result};
use std::path::PathBuf;

pub struct MarkdownReporter {
    output_path: Option<PathBuf>,
    base_path: Option<PathBuf>,
}

impl MarkdownReporter {
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
        let md = self.render(dead_code);
        if let Some(path) = &self.output_path {
            std::fs::write(path, &md).into_diagnostic()?;
            println!("Markdown report written to: {}", path.display());
        } else {
            println!("{md}");
        }
        Ok(())
    }

    fn render(&self, dead_code: &[DeadCode]) -> String {
        if dead_code.is_empty() {
            return "# SearchDeadCode report\n\nNo dead code found.\n".to_string();
        }

        let mut md = format!(
            "# SearchDeadCode report\n\n{} finding(s).\n\n\
             | Code | Symbol | File | Line | Confidence | Risk | Message |\n\
             |------|--------|------|------|------------|------|--------|\n",
            dead_code.len()
        );
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
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                dc.issue.code(),
                escape(&dc.declaration.name),
                escape(&file),
                dc.declaration.location.line,
                dc.confidence.as_str(),
                dc.risk,
                escape(&dc.message),
            ));
        }
        md
    }
}

fn escape(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipes_cannot_break_the_table() {
        assert_eq!(escape("a|b"), "a\\|b");
        assert_eq!(escape("line1\nline2"), "line1 line2");
    }
}
