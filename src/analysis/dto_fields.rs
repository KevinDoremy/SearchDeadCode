//! Dead DTO fields.
//!
//! Gson and friends write @SerializedName fields through reflection, so
//! the reachability graph always sees them as "written". A field nobody
//! ever READS is business deadness. Signal: the property name appears
//! exactly once in the whole corpus — its own declaration.

use crate::discovery::{FileType, SourceFile};
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

static SERIALIZED_PROP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"@SerializedName\([^)]*\)\s*(?:va[lr]|(?:private|public|protected|final|\s)*[A-Za-z_][A-Za-z0-9_<>,.\[\] ]*?)\s+([a-z]\w*)\s*[:;=,)]")
        .expect("Invalid serialized prop regex")
});

static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("Invalid word regex"));

/// (property name, file, 1-based line of the property)
pub fn dead_fields(files: &[SourceFile]) -> Vec<(String, PathBuf, usize)> {
    let mut token_counts: HashMap<String, usize> = HashMap::new();
    let mut candidates: Vec<(String, PathBuf, usize)> = Vec::new();

    for file in files {
        if !matches!(file.file_type, FileType::Kotlin | FileType::Java) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&file.path) else {
            continue;
        };
        for word in WORD.find_iter(&content) {
            *token_counts.entry(word.as_str().to_string()).or_default() += 1;
        }
        for captures in SERIALIZED_PROP.captures_iter(&content) {
            let name = captures[1].to_string();
            let offset = captures.get(1).map(|m| m.start()).unwrap_or(0);
            let line = content[..offset].matches('\n').count() + 1;
            candidates.push((name, file.path.clone(), line));
        }
    }

    candidates
        .into_iter()
        .filter(|(name, _, _)| token_counts.get(name).copied().unwrap_or(0) <= 1)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kotlin_and_java_serialized_props_are_captured() {
        let kotlin = "@SerializedName(\"u\")\n    val userName: String,";
        let java = "@SerializedName(\"s\") private int legacyScore;";
        let name_of = |s: &str| SERIALIZED_PROP.captures(s).map(|c| c[1].to_string());
        assert_eq!(name_of(kotlin), Some("userName".to_string()));
        assert_eq!(name_of(java), Some("legacyScore".to_string()));
    }

    #[test]
    fn unannotated_properties_are_ignored() {
        assert!(SERIALIZED_PROP.captures("val unread: Int = 0").is_none());
    }
}
