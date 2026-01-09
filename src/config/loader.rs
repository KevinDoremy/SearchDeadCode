// Configuration loader - some methods reserved for future use
#![allow(dead_code)]

use miette::{IntoDiagnostic, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Configuration for SearchDeadCode analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Target directories to analyze
    pub targets: Vec<PathBuf>,

    /// Patterns to exclude from analysis
    pub exclude: Vec<String>,

    /// Patterns to retain - never report as dead code
    pub retain_patterns: Vec<String>,

    /// Explicit entry points (fully qualified class names)
    pub entry_points: Vec<String>,

    /// Report configuration
    pub report: ReportConfig,

    /// Detection configuration
    pub detection: DetectionConfig,

    /// Android-specific configuration
    pub android: AndroidConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportConfig {
    /// Output format: terminal, json, sarif
    pub format: String,

    /// Group results by: file, type, severity
    pub group_by: String,

    /// Show code snippets in output
    pub show_code: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DetectionConfig {
    /// Enable unused class detection
    pub unused_class: bool,

    /// Enable unused method detection
    pub unused_method: bool,

    /// Enable unused property detection
    pub unused_property: bool,

    /// Enable unused import detection
    pub unused_import: bool,

    /// Enable unused parameter detection
    pub unused_param: bool,

    /// Enable unused enum case detection
    pub unused_enum_case: bool,

    /// Enable assign-only property detection
    pub assign_only: bool,

    /// Enable dead branch detection
    pub dead_branch: bool,

    /// Enable redundant public modifier detection
    pub redundant_public: bool,

    /// Anti-pattern detector groups
    pub anti_patterns: AntiPatternConfig,
}

/// Configuration for anti-pattern detectors
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AntiPatternConfig {
    /// Enable all anti-pattern detectors
    pub enabled: bool,

    /// Enable architecture patterns (AP001-AP006)
    /// Deep inheritance, EventBus, global mutable state, single-impl interfaces
    pub architecture: bool,

    /// Enable Kotlin patterns (AP007-AP010, AP021-AP025)
    /// GlobalScope, heavy ViewModel, lateinit abuse, scope function chaining,
    /// nullability, reflection, long params, complex conditions, string duplication
    pub kotlin: bool,

    /// Enable performance patterns (AP011-AP015)
    /// Memory leaks, long methods, large classes, collection inefficiencies, loop allocations
    pub performance: bool,

    /// Enable Android patterns (AP016-AP020, AP026-AP030)
    /// Mutable state exposure, view logic in ViewModel, missing UseCase,
    /// nested callbacks, hardcoded dispatchers, unclosed resources,
    /// main thread DB, WakeLock abuse, AsyncTask, onDraw allocations
    pub android: bool,

    /// Enable Compose patterns (AP031-AP034)
    /// State without remember, LaunchedEffect without key, business logic in composables,
    /// NavController passing
    pub compose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AndroidConfig {
    /// Parse AndroidManifest.xml for entry points
    pub parse_manifest: bool,

    /// Parse layout XMLs for class references
    pub parse_layouts: bool,

    /// Auto-retain Android component patterns
    pub auto_retain_components: bool,

    /// Additional component patterns to retain
    pub component_patterns: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            targets: vec![],
            exclude: vec![
                "**/build/**".to_string(),
                "**/generated/**".to_string(),
                "**/.gradle/**".to_string(),
                "**/.idea/**".to_string(),
            ],
            retain_patterns: vec![],
            entry_points: vec![],
            report: ReportConfig::default(),
            detection: DetectionConfig::default(),
            android: AndroidConfig::default(),
        }
    }
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            format: "terminal".to_string(),
            group_by: "file".to_string(),
            show_code: true,
        }
    }
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            unused_class: true,
            unused_method: true,
            unused_property: true,
            unused_import: true,
            unused_param: true,
            unused_enum_case: true,
            assign_only: true,
            dead_branch: true,
            redundant_public: true,
            anti_patterns: AntiPatternConfig::default(),
        }
    }
}

impl Default for AntiPatternConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            architecture: false,
            kotlin: false,
            performance: false,
            android: false,
            compose: false,
        }
    }
}

impl Default for AndroidConfig {
    fn default() -> Self {
        Self {
            parse_manifest: true,
            parse_layouts: true,
            auto_retain_components: true,
            component_patterns: vec![
                "*Activity".to_string(),
                "*Fragment".to_string(),
                "*Service".to_string(),
                "*BroadcastReceiver".to_string(),
                "*ContentProvider".to_string(),
                "*ViewModel".to_string(),
                "*Application".to_string(),
            ],
        }
    }
}

