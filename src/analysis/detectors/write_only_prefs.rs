//! Write-Only SharedPreferences Detector
//!
//! Detects SharedPreferences keys that are written (putString, putInt, etc.)
//! but never read (getString, getInt, etc.). This is a common form of dead code
//! where developers save data they never retrieve.
//!
//! ## Detection Algorithm
//!
//! 1. Find all SharedPreferences write calls (putString, putInt, putBoolean, putLong, putFloat)
//! 2. Extract the key being written (first argument)
//! 3. Find all SharedPreferences read calls (getString, getInt, getBoolean, getLong, getFloat)
//! 4. Extract the key being read (first argument)
//! 5. Report keys that are written but never read
//!
//! ## Examples Detected
//!
//! ```kotlin
//! class Example(context: Context) {
//!     val prefs = context.getSharedPreferences("app", Context.MODE_PRIVATE)
//!
//!     fun save() {
//!         prefs.edit().putString("unused_key", "value").apply()  // DEAD: never read
//!     }
//! }
//! ```

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{Declaration, DeclarationId, DeclarationKind, Graph, Language, Location};

/// Location where a preference key is used
#[derive(Debug, Clone)]
pub struct PrefKeyLocation {
    pub key: String,
    pub file: PathBuf,
    pub line: usize,
    pub is_write: bool,
}

/// Result of SharedPreferences analysis
#[derive(Debug, Default)]
pub struct SharedPrefsAnalysis {
    /// Keys that are written (key -> locations)
    pub writes: HashMap<String, Vec<PrefKeyLocation>>,
    /// Keys that are read (key -> locations)
    pub reads: HashMap<String, Vec<PrefKeyLocation>>,
}

impl SharedPrefsAnalysis {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a write location for a key
    pub fn add_write(&mut self, key: String, file: PathBuf, line: usize) {
        self.writes
            .entry(key.clone())
            .or_default()
            .push(PrefKeyLocation {
                key,
                file,
                line,
                is_write: true,
            });
    }

    /// Add a read location for a key
    pub fn add_read(&mut self, key: String, file: PathBuf, line: usize) {
        self.reads
            .entry(key.clone())
            .or_default()
            .push(PrefKeyLocation {
                key,
                file,
                line,
                is_write: false,
            });
    }

    /// Get keys that are written but never read
    pub fn get_write_only_keys(&self) -> Vec<&String> {
        self.writes
            .keys()
            .filter(|key| !self.reads.contains_key(*key))
            .collect()
    }

    /// Check if a specific key is write-only
    pub fn is_write_only(&self, key: &str) -> bool {
        self.writes.contains_key(key) && !self.reads.contains_key(key)
    }
}

/// Detector for write-only SharedPreferences keys
pub struct WriteOnlyPrefsDetector {
    /// Skip keys that match common SDK patterns
    skip_sdk_keys: bool,
}

impl WriteOnlyPrefsDetector {
    pub fn new() -> Self {
        Self {
            skip_sdk_keys: true,
        }
    }

    /// Check if a key should be skipped (SDK/framework keys)
    fn should_skip_key(&self, key: &str) -> bool {
        if !self.skip_sdk_keys {
            return false;
        }

        // Common SDK keys that are read by the framework, not user code
        let sdk_patterns = [
            "com_braze_",
            "com_appboy_",
            "google_",
            "firebase_",
            "facebook_",
            "crashlytics_",
            "appsflyer_",
        ];

        sdk_patterns.iter().any(|p| key.starts_with(p))
    }

    /// Analyze source code to find SharedPreferences usage
    pub fn analyze_source(&self, source: &str, file: &std::path::Path) -> SharedPrefsAnalysis {
        let mut analysis = SharedPrefsAnalysis::new();

        // Patterns for write operations
        let write_patterns = [
            "putString(",
            "putInt(",
            "putBoolean(",
            "putLong(",
            "putFloat(",
            "putStringSet(",
        ];

        // Patterns for read operations
        let read_patterns = [
            "getString(",
            "getInt(",
            "getBoolean(",
            "getLong(",
            "getFloat(",
            "getStringSet(",
            "contains(",
        ];

        for (line_num, line) in source.lines().enumerate() {
            // Check for write operations
            for pattern in &write_patterns {
                if let Some(key) = self.extract_key_from_line(line, pattern) {
                    if !self.should_skip_key(&key) {
                        analysis.add_write(key, file.to_path_buf(), line_num + 1);
                    }
                }
            }

            // Check for read operations
            for pattern in &read_patterns {
                if let Some(key) = self.extract_key_from_line(line, pattern) {
                    analysis.add_read(key, file.to_path_buf(), line_num + 1);
                }
            }
        }

        analysis
    }

