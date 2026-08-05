//! The one vocabulary for `@Suppress`, and the `@file:Suppress` reservation.
//!
//! Four spellings of the same test used to coexist, with three different case
//! semantics: `deep.rs` (case-sensitive), its own duplicate, `main.rs`
//! (`to_lowercase`), `unused_param.rs` (`to_uppercase`). They live here now,
//! once.
//!
//! The distinction that matters: `"unused"` is the inspection "nothing
//! references this declaration", and declining it declines the whole family.
//! `"UNUSED_PARAMETER"`, `"UNUSED_VARIABLE"` and `"UNUSED_EXPRESSION"` are
//! compiler warnings about something else — a file that silences the parameter
//! warning has said nothing about whether its classes are reachable. A
//! `contains("unused")` cannot separate them, because `UNUSED_PARAMETER`
//! contains `unused`. A word boundary can: `_` is a word character, so it
//! blocks the shorter match.
//!
//! A file-level suppression never REMOVES a finding. An opt-out written once
//! at the top of a file is not evidence that a symbol is alive, and hiding the
//! finding is exactly how a temporary silence becomes permanent. The finding
//! stays, carries the reservation, and loses one notch of confidence.
//!
//! A DECLARATION-level suppression does remove it, and the asymmetry is
//! deliberate: that annotation names one symbol, its author wrote it on the
//! line that produced the warning, and obeying it is what detekt, ktlint and
//! the compiler all do. A file annotation covers whatever the file will
//! contain next year. `DETECTORS.md`, section `## Suppressions`, carries the
//! full rule and the four sites that apply the declaration level.

use crate::analysis::{DeadCode, DeadCodeIssue};
use regex::Regex;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// `@Suppress(...)`, `@SuppressLint(...)`, `@SuppressWarnings(...)`, qualified
/// or not. Group 1 is the argument list, parentheses excluded.
static ANNOTATION_SUPPRESS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"@(?:[A-Za-z_][A-Za-z0-9_]*\s*\.\s*)*(?:Suppress|SuppressLint|SuppressWarnings)\s*\(([^)]*)\)",
    )
    .expect("invalid @Suppress regex")
});

/// `@file:Suppress(...)`, plain or bracketed
/// (`@file:[JvmName("X") Suppress("unused")]`).
static FILE_SUPPRESS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"@file\s*:\s*(?:\[[^\]]*?)?Suppress\s*\(([^)]*)\)")
        .expect("invalid @file:Suppress regex")
});

/// Declines "nothing references this declaration" across the whole family.
/// The `UnusedPrivate*` names are Detekt rules SDC already honoured.
pub const UNUSED_DECLARATION: &[&str] = &[
    "unused",
    "UnusedPrivateMember",
    "UnusedPrivateClass",
    "UnusedPrivateProperty",
];
/// …plus the compiler warning a parameter detector is the counterpart of.
pub const UNUSED_PARAMETER: &[&str] = &["unused", "UNUSED_PARAMETER"];
/// …plus the ones a write-only detector is the counterpart of.
pub const UNUSED_VARIABLE: &[&str] = &[
    "unused",
    "UNUSED_VARIABLE",
    "UNUSED_EXPRESSION",
    "ASSIGNED_VALUE_IS_NEVER_READ",
];
pub const UNUSED_IMPORT: &[&str] = &["unused", "UNUSED_IMPORT", "UnusedImports"];

/// `needle` appears in `args` as a whole word, case ignored. Diagnostic names
/// are ASCII, so lowercasing preserves byte indices.
fn names(args: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let hay = args.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    hay.match_indices(&needle).any(|(i, _)| {
        let before = i == 0 || !is_word(hay.as_bytes()[i - 1]);
        let end = i + needle.len();
        let after = end >= hay.len() || !is_word(hay.as_bytes()[end]);
        before && after
    })
}

/// The first diagnostic of `family` this argument list actually names.
fn first_named(args: &str, family: &[&str]) -> Option<String> {
    family
        .iter()
        .find(|d| names(args, d))
        .map(|d| (*d).to_string())
}

/// Does one of the declaration's own annotations name this diagnostic? The
/// predicate the four divergent spellings each wrote their own way.
pub fn annotations_suppress(annotations: &[String], family: &[&str]) -> bool {
    annotations.iter().any(|a| {
        ANNOTATION_SUPPRESS_RE
            .captures_iter(a)
            .any(|c| first_named(c.get(1).map_or("", |m| m.as_str()), family).is_some())
    })
}

