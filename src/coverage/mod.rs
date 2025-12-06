// Coverage module - Parse runtime coverage data for hybrid static+dynamic analysis
//
// Supports:
// - JaCoCo XML format (Android/Java standard)
// - Kover XML format (Kotlin coverage)
// - LCOV format (generic)

#![allow(dead_code)] // Coverage API methods reserved for future use

mod jacoco;
mod kover;
mod lcov;

pub use jacoco::JacocoParser;
pub use kover::KoverParser;
pub use lcov::LcovParser;

use miette::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Represents coverage data for a single source file
#[derive(Debug, Clone, Default)]
pub struct FileCoverage {
    /// File path (relative or absolute)
    pub file_path: PathBuf,

    /// Lines that were executed at least once
    pub covered_lines: HashSet<u32>,

    /// Lines that were never executed
    pub uncovered_lines: HashSet<u32>,

    /// Methods/functions that were executed
    pub covered_methods: HashSet<String>,

    /// Methods/functions that were never executed
    pub uncovered_methods: HashSet<String>,

    /// Classes that were loaded/instantiated
    pub covered_classes: HashSet<String>,

    /// Classes that were never loaded
    pub uncovered_classes: HashSet<String>,

    /// Branch coverage (line -> (covered_branches, total_branches))
    pub branch_coverage: HashMap<u32, (u32, u32)>,
}

impl FileCoverage {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            ..Default::default()
        }
    }

    /// Check if a specific line was covered
    pub fn is_line_covered(&self, line: u32) -> Option<bool> {
        if self.covered_lines.contains(&line) {
            Some(true)
        } else if self.uncovered_lines.contains(&line) {
            Some(false)
        } else {
            None // Line not tracked (e.g., comment, blank)
        }
    }

    /// Check if a method was covered
    pub fn is_method_covered(&self, method_name: &str) -> Option<bool> {
        if self.covered_methods.contains(method_name) {
            Some(true)
        } else if self.uncovered_methods.contains(method_name) {
            Some(false)
        } else {
            None
        }
    }

    /// Check if a class was covered
    pub fn is_class_covered(&self, class_name: &str) -> Option<bool> {
        if self.covered_classes.contains(class_name) {
            Some(true)
        } else if self.uncovered_classes.contains(class_name) {
            Some(false)
        } else {
            None
        }
    }

    /// Get line coverage percentage
    pub fn line_coverage_percent(&self) -> f64 {
        let total = self.covered_lines.len() + self.uncovered_lines.len();
        if total == 0 {
            return 0.0;
        }
        (self.covered_lines.len() as f64 / total as f64) * 100.0
    }

    /// Get method coverage percentage
    pub fn method_coverage_percent(&self) -> f64 {
        let total = self.covered_methods.len() + self.uncovered_methods.len();
        if total == 0 {
            return 0.0;
        }
        (self.covered_methods.len() as f64 / total as f64) * 100.0
    }
}

/// Aggregated coverage data from all sources
#[derive(Debug, Clone, Default)]
pub struct CoverageData {
    /// Coverage data indexed by file path
    pub files: HashMap<PathBuf, FileCoverage>,

    /// Global set of covered classes (fully qualified names)
    pub covered_classes: HashSet<String>,

    /// Global set of uncovered classes
    pub uncovered_classes: HashSet<String>,

    /// Global set of covered methods (class.method format)
    pub covered_methods: HashSet<String>,

    /// Global set of uncovered methods
    pub uncovered_methods: HashSet<String>,

    /// Source directories used to resolve relative paths
    pub source_roots: Vec<PathBuf>,
}

