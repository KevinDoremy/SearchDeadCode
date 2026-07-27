//! Dead code kept company by its tests.
//!
//! A dead symbol whose name still appears in a test file means the test
//! outlived its target (often disabled, often forgotten): delete them
//! together. Token-level matching — 'Zombie' inside 'MegaZombieHelper'
//! is not a reference.

use crate::analysis::DeadCode;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("Invalid word regex"));

fn is_test_file(path: &Path) -> bool {
    let is_source = path
        .extension()
        .is_some_and(|e| e == "kt" || e == "kts" || e == "java");
    if !is_source {
        return false;
    }
    let in_test_dir = path.components().any(|c| {
        let name = c.as_os_str().to_string_lossy();
        name == "test" || name == "androidTest" || name == "sharedTest"
    });
    let test_named = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .is_some_and(|stem| stem.ends_with("Test") || stem.ends_with("Tests"));
    in_test_dir || test_named
}

pub fn annotate(dead_code: &mut [DeadCode], root: &Path) {
    let mut tokens: HashSet<String> = HashSet::new();
    let walker = walkdir::WalkDir::new(root).into_iter().filter_entry(|e| {
        if e.depth() == 0 {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        !name.starts_with('.') && name != "build" && name != "generated"
    });
    for entry in walker.flatten() {
        if !entry.file_type().is_file() || !is_test_file(entry.path()) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            for word in WORD.find_iter(&content) {
                tokens.insert(word.as_str().to_string());
            }
        }
    }
    if tokens.is_empty() {
        return;
    }

    for dc in dead_code.iter_mut() {
        // a dead test naming itself is not "kept company by a test"
        if is_test_file(&dc.declaration.location.file) {
            continue;
        }
        if tokens.contains(&dc.declaration.name) {
            dc.message
                .push_str(" [still referenced by tests — delete them together]");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directories_and_test_names_are_recognized() {
        assert!(is_test_file(Path::new("src/test/kotlin/FooTest.kt")));
        assert!(is_test_file(Path::new("app/src/androidTest/Bar.kt")));
        assert!(is_test_file(Path::new("anywhere/BazTests.java")));
        assert!(!is_test_file(Path::new("src/main/kotlin/Foo.kt")));
        assert!(!is_test_file(Path::new("src/test/resources/data.json")));
    }
}
