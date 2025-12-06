//! Unused Intent Extra Detector
//!
//! Detects Android Intent extras that are put but never retrieved.
//! This is a common pattern when refactoring Activity/Fragment communication.
//!
//! ## Detection Algorithm
//!
//! 1. Find all `intent.putExtra("KEY", value)` calls
//! 2. Find all `intent.getStringExtra("KEY")`, `intent.getIntExtra("KEY", default)`, etc.
//! 3. Match keys that are put but never retrieved
//! 4. Report unused extras
//!
//! ## Examples Detected
//!
//! ```kotlin
//! // In ActivityA.kt
//! intent.putExtra("USER_ID", userId)      // Key is set
//! intent.putExtra("LEGACY_FLAG", true)    // DEAD: never read
//! startActivity(intent)
//!
//! // In ActivityB.kt
//! val userId = intent.getStringExtra("USER_ID")  // Key is read
//! // LEGACY_FLAG is never read anywhere!
//! ```

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Android system extras that are read by external apps (not our code)
/// These should NOT be flagged as unused
const SYSTEM_EXTRAS: &[&str] = &[
    // Android provider extras (used by Settings app)
    "android.provider.extra.APP_PACKAGE",
    "android.provider.extra.CHANNEL_ID",
    "android.provider.extra.CHANNEL_GROUP_ID",
    "android.provider.extra.CONVERSATION_ID",
    // Legacy notification settings extras
    "app_package",
    "app_uid",
    // Common Intent extras for external apps
    "android.intent.extra.TEXT",
    "android.intent.extra.SUBJECT",
    "android.intent.extra.EMAIL",
    "android.intent.extra.STREAM",
    "android.intent.extra.TITLE",
    "android.intent.extra.INTENT",
    "android.intent.extra.PACKAGE_NAME",
    "android.intent.extra.UID",
];

/// Location info for an extra
#[derive(Debug, Clone)]
pub struct ExtraLocation {
    pub file: std::path::PathBuf,
    pub line: usize,
    pub key: String,
}

/// Result of intent extra analysis
#[derive(Debug)]
pub struct IntentExtraAnalysis {
    /// Extras that are put but never retrieved
    pub unused_extras: Vec<ExtraLocation>,
    /// Total extras put
    pub total_put: usize,
    /// Total extras retrieved
    pub total_get: usize,
}

/// Detector for unused Intent extras
pub struct UnusedIntentExtraDetector {
    // Patterns to match putExtra calls
    put_extra_pattern: Regex,
    // Patterns to match getExtra calls
    get_extra_pattern: Regex,
    // Pattern to match hasExtra calls (also counts as "reading")
    has_extra_pattern: Regex,
}

impl UnusedIntentExtraDetector {
    pub fn new() -> Self {
        // Match: putExtra("KEY", value) or putExtra(KEY_CONST, value)
        let put_extra_pattern = Regex::new(r#"putExtra\s*\(\s*"([^"]+)""#).unwrap();

        // Match: getStringExtra("KEY"), getIntExtra("KEY", ...), getBooleanExtra("KEY", ...), etc.
        let get_extra_pattern = Regex::new(
            r#"get(?:String|Int|Long|Float|Double|Boolean|Char|Byte|Short|Serializable|Parcelable|Bundle)?Extra(?:s)?\s*\(\s*"([^"]+)""#
        ).unwrap();

        // Match: hasExtra("KEY")
        let has_extra_pattern = Regex::new(r#"hasExtra\s*\(\s*"([^"]+)""#).unwrap();

        Self {
            put_extra_pattern,
            get_extra_pattern,
            has_extra_pattern,
        }
    }

    /// Analyze a directory for unused intent extras
    pub fn analyze(&self, root: &Path) -> IntentExtraAnalysis {
        use ignore::WalkBuilder;

        // Collect all put_extra keys with locations
        let mut put_extras: HashMap<String, Vec<ExtraLocation>> = HashMap::new();
        // Collect all get_extra keys (including hasExtra)
        let mut get_extras: HashSet<String> = HashSet::new();

        let walker = WalkBuilder::new(root).hidden(true).git_ignore(true).build();

        for entry in walker.flatten() {
            let path = entry.path();

            // Only process Kotlin and Java files
            let ext = path.extension().and_then(|e| e.to_str());
            if !matches!(ext, Some("kt") | Some("java")) {
                continue;
            }

            // Skip test files
            let path_str = path.to_string_lossy();
            if path_str.contains("/test/") || path_str.contains("/androidTest/") {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(path) {
                for (line_num, line) in content.lines().enumerate() {
                    // Find putExtra calls
                    for caps in self.put_extra_pattern.captures_iter(line) {
                        if let Some(key) = caps.get(1) {
                            let key_str = key.as_str().to_string();
                            put_extras
                                .entry(key_str.clone())
                                .or_default()
                                .push(ExtraLocation {
                                    file: path.to_path_buf(),
                                    line: line_num + 1,
                                    key: key_str,
                                });
                        }
                    }

                    // Find getXxxExtra calls
                    for caps in self.get_extra_pattern.captures_iter(line) {
                        if let Some(key) = caps.get(1) {
                            get_extras.insert(key.as_str().to_string());
                        }
                    }

                    // Find hasExtra calls
                    for caps in self.has_extra_pattern.captures_iter(line) {
                        if let Some(key) = caps.get(1) {
                            get_extras.insert(key.as_str().to_string());
                        }
                    }
                }
            }
        }

        let total_put = put_extras.values().map(|v| v.len()).sum();
        let total_get = get_extras.len();

        // Build a set of system extras for fast lookup
        let system_extras: HashSet<&str> = SYSTEM_EXTRAS.iter().copied().collect();

        // Find unused extras (put but never get)
        let mut unused_extras = Vec::new();
        for (key, locations) in &put_extras {
            // Skip system extras that are read by external apps
            if system_extras.contains(key.as_str()) {
                continue;
            }
            // Skip keys that look like Android system extras
            if key.starts_with("android.") {
                continue;
            }
            if !get_extras.contains(key) {
                // Report only the first location for each key
                if let Some(first_loc) = locations.first() {
                    unused_extras.push(first_loc.clone());
                }
            }
        }

        // Sort by file and line
        unused_extras.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

        IntentExtraAnalysis {
            unused_extras,
            total_put,
            total_get,
        }
    }
}

impl Default for UnusedIntentExtraDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put_extra_pattern() {
        let detector = UnusedIntentExtraDetector::new();

        let code = r#"intent.putExtra("USER_ID", userId)"#;
        let caps = detector.put_extra_pattern.captures(code);
        assert!(caps.is_some());
        assert_eq!(caps.unwrap().get(1).unwrap().as_str(), "USER_ID");
    }

    #[test]
    fn test_get_extra_pattern() {
        let detector = UnusedIntentExtraDetector::new();

        let code = r#"intent.getStringExtra("USER_ID")"#;
        let caps = detector.get_extra_pattern.captures(code);
        assert!(caps.is_some());
        assert_eq!(caps.unwrap().get(1).unwrap().as_str(), "USER_ID");

        let code2 = r#"intent.getIntExtra("COUNT", 0)"#;
        let caps2 = detector.get_extra_pattern.captures(code2);
        assert!(caps2.is_some());
        assert_eq!(caps2.unwrap().get(1).unwrap().as_str(), "COUNT");
    }
}
