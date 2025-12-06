//! Baseline support for SearchDeadCode
//!
//! This module provides functionality for creating and using baselines
//! to ignore existing dead code issues and only report new ones.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use thiserror::Error;

use crate::analysis::DeadCode;

/// Baseline errors
#[derive(Error, Debug)]
pub enum BaselineError {
    #[error("Failed to read baseline file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse baseline: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("Baseline version mismatch")]
    VersionMismatch,
}

/// Current baseline format version
const BASELINE_VERSION: u32 = 1;

/// A fingerprint for a dead code issue that can be matched across runs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IssueFingerprint {
    /// Relative file path
    pub file: String,
    /// Declaration name
    pub name: String,
    /// Declaration kind
    pub kind: String,
    /// Line number (approximate, may shift slightly)
    pub line: usize,
    /// Fully qualified name if available
    pub fqn: Option<String>,
}

impl IssueFingerprint {
    /// Create a fingerprint from a dead code issue
    pub fn from_dead_code(dc: &DeadCode, project_root: &Path) -> Self {
        let file = dc
            .declaration
            .location
            .file
            .strip_prefix(project_root)
            .unwrap_or(&dc.declaration.location.file)
            .to_string_lossy()
            .to_string();

        Self {
            file,
            name: dc.declaration.name.clone(),
            kind: dc.declaration.kind.display_name().to_string(),
            line: dc.declaration.location.line,
            fqn: dc.declaration.fully_qualified_name.clone(),
        }
    }

    /// Check if this fingerprint matches a dead code issue (with some tolerance)
    pub fn matches(&self, dc: &DeadCode, project_root: &Path) -> bool {
        let dc_file = dc
            .declaration
            .location
            .file
            .strip_prefix(project_root)
            .unwrap_or(&dc.declaration.location.file)
            .to_string_lossy()
            .to_string();

        // Must match file, name, and kind exactly
        if self.file != dc_file || self.name != dc.declaration.name {
            return false;
        }

        let dc_kind = dc.declaration.kind.display_name();
        if self.kind != dc_kind {
            return false;
        }

        // If FQN is available, use it for more precise matching
        if self.fqn.is_some() && dc.declaration.fully_qualified_name.is_some() {
            return self.fqn == dc.declaration.fully_qualified_name;
        }

        // Allow line number to drift by up to 10 lines
        let line_diff = (self.line as i64 - dc.declaration.location.line as i64).abs();
        line_diff <= 10
    }
}

/// A baseline containing known dead code issues to ignore
#[derive(Debug, Serialize, Deserialize)]
pub struct Baseline {
    /// Baseline format version
    pub version: u32,
    /// When the baseline was created
    pub created_at: String,
    /// Known issues to ignore
    pub issues: Vec<IssueFingerprint>,
    /// Total count at baseline time
    pub total_at_baseline: usize,
}

impl Baseline {
    /// Create a new baseline from dead code findings
    pub fn from_findings(findings: &[DeadCode], project_root: &Path) -> Self {
        let issues: Vec<IssueFingerprint> = findings
            .iter()
            .map(|dc| IssueFingerprint::from_dead_code(dc, project_root))
            .collect();

        Self {
            version: BASELINE_VERSION,
            created_at: chrono_lite_now(),
            issues,
            total_at_baseline: findings.len(),
        }
    }

    /// Load a baseline from a file
    pub fn load(path: &Path) -> Result<Self, BaselineError> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let baseline: Self = serde_json::from_reader(reader)?;

        if baseline.version != BASELINE_VERSION {
            return Err(BaselineError::VersionMismatch);
        }

