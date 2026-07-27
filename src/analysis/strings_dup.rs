//! String values duplicated across modules. Each copy drifts and gets
//! translated on its own — one shared resource is cheaper. Only base
//! locale files (res/values/, no -fr/-night qualifiers) are compared:
//! a translation matching another module's base value is noise.

use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct DuplicateString {
    pub value: String,
    /// (module, resource name) pairs, module-sorted
    pub declarations: Vec<(String, String)>,
}

static STRING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<string\s+name="([^"]+)"[^>]*>([^<]+)</string>"#).unwrap());

/// The module owning a file: the path (relative to root) before src/.
fn module_of(root: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let mut parts: Vec<String> = Vec::new();
    let mut crossed_src = false;
    for component in rel.components() {
        let name = component.as_os_str().to_string_lossy().to_string();
        if name == "src" {
            crossed_src = true;
            break;
        }
        parts.push(name);
    }
    if !crossed_src {
        parts.pop(); // the last segment is the file itself
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// None when the tree has no base-locale strings.xml at all.
pub fn duplicate_strings(root: &Path) -> Option<Vec<DuplicateString>> {
    let mut by_value: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut found_any = false;

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
            e.file_type().is_file()
                && e.file_name().to_string_lossy().ends_with(".xml")
                && e.path()
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n == "values")
                    .unwrap_or(false)
        })
    {
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        if !content.contains("<string") {
            continue;
        }
        found_any = true;
        let module = module_of(root, entry.path());
        for cap in STRING_RE.captures_iter(&content) {
            by_value
                .entry(cap[2].trim().to_string())
                .or_default()
                .push((module.clone(), cap[1].to_string()));
        }
    }

    if !found_any {
        return None;
    }

    let duplicates: Vec<DuplicateString> = by_value
        .into_iter()
        .filter(|(_, declarations)| {
            let modules: std::collections::BTreeSet<&str> =
                declarations.iter().map(|(m, _)| m.as_str()).collect();
            modules.len() >= 2
        })
        .map(|(value, mut declarations)| {
            declarations.sort();
            DuplicateString {
                value,
                declarations,
            }
        })
        .collect();
    Some(duplicates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_is_the_path_before_src() {
        let root = Path::new("/repo");
        assert_eq!(
            module_of(
                root,
                Path::new("/repo/feature/login/src/main/res/values/strings.xml")
            ),
            "feature/login"
        );
    }

    #[test]
    fn a_rootless_file_falls_back_to_dot() {
        let root = Path::new("/repo");
        assert_eq!(module_of(root, Path::new("/repo/strings.xml")), ".");
    }
}
