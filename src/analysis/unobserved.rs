//! Exposed LiveData/StateFlow/SharedFlow properties that nothing ever
//! dereferences: no collect, no observe, no operator chain. The whole
//! upstream computation feeding them runs for nobody.
//!
//! Conservative by design: ANY qualified access (`name.anything`)
//! anywhere in the codebase counts as usage, so a mapped or merged
//! stream is never flagged. Private/protected properties are skipped —
//! the `_uiState` backing-field half of the classic pair is internal
//! plumbing, only the exposed half matters.

use regex::Regex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct StreamFinding {
    pub name: String,
    pub stream_type: String,
    pub file: PathBuf,
    pub line: usize,
}

static STREAM_DECL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[^\S\n]*(?:(?:public|internal|override|open|final|lateinit)\s+)*val\s+(\w+)\s*:\s*(?:[\w.]*\.)?((?:Mutable)?(?:LiveData|StateFlow|SharedFlow)|Flow)\s*<",
    )
    .unwrap()
});

static WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+").unwrap());

/// Every identifier followed by `.letter` — overlap-safe, unlike a
/// single capture regex which would eat the first char of the next
/// segment in `model.ticks.collect`.
fn dereferenced_names(content: &str, into: &mut BTreeSet<String>) {
    for word in WORD_RE.find_iter(content) {
        let rest = content[word.end()..].trim_start();
        if let Some(after_dot) = rest.strip_prefix('.') {
            if after_dot
                .trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic())
            {
                into.insert(word.as_str().to_string());
            }
        }
    }
}

pub fn unobserved_streams(root: &Path) -> Vec<StreamFinding> {
    let mut declared: Vec<StreamFinding> = Vec::new();
    let mut dereferenced: BTreeSet<String> = BTreeSet::new();

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
        .filter(|e| e.file_type().is_file() && e.file_name().to_string_lossy().ends_with(".kt"))
    {
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for cap in STREAM_DECL_RE.captures_iter(&content) {
            let whole = cap.get(0).unwrap();
            let line_start = content[..whole.start()].rfind('\n').map_or(0, |i| i + 1);
            let line_text = content[line_start..].lines().next().unwrap_or("");
            if line_text.contains("private") || line_text.contains("protected") {
                continue;
            }
            let line = content[..whole.start()].matches('\n').count() + 1;
            declared.push(StreamFinding {
                name: cap[1].to_string(),
                stream_type: cap[2].to_string(),
                file: entry.path().to_path_buf(),
                line,
            });
        }
        dereferenced_names(&content, &mut dereferenced);
    }

    declared.retain(|finding| !dereferenced.contains(&finding.name));
    declared.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    declared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decl_regex_matches_annotated_stream_property() {
        let caps = STREAM_DECL_RE
            .captures("    val ticks: StateFlow<Int> = MutableStateFlow(0)\n")
            .unwrap();
        assert_eq!(&caps[1], "ticks");
        assert_eq!(&caps[2], "StateFlow");
    }

    #[test]
    fn decl_regex_matches_qualified_type() {
        let caps = STREAM_DECL_RE
            .captures("    val user: androidx.lifecycle.LiveData<String> = load()\n")
            .unwrap();
        assert_eq!(&caps[1], "user");
    }

    #[test]
    fn decl_regex_skips_untyped_backing_field() {
        assert!(STREAM_DECL_RE
            .captures("    private val _uiState = MutableStateFlow(0)\n")
            .is_none());
    }

    #[test]
    fn every_segment_of_a_chain_counts_as_dereferenced() {
        let mut names = BTreeSet::new();
        dereferenced_names("model.ticks.collect { }", &mut names);
        assert!(names.contains("model") && names.contains("ticks"));
        assert!(
            !names.contains("collect"),
            "the last segment has no dot after it"
        );
    }

    #[test]
    fn a_bare_mention_is_not_a_dereference() {
        let mut names = BTreeSet::new();
        dereferenced_names("println(model.ticks)", &mut names);
        assert!(!names.contains("ticks"), "no dot after ticks");
    }
}