impl CoverageData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a source root for resolving file paths
    pub fn add_source_root(&mut self, root: PathBuf) {
        self.source_roots.push(root);
    }

    /// Add coverage for a file
    pub fn add_file_coverage(&mut self, coverage: FileCoverage) {
        // Update global class coverage
        for class in &coverage.covered_classes {
            self.covered_classes.insert(class.clone());
            self.uncovered_classes.remove(class);
        }
        for class in &coverage.uncovered_classes {
            if !self.covered_classes.contains(class) {
                self.uncovered_classes.insert(class.clone());
            }
        }

        // Update global method coverage
        for method in &coverage.covered_methods {
            self.covered_methods.insert(method.clone());
            self.uncovered_methods.remove(method);
        }
        for method in &coverage.uncovered_methods {
            if !self.covered_methods.contains(method) {
                self.uncovered_methods.insert(method.clone());
            }
        }

        self.files.insert(coverage.file_path.clone(), coverage);
    }

    /// Merge coverage data from another source
    pub fn merge(&mut self, other: CoverageData) {
        for (path, coverage) in other.files {
            if let Some(existing) = self.files.get_mut(&path) {
                // Merge coverage - if covered in ANY run, it's covered
                existing.covered_lines.extend(coverage.covered_lines);
                existing.covered_methods.extend(coverage.covered_methods);
                existing.covered_classes.extend(coverage.covered_classes);

                // Remove from uncovered if now covered
                for line in &existing.covered_lines {
                    existing.uncovered_lines.remove(line);
                }
                for method in &existing.covered_methods {
                    existing.uncovered_methods.remove(method);
                }
                for class in &existing.covered_classes {
                    existing.uncovered_classes.remove(class);
                }
            } else {
                self.add_file_coverage(coverage);
            }
        }

        self.source_roots.extend(other.source_roots);
    }

    /// Check if a class was covered at runtime
    pub fn is_class_covered(&self, fully_qualified_name: &str) -> Option<bool> {
        if self.covered_classes.contains(fully_qualified_name) {
            Some(true)
        } else if self.uncovered_classes.contains(fully_qualified_name) {
            Some(false)
        } else {
            None // Not in coverage data
        }
    }

    /// Check if a method was covered at runtime
    pub fn is_method_covered(&self, class_name: &str, method_name: &str) -> Option<bool> {
        let full_name = format!("{}.{}", class_name, method_name);
        if self.covered_methods.contains(&full_name) {
            Some(true)
        } else if self.uncovered_methods.contains(&full_name) {
            Some(false)
        } else {
            None
        }
    }

    /// Check if a line in a file was covered
    pub fn is_line_covered(&self, file: &Path, line: u32) -> Option<bool> {
        // Try exact match first
        if let Some(coverage) = self.files.get(file) {
            return coverage.is_line_covered(line);
        }

        // Try matching by filename only
        if let Some(file_name) = file.file_name() {
            for (path, coverage) in &self.files {
                if path.file_name() == Some(file_name) {
                    if let Some(result) = coverage.is_line_covered(line) {
                        return Some(result);
                    }
                }
            }
        }

        None
    }

    /// Get file coverage for a specific file
    pub fn get_file_coverage(&self, file: &Path) -> Option<&FileCoverage> {
        self.files.get(file).or_else(|| {
            // Try matching by filename
            file.file_name().and_then(|file_name| {
                self.files
                    .iter()
                    .find(|(path, _)| path.file_name() == Some(file_name))
                    .map(|(_, coverage)| coverage)
            })
        })
    }

    /// Get overall statistics
    pub fn stats(&self) -> CoverageStats {
        let total_lines: usize = self
            .files
            .values()
            .map(|f| f.covered_lines.len() + f.uncovered_lines.len())
            .sum();
        let covered_lines: usize = self.files.values().map(|f| f.covered_lines.len()).sum();

        CoverageStats {
            total_files: self.files.len(),
            total_classes: self.covered_classes.len() + self.uncovered_classes.len(),
            covered_classes: self.covered_classes.len(),
            total_methods: self.covered_methods.len() + self.uncovered_methods.len(),
            covered_methods: self.covered_methods.len(),
            total_lines,
            covered_lines,
        }
    }
}

/// Summary statistics for coverage data
#[derive(Debug, Clone)]
pub struct CoverageStats {
    pub total_files: usize,
    pub total_classes: usize,
    pub covered_classes: usize,
    pub total_methods: usize,
    pub covered_methods: usize,
    pub total_lines: usize,
    pub covered_lines: usize,
}

impl CoverageStats {
    pub fn class_coverage_percent(&self) -> f64 {
        if self.total_classes == 0 {
            return 0.0;
        }
        (self.covered_classes as f64 / self.total_classes as f64) * 100.0
    }

    pub fn method_coverage_percent(&self) -> f64 {
        if self.total_methods == 0 {
            return 0.0;
        }
        (self.covered_methods as f64 / self.total_methods as f64) * 100.0
    }

    pub fn line_coverage_percent(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        (self.covered_lines as f64 / self.total_lines as f64) * 100.0
    }
}

/// Trait for coverage file parsers
pub trait CoverageParser {
    /// Parse coverage data from a file
    fn parse(&self, path: &Path) -> Result<CoverageData>;

    /// Check if this parser can handle the given file
    fn can_parse(&self, path: &Path) -> bool;
}

/// Auto-detect coverage format and parse
pub fn parse_coverage_file(path: &Path) -> Result<CoverageData> {
    let jacoco = JacocoParser::new();
    let kover = KoverParser::new();
    let lcov = LcovParser::new();

    if jacoco.can_parse(path) {
        return jacoco.parse(path);
    }
    if kover.can_parse(path) {
        return kover.parse(path);
    }
    if lcov.can_parse(path) {
        return lcov.parse(path);
    }

    // Default to trying JaCoCo for XML files
    if path.extension().is_some_and(|e| e == "xml") {
        return jacoco.parse(path);
    }

    miette::bail!("Unknown coverage file format: {}", path.display())
}

/// Parse multiple coverage files and merge results
pub fn parse_coverage_files(paths: &[PathBuf]) -> Result<CoverageData> {
    let mut merged = CoverageData::new();

    for path in paths {
        let data = parse_coverage_file(path)?;
        merged.merge(data);
    }

    Ok(merged)
}
