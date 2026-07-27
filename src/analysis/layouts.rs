//! Dead layout detection (ViewBinding-aware).
//!
//! A layout survives when anything mentions its generated Binding class
//! (`old_checkout.xml` → `OldCheckoutBinding`), inflates it via
//! `R.layout.<name>`, or includes it from another layout. Otherwise nothing
//! can display it: it is dead.

use crate::discovery::{FileType, SourceFile};
use std::fs;
use std::path::PathBuf;

/// snake_case layout stem → PascalCase binding class name
pub fn to_binding_name(stem: &str) -> String {
    let mut out = String::new();
    for part in stem.split(['_', '-']) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out.push_str("Binding");
    out
}

/// Layouts with no binding usage, no R.layout reference and no include
pub fn find_dead_layouts(files: &[SourceFile]) -> Vec<PathBuf> {
    let layouts: Vec<&SourceFile> = files
        .iter()
        .filter(|f| matches!(f.file_type, FileType::XmlLayout))
        .collect();
    if layouts.is_empty() {
        return Vec::new();
    }

    // One corpus of code and one of XML; layout stems are searched with
    // their delimiters so `screen` never matches `screen_v2`
    let mut code_corpus = String::new();
    let mut xml_corpus = String::new();
    for file in files {
        match file.file_type {
            FileType::Kotlin | FileType::Java => {
                if let Ok(content) = fs::read_to_string(&file.path) {
                    code_corpus.push_str(&content);
                    code_corpus.push('\n');
                }
            }
            FileType::XmlLayout
            | FileType::XmlManifest
            | FileType::XmlNavigation
            | FileType::XmlMenu
            | FileType::XmlOther => {
                if let Ok(content) = fs::read_to_string(&file.path) {
                    xml_corpus.push_str(&content);
                    xml_corpus.push('\n');
                }
            }
        }
    }

    let mut dead: Vec<PathBuf> = layouts
        .iter()
        .filter_map(|layout| {
            let stem = layout.path.file_stem()?.to_string_lossy();
            let binding = to_binding_name(&stem);
            // Word boundaries: R.layout.screen must not match R.layout.screen_v2
            let inflate_ref =
                regex::Regex::new(&format!(r"R\.layout\.{}\b", regex::escape(&stem))).ok()?;
            let include_ref =
                regex::Regex::new(&format!(r"@layout/{}\b", regex::escape(&stem))).ok()?;

            let used = code_corpus.contains(&binding)
                || inflate_ref.is_match(&code_corpus)
                || include_ref.is_match(&xml_corpus);
            if used {
                None
            } else {
                Some(layout.path.clone())
            }
        })
        .collect();
    dead.sort();
    dead
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_names_follow_the_viewbinding_convention() {
        assert_eq!(to_binding_name("old_checkout"), "OldCheckoutBinding");
        assert_eq!(to_binding_name("item_row_v2"), "ItemRowV2Binding");
        assert_eq!(to_binding_name("main"), "MainBinding");
    }
}
