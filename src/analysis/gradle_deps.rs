//! Declared-but-never-imported Gradle dependencies. A dependency that
//! no source file imports still costs resolution, download and build
//! time on every compile — the build-file counterpart of dead code.
//!
//! Scope is deliberately conservative: only string coordinates in
//! import-visible configurations. Annotation processors (ksp/kapt),
//! runtimeOnly, platform() BOMs, project() modules and version-catalog
//! refs are skipped — none of them is expected to appear in an import.

use regex::Regex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct DeclaredDep {
    /// group:artifact, version stripped
    pub coordinate: String,
    pub build_file: PathBuf,
}

static DEP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?m)^\s*(?:implementation|api|compileOnly|testImplementation|androidTestImplementation|debugImplementation|releaseImplementation)\s*\(?\s*["']([^"']+)["']"#,
    )
    .unwrap()
});

static IMPORT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*import\s+(?:static\s+)?([\w.]+)").unwrap());

/// Artifact-name tokens too generic to prove usage on their own
/// (core-ktx would otherwise match half of androidx).
const GENERIC_TOKENS: &[&str] = &[
    "core",
    "ktx",
    "runtime",
    "android",
    "common",
    "api",
    "impl",
    "lib",
    "library",
    "jvm",
    "kotlin",
    "test",
    "annotations",
    "plugin",
    "bom",
];

/// None when the tree has no build.gradle(.kts) at all.
pub fn unused_dependencies(root: &Path) -> Option<Vec<DeclaredDep>> {
    let mut build_files: Vec<PathBuf> = Vec::new();
    let mut source_files: Vec<PathBuf> = Vec::new();
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
        .filter(|e| e.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy();
        if name == "build.gradle" || name == "build.gradle.kts" {
            build_files.push(entry.into_path());
        } else if name.ends_with(".kt") || name.ends_with(".java") {
            source_files.push(entry.into_path());
        }
    }
    if build_files.is_empty() {
        return None;
    }

    let mut declared: Vec<DeclaredDep> = Vec::new();
    for build_file in &build_files {
        let Ok(content) = std::fs::read_to_string(build_file) else {
            continue;
        };
        for cap in DEP_RE.captures_iter(&content) {
            let raw = &cap[1];
            let mut parts = raw.split(':');
            let (Some(group), Some(artifact)) = (parts.next(), parts.next()) else {
                continue;
            };
            declared.push(DeclaredDep {
                coordinate: format!("{group}:{artifact}"),
                build_file: build_file.clone(),
            });
        }
    }

    let mut import_segments: BTreeSet<String> = BTreeSet::new();
    let mut imports: Vec<String> = Vec::new();
    for source in &source_files {
        let Ok(content) = std::fs::read_to_string(source) else {
            continue;
        };
        for cap in IMPORT_RE.captures_iter(&content) {
            let path = cap[1].to_string();
            for segment in path.split('.') {
                import_segments.insert(segment.to_string());
            }
            imports.push(path);
        }
    }

    let mut unused: Vec<DeclaredDep> = declared
        .into_iter()
        .filter(|dep| !is_used(&dep.coordinate, &imports, &import_segments))
        .collect();
    unused.sort_by(|a, b| a.coordinate.cmp(&b.coordinate));
    unused.dedup_by(|a, b| a.coordinate == b.coordinate && a.build_file == b.build_file);
    Some(unused)
}

fn is_used(coordinate: &str, imports: &[String], import_segments: &BTreeSet<String>) -> bool {
    let (group, artifact) = coordinate.split_once(':').unwrap_or((coordinate, ""));

    let group_prefix = format!("{group}.");
    if imports
        .iter()
        .any(|i| i == group || i.starts_with(&group_prefix))
    {
        return true;
    }

    // gson: group com.google.code.gson, package com.google.gson — the
    // last group segment showing up anywhere in an import is the signal
    if let Some(last) = group.split('.').next_back() {
        if import_segments.contains(last) {
            return true;
        }
    }

    artifact
        .split('-')
        .filter(|token| !GENERIC_TOKENS.contains(token))
        .any(|token| import_segments.contains(token))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments(imports: &[&str]) -> (Vec<String>, BTreeSet<String>) {
        let owned: Vec<String> = imports.iter().map(|s| s.to_string()).collect();
        let segs = owned
            .iter()
            .flat_map(|i| i.split('.').map(str::to_string))
            .collect();
        (owned, segs)
    }

    #[test]
    fn group_prefix_match_is_usage() {
        let (imports, segs) = segments(&["androidx.room.Room"]);
        assert!(is_used("androidx.room:room-runtime", &imports, &segs));
    }

    #[test]
    fn last_group_segment_match_is_usage() {
        let (imports, segs) = segments(&["com.google.gson.Gson"]);
        assert!(is_used("com.google.code.gson:gson", &imports, &segs));
    }

    #[test]
    fn artifact_token_match_is_usage() {
        let (imports, segs) = segments(&["kotlinx.coroutines.flow.Flow"]);
        assert!(is_used(
            "org.jetbrains.kotlinx:kotlinx-coroutines-core",
            &imports,
            &segs
        ));
    }

    #[test]
    fn generic_tokens_alone_prove_nothing() {
        let (imports, segs) = segments(&["androidx.core.view.ViewCompat"]);
        assert!(!is_used("com.example.thing:thing-core", &imports, &segs));
    }

    #[test]
    fn no_import_means_unused() {
        let (imports, segs) = segments(&["sample.Main"]);
        assert!(!is_used("com.squareup.moshi:moshi", &imports, &segs));
    }
}
