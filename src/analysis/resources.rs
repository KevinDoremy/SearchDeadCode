//! Dead Android resource detection
//!
//! This module detects unused Android resources like strings, colors, dimensions,
//! drawables, etc. by cross-referencing resource definitions with code references.

use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Represents an Android resource
#[derive(Debug, Clone)]
pub struct AndroidResource {
    /// Resource name (e.g., "app_name")
    pub name: String,
    /// Resource type (e.g., "string", "color", "dimen")
    pub resource_type: String,
    /// File where resource is defined
    pub file: PathBuf,
    /// Line number in the file
    pub line: usize,
}

/// Result of resource analysis
#[derive(Debug, Default)]
pub struct ResourceAnalysis {
    /// All defined resources by type -> name
    pub defined: HashMap<String, HashMap<String, AndroidResource>>,
    /// Resources referenced in code
    pub referenced: HashSet<(String, String)>, // (type, name)
    /// Unused resources (defined but not referenced)
    pub unused: Vec<AndroidResource>,
}

/// Detector for unused Android resources
pub struct ResourceDetector {
    /// Minimum reference count to consider a resource as used
    min_references: usize,
}

impl ResourceDetector {
    pub fn new() -> Self {
        Self { min_references: 1 }
    }

    /// Analyze a project for unused resources
    pub fn analyze(&self, project_root: &Path) -> ResourceAnalysis {
        let mut analysis = ResourceAnalysis::default();

        // Find all resource directories
        let res_dirs = self.find_resource_dirs(project_root);

        // Parse all resource XML files
        for res_dir in &res_dirs {
            self.parse_resource_dir(res_dir, &mut analysis);
        }

        // Collect all references from Kotlin/Java files
        self.collect_code_references(project_root, &mut analysis);

        // Find unused resources
        for (res_type, resources) in &analysis.defined {
            for (name, resource) in resources {
                if !analysis
                    .referenced
                    .contains(&(res_type.clone(), name.clone()))
                {
                    // Check for common false positives
                    if !self.should_skip_resource(name, res_type) {
                        analysis.unused.push(resource.clone());
                    }
                }
            }
        }

        // Sort by file and line
        analysis
            .unused
            .sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

        analysis
    }

