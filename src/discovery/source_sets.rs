//! Phantom source set detection.
//!
//! A directory under `src/` that is neither a conventional Gradle/AGP source
//! set nor declared in the module's build file is never compiled. Code inside
//! it must not feed the reference graph: it would keep dead code alive.

use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Source-set roots Gradle and AGP create without any declaration.
/// Combined names (testDebug, androidTestRelease, ...) start with one of these.
const CONVENTIONAL_PREFIXES: &[&str] = &[
    "main",
    "test",
    "androidTest",
    "testFixtures",
    "debug",
    "release",
];

/// Names declared in a build file: create("x"), register("x"), getByName("x"), ...
static DECLARED_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:create|register|getByName|maybeCreate|named)\(\s*"([A-Za-z0-9_]+)""#)
        .expect("Invalid declared-name regex")
});

/// Directories wired into an existing source set: java.srcDir("src/x/java"),
/// resources.srcDirs("src/x/assets"), ...
static SRC_DIR_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"srcDirs?\(\s*"src/([A-Za-z0-9_]+)/"#).expect("Invalid srcDir regex")
});

/// Result of scanning a project for phantom source sets
#[derive(Debug, Default)]
pub struct SourceSetAudit {
    /// Directories under src/ that are not part of any build
    pub phantom_dirs: Vec<PathBuf>,
}

/// Scan the project root and its immediate subdirectories (Gradle modules)
/// for src/ subdirectories that no build file accounts for.
pub fn detect_phantom_source_sets(project_root: &Path) -> SourceSetAudit {
    let mut audit = SourceSetAudit::default();
    scan_module(project_root, &mut audit.phantom_dirs);

    if let Ok(entries) = fs::read_dir(project_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_module(&path, &mut audit.phantom_dirs);
            }
        }
    }

    audit.phantom_dirs.sort();
    audit
}

fn scan_module(module_dir: &Path, out: &mut Vec<PathBuf>) {
    let build_file = ["build.gradle.kts", "build.gradle"]
        .iter()
        .map(|name| module_dir.join(name))
        .find(|p| p.is_file());
    let Some(build_file) = build_file else {
        return; // no build file, no ground truth
    };

    let src = module_dir.join("src");
    if !src.is_dir() {
        return;
    }

    let declared = declared_names(&fs::read_to_string(&build_file).unwrap_or_default());

    if let Ok(entries) = fs::read_dir(&src) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_known_source_set(&name, &declared) {
                out.push(path);
            }
        }
    }
}

fn declared_names(build_file_text: &str) -> HashSet<String> {
    DECLARED_NAME
        .captures_iter(build_file_text)
        .chain(SRC_DIR_NAME.captures_iter(build_file_text))
        .map(|c| c[1].to_string())
        .collect()
}

fn is_known_source_set(name: &str, declared: &HashSet<String>) -> bool {
    CONVENTIONAL_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || declared.iter().any(|d| name.starts_with(d.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conventional_and_combined_names_are_known() {
        let declared = HashSet::new();
        for name in [
            "main",
            "test",
            "androidTest",
            "testDebug",
            "androidTestRelease",
        ] {
            assert!(
                is_known_source_set(name, &declared),
                "{name} should be known"
            );
        }
    }

    #[test]
    fn undeclared_custom_name_is_phantom() {
        let declared = HashSet::new();
        assert!(!is_known_source_set("savedTests", &declared));
    }

    #[test]
    fn src_dir_additions_count_as_declared() {
        // Real-world pattern: getByName("test") { java.srcDir("src/sharedTest/java") }
        let names = declared_names(
            "sourceSets {\n    getByName(\"test\") {\n        java.srcDir(\"src/sharedTest/java\")\n        resources.srcDirs(\"src/test/assets\")\n    }\n}\n",
        );
        assert!(names.contains("sharedTest"), "names were: {names:?}");
    }

    #[test]
    fn declared_names_are_extracted_from_build_text() {
        let names = declared_names("sourceSets {\n    create(\"savedTests\")\n}\n");
        assert!(names.contains("savedTests"));
    }
}
