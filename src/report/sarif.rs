use crate::analysis::{DeadCode, Severity};
use miette::{IntoDiagnostic, Result};
use serde::Serialize;
use std::path::PathBuf;

/// SARIF reporter for CI/CD integration (GitHub, Azure DevOps, etc.)
pub struct SarifReporter {
    output_path: Option<PathBuf>,
    base_path: Option<PathBuf>,
}

impl SarifReporter {
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
        let sarif = SarifReport::from_dead_code(dead_code, self.base_path.as_deref());
        let json = serde_json::to_string_pretty(&sarif).into_diagnostic()?;

        if let Some(path) = &self.output_path {
            std::fs::write(path, &json).into_diagnostic()?;
            println!("SARIF report written to: {}", path.display());
        } else {
            println!("{}", json);
        }

        Ok(())
    }
}

/// SARIF 2.1.0 format
#[derive(Serialize)]
struct SarifReport {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

#[derive(Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
struct SarifDriver {
    name: &'static str,
    version: &'static str,
    #[serde(rename = "informationUri")]
    information_uri: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
struct SarifRule {
    id: String,
    name: String,
    #[serde(rename = "shortDescription")]
    short_description: SarifMessage,
    #[serde(rename = "helpUri")]
    help_uri: &'static str,
    #[serde(rename = "defaultConfiguration")]
    default_configuration: SarifConfiguration,
}

#[derive(Serialize)]
struct SarifConfiguration {
    level: &'static str,
}

#[derive(Serialize)]
struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id: &'static str,
    level: &'static str,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
    #[serde(rename = "partialFingerprints")]
    partial_fingerprints: std::collections::BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixes: Option<Vec<SarifFix>>,
}

#[derive(Serialize)]
struct SarifFix {
    description: SarifMessage,
    #[serde(rename = "artifactChanges")]
    artifact_changes: Vec<SarifArtifactChange>,
}

#[derive(Serialize)]
struct SarifArtifactChange {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifactLocation,
    replacements: Vec<SarifReplacement>,
}

#[derive(Serialize)]
struct SarifReplacement {
    #[serde(rename = "deletedRegion")]
    deleted_region: SarifDeletedRegion,
    #[serde(rename = "insertedContent")]
    inserted_content: SarifMessage,
}

#[derive(Serialize)]
struct SarifDeletedRegion {
    #[serde(rename = "startLine")]
    start_line: usize,
    #[serde(rename = "endLine")]
    end_line: usize,
}

#[derive(Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Serialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
struct SarifRegion {
    #[serde(rename = "startLine")]
    start_line: usize,
    #[serde(rename = "startColumn")]
    start_column: usize,
}

/// A one-click deletion fix, only for graph-backed findings whose
/// span is real (synthetic findings carry fabricated offsets — a
/// deletedRegion built from them would point at the wrong lines).
fn deletion_fix(dc: &DeadCode, uri: String) -> Option<Vec<SarifFix>> {
    if dc.issue != crate::analysis::DeadCodeIssue::Unreferenced {
        return None;
    }
    let content = std::fs::read_to_string(&dc.declaration.location.file).ok()?;
    let start = dc.declaration.id.start.min(content.len());
    let end = dc.declaration.id.end.min(content.len());
    if end <= start {
        return None;
    }
    // the span must agree with the recorded line, or it is not trustworthy
    let span_start_line = content[..start].matches('\n').count() + 1;
    if span_start_line != dc.declaration.location.line {
        return None;
    }
    let end_line = span_start_line + content[start..end].matches('\n').count();
    Some(vec![SarifFix {
        description: SarifMessage {
            text: format!(
                "delete unused {} '{}'",
                dc.declaration.kind.display_name(),
                dc.declaration.name
            ),
        },
        artifact_changes: vec![SarifArtifactChange {
            artifact_location: SarifArtifactLocation { uri },
            replacements: vec![SarifReplacement {
                deleted_region: SarifDeletedRegion {
                    start_line: span_start_line,
                    end_line,
                },
                inserted_content: SarifMessage {
                    text: String::new(),
                },
            }],
        }],
    }])
}

impl SarifReport {
    fn from_dead_code(dead_code: &[DeadCode], base: Option<&std::path::Path>) -> Self {
        // One declared rule per code actually emitted: Code Scanning
        // requires every ruleId to exist in driver.rules
        let mut seen: Vec<&'static str> = Vec::new();
        for dc in dead_code {
            let code = dc.issue.code();
            if !seen.contains(&code) {
                seen.push(code);
            }
        }
        let rules: Vec<SarifRule> =
            seen.iter()
                .map(|code| {
                    let label = crate::report::summary::rule_label(code);
                    SarifRule {
                    id: code.to_string(),
                    name: label.to_lowercase().replace([' ', '/'], "-").replace("()", ""),
                    short_description: SarifMessage {
                        text: label.to_string(),
                    },
                    help_uri:
                        "https://github.com/KevinDoremy/SearchDeadCode/blob/main/docs/detectors.md",
                    default_configuration: SarifConfiguration { level: "warning" },
                }
                })
                .collect();

        let results: Vec<SarifResult> = dead_code
            .iter()
            .map(|dc| {
                let level = match dc.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                    Severity::Info => "note",
                };

                let uri = match base {
                    Some(root) => dc
                        .declaration
                        .location
                        .file
                        .strip_prefix(root)
                        .unwrap_or(&dc.declaration.location.file)
                        .to_string_lossy()
                        .replace('\\', "/"),
                    None => dc.declaration.location.file.to_string_lossy().to_string(),
                };
                // Line-free fingerprint: relative file + symbol + rule.
                // A line shift must never re-open the alert.
                let mut fingerprints = std::collections::BTreeMap::new();
                fingerprints.insert(
                    "searchdeadcode/v1".to_string(),
                    stable_hash(&format!(
                        "{uri}|{}|{}",
                        dc.declaration.name,
                        dc.issue.code()
                    )),
                );

                SarifResult {
                    rule_id: dc.issue.code(),
                    level,
                    message: SarifMessage {
                        text: dc.message.clone(),
                    },
                    locations: vec![SarifLocation {
                        physical_location: SarifPhysicalLocation {
                            artifact_location: SarifArtifactLocation { uri: uri.clone() },
                            region: SarifRegion {
                                start_line: dc.declaration.location.line,
                                start_column: dc.declaration.location.column,
                            },
                        },
                    }],
                    partial_fingerprints: fingerprints,
                    fixes: deletion_fix(dc, uri),
                }
            })
            .collect();

        SarifReport {
            schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
            version: "2.1.0",
            runs: vec![SarifRun {
                tool: SarifTool {
                    driver: SarifDriver {
                        name: "searchdeadcode",
                        version: env!("CARGO_PKG_VERSION"),
                        information_uri: "https://github.com/KevinDoremy/SearchDeadCode",
                        rules,
                    },
                },
                results,
            }],
        }
    }
}

/// djb2 — deliberately hand-rolled: std's DefaultHasher is not stable
/// across Rust versions, and fingerprints must never drift
fn stable_hash(input: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(33) ^ u64::from(byte);
    }
    format!("{hash:016x}")
}
