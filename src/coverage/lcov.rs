// LCOV coverage parser
//
// LCOV is a generic coverage format used by many tools including:
// - gcov (C/C++)
// - Istanbul (JavaScript/TypeScript)
// - Coverage.py (Python)
// - PHPUnit
// https://ltp.sourceforge.net/coverage/lcov/geninfo.1.php

#![allow(dead_code)] // Builder pattern method for future configuration

use super::{CoverageData, CoverageParser, FileCoverage};
use miette::{IntoDiagnostic, Result};
use std::path::{Path, PathBuf};

/// Parser for LCOV coverage reports
pub struct LcovParser {
    /// Source directories to help resolve file paths
    source_roots: Vec<PathBuf>,
}

impl LcovParser {
    pub fn new() -> Self {
        Self {
            source_roots: Vec::new(),
        }
    }

    pub fn with_source_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.source_roots = roots;
        self
    }

    /// Parse LCOV format content
    fn parse_lcov(&self, content: &str) -> Result<CoverageData> {
        let mut coverage_data = CoverageData::new();
        let mut current_file_coverage: Option<FileCoverage> = None;
        let mut current_functions: Vec<(String, u32)> = Vec::new(); // (name, line)

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(path) = line.strip_prefix("SF:") {
                // Start of a new source file
                if let Some(fc) = current_file_coverage.take() {
                    coverage_data.add_file_coverage(fc);
                }
                let file_path = self.resolve_source_file(path.trim());
                current_file_coverage = Some(FileCoverage::new(file_path));
                current_functions.clear();
            } else if let Some(fn_data) = line.strip_prefix("FN:") {
                // Function definition: FN:line_number,function_name
                if let Some(comma_pos) = fn_data.find(',') {
                    let line_nr: u32 = fn_data[..comma_pos].parse().unwrap_or(0);
                    let fn_name = fn_data[comma_pos + 1..].to_string();
                    if line_nr > 0 {
                        current_functions.push((fn_name, line_nr));
                    }
                }
            } else if let Some(fnda_data) = line.strip_prefix("FNDA:") {
                // Function hit data: FNDA:hit_count,function_name
                if let Some(comma_pos) = fnda_data.find(',') {
                    let hit_count: u32 = fnda_data[..comma_pos].parse().unwrap_or(0);
                    let fn_name = &fnda_data[comma_pos + 1..];

                    if let Some(ref mut fc) = current_file_coverage {
                        if hit_count > 0 {
                            fc.covered_methods.insert(fn_name.to_string());
                            fc.uncovered_methods.remove(fn_name);
                            coverage_data.covered_methods.insert(fn_name.to_string());
                            coverage_data.uncovered_methods.remove(fn_name);
                        } else {
                            if !fc.covered_methods.contains(fn_name) {
                                fc.uncovered_methods.insert(fn_name.to_string());
                            }
                            if !coverage_data.covered_methods.contains(fn_name) {
                                coverage_data.uncovered_methods.insert(fn_name.to_string());
                            }
                        }
                    }
                }
            } else if let Some(da_data) = line.strip_prefix("DA:") {
                // Line data: DA:line_number,hit_count[,checksum]
                let parts: Vec<&str> = da_data.split(',').collect();
                if parts.len() >= 2 {
                    let line_nr: u32 = parts[0].parse().unwrap_or(0);
                    let hit_count: u32 = parts[1].parse().unwrap_or(0);

                    if line_nr > 0 {
                        if let Some(ref mut fc) = current_file_coverage {
                            if hit_count > 0 {
                                fc.covered_lines.insert(line_nr);
                                fc.uncovered_lines.remove(&line_nr);
                            } else if !fc.covered_lines.contains(&line_nr) {
                                fc.uncovered_lines.insert(line_nr);
                            }
                        }
                    }
                }
            } else if let Some(brda_data) = line.strip_prefix("BRDA:") {
                // Branch data: BRDA:line_number,block_number,branch_number,hit_count
                let parts: Vec<&str> = brda_data.split(',').collect();
                if parts.len() >= 4 {
                    let line_nr: u32 = parts[0].parse().unwrap_or(0);
                    let hit_count: u32 = if parts[3] == "-" {
                        0
                    } else {
                        parts[3].parse().unwrap_or(0)
                    };

                    if line_nr > 0 {
                        if let Some(ref mut fc) = current_file_coverage {
                            let entry = fc.branch_coverage.entry(line_nr).or_insert((0, 0));
                            entry.1 += 1; // total branches
                            if hit_count > 0 {
                                entry.0 += 1; // covered branches
                            }
                        }
                    }
                }
            } else if line == "end_of_record" {
                // End of current file record
                if let Some(fc) = current_file_coverage.take() {
                    coverage_data.add_file_coverage(fc);
                }
                current_functions.clear();
            }
            // Skip other lines (LF, LH, BRF, BRH, FNF, FNH, TN)
        }

        // Handle case where file doesn't end with end_of_record
        if let Some(fc) = current_file_coverage.take() {
            coverage_data.add_file_coverage(fc);
        }

        for root in &self.source_roots {
            coverage_data.add_source_root(root.clone());
        }

        Ok(coverage_data)
    }

    fn resolve_source_file(&self, path: &str) -> PathBuf {
        let path = PathBuf::from(path);

        // If it's already absolute and exists, use it
        if path.is_absolute() && path.exists() {
            return path;
        }

        // Try to find in source roots
        for root in &self.source_roots {
            let full_path = root.join(&path);
            if full_path.exists() {
                return full_path;
            }
        }

        // Return as-is
        path
    }
}

impl Default for LcovParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CoverageParser for LcovParser {
    fn parse(&self, path: &Path) -> Result<CoverageData> {
        let content = std::fs::read_to_string(path).into_diagnostic()?;
        self.parse_lcov(&content)
    }

    fn can_parse(&self, path: &Path) -> bool {
        // Check for common LCOV file extensions
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(extension, "lcov" | "info") {
            return true;
        }

        // Check file name patterns
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.contains("lcov") || name.contains("coverage.info") {
                return true;
            }
        }

        // Check content for LCOV markers
        if let Ok(content) = std::fs::read_to_string(path) {
            // LCOV files start with TN: or SF:
            let first_line = content.lines().next().unwrap_or("");
            return first_line.starts_with("TN:") || first_line.starts_with("SF:");
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lcov() {
        let lcov = r#"TN:
SF:/path/to/source/MyFile.kt
FN:10,myFunction
FN:20,anotherFunction
FNDA:5,myFunction
FNDA:0,anotherFunction
DA:10,5
DA:11,5
DA:12,0
DA:20,0
DA:21,0
BRDA:10,0,0,5
BRDA:10,0,1,0
LF:5
LH:2
FNF:2
FNH:1
end_of_record
"#;

        let parser = LcovParser::new();
        let data = parser.parse_lcov(lcov).unwrap();

        // Check method coverage
        assert!(data.covered_methods.contains("myFunction"));
        assert!(data.uncovered_methods.contains("anotherFunction"));

        // Check line coverage
        let file_path = Path::new("/path/to/source/MyFile.kt");
        assert_eq!(data.is_line_covered(file_path, 10), Some(true));
        assert_eq!(data.is_line_covered(file_path, 12), Some(false));
    }
}
