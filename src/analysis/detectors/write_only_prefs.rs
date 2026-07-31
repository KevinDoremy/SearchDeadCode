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
    /// Reads through a variable key (`prefs.getString(key, …)`): the read
    /// side cannot be enumerated, so no write-only verdict is provable
    pub dynamic_reads: usize,
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
            // Bundle et Intent ont les mêmes signatures put*/get* que
            // SharedPreferences.Editor — des arguments de navigation ne
            // sont pas des préférences
            if Self::is_bundle_or_intent_line(line) {
                continue;
            }

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
                } else if Self::is_dynamic_pref_read(line, pattern) {
                    analysis.dynamic_reads += 1;
                }
            }
        }

        analysis
    }

    /// A read whose key is a variable, on a preferences receiver: the
    /// wrapper-with-parameterized-keys idiom. `resources.getString(resId)`
    /// and `list.contains(x)` share the pattern but not the receiver.
    fn is_dynamic_pref_read(line: &str, pattern: &str) -> bool {
        if !line.to_lowercase().contains("pref") {
            return false;
        }
        let Some(idx) = line.find(pattern) else {
            return false;
        };
        let arg = line[idx + pattern.len()..].trim_start();
        arg.chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
    }

    /// Une ligne qui opère sur un Bundle/Intent, pas sur des prefs :
    /// receveur au nom parlant (`bundle.putBoolean`, `outState.putString`)
    /// ou construction/API propre aux extras sur la même ligne
    fn is_bundle_or_intent_line(line: &str) -> bool {
        if line.contains("Bundle(") || line.contains("Intent(") || line.contains("putExtra(") {
            return true;
        }
        const CARRIER_RECEIVERS: &[&str] = &[
            "bundle.",
            "intent.",
            "args.",
            "arguments.",
            "outstate.",
            "savedinstancestate.",
            "extras.",
        ];
        let lowered = line.to_lowercase();
        CARRIER_RECEIVERS.iter().any(|r| lowered.contains(r))
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

        // Handle constant reference: putString(KEY_NAME, ...) — possibly
        // qualified (`PrefKeys.KEY_NAME`): the constant is the last segment
        let trimmed = after_pattern.trim_start();
        if let Some(end) = trimmed.find(',').or_else(|| trimmed.find(')')) {
            let key_ref = trimmed[..end].trim();
            let last = key_ref.rsplit('.').next().unwrap_or(key_ref);
            if !last.is_empty()
                && last.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && last
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            {
                // The `$` marks a symbolic key; resolve_constant_keys() maps
                // it to its literal when the corpus pins a single value
                return Some(format!("${}", last));
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

/// Resolve `$CONSTANT` pseudo-keys against `CONSTANT = "literal"`
/// assignments found in the corpus, unifying a write through the constant
/// with a read through the literal (and vice versa). A constant bound to
/// two different literals stays symbolic — guessing would fabricate reads.
pub fn resolve_constant_keys(analysis: &mut SharedPrefsAnalysis, corpus: &str) {
    use regex::Regex;
    use std::sync::LazyLock;
    static CONST_DEF: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"\b([A-Z][A-Z0-9_]*)\s*=\s*"([^"]*)""#).expect("Invalid constant regex")
    });

    let mut values: HashMap<String, Option<String>> = HashMap::new();
    for cap in CONST_DEF.captures_iter(corpus) {
        let name = cap[1].to_string();
        let value = cap[2].to_string();
        values
            .entry(name)
            .and_modify(|v| {
                if v.as_deref() != Some(value.as_str()) {
                    *v = None; // ambiguous
                }
            })
            .or_insert(Some(value));
    }

    let rename = |map: &mut HashMap<String, Vec<PrefKeyLocation>>| {
        let symbolic: Vec<String> = map.keys().filter(|k| k.starts_with('$')).cloned().collect();
        for key in symbolic {
            if let Some(Some(value)) = values.get(&key[1..]) {
                if let Some(locations) = map.remove(&key) {
                    map.entry(value.clone()).or_default().extend(locations);
                }
            }
        }
    };
    rename(&mut analysis.writes);
    rename(&mut analysis.reads);
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
    fn a_bundle_put_is_not_a_preference_write() {
        // Cas réel : `bundle.putBoolean("EXTRA_HEADER_TITLE", true)` avant
        // un navigate(args = bundle) — Bundle a les mêmes signatures que
        // SharedPreferences.Editor, la clé sortait « written but never
        // read » alors que la destination la lit dans ses arguments.
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun goLive() {
                val bundle = Bundle()
                bundle.putBoolean("EXTRA_HEADER_TITLE", true)
                bundle.putString("EXTRA_ARTICLE_URI", url)
                navigate(resId, args = bundle)
            }
            fun relay(intent: Intent) {
                intent.putExtra("origin", name)
            }
            fun stash(outState: Bundle) {
                outState.putString("pending", value)
            }
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("test.kt"));
        assert!(
            analysis.writes.is_empty(),
            "aucune écriture Bundle/Intent ne compte comme une pref, trouvé: {:?}",
            analysis.writes.keys().collect::<Vec<_>>()
        );
    }

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
    fn a_parameterized_read_wrapper_is_counted_as_dynamic() {
        // Cas réel : un PreferenceService expose `get(key: String)` — la
        // lecture est inénumérable, aucun verdict write-only n'est prouvable.
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun save() {
                prefs.edit().putString("orphan_key", "value").apply()
            }
            fun read(key: String): String? {
                return prefs.getString(key, null)
            }
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("PreferenceService.kt"));
        assert_eq!(
            analysis.dynamic_reads, 1,
            "une lecture à clé variable est comptée, pas devinée"
        );
    }

    #[test]
    fn resources_get_string_is_not_a_dynamic_pref_read() {
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun label(): String {
                return resources.getString(R.string.title)
            }
            fun member(item: String): Boolean {
                return allowed.contains(item)
            }
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("Ui.kt"));
        assert_eq!(
            analysis.dynamic_reads, 0,
            "getString(resId) et contains() hors prefs ne comptent pas"
        );
    }

    #[test]
    fn a_qualified_constant_read_matches_the_constant_write() {
        // Cas réel : écriture via `KEY_SESSION`, lecture via
        // `PrefKeys.KEY_SESSION` — même constante, la clé n'est pas orpheline.
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun save(id: String) {
                prefs.edit().putString(KEY_SESSION, id).apply()
            }
            fun load(): String? {
                return prefs.getString(PrefKeys.KEY_SESSION, null)
            }
        "#;

        let analysis = detector.analyze_source(source, &PathBuf::from("Session.kt"));
        assert!(
            !analysis.is_write_only("$KEY_SESSION"),
            "la référence qualifiée résout vers la même constante: {:?}",
            analysis.reads.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_constant_key_resolves_to_its_literal() {
        // Écriture via constante, lecture via littéral : même clé une fois
        // la constante résolue sur le corpus.
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun save(t: String) {
                prefs.edit().putString(KEY_TOKEN, t).apply()
            }
            fun load(): String? {
                return prefs.getString("auth_token", null)
            }
        "#;

        let mut analysis = detector.analyze_source(source, &PathBuf::from("Auth.kt"));
        let corpus = "const val KEY_TOKEN = \"auth_token\"\n";
        resolve_constant_keys(&mut analysis, corpus);
        assert!(
            !analysis.is_write_only("auth_token") && !analysis.is_write_only("$KEY_TOKEN"),
            "writes: {:?}, reads: {:?}",
            analysis.writes.keys().collect::<Vec<_>>(),
            analysis.reads.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn an_ambiguous_constant_is_left_alone() {
        // Deux constantes homonymes avec des valeurs différentes : résoudre
        // au hasard fabriquerait des lectures fantômes.
        let detector = WriteOnlyPrefsDetector::new();
        let source = r#"
            fun save(t: String) {
                prefs.edit().putString(KEY_MODE, t).apply()
            }
        "#;

        let mut analysis = detector.analyze_source(source, &PathBuf::from("A.kt"));
        let corpus = "const val KEY_MODE = \"mode_a\"\nconst val KEY_MODE = \"mode_b\"\n";
        resolve_constant_keys(&mut analysis, corpus);
        assert!(
            analysis.writes.contains_key("$KEY_MODE"),
            "une constante ambiguë reste symbolique: {:?}",
            analysis.writes.keys().collect::<Vec<_>>()
        );
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
