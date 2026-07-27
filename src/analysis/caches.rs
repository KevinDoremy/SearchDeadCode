//! Write-only cache keys: a literal key put into a cache-named
//! receiver and never read back means the whole compute-and-store
//! pipeline runs for nothing. Scope is honest about its reach: only
//! receivers whose name smells like a cache are judged.

use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct WriteOnlyCacheKey {
    pub key: String,
    pub file: PathBuf,
    pub line: usize,
}

static PUT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(\w*[Cc]ache\w*)\s*\.\s*put\s*\(\s*"([^"]+)""#).unwrap());
static GET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\w*[Cc]ache\w*\s*(?:\.\s*get\s*\(\s*"([^"]+)"|\[\s*"([^"]+)"\s*\])"#).unwrap()
});

/// None when no cache write exists at all.
pub fn write_only_cache_keys(root: &Path) -> Option<Vec<WriteOnlyCacheKey>> {
    let mut writes: BTreeMap<String, (PathBuf, usize)> = BTreeMap::new();
    let mut reads: BTreeSet<String> = BTreeSet::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !(name.starts_with('.') || name == "build" || name == "node_modules")
        })
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name().to_string_lossy();
            e.file_type().is_file() && (name.ends_with(".kt") || name.ends_with(".java"))
        })
    {
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for cap in PUT_RE.captures_iter(&content) {
            let line = content[..cap.get(0).unwrap().start()].matches('\n').count() + 1;
            writes
                .entry(cap[2].to_string())
                .or_insert((entry.path().to_path_buf(), line));
        }
        for cap in GET_RE.captures_iter(&content) {
            if let Some(key) = cap.get(1).or(cap.get(2)) {
                reads.insert(key.as_str().to_string());
            }
        }
    }

    if writes.is_empty() {
        return None;
    }
    Some(
        writes
            .into_iter()
            .filter(|(key, _)| !reads.contains(key))
            .map(|(key, (file, line))| WriteOnlyCacheKey { key, file, line })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_regex_requires_a_cache_flavored_receiver() {
        assert!(PUT_RE.is_match("cache.put(\"k\", 1)"));
        assert!(PUT_RE.is_match("imageCache.put(\"k\", 1)"));
        assert!(!PUT_RE.is_match("registry.put(\"k\", 1)"));
    }

    #[test]
    fn get_regex_accepts_both_call_and_index_forms() {
        assert!(GET_RE.is_match("cache.get(\"k\")"));
        assert!(GET_RE.is_match("memCache[\"k\"]"));
    }
}
