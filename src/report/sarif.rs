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
                            artifact_location: SarifArtifactLocation { uri },
                            region: SarifRegion {
                                start_line: dc.declaration.location.line,
                                start_column: dc.declaration.location.column,
                            },
                        },
                    }],
                    partial_fingerprints: fingerprints,
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
