// Dead code report generator from ProGuard/R8 usage.txt
//
// Filters out generated code and produces a clean list of
// removable application classes.

#![allow(dead_code)] // Builder pattern methods for future configuration

use super::{ProguardUsage, UsageEntryKind};
use miette::{IntoDiagnostic, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Patterns for generated code that should be filtered out
const GENERATED_PATTERNS: &[&str] = &[
    // Dagger/Hilt generated code
    "_Factory",
    "_Impl",
    "_HiltModules",
    "_ComponentTreeDeps",
    "_GeneratedInjector",
    "_MembersInjector",
    "Dagger",
    "Hilt_",
    "_HiltComponents",
    "_ProvideFactory",
    "_AssistedFactory",
    // Data binding
    "BindingImpl",
    "DataBinderMapper",
    "DataBindingComponent",
    // Room database
    "Dao_Impl",
    // Kotlin generated
    "$$serializer",
    "$Creator",
    // Android generated
    "BuildConfig",
    "_ViewBinding",
    // Parcelize
    "CREATOR",
    // AutoValue/AutoParcel
    "AutoValue_",
    "AutoParcel_",
    // Moshi
    "JsonAdapter",
    // Retrofit
    "_Proxy",
    // Arrow/Optics generated
    "__OpticsKt",
    // Compose singletons
    "ComposableSingletons$",
];

/// Exact class name suffixes to filter
const GENERATED_SUFFIXES: &[&str] = &[
    "Module_ProvideFactory",
    "Module_Provides",
    "_Factory",
    "_Impl",
    "_MembersInjector",
];

/// Patterns that indicate R resource classes
const R_CLASS_PATTERNS: &[&str] = &[".R$", ".R", ".BR"];

/// Report generator configuration
pub struct ReportGenerator {
    /// Package prefix filter (e.g., "com.example")
    package_filter: Option<String>,
    /// Project name for the report header
    project_name: Option<String>,
    /// Include methods in report
    include_methods: bool,
    /// Include fields in report
    include_fields: bool,
}

impl ReportGenerator {
    pub fn new() -> Self {
        Self {
            package_filter: None,
            project_name: None,
            include_methods: false,
            include_fields: false,
        }
    }

    pub fn with_package_filter(mut self, prefix: Option<String>) -> Self {
        self.package_filter = prefix;
        self
    }

    pub fn with_project_name(mut self, name: Option<String>) -> Self {
        self.project_name = name;
        self
    }

    pub fn with_methods(mut self, include: bool) -> Self {
        self.include_methods = include;
        self
    }

    pub fn with_fields(mut self, include: bool) -> Self {
        self.include_fields = include;
        self
    }

    /// Check if a class name matches generated code patterns
    fn is_generated_code(class_name: &str) -> bool {
        let simple_name = class_name.split('.').next_back().unwrap_or(class_name);

        // Check R class patterns
        for pattern in R_CLASS_PATTERNS {
            if class_name.contains(pattern) || simple_name == "R" || simple_name == "BR" {
                return true;
            }
        }

        // Check patterns
        for pattern in GENERATED_PATTERNS {
            if simple_name.contains(pattern) {
                return true;
            }
        }

        // Check suffixes
        for suffix in GENERATED_SUFFIXES {
            if simple_name.ends_with(suffix) {
                return true;
            }
        }

        // Check for anonymous classes (contain $1, $2, etc.)
        if simple_name.contains('$') {
            let parts: Vec<&str> = simple_name.split('$').collect();
            for part in parts.iter().skip(1) {
                if part.chars().all(|c| c.is_ascii_digit()) {
                    return true;
                }
            }
        }

        false
    }

    /// Generate a filtered dead code report with nice formatting
    pub fn generate(&self, usage: &ProguardUsage, output_path: &Path) -> Result<ReportStats> {
        let file = File::create(output_path).into_diagnostic()?;
        let mut writer = BufWriter::new(file);

        let mut stats = ReportStats::default();

        // Collect dead classes first
        let mut dead_classes: Vec<String> = Vec::new();
        for class_name in usage.dead_classes().iter() {
            // Apply package filter
            if let Some(ref prefix) = self.package_filter {
                if !class_name.starts_with(prefix) {
                    continue;
                }
            }

            // Filter out generated code
            if Self::is_generated_code(class_name) {
                stats.filtered_generated += 1;
                continue;
            }

            dead_classes.push(class_name.clone());
            stats.classes += 1;
        }
        dead_classes.sort();

        // Get project name
        let project_name = self
            .project_name
            .clone()
            .or_else(|| {
                self.package_filter
                    .as_ref()
                    .and_then(|p| p.split('.').next_back().map(|s| s.to_uppercase()))
            })
            .unwrap_or_else(|| "PROJECT".to_string());

        // Write header
        let header_width = 78;
        let border = "═".repeat(header_width);

        writeln!(writer, "╔{}╗", border).into_diagnostic()?;

        let title1 = format!("DEAD CODE REPORT - {} PROJECT", project_name);
        let padding1 = (header_width - title1.len()) / 2;
        writeln!(
            writer,
            "║{:>width$}{}{}║",
            "",
            title1,
            " ".repeat(header_width - padding1 - title1.len()),
            width = padding1
        )
        .into_diagnostic()?;

        let title2 = "Generated from R8/ProGuard usage.txt";
        let padding2 = (header_width - title2.len()) / 2;
        writeln!(
            writer,
            "║{:>width$}{}{}║",
            "",
            title2,
            " ".repeat(header_width - padding2 - title2.len()),
            width = padding2
        )
        .into_diagnostic()?;

        writeln!(writer, "╚{}╝", border).into_diagnostic()?;
        writeln!(writer).into_diagnostic()?;

        // Write description
        writeln!(
            writer,
            "This report shows code that R8/ProGuard determined is NEVER USED in your"
        )
        .into_diagnostic()?;
        writeln!(
            writer,
            "release build. These items are removed by R8 during optimization, but remain"
        )
        .into_diagnostic()?;
        writeln!(
            writer,
            "in your source code - cluttering the codebase and slowing down builds."
        )
        .into_diagnostic()?;
        writeln!(writer).into_diagnostic()?;

        // Section: Completely unused classes
        writeln!(writer, "{}", "═".repeat(78)).into_diagnostic()?;
        writeln!(writer, "SAFE TO REMOVE - ENTIRE CLASSES").into_diagnostic()?;
        writeln!(writer, "{}", "═".repeat(78)).into_diagnostic()?;
        writeln!(writer).into_diagnostic()?;

        for class_name in &dead_classes {
            writeln!(writer, "{}", class_name).into_diagnostic()?;
        }

        // Section: Unused methods (optional)
        if self.include_methods {
            let mut methods: Vec<(String, String)> = Vec::new();

            for entry in usage.all_entries() {
                if entry.kind != UsageEntryKind::Method {
                    continue;
                }

                // Apply package filter
                if let Some(ref prefix) = self.package_filter {
                    if !entry.class_name.starts_with(prefix) {
                        continue;
                    }
                }

                // Filter out generated code
                if Self::is_generated_code(&entry.class_name) {
                    continue;
                }

                if let Some(ref sig) = entry.signature {
                    methods.push((entry.class_name.clone(), sig.clone()));
                    stats.methods += 1;
                }
            }

            if !methods.is_empty() {
                writeln!(writer).into_diagnostic()?;
                writeln!(writer, "{}", "═".repeat(78)).into_diagnostic()?;
                writeln!(writer, "UNUSED METHODS").into_diagnostic()?;
                writeln!(writer, "{}", "═".repeat(78)).into_diagnostic()?;
                writeln!(writer).into_diagnostic()?;

                methods.sort();
                for (class_name, sig) in &methods {
                    writeln!(writer, "{}: {}", class_name, sig).into_diagnostic()?;
                }
            }
        }

        // Section: Unused fields (optional)
        if self.include_fields {
            let mut fields: Vec<(String, String)> = Vec::new();

            for entry in usage.all_entries() {
                if entry.kind != UsageEntryKind::Field {
                    continue;
                }

                // Apply package filter
                if let Some(ref prefix) = self.package_filter {
                    if !entry.class_name.starts_with(prefix) {
                        continue;
                    }
                }

                // Filter out generated code
                if Self::is_generated_code(&entry.class_name) {
                    continue;
                }

                if let Some(ref sig) = entry.signature {
                    fields.push((entry.class_name.clone(), sig.clone()));
                    stats.fields += 1;
                }
            }

            if !fields.is_empty() {
                writeln!(writer).into_diagnostic()?;
                writeln!(writer, "{}", "═".repeat(78)).into_diagnostic()?;
                writeln!(writer, "UNUSED FIELDS").into_diagnostic()?;
                writeln!(writer, "{}", "═".repeat(78)).into_diagnostic()?;
                writeln!(writer).into_diagnostic()?;

                fields.sort();
                for (class_name, sig) in &fields {
                    writeln!(writer, "{}: {}", class_name, sig).into_diagnostic()?;
                }
            }
        }

        // Summary section
        writeln!(writer).into_diagnostic()?;
        writeln!(writer, "{}", "═".repeat(78)).into_diagnostic()?;
        writeln!(writer, "SUMMARY").into_diagnostic()?;
        writeln!(writer, "{}", "═".repeat(78)).into_diagnostic()?;
        writeln!(writer).into_diagnostic()?;

        let total = stats.classes + stats.methods + stats.fields;
        writeln!(writer, "Total removable classes/items:      {}", total).into_diagnostic()?;
        if stats.classes > 0 {
            writeln!(
                writer,
                "  - Classes:                        {}",
                stats.classes
            )
            .into_diagnostic()?;
        }
        if stats.methods > 0 {
            writeln!(
                writer,
                "  - Methods:                        {}",
                stats.methods
            )
            .into_diagnostic()?;
        }
        if stats.fields > 0 {
            writeln!(
                writer,
                "  - Fields:                         {}",
                stats.fields
            )
            .into_diagnostic()?;
        }
        writeln!(
            writer,
            "Generated code filtered:            {}",
            stats.filtered_generated
        )
        .into_diagnostic()?;
        writeln!(writer).into_diagnostic()?;

        writer.flush().into_diagnostic()?;

        Ok(stats)
    }
}

impl Default for ReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct ReportStats {
    pub classes: usize,
    pub methods: usize,
    pub fields: usize,
    pub filtered_generated: usize,
}

impl std::fmt::Display for ReportStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} classes, {} methods, {} fields ({} generated items filtered)",
            self.classes, self.methods, self.fields, self.filtered_generated
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generated_code_detection() {
        // Should be filtered
        assert!(ReportGenerator::is_generated_code("MyClass_Factory"));
        assert!(ReportGenerator::is_generated_code("MyClass_Impl"));
        assert!(ReportGenerator::is_generated_code("DaggerAppComponent"));
        assert!(ReportGenerator::is_generated_code("Hilt_MainActivity"));
        assert!(ReportGenerator::is_generated_code(
            "MyModule_ProvideFactory"
        ));
        assert!(ReportGenerator::is_generated_code("MyClass$1")); // Anonymous
        assert!(ReportGenerator::is_generated_code(
            "com.example.MyClass_MembersInjector"
        ));
        assert!(ReportGenerator::is_generated_code("com.example.R"));
        assert!(ReportGenerator::is_generated_code("com.example.R$string"));
        assert!(ReportGenerator::is_generated_code("com.example.BR"));

        // Should NOT be filtered
        assert!(!ReportGenerator::is_generated_code("com.example.MyClass"));
        assert!(!ReportGenerator::is_generated_code(
            "com.example.MyFragment"
        ));
        assert!(!ReportGenerator::is_generated_code(
            "com.example.MyViewModel"
        ));
        assert!(!ReportGenerator::is_generated_code(
            "com.example.UserRepository"
        ));
    }
}