impl Config {
    /// Load configuration from a file (YAML or TOML)
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read config file: {}", path.display()))?;

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match extension {
            "yml" | "yaml" => serde_yaml::from_str(&contents)
                .into_diagnostic()
                .wrap_err("Failed to parse YAML config"),
            "toml" => toml::from_str(&contents)
                .into_diagnostic()
                .wrap_err("Failed to parse TOML config"),
            _ => {
                // Try YAML first, then TOML
                if let Ok(config) = serde_yaml::from_str(&contents) {
                    Ok(config)
                } else {
                    toml::from_str(&contents)
                        .into_diagnostic()
                        .wrap_err("Failed to parse config file")
                }
            }
        }
    }

    /// Try to load configuration from default locations
    pub fn from_default_locations(project_root: &Path) -> Result<Self> {
        let default_names = [
            ".deadcode.yml",
            ".deadcode.yaml",
            ".deadcode.toml",
            "deadcode.yml",
            "deadcode.yaml",
            "deadcode.toml",
        ];

        for name in &default_names {
            let path = project_root.join(name);
            if path.exists() {
                return Self::from_file(&path);
            }
        }

        // No config file found, use defaults
        Ok(Self::default())
    }

    /// Check if a pattern matches for exclusion
    pub fn should_exclude(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.exclude
            .iter()
            .any(|pattern| glob_match(pattern, &path_str))
    }

    /// Check if a declaration should be retained
    pub fn should_retain(&self, name: &str) -> bool {
        // Check explicit retain patterns
        if self.retain_patterns.iter().any(|p| glob_match(p, name)) {
            return true;
        }

        // Check Android component patterns if enabled
        if self.android.auto_retain_components
            && self
                .android
                .component_patterns
                .iter()
                .any(|p| glob_match(p, name))
        {
            return true;
        }

        false
    }
}

/// Simple glob matching for patterns like "*Activity" or "**/*.kt"
fn glob_match(pattern: &str, text: &str) -> bool {
    // Handle simple wildcard patterns
    if pattern.starts_with('*') && !pattern.contains('/') {
        // Pattern like "*Activity" matches "MainActivity"
        let suffix = &pattern[1..];
        return text.ends_with(suffix);
    }

    if pattern.ends_with('*') && !pattern.contains('/') {
        // Pattern like "Test*" matches "TestHelper"
        let prefix = &pattern[..pattern.len() - 1];
        return text.starts_with(prefix);
    }

    // Handle path patterns with **
    if pattern.contains("**") {
        // Pattern like "**/test/**" - check if "test" directory is anywhere in path
        // Pattern like "**/build/**" - check if "build" directory is anywhere in path
        let cleaned = pattern.replace("**/", "").replace("/**", "");

        // If pattern is like "**/test/**", check if "/test/" is in the path
        if pattern.starts_with("**/") && pattern.ends_with("/**") {
            let dir_name = cleaned.trim_matches('/');
            // Must match as a complete directory name, not substring
            // "/test/" matches, but "/testproject/" should not match
            let dir_pattern = format!("/{}/", dir_name);
            return text.contains(&dir_pattern);
        }

        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            let prefix = parts[0].trim_end_matches('/');
            let suffix = parts[1].trim_start_matches('/');

            if prefix.is_empty() && suffix.is_empty() {
                return true; // Pattern is just "**"
            }

            if prefix.is_empty() {
                return text.ends_with(suffix) || text.contains(&format!("/{}", suffix));
            }

            if suffix.is_empty() {
                return text.starts_with(prefix) || text.contains(&format!("{}/", prefix));
            }

            // Both prefix and suffix
            return (text.starts_with(prefix) || text.contains(&format!("/{}/", prefix)))
                && (text.ends_with(suffix) || text.contains(&format!("/{}", suffix)));
        }
    }

    // Exact match
    text == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_suffix() {
        assert!(glob_match("*Activity", "MainActivity"));
        assert!(glob_match("*Activity", "LoginActivity"));
        assert!(!glob_match("*Activity", "ActivityHelper"));
    }

    #[test]
    fn test_glob_match_prefix() {
        assert!(glob_match("Test*", "TestHelper"));
        assert!(glob_match("Test*", "TestCase"));
        assert!(!glob_match("Test*", "HelperTest"));
    }

    #[test]
    fn test_glob_match_path() {
        assert!(glob_match("**/build/**", "/project/build/output"));
        assert!(glob_match("**/build/**", "app/build/generated"));
        assert!(!glob_match("**/build/**", "/project/src/main"));
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.detection.unused_class);
        assert!(config.android.parse_manifest);
    }
}