/// The diagnostics this rule answers to, beyond its own code (which the caller
/// always prepends). A rule with no honest counterpart gets an empty list
/// rather than an invented identifier that would never match.
pub fn family_of(issue: DeadCodeIssue) -> &'static [&'static str] {
    use DeadCodeIssue as I;
    match issue {
        I::Unreferenced
        | I::UnusedEnumCase
        | I::UnusedSealedVariant
        | I::DeadDtoField
        | I::WriteOnlyPreference
        | I::WriteOnlyDao
        | I::UnusedIntentExtra
        | I::DeadConfigKey => UNUSED_DECLARATION,

        I::AssignOnly => UNUSED_VARIABLE,
        I::UnusedParameter => UNUSED_PARAMETER,
        I::UnusedImport => UNUSED_IMPORT,

        I::RedundantPublic => &[
            "MemberVisibilityCanBePrivate",
            "RedundantVisibilityModifier",
        ],
        I::DeadBranch => &["UNREACHABLE_CODE", "SENSELESS_COMPARISON"],
        I::RedundantOverride => &["RedundantOverride"],
        I::UnusedResource | I::UnusedLayout => &["UnusedResources"],

        // Everything else answers to its own code, which is always correct.
        _ => &[],
    }
}

/// A file annotation always lives in the first bytes. Reading more is waste on
/// a four-thousand-line file.
const HEADER_BYTES: u64 = 8 * 1024;
/// With no package header to stop at, keep this much: enough for any real
/// annotation block, short enough to stay honest.
const HEADER_LINES_WITHOUT_PACKAGE: usize = 50;

