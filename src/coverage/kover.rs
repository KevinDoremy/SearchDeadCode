// Kover XML coverage parser
//
// Kover is JetBrains' code coverage tool for Kotlin.
// It can output in JaCoCo-compatible XML format or its own format.
// https://github.com/Kotlin/kotlinx-kover

#![allow(dead_code)] // Builder pattern method for future configuration

use super::{CoverageData, CoverageParser, FileCoverage};
use miette::{IntoDiagnostic, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::{Path, PathBuf};

/// Parser for Kover XML coverage reports
pub struct KoverParser {
    /// Source directories to help resolve file paths
    source_roots: Vec<PathBuf>,
}

impl KoverParser {
    pub fn new() -> Self {
        Self {
            source_roots: Vec::new(),
        }
    }

    pub fn with_source_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.source_roots = roots;
        self
    }

    /// Parse the Kover XML report
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
                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                if attr.key.as_ref() == b"name" {
                                    current_package =
                                        String::from_utf8_lossy(&attr.value).replace('/', ".");
                                }
                            }
                        }
                        b"class" => {
                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                match attr.key.as_ref() {
                                    b"name" => {
                                        let name =
                                            String::from_utf8_lossy(&attr.value).replace('/', ".");
                                        // Kover may include inner class notation with $
                                        current_class = name.replace('$', ".");
                                    }
                                    b"sourcefilename" | b"sourceFileName" => {
                                        current_source_file =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    _ => {}
                                }
                            }

                            if !current_source_file.is_empty() {
                                let file_path = self
                                    .resolve_source_file(&current_package, &current_source_file);
                                current_file_coverage = Some(FileCoverage::new(file_path));
                            }
                        }
                        b"method" => {
                            let mut method_name = String::new();

                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                if attr.key.as_ref() == b"name" {
                                    method_name = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }

                            if !method_name.is_empty() && !current_class.is_empty() {
                                let full_method = format!("{}.{}", current_class, method_name);
                                // Initially mark as uncovered, update from counters
                                if let Some(ref mut fc) = current_file_coverage {
                                    fc.uncovered_methods.insert(full_method.clone());
                                }
                                coverage_data.uncovered_methods.insert(full_method);
                            }
                        }
                        b"counter" => {
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

                            match counter_type.as_str() {
                                "METHOD" | "FUNCTION" => {
                                    if covered > 0 && !current_class.is_empty() {
                                        coverage_data.covered_classes.insert(current_class.clone());
                                        coverage_data.uncovered_classes.remove(&current_class);

                                        if let Some(ref mut fc) = current_file_coverage {
                                            fc.covered_classes.insert(current_class.clone());
                                            fc.uncovered_classes.remove(&current_class);
                                        }
                                    } else if missed > 0
                                        && covered == 0
                                        && !coverage_data.covered_classes.contains(&current_class)
                                    {
                                        coverage_data
                                            .uncovered_classes
                                            .insert(current_class.clone());
                                    }
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
                        b"sourcefile" | b"sourceFile" => {
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
                            let mut line_nr = 0u32;
                            let mut covered_instructions = 0u32;
                            let mut missed_instructions = 0u32;
                            let mut covered_branches = 0u32;
                            let mut missed_branches = 0u32;

                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                match attr.key.as_ref() {
                                    b"nr" | b"number" => {
                                        line_nr = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    b"ci" | b"coveredInstructions" => {
                                        covered_instructions = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    b"mi" | b"missedInstructions" => {
                                        missed_instructions = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    b"cb" | b"coveredBranches" => {
                                        covered_branches = String::from_utf8_lossy(&attr.value)
                                            .parse()
                                            .unwrap_or(0);
                                    }
                                    b"mb" | b"missedBranches" => {
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
                Ok(Event::End(ref e)) => match e.name().as_ref() {
                    b"class" => {
                        if let Some(fc) = current_file_coverage.take() {
                            coverage_data.add_file_coverage(fc);
                        }
                        current_class.clear();
                    }
                    b"sourcefile" | b"sourceFile" => {
                        if let Some(fc) = current_file_coverage.take() {
                            coverage_data.add_file_coverage(fc);
                        }
                    }
                    b"package" => {
                        current_package.clear();
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(miette::miette!("Error parsing Kover XML: {}", e));
                }
                _ => {}
            }
            buf.clear();
        }

        for root in &self.source_roots {
            coverage_data.add_source_root(root.clone());
        }

        Ok(coverage_data)
    }

    fn resolve_source_file(&self, package: &str, filename: &str) -> PathBuf {
        let package_path = package.replace('.', "/");

        for root in &self.source_roots {
            let full_path = root.join(&package_path).join(filename);
            if full_path.exists() {
                return full_path;
            }
        }

        PathBuf::from(package_path).join(filename)
    }
}

impl Default for KoverParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CoverageParser for KoverParser {
    fn parse(&self, path: &Path) -> Result<CoverageData> {
        let content = std::fs::read_to_string(path).into_diagnostic()?;
        self.parse_xml(&content)
    }

    fn can_parse(&self, path: &Path) -> bool {
        if path.extension().map_or(true, |e| e != "xml") {
            return false;
        }

        if let Ok(content) = std::fs::read_to_string(path) {
            // Kover reports contain "kover" in various places
            return content.contains("kover") || content.contains("Kover");
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kover_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<report name="Kover Report">
    <package name="com/example">
        <class name="com/example/KotlinClass" sourcefilename="KotlinClass.kt">
            <method name="doSomething" desc="()V">
                <counter type="INSTRUCTION" missed="0" covered="10"/>
                <counter type="METHOD" missed="0" covered="1"/>
            </method>
            <counter type="CLASS" missed="0" covered="1"/>
        </class>
        <sourcefile name="KotlinClass.kt">
            <line nr="5" mi="0" ci="3" mb="0" cb="0"/>
            <line nr="6" mi="0" ci="4" mb="0" cb="0"/>
            <line nr="10" mi="5" ci="0" mb="0" cb="0"/>
        </sourcefile>
    </package>
</report>"#;

        let parser = KoverParser::new();
        let data = parser.parse_xml(xml).unwrap();

        assert!(data.covered_classes.contains("com.example.KotlinClass"));
        assert!(data.is_line_covered(Path::new("com/example/KotlinClass.kt"), 5) == Some(true));
        assert!(data.is_line_covered(Path::new("com/example/KotlinClass.kt"), 10) == Some(false));
    }
}
