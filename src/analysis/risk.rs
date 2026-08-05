//! Per-finding deletion risk.
//!
//! Static analysis cannot see through Class.forName, JSON keys, FQN strings
//! or event-bus dispatch. Findings touched by those signals are flagged so
//! the user knows which deletions need a second look.

use crate::analysis::{DeadCode, RiskLevel};
use crate::discovery::{FileType, SourceFile};
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

static STRING_LITERAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""([^"\\]|\\.)*""#).expect("Invalid string literal regex"));

static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("Invalid word regex"));

const REFLECTION_MARKERS: &[&str] = &[
    "Class.forName",
    "getDeclaredMethod",
    "getDeclaredField",
    "::class.java",
    "@Subscribe",
    "EventBus",
];

const SERIALIZATION_ANNOTATIONS: &[&str] = &[
    "SerializedName",
    "Serializable",
    "SerialName",
    "Parcelize",
    "JsonProperty",
    "JsonClass",
    "Keep",
    "Expose",
];

/// Assign a risk level to every finding, in place. A finding whose name
/// appears as a whole word inside any string literal of the project is
/// soft-referenced: risk High, confidence down one notch (Confirmed
/// stays — runtime evidence outranks a string), and the message says so.
pub fn assess(dead_code: &mut [DeadCode], files: &[SourceFile]) {
    let mut literal_tokens: HashSet<String> = HashSet::new();
    let mut marker_files: HashSet<PathBuf> = HashSet::new();

    for file in files {
        if !matches!(file.file_type, FileType::Kotlin | FileType::Java) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file.path) else {
            continue;
        };
        for m in STRING_LITERAL.find_iter(&content) {
            for word in WORD.find_iter(m.as_str()) {
                literal_tokens.insert(word.as_str().to_string());
            }
        }
        if REFLECTION_MARKERS.iter().any(|m| content.contains(m)) {
            marker_files.insert(file.path.clone());
        }
    }

    for finding in dead_code.iter_mut() {
        let soft_referenced = literal_tokens.contains(&finding.declaration.name);
        finding.risk = risk_of(finding, soft_referenced, &marker_files);
        if soft_referenced {
            finding.message.push_str(
                " — name appears in string literals (reflective or serialized use possible)",
            );
            finding.confidence = finding.confidence.downgraded();
        }
    }
}

fn risk_of(
    finding: &DeadCode,
    soft_referenced: bool,
    marker_files: &HashSet<PathBuf>,
) -> RiskLevel {
    if soft_referenced {
        return RiskLevel::High;
    }
    if finding
        .declaration
        .annotations
        .iter()
        .any(|a| SERIALIZATION_ANNOTATIONS.iter().any(|s| a.contains(s)))
    {
        return RiskLevel::High;
    }
    if marker_files.contains(&finding.declaration.location.file) {
        return RiskLevel::Medium;
    }
    RiskLevel::Low
}