/// Blank every comment to spaces, byte offsets preserved, string literals
/// LEFT INTACT — the annotation's own argument lives in one.
///
/// A `// TODO remove: @file:Suppress("unused")` is a note about an opt-out,
/// not an opt-out. Without this the reservation named a suppression that was
/// not in effect, and lowered the confidence of a finding nothing had
/// declined. A license header mentioning the annotation would do the same.
fn blank_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                // Raw strings first: `"""…"""` swallows quotes and newlines.
                if bytes[i..].starts_with(b"\"\"\"") {
                    i += 3;
                    while i < bytes.len() && !bytes[i..].starts_with(b"\"\"\"") {
                        i += 1;
                    }
                    i = (i + 3).min(bytes.len());
                } else {
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'"' {
                        i += if bytes[i] == b'\\' { 2 } else { 1 };
                    }
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let start = i;
                i += 2;
                while i < bytes.len() && !bytes[i..].starts_with(b"*/") {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                for slot in &mut out[start..i] {
                    if *slot != b'\n' {
                        *slot = b' ';
                    }
                }
            }
            _ => i += 1,
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Kotlin requires file annotations BEFORE `package`. Cutting there stops a
/// `@file:Suppress` quoted inside a string four hundred lines down from
/// silencing the whole file.
fn cut_at_body(text: &str) -> &str {
    let mut offset = 0usize;
    for (seen, line) in text.split_inclusive('\n').enumerate() {
        let head = line.trim_start();
        if head.starts_with("package") || head.starts_with("import") {
            return &text[..offset];
        }
        offset += line.len();
        if seen + 1 >= HEADER_LINES_WITHOUT_PACKAGE {
            return &text[..offset];
        }
    }
    text
}

fn read_header(path: &Path) -> String {
    let Ok(file) = std::fs::File::open(path) else {
        return String::new();
    };
    let mut buf = Vec::with_capacity(HEADER_BYTES as usize);
    if file.take(HEADER_BYTES).read_to_end(&mut buf).is_err() {
        return String::new();
    }
    // Truncation can split a multi-byte character; annotations are ASCII, so
    // the loss is inconsequential. Comments go before the cut, so a commented
    // `// package` cannot end the header early either.
    let text = blank_comments(&String::from_utf8_lossy(&buf));
    cut_at_body(&text).to_string()
}

/// Marks the findings whose file declines their diagnostic. Returns how many
/// were reserved. Removes NOTHING — see the module doc.
pub fn annotate(dead_code: &mut [DeadCode]) -> usize {
    let mut headers: HashMap<PathBuf, String> = HashMap::new();
    let mut reserved = 0usize;

    for dc in dead_code.iter_mut() {
        let file = &dc.declaration.location.file;
        // `@file:` is Kotlin only: not Java, not resource XML.
        if !matches!(
            file.extension().and_then(|e| e.to_str()),
            Some("kt") | Some("kts")
        ) {
            continue;
        }
        let header = headers
            .entry(file.clone())
            .or_insert_with(|| read_header(file));
        if !header.contains("@file") {
            continue;
        }

        let extra = family_of(dc.issue);
        let mut family: Vec<&str> = Vec::with_capacity(1 + extra.len());
        family.push(dc.issue.code());
        family.extend_from_slice(extra);

        let named = FILE_SUPPRESS_RE
            .captures_iter(header)
            .find_map(|c| first_named(c.get(1).map_or("", |m| m.as_str()), &family));
        let Some(named) = named else { continue };

        dc.message.push_str(&format!(
            " [file marked @file:Suppress(\"{named}\") — kept in the report, confidence lowered]"
        ));
        dc.confidence = dc.confidence.downgraded();
        reserved += 1;
    }

    reserved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_matches_but_unused_parameter_does_not() {
        // The whole point of the word boundary: `_` is a word character, so
        // the short name cannot match inside the long one.
        assert!(names("\"unused\"", "unused"));
        assert!(!names("\"UNUSED_PARAMETER\"", "unused"));
        assert!(names("\"UNUSED_PARAMETER\"", "UNUSED_PARAMETER"));
    }

    #[test]
    fn the_test_ignores_case() {
        for spelling in ["\"unused\"", "\"UNUSED\"", "\"Unused\""] {
            assert!(names(spelling, "unused"), "{spelling} should match");
        }
    }

    #[test]
    fn a_longer_word_is_not_a_match() {
        assert!(!names("\"unusedThing\"", "unused"));
        assert!(!names("\"notunused\"", "unused"));
    }

    #[test]
    fn the_bracketed_file_form_is_read() {
        let header = "@file:[JvmName(\"Helpers\") Suppress(\"unused\")]\n";
        let found = FILE_SUPPRESS_RE
            .captures_iter(header)
            .find_map(|c| first_named(c.get(1).map_or("", |m| m.as_str()), UNUSED_DECLARATION));
        assert_eq!(found.as_deref(), Some("unused"));
    }

    #[test]
    fn a_file_jvmname_alone_is_not_a_suppression() {
        assert!(!FILE_SUPPRESS_RE.is_match("@file:JvmName(\"Helpers\")\n"));
    }

    #[test]
    fn the_header_stops_at_the_package_line() {
        let source = "@file:Suppress(\"unused\")\npackage s\n\nval quoted = \"@file:Suppress(\\\"DC001\\\")\"\n";
        let header = cut_at_body(source);
        assert!(header.contains("@file:Suppress(\"unused\")"));
        assert!(!header.contains("DC001"), "header was:\n{header}");
    }

    #[test]
    fn the_header_stops_after_fifty_lines_without_a_package() {
        let source = "// a comment\n".repeat(80) + "@file:Suppress(\"unused\")\n";
        assert!(!cut_at_body(&source).contains("Suppress"));
    }

    #[test]
    fn a_qualified_annotation_counts() {
        assert!(annotations_suppress(
            &["@kotlin.Suppress(\"unused\")".to_string()],
            UNUSED_DECLARATION
        ));
    }

    #[test]
    fn suppresslint_and_suppresswarnings_count() {
        assert!(annotations_suppress(
            &["@SuppressWarnings(\"unused\")".to_string()],
            UNUSED_DECLARATION
        ));
        assert!(annotations_suppress(
            &["@SuppressLint(\"unused\")".to_string()],
            UNUSED_DECLARATION
        ));
    }

    #[test]
    fn a_parameter_suppression_is_not_a_declaration_suppression() {
        let annotations = ["@Suppress(\"UNUSED_PARAMETER\")".to_string()];
        assert!(annotations_suppress(&annotations, UNUSED_PARAMETER));
        assert!(!annotations_suppress(&annotations, UNUSED_DECLARATION));
    }

    #[test]
    fn every_rule_answers_to_its_own_code() {
        // `family_of` may be empty for a rule with no counterpart; the caller
        // always prepends the code, so the empty list is still usable.
        assert!(names("\"DC019\"", DeadCodeIssue::UnusedIntentExtra.code()));
    }
}
