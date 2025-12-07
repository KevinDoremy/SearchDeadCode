// JaCoCo XML coverage parser
//
// JaCoCo is the standard code coverage library for Java/Android projects.
// XML format: https://www.jacoco.org/jacoco/trunk/doc/

#![allow(dead_code)] // Builder pattern method for future configuration

use super::{CoverageData, CoverageParser, FileCoverage};
use miette::{IntoDiagnostic, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::{Path, PathBuf};

/// Parser for JaCoCo XML coverage reports
pub struct JacocoParser {
    /// Source directories to help resolve file paths
    source_roots: Vec<PathBuf>,
}

impl JacocoParser {
    pub fn new() -> Self {
        Self {
            source_roots: Vec::new(),
        }
    }

    pub fn with_source_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.source_roots = roots;
        self
    }

    /// Parse the JaCoCo XML report
    fn parse_xml(&self, content: &str) -> Result<CoverageData> {
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);

        let mut coverage_data = CoverageData::new();
        let mut current_package = String::new();
        let mut current_class = String::new();
        let mut current_source_file = String::new();
        let mut current_file_coverage: Option<FileCoverage> = None;

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    match e.name().as_ref() {
                        b"package" => {
                            // Extract package name
                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                if attr.key.as_ref() == b"name" {
                                    current_package =
                                        String::from_utf8_lossy(&attr.value).replace('/', ".");
                                }
                            }
                        }
                        b"class" => {
                            // Extract class name and source file
                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                match attr.key.as_ref() {
                                    b"name" => {
                                        let name =
                                            String::from_utf8_lossy(&attr.value).replace('/', ".");
                                        current_class = name;
                                    }
                                    b"sourcefilename" => {
                                        current_source_file =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    _ => {}
                                }
                            }

                            // Create file coverage entry if we have a source file
                            if !current_source_file.is_empty() {
                                let file_path = self
                                    .resolve_source_file(&current_package, &current_source_file);
                                current_file_coverage = Some(FileCoverage::new(file_path));
                            }
                        }
                        b"method" => {
                            // Extract method coverage
                            let mut method_name = String::new();

                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                if attr.key.as_ref() == b"name" {
                                    method_name = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }

                            if !method_name.is_empty() {
                                let full_method = format!("{}.{}", current_class, method_name);
                                // We'll update covered/uncovered status from counter elements
                                if let Some(ref mut fc) = current_file_coverage {
                                    fc.uncovered_methods.insert(full_method.clone());
                                }
                                coverage_data.uncovered_methods.insert(full_method);
                            }
                        }
                        b"counter" => {
                            // Counter elements contain coverage metrics
                            let mut counter_type = String::new();
                            let mut covered = 0u32;
                            let mut missed = 0u32;

                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                match attr.key.as_ref() {
                                    b"type" => {
                                        counter_type =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    b"covered" => {
                                        covered = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    b"missed" => {
                                        missed = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    _ => {}
                                }
                            }

                            // Update coverage based on counter type
                            match counter_type.as_str() {
                                "METHOD" => {
                                    if covered > 0 && !current_class.is_empty() {
                                        // Class has at least one covered method
                                        coverage_data.covered_classes.insert(current_class.clone());
                                        coverage_data.uncovered_classes.remove(&current_class);

                                        if let Some(ref mut fc) = current_file_coverage {
                                            fc.covered_classes.insert(current_class.clone());
                                            fc.uncovered_classes.remove(&current_class);
                                        }
                                    } else if missed > 0 && covered == 0 {
                                        if !coverage_data.covered_classes.contains(&current_class) {
                                            coverage_data
                                                .uncovered_classes
                                                .insert(current_class.clone());
                                        }
                                        if let Some(ref mut fc) = current_file_coverage {
                                            if !fc.covered_classes.contains(&current_class) {
                                                fc.uncovered_classes.insert(current_class.clone());
                                            }
                                        }
                                    }
                                }
                                "LINE" => {
                                    // Line coverage at class level
                                    // We don't have individual line numbers here,
                                    // just counts - actual line info comes from sourcefile
                                    let _ = &current_file_coverage;
                                }
                                "CLASS" => {
                                    if covered > 0 {
                                        coverage_data.covered_classes.insert(current_class.clone());
                                        coverage_data.uncovered_classes.remove(&current_class);
                                    }
                                }
                                _ => {}
                            }
                        }
                        b"sourcefile" => {
                            // Source file element contains line-level coverage
                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                if attr.key.as_ref() == b"name" {
                                    current_source_file =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                    let file_path = self.resolve_source_file(
                                        &current_package,
                                        &current_source_file,
                                    );
                                    current_file_coverage = Some(FileCoverage::new(file_path));
                                }
                            }
                        }
                        b"line" => {
                            // Individual line coverage
                            let mut line_nr = 0u32;
                            let mut covered_instructions = 0u32;
                            let mut missed_instructions = 0u32;
                            let mut covered_branches = 0u32;
                            let mut missed_branches = 0u32;

                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                match attr.key.as_ref() {
                                    b"nr" => {
                                        line_nr = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    b"ci" => {
                                        covered_instructions = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    b"mi" => {
                                        missed_instructions = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    b"cb" => {
                                        covered_branches = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    b"mb" => {
                                        missed_branches = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    _ => {}
                                }
                            }

                            if line_nr > 0 {
                                if let Some(ref mut fc) = current_file_coverage {
                                    if covered_instructions > 0 {
                                        fc.covered_lines.insert(line_nr);
                                        fc.uncovered_lines.remove(&line_nr);
                                    } else if missed_instructions > 0
                                        && !fc.covered_lines.contains(&line_nr)
                                    {
                                        fc.uncovered_lines.insert(line_nr);
                                    }

                                    // Branch coverage
                                    let total_branches = covered_branches + missed_branches;
                                    if total_branches > 0 {
                                        fc.branch_coverage
                                            .insert(line_nr, (covered_branches, total_branches));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.name().as_ref() {
                        b"class" => {
                            // Finalize class coverage
                            if let Some(fc) = current_file_coverage.take() {
                                coverage_data.add_file_coverage(fc);
                            }
                            current_class.clear();
                        }
                        b"sourcefile" => {
                            // Finalize source file coverage
                            if let Some(fc) = current_file_coverage.take() {
                                coverage_data.add_file_coverage(fc);
                            }
                        }
                        b"package" => {
                            current_package.clear();
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(miette::miette!("Error parsing JaCoCo XML: {}", e));
                }
                _ => {}
            }
            buf.clear();
        }

        // Add source roots
        for root in &self.source_roots {
            coverage_data.add_source_root(root.clone());
        }

        Ok(coverage_data)
    }

    /// Resolve source file path from package and filename
    fn resolve_source_file(&self, package: &str, filename: &str) -> PathBuf {
        // Convert package to path: com.example -> com/example
        let package_path = package.replace('.', "/");

        // Try to find the file in source roots
        for root in &self.source_roots {
            let full_path = root.join(&package_path).join(filename);
            if full_path.exists() {
                return full_path;
            }
        }

        // Return relative path if not found
        PathBuf::from(package_path).join(filename)
    }
}

impl Default for JacocoParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CoverageParser for JacocoParser {
    fn parse(&self, path: &Path) -> Result<CoverageData> {
        let content = std::fs::read_to_string(path).into_diagnostic()?;
        self.parse_xml(&content)
    }

    fn can_parse(&self, path: &Path) -> bool {
        // Check file extension
        if path.extension().is_none_or(|e| e != "xml") {
            return false;
        }

        // Check for JaCoCo-specific content
        if let Ok(content) = std::fs::read_to_string(path) {
            // JaCoCo reports have a <report> root element with name attribute
            // or contain "jacoco" in DOCTYPE
            return content.contains("<report ")
                || content.contains("<!DOCTYPE report")
                || content.contains("jacoco");
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_jacoco() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE report PUBLIC "-//JACOCO//DTD Report 1.1//EN" "report.dtd">
<report name="test">
    <package name="com/example">
        <class name="com/example/MyClass" sourcefilename="MyClass.kt">
            <method name="myMethod" desc="()V" line="10">
                <counter type="INSTRUCTION" missed="0" covered="5"/>
                <counter type="LINE" missed="0" covered="2"/>
                <counter type="METHOD" missed="0" covered="1"/>
            </method>
            <counter type="CLASS" missed="0" covered="1"/>
        </class>
        <sourcefile name="MyClass.kt">
            <line nr="10" mi="0" ci="2" mb="0" cb="0"/>
            <line nr="11" mi="0" ci="3" mb="0" cb="0"/>
            <line nr="15" mi="2" ci="0" mb="0" cb="0"/>
        </sourcefile>
    </package>
</report>"#;

        let parser = JacocoParser::new();
        let data = parser.parse_xml(xml).unwrap();

        assert!(data.covered_classes.contains("com.example.MyClass"));
        assert!(data.is_line_covered(Path::new("com/example/MyClass.kt"), 10) == Some(true));
        assert!(data.is_line_covered(Path::new("com/example/MyClass.kt"), 15) == Some(false));
    }
}
