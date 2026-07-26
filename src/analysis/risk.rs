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

/// Assign a risk level to every finding, in place.
pub fn assess(dead_code: &mut [DeadCode], files: &[SourceFile]) {
    let mut literal_text = String::new();
    let mut marker_files: HashSet<PathBuf> = HashSet::new();

    for file in files {
        if !matches!(file.file_type, FileType::Kotlin | FileType::Java) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file.path) else {
            continue;
        };
        for m in STRING_LITERAL.find_iter(&content) {
            literal_text.push_str(m.as_str());
            literal_text.push('\n');
        }
        if REFLECTION_MARKERS.iter().any(|m| content.contains(m)) {
            marker_files.insert(file.path.clone());
        }
    }

    for finding in dead_code.iter_mut() {
        finding.risk = risk_of(finding, &literal_text, &marker_files);
    }
}

fn risk_of(finding: &DeadCode, literal_text: &str, marker_files: &HashSet<PathBuf>) -> RiskLevel {
    if literal_text.contains(&finding.declaration.name) {
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
