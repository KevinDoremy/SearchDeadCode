//! Dead Remote Config keys.
//!
//! Keys declared in remote_config_defaults.xml that no source literal
//! ever mentions are configuration deadness — nobody sees them, nobody
//! reads them. A key read through a constant still shows up as a string
//! literal somewhere, so literal presence is the honest signal.

use crate::discovery::{FileType, SourceFile};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static KEY_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<key>\s*([^<]+?)\s*</key>").expect("Invalid key regex"));

static KEY_VALUE_ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<key>\s*([^<]+?)\s*</key>\s*<value>\s*([^<]*?)\s*</value>")
        .expect("Invalid key/value regex")
});

/// Does the corpus read the whole key set at once? `getInstance().all`,
/// `getAll()`, or `getKeysByPrefix` enumerate every declared default, which
/// makes "declared but never read" unprovable for every key at once.
pub fn enumerates_every_key(corpus: &str) -> bool {
    const SWEEPS: &[&str] = &[
        "RemoteConfig.getInstance().all",
        "remoteConfig.all",
        "getKeysByPrefix",
        ".getAll()",
    ];
    SWEEPS.iter().any(|needle| corpus.contains(needle))
}

/// (key, defaults file, line of the <key> entry)
pub fn dead_keys(root: &Path, files: &[SourceFile]) -> Vec<(String, PathBuf, usize)> {
    let defaults_files: Vec<PathBuf> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "build" && name != "generated"
        })
        .flatten()
        .filter(|e| e.file_name() == "remote_config_defaults.xml")
        .map(|e| e.path().to_path_buf())
        .collect();
    if defaults_files.is_empty() {
        return Vec::new();
    }

    let mut corpus = String::new();
    for file in files {
        if matches!(file.file_type, FileType::Kotlin | FileType::Java) {
            if let Ok(content) = std::fs::read_to_string(&file.path) {
                corpus.push_str(&content);
                corpus.push('\n');
            }
        }
    }

    let mut dead = Vec::new();
    for defaults in defaults_files {
        let Ok(content) = std::fs::read_to_string(&defaults) else {
            continue;
        };
        for captures in KEY_ENTRY.captures_iter(&content) {
            let key = captures[1].to_string();
            if corpus.contains(&format!("\"{key}\"")) {
                continue;
            }
            let line = content[..captures.get(0).map(|m| m.start()).unwrap_or(0)]
                .matches('\n')
                .count()
                + 1;
            dead.push((key, defaults.clone(), line));
        }
    }
    dead
}

/// Boolean defaults are feature flags — the Piranha inventory.
/// None when no defaults file exists at all.
pub fn boolean_flags(root: &Path) -> Option<Vec<(String, bool)>> {
    let defaults_files: Vec<PathBuf> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "build" && name != "generated"
        })
        .flatten()
        .filter(|e| e.file_name() == "remote_config_defaults.xml")
        .map(|e| e.path().to_path_buf())
        .collect();
    if defaults_files.is_empty() {
        return None;
    }

    let mut flags = Vec::new();
    for defaults in defaults_files {
        let Ok(content) = std::fs::read_to_string(&defaults) else {
            continue;
        };
        for captures in KEY_VALUE_ENTRY.captures_iter(&content) {
            let value = captures[2].trim().to_lowercase();
            if value == "true" || value == "false" {
                flags.push((captures[1].to_string(), value == "true"));
            }
        }
    }
    flags.sort();
    Some(flags)
}

/// Where this project reads its Remote Config keys as a whole set, if it
/// does: `file:line` of the sweep. A sweep does not make a key USED by
/// product code, it only means a debug surface still shows it — so the
/// finding stays and carries the caveat instead of disappearing.
pub fn key_sweep_site(files: &[SourceFile]) -> Option<String> {
    for file in files {
        if !matches!(file.file_type, FileType::Kotlin | FileType::Java) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&file.path) else {
            continue;
        };
        for (i, line) in content.lines().enumerate() {
            if enumerates_every_key(line) {
                let name = file
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                return Some(format!("{name}:{}", i + 1));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_extracted_with_their_lines() {
        let xml = "<defaultsMap>\n  <entry>\n    <key>alpha</key>\n  </entry>\n  <entry>\n    <key> beta </key>\n  </entry>\n</defaultsMap>\n";
        let keys: Vec<(String, usize)> = KEY_ENTRY
            .captures_iter(xml)
            .map(|c| {
                let line = xml[..c.get(0).unwrap().start()].matches('\n').count() + 1;
                (c[1].trim().to_string(), line)
            })
            .collect();
        assert_eq!(
            keys,
            vec![("alpha".to_string(), 3), ("beta".to_string(), 6)]
        );
    }

    #[test]
    fn a_missing_root_yields_nothing() {
        let temp = tempfile::tempdir().unwrap();
        assert!(dead_keys(temp.path(), &[]).is_empty());
    }
}
