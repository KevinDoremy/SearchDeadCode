//! Orphan translations.
//!
//! A string living in a locale folder (values-fr, values-es…) whose key
//! no longer exists in the base values/ can never be resolved: the base
//! removed it, the locale kept it. Localization deadness.

use regex::Regex;
use std::collections::{HashMap, HashSet};

/// base keys of a res/ dir, plus its locale entries (key, file, line)
type ResSlot = (HashSet<String>, Vec<(String, PathBuf, usize)>);
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static STRING_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<string\s+name="([^"]+)""#).expect("Invalid string key regex"));

/// (key, file, 1-based line) for every locale string missing its base
pub fn orphan_translations(root: &Path) -> Vec<(String, PathBuf, usize)> {
    // res/ dir → (base keys, locale entries)
    let mut per_res: HashMap<PathBuf, ResSlot> = HashMap::new();

    let walker = walkdir::WalkDir::new(root).into_iter().filter_entry(|e| {
        if e.depth() == 0 {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        !name.starts_with('.') && name != "build" && name != "generated"
    });
    for entry in walker.flatten() {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().map_or(true, |e| e != "xml") {
            continue;
        }
        let Some(values_dir) = path.parent() else {
            continue;
        };
        let dir_name = values_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(res_dir) = values_dir.parent() else {
            continue;
        };
        let is_base = dir_name == "values";
        let is_locale = dir_name.starts_with("values-");
        if !is_base && !is_locale {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let slot = per_res.entry(res_dir.to_path_buf()).or_default();
        for captures in STRING_KEY.captures_iter(&content) {
            let key = captures[1].to_string();
            if is_base {
                slot.0.insert(key);
            } else {
                let offset = captures.get(0).map(|m| m.start()).unwrap_or(0);
                let line = content[..offset].matches('\n').count() + 1;
                slot.1.push((key, path.to_path_buf(), line));
            }
        }
    }

    let mut orphans = Vec::new();
    for (_res, (base_keys, locale_entries)) in per_res {
        for (key, file, line) in locale_entries {
            if !base_keys.contains(&key) {
                orphans.push((key, file, line));
            }
        }
    }
    orphans.sort();
    orphans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_keys_are_extracted() {
        let xml = "<resources>\n    <string name=\"alpha\">A</string>\n</resources>";
        let keys: Vec<String> = STRING_KEY
            .captures_iter(xml)
            .map(|c| c[1].to_string())
            .collect();
        assert_eq!(keys, vec!["alpha"]);
    }

    #[test]
    fn an_empty_root_yields_nothing() {
        let temp = tempfile::tempdir().unwrap();
        assert!(orphan_translations(temp.path()).is_empty());
    }
}