        Ok(baseline)
    }

    /// Save baseline to a file
    pub fn save(&self, path: &Path) -> Result<(), BaselineError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = fs::File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }

    /// Filter out findings that are in the baseline
    pub fn filter_new<'a>(
        &self,
        findings: &'a [DeadCode],
        project_root: &Path,
    ) -> Vec<&'a DeadCode> {
        findings
            .iter()
            .filter(|dc| !self.is_baselined(dc, project_root))
            .collect()
    }

    /// Check if a finding is in the baseline
    pub fn is_baselined(&self, dc: &DeadCode, project_root: &Path) -> bool {
        self.issues.iter().any(|fp| fp.matches(dc, project_root))
    }

    /// Get statistics about baseline coverage
    pub fn stats(&self, findings: &[DeadCode], project_root: &Path) -> BaselineStats {
        let mut baselined = 0;
        let mut new = 0;

        for dc in findings {
            if self.is_baselined(dc, project_root) {
                baselined += 1;
            } else {
                new += 1;
            }
        }

        BaselineStats {
            total_in_baseline: self.issues.len(),
            baselined_found: baselined,
            new_issues: new,
        }
    }
}

/// Statistics about baseline comparison
#[derive(Debug, Clone)]
pub struct BaselineStats {
    /// Total issues recorded in baseline
    pub total_in_baseline: usize,
    /// Number of current findings that match baseline
    pub baselined_found: usize,
    /// Number of new issues not in baseline
    pub new_issues: usize,
}

impl std::fmt::Display for BaselineStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} new issues ({} baselined, {} in baseline file)",
            self.new_issues, self.baselined_found, self.total_in_baseline
        )
    }
}

/// Simple datetime string without chrono dependency
fn chrono_lite_now() -> String {
    use std::time::SystemTime;

    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();
    // Simple ISO-8601 like format
    format!("{}", secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::DeadCodeIssue;
    use crate::graph::{Declaration, DeclarationId, DeclarationKind, Language, Location};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_dead_code(name: &str, file: &str, line: usize) -> DeadCode {
        let path = PathBuf::from(file);
        let decl = Declaration::new(
            DeclarationId::new(path.clone(), 0, 100),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, 0, 100),
            Language::Kotlin,
        );
        DeadCode::new(decl, DeadCodeIssue::Unreferenced)
    }

    #[test]
    fn test_fingerprint_matching() {
        let project_root = PathBuf::from("/project");
        let dc = make_dead_code("TestClass", "/project/src/test.kt", 10);
        let fp = IssueFingerprint::from_dead_code(&dc, &project_root);

        assert!(fp.matches(&dc, &project_root));

        // Line drift within tolerance
        let dc2 = make_dead_code("TestClass", "/project/src/test.kt", 15);
        assert!(fp.matches(&dc2, &project_root));

        // Line drift outside tolerance
        let dc3 = make_dead_code("TestClass", "/project/src/test.kt", 50);
        assert!(!fp.matches(&dc3, &project_root));

        // Different name
        let dc4 = make_dead_code("OtherClass", "/project/src/test.kt", 10);
        assert!(!fp.matches(&dc4, &project_root));
    }

    #[test]
    fn test_baseline_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let baseline_path = temp_dir.path().join("baseline.json");
        let project_root = PathBuf::from("/project");

        let findings = vec![
            make_dead_code("ClassA", "/project/src/a.kt", 10),
            make_dead_code("ClassB", "/project/src/b.kt", 20),
        ];

        let baseline = Baseline::from_findings(&findings, &project_root);
        baseline.save(&baseline_path).unwrap();

        let loaded = Baseline::load(&baseline_path).unwrap();
        assert_eq!(loaded.issues.len(), 2);
    }

    #[test]
    fn test_baseline_filter() {
        let project_root = PathBuf::from("/project");
        let findings = vec![
            make_dead_code("ClassA", "/project/src/a.kt", 10),
            make_dead_code("ClassB", "/project/src/b.kt", 20),
        ];

        let baseline = Baseline::from_findings(&findings[..1], &project_root);

        let new_findings = baseline.filter_new(&findings, &project_root);
        assert_eq!(new_findings.len(), 1);
        assert_eq!(new_findings[0].declaration.name, "ClassB");
    }
}
