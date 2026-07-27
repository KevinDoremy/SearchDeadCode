//! Whole dead Gradle modules.
//!
//! A module included in settings.gradle(.kts) that no other build file
//! depends on — neither `project(":name")` nor the typesafe
//! `projects.name` accessor — and that is not an application module is
//! a whole-module deletion candidate: the biggest LOC wins there are.

use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static INCLUDE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"include\s*[\(]?\s*["']([:A-Za-z0-9_.-]+)["']"#).expect("Invalid include regex")
});

#[derive(Debug)]
pub struct DeadModule {
    /// Gradle path, e.g. ":orphan"
    pub gradle_path: String,
    /// Directory relative to the repo root
    pub dir: PathBuf,
}

/// None when no settings file exists (single-module repo — nothing to say)
pub fn dead_modules(root: &Path) -> Option<Vec<DeadModule>> {
    let settings = ["settings.gradle.kts", "settings.gradle"]
        .iter()
        .map(|name| root.join(name))
        .find(|p| p.exists())?;
    let settings_content = std::fs::read_to_string(&settings).ok()?;

    let included: Vec<String> = INCLUDE
        .captures_iter(&settings_content)
        .map(|c| {
            let raw = c[1].to_string();
            if raw.starts_with(':') {
                raw
            } else {
                format!(":{raw}")
            }
        })
        .collect();
    if included.is_empty() {
        return Some(Vec::new());
    }

    // Every build file in the repo, with its module dir
    let mut build_files: Vec<(PathBuf, String)> = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "build" && name != "generated"
        })
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "build.gradle" || name == "build.gradle.kts" {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                build_files.push((entry.path().to_path_buf(), content));
            }
        }
    }

    let mut dead = Vec::new();
    for module in &included {
        let dir: PathBuf = module
            .trim_start_matches(':')
            .split(':')
            .collect::<Vec<_>>()
            .join("/")
            .into();
        let module_build = root.join(&dir);

        // `project(":name")` and the typesafe accessor `projects.name`
        // (last segment, kebab/underscore folded to camelCase)
        let accessor = typesafe_accessor(module);
        let project_needle = format!("\"{module}\"");
        let project_needle_single = format!("'{module}'");
        let referenced = build_files.iter().any(|(path, content)| {
            if path.starts_with(&module_build) {
                return false; // a module referencing itself proves nothing
            }
            content.contains(&project_needle)
                || content.contains(&project_needle_single)
                || content.contains(&accessor)
        });
        if referenced {
            continue;
        }

        // Application modules are roots by definition
        let is_application = build_files
            .iter()
            .filter(|(path, _)| path.starts_with(&module_build))
            .any(|(_, content)| content.contains("com.android.application"));
        if is_application {
            continue;
        }

        dead.push(DeadModule {
            gradle_path: module.clone(),
            dir,
        });
    }
    Some(dead)
}

/// ":feature:my-thing" → "projects.feature.myThing"
fn typesafe_accessor(gradle_path: &str) -> String {
    let camel: Vec<String> = gradle_path
        .trim_start_matches(':')
        .split(':')
        .map(|segment| {
            let mut out = String::new();
            let mut upper_next = false;
            for ch in segment.chars() {
                match ch {
                    '-' | '_' => upper_next = true,
                    c if upper_next => {
                        out.extend(c.to_uppercase());
                        upper_next = false;
                    }
                    c => out.push(c),
                }
            }
            out
        })
        .collect();
    format!("projects.{}", camel.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_are_parsed_in_both_dialects() {
        let kts = "include(\":app\")\ninclude(\":core\")";
        let groovy = "include ':app'\ninclude ':feature-login'";
        let from = |s: &str| -> Vec<String> {
            INCLUDE.captures_iter(s).map(|c| c[1].to_string()).collect()
        };
        assert_eq!(from(kts), vec![":app", ":core"]);
        assert_eq!(from(groovy), vec![":app", ":feature-login"]);
    }

    #[test]
    fn typesafe_accessors_fold_dashes_to_camel_case() {
        assert_eq!(typesafe_accessor(":orphan"), "projects.orphan");
        assert_eq!(
            typesafe_accessor(":feature:my-thing"),
            "projects.feature.myThing"
        );
    }
}