    /// Find all res/ directories in the project
    fn find_resource_dirs(&self, project_root: &Path) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // Walk the project looking for res/ directories
        let walker = walkdir::WalkDir::new(project_root)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "build" && name != "generated"
            });

        for entry in walker.flatten() {
            if entry.file_type().is_dir() {
                let name = entry.file_name().to_string_lossy();
                if name == "res" {
                    dirs.push(entry.path().to_path_buf());
                }
            }
        }

        dirs
    }

    /// Parse all resource files in a res directory
    fn parse_resource_dir(&self, res_dir: &Path, analysis: &mut ResourceAnalysis) {
        // Check common resource subdirectories
        let subdirs = [
            "values",
            "values-en",
            "values-fr",
            "values-es",
            "values-de",
            "values-night",
            "values-v21",
            "values-w600dp",
        ];

        for subdir in subdirs {
            let values_dir = res_dir.join(subdir);
            if values_dir.exists() && values_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&values_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map(|e| e == "xml").unwrap_or(false) {
                            self.parse_values_xml(&path, analysis);
                        }
                    }
                }
            }
        }
    }

    /// Parse a values XML file for resource definitions
    fn parse_values_xml(&self, file_path: &Path, analysis: &mut ResourceAnalysis) {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut reader = Reader::from_str(&content);

        let mut line = 1;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    // Map XML tag to resource type
                    let resource_type = match tag_name.as_str() {
                        "string" => Some("string"),
                        "color" => Some("color"),
                        "dimen" => Some("dimen"),
                        "style" => Some("style"),
                        "string-array" => Some("array"),
                        "integer-array" => Some("array"),
                        "array" => Some("array"),
                        "plurals" => Some("plurals"),
                        "bool" => Some("bool"),
                        "integer" => Some("integer"),
                        "attr" => Some("attr"),
                        "declare-styleable" => Some("styleable"),
                        _ => None,
                    };

                    if let Some(res_type) = resource_type {
                        // Get the name attribute
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                let name = String::from_utf8_lossy(&attr.value).to_string();

                                let resource = AndroidResource {
                                    name: name.clone(),
                                    resource_type: res_type.to_string(),
                                    file: file_path.to_path_buf(),
                                    line,
                                };

                                analysis
                                    .defined
                                    .entry(res_type.to_string())
                                    .or_default()
                                    .insert(name, resource);

                                break;
                            }
                        }
                    }
                }
                Ok(Event::Text(ref e)) => {
                    // Count newlines in text content to track line number
                    let bytes: &[u8] = e.as_ref();
                    line += bytes.iter().filter(|&&b| b == b'\n').count();
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
    }

    /// Collect resource references from Kotlin/Java code
    fn collect_code_references(&self, project_root: &Path, analysis: &mut ResourceAnalysis) {
        // Patterns for resource references:
        // - R.string.name
        // - R.color.name
        // - R.dimen.name
        // - @string/name (in XML)
        // - getString(R.string.name)

        let walker = walkdir::WalkDir::new(project_root)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "build" && name != "generated"
            });

        for entry in walker.flatten() {
            if entry.file_type().is_file() {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

                match ext {
                    "kt" | "java" => self.extract_code_references(path, analysis),
                    "xml" => self.extract_xml_references(path, analysis),
                    _ => {}
                }
            }
        }
    }

    /// Extract R.type.name references from Kotlin/Java code
    fn extract_code_references(&self, file_path: &Path, analysis: &mut ResourceAnalysis) {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        // Pattern: R.type.name
        let r_pattern = regex::Regex::new(r"R\.(\w+)\.(\w+)").unwrap();

        for cap in r_pattern.captures_iter(&content) {
            let res_type = &cap[1];
            let res_name = &cap[2];
            analysis
                .referenced
                .insert((res_type.to_string(), res_name.to_string()));
        }
    }

    /// Extract @type/name references from XML files
    fn extract_xml_references(&self, file_path: &Path, analysis: &mut ResourceAnalysis) {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        // Pattern: @type/name
        let ref_pattern = regex::Regex::new(r"@(\w+)/(\w+)").unwrap();

        for cap in ref_pattern.captures_iter(&content) {
            let res_type = &cap[1];
            let res_name = &cap[2];
            analysis
                .referenced
                .insert((res_type.to_string(), res_name.to_string()));
        }
    }

    /// Check if a resource should be skipped (common false positives)
    fn should_skip_resource(&self, name: &str, res_type: &str) -> bool {
        // Skip resources that are likely framework-required
        if res_type == "style" {
            // Base themes are often required
            if name.starts_with("Theme.") || name.starts_with("Base.") {
                return true;
            }
        }

        // Skip common Android-required resources
        let required_strings = ["app_name", "content_description"];
        if res_type == "string" && required_strings.contains(&name) {
            return true;
        }

        // Skip resources with "_" prefix (intentionally hidden)
        if name.starts_with('_') {
            return true;
        }

        false
    }
}

impl Default for ResourceDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detector_creation() {
        let detector = ResourceDetector::new();
        assert_eq!(detector.min_references, 1);
    }

    #[test]
    fn test_parse_strings_xml() {
        let temp_dir = TempDir::new().unwrap();
        let res_dir = temp_dir.path().join("res").join("values");
        fs::create_dir_all(&res_dir).unwrap();

        let strings_xml = res_dir.join("strings.xml");
        fs::write(
            &strings_xml,
            r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="test_string">Test</string>
    <string name="another_string">Another</string>
</resources>"#,
        )
        .unwrap();

        let mut analysis = ResourceAnalysis::default();
        let detector = ResourceDetector::new();
        detector.parse_values_xml(&strings_xml, &mut analysis);

        assert!(analysis.defined.contains_key("string"));
        let strings = analysis.defined.get("string").unwrap();
        assert!(strings.contains_key("test_string"));
        assert!(strings.contains_key("another_string"));
    }
}
