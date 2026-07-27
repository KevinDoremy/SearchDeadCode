//! --diff-base <ref>: what became dead since a git reference.
//!
//! The reference state is materialized in a temporary git worktree and
//! analyzed by re-running this very binary with --format json; findings
//! are compared by (relative file, symbol name, rule code). Line numbers
//! stay out of the fingerprint — they drift.

use crate::analysis::DeadCode;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

pub type Fingerprint = (String, String, String);

pub fn fingerprint_of(dc: &DeadCode, root: &Path) -> Fingerprint {
    let file = dc
        .declaration
        .location
        .file
        .strip_prefix(root)
        .unwrap_or(&dc.declaration.location.file)
        .to_string_lossy()
        .into_owned();
    (
        file,
        dc.declaration.name.clone(),
        dc.issue.code().to_string(),
    )
}

/// Analyze `git_ref` in a throwaway worktree and return its finding
/// fingerprints. Any git or analysis failure is a hard error: comparing
/// against a state we could not build would silently report everything
/// as new.
pub fn reference_fingerprints(root: &Path, git_ref: &str) -> Result<HashSet<Fingerprint>, String> {
    let resolved = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--verify", &format!("{git_ref}^{{commit}}")])
        .output()
        .map_err(|e| format!("git unavailable: {e}"))?;
    if !resolved.status.success() {
        return Err(format!(
            "cannot resolve '{git_ref}': {}",
            String::from_utf8_lossy(&resolved.stderr).trim()
        ));
    }
    let sha = String::from_utf8_lossy(&resolved.stdout).trim().to_string();

    let worktree = std::env::temp_dir().join(format!(
        "searchdeadcode-diffbase-{}-{}",
        std::process::id(),
        &sha[..sha.len().min(12)]
    ));
    let _ = std::fs::remove_dir_all(&worktree);

    let added = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("worktree")
        .arg("add")
        .arg("--detach")
        .arg(&worktree)
        .arg(&sha)
        .output()
        .map_err(|e| format!("git worktree failed to spawn: {e}"))?;
    if !added.status.success() {
        return Err(format!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&added.stderr).trim()
        ));
    }

    let result = analyze_worktree(&worktree);

    let _ = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["worktree", "remove", "--force"])
        .arg(&worktree)
        .output();

    result
}

fn analyze_worktree(worktree: &Path) -> Result<HashSet<Fingerprint>, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find own binary: {e}"))?;
    let output = Command::new(exe)
        .arg(worktree)
        .args(["--format", "json", "--quiet"])
        .output()
        .map_err(|e| format!("reference analysis failed to spawn: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "reference analysis failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("reference analysis produced invalid JSON: {e}"))?;

    let mut fingerprints = HashSet::new();
    if let Some(issues) = json["issues"].as_array() {
        for issue in issues {
            let (Some(file), Some(code), Some(name)) = (
                issue["file"].as_str(),
                issue["code"].as_str(),
                issue["declaration"]["name"].as_str(),
            ) else {
                continue;
            };
            let relative = Path::new(file)
                .strip_prefix(worktree)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| file.to_string());
            fingerprints.insert((relative, name.to_string(), code.to_string()));
        }
    }
    Ok(fingerprints)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::DeadCodeIssue;
    use crate::graph::{Declaration, DeclarationId, DeclarationKind, Language, Location};
    use std::path::PathBuf;

    #[test]
    fn fingerprints_are_relative_and_line_free() {
        let path = PathBuf::from("/project/src/Zombie.kt");
        let decl = Declaration::new(
            DeclarationId::new(path.clone(), 0, 10),
            "Zombie".to_string(),
            DeclarationKind::Class,
            Location::new(path, 42, 1, 0, 10),
            Language::Kotlin,
        );
        let dc = DeadCode::new(decl, DeadCodeIssue::Unreferenced);
        let fp = fingerprint_of(&dc, Path::new("/project"));
        assert_eq!(
            fp,
            (
                "src/Zombie.kt".to_string(),
                "Zombie".to_string(),
                "DC001".to_string()
            )
        );
    }

    #[test]
    fn an_unresolvable_ref_is_an_error() {
        let temp = tempfile::tempdir().unwrap();
        let err = reference_fingerprints(temp.path(), "no-such-ref");
        assert!(err.is_err());
    }
}
