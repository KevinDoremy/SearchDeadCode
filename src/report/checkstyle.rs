//! Checkstyle XML: the format the static-analysis ecosystem already speaks.
//!
//! Chosen over JUnit XML on purpose. detekt — the tool teams compare this one
//! to — publishes exactly this for CI, and Jenkins reads it natively through
//! the Warnings Next Generation plugin, as does SonarQube. JUnit would have
//! reached the same platforms by filing findings as failed tests, which
//! pollutes test metrics and history with things that are not tests.
//!
//! One `<file>` per source file, one `<error>` per finding. `source` carries
//! the rule code (`DC001`…), which is what Warnings NG shows as the category
//! and what lets a team mute or chart one rule.
//!
//! A clean project must still produce a valid, empty document rather than an
//! empty file: a parser handed zero bytes reports a broken build on the day
//! everything is fine.

use crate::analysis::{DeadCode, Severity};
use miette::{IntoDiagnostic, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct CheckstyleReporter {
    output_path: Option<PathBuf>,
    base_path: Option<PathBuf>,
}

impl CheckstyleReporter {
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
        if let Some(out) = &self.output_path {
            std::fs::write(out, &body).into_diagnostic()?;
            eprintln!("Checkstyle report written to: {}", out.display());
        } else {
            println!("{body}");
        }
        Ok(())
    }

    /// Le document, isolé de l'écriture : c'est ce qui se teste.
    fn render(&self, dead_code: &[DeadCode]) -> String {
        // BTreeMap: the grouping decides the document order, and a report that
        // reshuffles between two identical runs turns any diff into noise.
        let mut by_file: BTreeMap<String, Vec<&DeadCode>> = BTreeMap::new();
        for dc in dead_code {
            by_file.entry(self.relative(dc)).or_default().push(dc);
        }

        let mut body = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        body.push_str("<checkstyle version=\"8.0\">\n");
        for (file, findings) in &by_file {
            body.push_str(&format!("  <file name=\"{}\">\n", escape(file)));
            for dc in findings {
                body.push_str(&format!(
                    "    <error line=\"{}\" column=\"{}\" severity=\"{}\" message=\"{}\" source=\"{}\"/>\n",
                    dc.declaration.location.line.max(1),
                    dc.declaration.location.column.max(1),
                    severity_of(dc.severity),
                    escape(&dc.message),
                    escape(dc.issue.code()),
                ));
            }
            body.push_str("  </file>\n");
        }
        body.push_str("</checkstyle>\n");
        body
    }

    fn relative(&self, dc: &DeadCode) -> String {
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
        path.to_string_lossy().replace('\\', "/")
    }
}

/// Checkstyle only knows three levels, and every consumer keys its filtering
/// on these exact spellings.
fn severity_of(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// Attribute values, so the apostrophe matters as much as the angle bracket:
/// a Kotlin message quotes the symbol (`class 'Foo' is never used`).
///
/// Les caractères interdits par XML 1.0 (C0 sauf tab/LF/CR) sont remplacés
/// AVANT les entités : un identifiant Kotlin entre backticks admet n'importe
/// quel caractère hors CR/LF/backtick, donc un 0x08 grammaticalement valide
/// atterrissait brut dans l'attribut — et un seul symbole pathologique
/// faisait rejeter le document entier par le parseur, masquant toutes les
/// autres trouvailles du run.
fn escape(text: &str) -> String {
    let legal: String = text
        .chars()
        .map(|c| match c {
            '\t' | '\n' | '\r' => c,
            c if c >= '\u{20}' && c != '\u{FFFE}' && c != '\u{FFFF}' => c,
            _ => '\u{FFFD}',
        })
        .collect();
    legal
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_metacharacters_never_reach_the_document() {
        assert_eq!(
            escape("a<b>&\"c\" 'd'"),
            "a&lt;b&gt;&amp;&quot;c&quot; &apos;d&apos;"
        );
    }

    #[test]
    fn xml_illegal_control_chars_are_replaced_not_emitted() {
        // Un identifiant Kotlin entre backticks peut contenir un 0x08 : le
        // laisser passer fait rejeter TOUT le document par le parseur.
        assert_eq!(escape("bad\u{8}name"), "bad\u{FFFD}name");
        assert_eq!(escape("a\u{0}b\u{1F}c"), "a\u{FFFD}b\u{FFFD}c");
        // Tab, LF et CR sont légaux en XML 1.0 et restent intacts.
        assert_eq!(escape("a\tb\nc"), "a\tb\nc");
    }

    #[test]
    fn a_clean_project_still_produces_a_document() {
        // Le cas qui casse les consommateurs : zéro octet fait échouer le
        // parseur de Warnings NG le jour où tout va bien.
        let body = CheckstyleReporter::new(None).render(&[]);
        assert!(body.starts_with("<?xml version=\"1.0\""));
        assert!(body.contains("<checkstyle"));
        assert!(body.trim_end().ends_with("</checkstyle>"));
        assert!(!body.contains("<error"));
    }
}