    /// Extract the key argument from a SharedPreferences method call
    fn extract_key_from_line(&self, line: &str, pattern: &str) -> Option<String> {
        let idx = line.find(pattern)?;
        let after_pattern = &line[idx + pattern.len()..];

        // Handle string literal: putString("key", ...)
        if after_pattern.trim_start().starts_with('"') {
            let start = after_pattern.find('"')? + 1;
            let rest = &after_pattern[start..];
            let end = rest.find('"')?;
            return Some(rest[..end].to_string());
        }

        // Handle constant reference: putString(KEY_NAME, ...)
        // We need to track these separately
        let trimmed = after_pattern.trim_start();
        if let Some(end) = trimmed.find(',').or_else(|| trimmed.find(')')) {
            let key_ref = trimmed[..end].trim();
            // If it looks like a constant (all caps or contains underscore)
            if key_ref
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            {
                // Return the constant name as the key for now
                // A more sophisticated approach would resolve the constant value
                return Some(format!("${}", key_ref));
            }
        }

        None
    }
}

impl Default for WriteOnlyPrefsDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert analysis results to DeadCode issues
pub fn analysis_to_issues(analysis: &SharedPrefsAnalysis) -> Vec<DeadCode> {
    let mut issues = Vec::new();

    for key in analysis.get_write_only_keys() {
        if let Some(locations) = analysis.writes.get(key) {
            for loc in locations {
                // Create a synthetic declaration for the preference key
                let decl = Declaration::new(
                    DeclarationId::new(loc.file.clone(), loc.line, 0),
                    format!("SharedPreferences key '{}'", key),
                    DeclarationKind::Property,
                    Location::new(loc.file.clone(), loc.line, 1, 0, 0),
                    Language::Kotlin,
                );

                let mut dead = DeadCode::new(decl, DeadCodeIssue::WriteOnlyPreference);
                dead = dead.with_message(format!(
                    "SharedPreferences key '{}' is written but never read",
                    key
                ));
                dead = dead.with_confidence(Confidence::High);
                issues.push(dead);
            }
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_creation() {
        let detector = WriteOnlyPrefsDetector::new();
        assert!(detector.skip_sdk_keys);
    }

    #[test]
    fn test_extract_string_literal_key() {
        let detector = WriteOnlyPrefsDetector::new();
        let line = r#"prefs.edit().putString("user_token", token).apply()"#;
        let key = detector.extract_key_from_line(line, "putString(");
        assert_eq!(key, Some("user_token".to_string()));
    }

    #[test]
    fn test_extract_key_with_spaces() {
        let detector = WriteOnlyPrefsDetector::new();
        let line = r#"prefs.edit().putLong( "last_sync_time" , time).apply()"#;
        let key = detector.extract_key_from_line(line, "putLong(");
        assert_eq!(key, Some("last_sync_time".to_string()));
    }

    #[test]
    fn test_extract_constant_key() {
        let detector = WriteOnlyPrefsDetector::new();
        let line = r#"prefs.edit().putString(KEY_SESSION_ID, id).apply()"#;
        let key = detector.extract_key_from_line(line, "putString(");
        assert_eq!(key, Some("$KEY_SESSION_ID".to_string()));
    }

    #[test]
    fn test_analyze_write_only() {
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun save() {
                prefs.edit().putString("unused_key", "value").apply()
            }
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("test.kt"));
        assert!(analysis.writes.contains_key("unused_key"));
        assert!(!analysis.reads.contains_key("unused_key"));
        assert!(analysis.is_write_only("unused_key"));
    }

    #[test]
    fn test_analyze_read_write() {
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun save() {
                prefs.edit().putString("used_key", "value").apply()
            }
            fun load(): String {
                return prefs.getString("used_key", "") ?: ""
            }
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("test.kt"));
        assert!(analysis.writes.contains_key("used_key"));
        assert!(analysis.reads.contains_key("used_key"));
        assert!(!analysis.is_write_only("used_key"));
    }

    #[test]
    fn test_skip_sdk_keys() {
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun save() {
                prefs.edit().putString("com_braze_api_key", "value").apply()
            }
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("test.kt"));
        // SDK keys should be skipped
        assert!(!analysis.writes.contains_key("com_braze_api_key"));
    }

    #[test]
    fn test_multiple_keys() {
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun save() {
                prefs.edit()
                    .putString("key1", "value1")
                    .putInt("key2", 42)
                    .putBoolean("key3", true)
                    .apply()
            }
            fun load() {
                val v1 = prefs.getString("key1", "")
            }
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("test.kt"));
        assert!(!analysis.is_write_only("key1")); // has read
        assert!(analysis.is_write_only("key2")); // no read
        assert!(analysis.is_write_only("key3")); // no read
    }

    #[test]
    fn test_get_write_only_keys() {
        let mut analysis = SharedPrefsAnalysis::new();
        analysis.add_write("key1".to_string(), PathBuf::from("test.kt"), 1);
        analysis.add_write("key2".to_string(), PathBuf::from("test.kt"), 2);
        analysis.add_read("key1".to_string(), PathBuf::from("test.kt"), 10);

        let write_only = analysis.get_write_only_keys();
        assert_eq!(write_only.len(), 1);
        assert!(write_only.contains(&&"key2".to_string()));
    }

    #[test]
    fn test_contains_as_read() {
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun save() {
                prefs.edit().putString("checked_key", "value").apply()
            }
            fun check(): Boolean {
                return prefs.contains("checked_key")
            }
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("test.kt"));
        // contains() counts as a read
        assert!(!analysis.is_write_only("checked_key"));
    }
}
