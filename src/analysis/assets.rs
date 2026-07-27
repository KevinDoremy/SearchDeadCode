//! Unused files under assets/ directories. Assets are read by string
//! path (assets.open, Typeface.createFromAsset, android_asset URLs), so
//! an asset whose relative path or bare filename appears nowhere in the
//! sources — or in any other asset, web assets reference each other —
//! ships dead bytes in every APK.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct UnusedAsset {
    /// Path relative to its assets/ root, forward slashes
    pub rel_path: String,
    pub file: PathBuf,
}

/// Only text-ish assets can reference other assets; probing binaries
/// for substrings would be noise.
const REFERENCING_ASSET_EXTS: &[&str] = &["html", "htm", "css", "js", "json", "svg", "xml", "txt"];

/// None when no assets/ directory exists under the root.
pub fn unused_assets(root: &Path) -> Option<Vec<UnusedAsset>> {
    let mut assets: Vec<(String, PathBuf)> = Vec::new();
    let mut corpus: Vec<(PathBuf, String)> = Vec::new();

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
        let path = entry.path();
        let name = entry.file_name().to_string_lossy();
        if let Some(rel) = asset_relative_path(path) {
            assets.push((rel, path.to_path_buf()));
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if REFERENCING_ASSET_EXTS.contains(&ext.as_str()) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    corpus.push((path.to_path_buf(), content));
                }
            }
        } else if name.ends_with(".kt") || name.ends_with(".java") {
            if let Ok(content) = std::fs::read_to_string(path) {
                corpus.push((path.to_path_buf(), content));
            }
        }
    }

    if assets.is_empty() {
        return None;
    }

    let mut unused: Vec<UnusedAsset> = assets
        .into_iter()
        .filter(|(rel, file)| {
            let basename = rel.rsplit('/').next().unwrap_or(rel);
            // a file mentioning its own name does not keep itself alive
            !corpus.iter().any(|(src, content)| {
                src != file && (content.contains(rel.as_str()) || content.contains(basename))
            })
        })
        .map(|(rel_path, file)| UnusedAsset { rel_path, file })
        .collect();
    unused.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Some(unused)
}

/// The path below the nearest `assets` directory component, forward
/// slashes — how the file is addressed at runtime.
fn asset_relative_path(path: &Path) -> Option<String> {
    let components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    let idx = components.iter().rposition(|c| c == "assets")?;
    if idx + 1 >= components.len() {
        return None;
    }
    Some(components[idx + 1..].join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_starts_below_assets() {
        assert_eq!(
            asset_relative_path(Path::new("/repo/src/main/assets/data/config.json")).as_deref(),
            Some("data/config.json")
        );
    }

    #[test]
    fn non_asset_paths_are_none() {
        assert_eq!(
            asset_relative_path(Path::new("/repo/src/main/kotlin/Main.kt")),
            None
        );
    }
}
