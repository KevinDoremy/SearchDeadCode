// Hybrid analyzer - combines static reachability with runtime coverage
//
// This implements the Static + Dynamic hybrid analysis technique,
// inspired by Meta's SCARF system. By combining compile-time graph
// analysis with runtime coverage data, we can:
// 1. Increase confidence in dead code findings
// 2. Reduce false positives from dynamic dispatch
// 3. Identify code that is reachable but never actually executed

use super::{Confidence, DeadCode, DeadCodeIssue};
use crate::coverage::CoverageData;
use crate::graph::{Declaration, DeclarationKind, Graph, Visibility};
use crate::proguard::ProguardUsage;
use std::collections::HashSet;

/// Hybrid analyzer that combines static and dynamic analysis
pub struct HybridAnalyzer {
    /// Runtime coverage data (optional)
    coverage: Option<CoverageData>,
    /// ProGuard/R8 usage.txt data (optional)
    proguard: Option<ProguardUsage>,
}

impl HybridAnalyzer {
    pub fn new() -> Self {
        Self {
            coverage: None,
            proguard: None,
        }
    }

    pub fn with_coverage(mut self, coverage: CoverageData) -> Self {
        self.coverage = Some(coverage);
        self
    }

    pub fn with_proguard(mut self, proguard: ProguardUsage) -> Self {
        self.proguard = Some(proguard);
        self
    }

    /// Check if we have any enhancement data
    pub fn has_data(&self) -> bool {
        self.coverage.is_some() || self.proguard.is_some()
    }

    /// Get ProGuard data if available
    pub fn proguard(&self) -> Option<&ProguardUsage> {
        self.proguard.as_ref()
    }

    /// Enhance dead code findings with runtime coverage and/or ProGuard data
    ///
    /// This method takes static analysis results and cross-references them
    /// with runtime coverage and ProGuard usage.txt to adjust confidence levels.
    pub fn enhance_findings(&self, dead_code: Vec<DeadCode>) -> Vec<DeadCode> {
        dead_code
            .into_iter()
            .map(|dc| self.enhance_single_full(dc))
            .collect()
    }

    fn enhance_single_full(&self, mut dc: DeadCode) -> DeadCode {
        let decl = &dc.declaration;

        // Check ProGuard data first (strongest signal)
        if let Some(ref proguard) = self.proguard {
            let class_name = decl.fully_qualified_name.as_deref();
            if let Some(confidence_boost) = proguard.get_confidence_for(class_name, &decl.name) {
                if confidence_boost >= 1.0 {
                    dc.confidence = Confidence::Confirmed;
                    dc.message = format!("{} (confirmed by R8/ProGuard)", dc.message);
                    return dc;
                } else if confidence_boost >= 0.8 {
                    dc.confidence = Confidence::High;
                }
            }
        }

        // Then check coverage data
        if let Some(ref coverage) = self.coverage {
            return self.enhance_single(dc, coverage);
        }

        // No enhancement data - use heuristics
        dc.confidence = self.estimate_confidence(decl);
        dc
    }

    fn enhance_single(&self, mut dc: DeadCode, coverage: &CoverageData) -> DeadCode {
        let decl = &dc.declaration;

        // Check if this declaration appears in coverage data
        let coverage_status = match decl.kind {
            DeclarationKind::Class | DeclarationKind::Object | DeclarationKind::Interface => {
                self.check_class_coverage(decl, coverage)
            }
            DeclarationKind::Function | DeclarationKind::Method => {
                self.check_method_coverage(decl, coverage)
            }
            DeclarationKind::Property | DeclarationKind::Field => {
                self.check_line_coverage(decl, coverage)
            }
            _ => CoverageStatus::Unknown,
        };

        match coverage_status {
            CoverageStatus::NeverExecuted => {
                // Runtime confirms this is dead code
                dc.runtime_confirmed = true;
                dc.confidence = Confidence::Confirmed;
                dc.message = format!("{} (confirmed by runtime coverage)", dc.message);
            }
            CoverageStatus::Executed => {
                // Runtime shows this WAS executed - false positive from static analysis
                // This shouldn't normally happen, but could with dynamic dispatch
                dc.confidence = Confidence::Low;
                dc.message = format!(
                    "{} (but was executed at runtime - may be dynamically called)",
                    dc.message
                );
            }
            CoverageStatus::PartiallyExecuted => {
                // Some parts executed, some not
                dc.confidence = Confidence::Medium;
            }
            CoverageStatus::Unknown => {
                // Not in coverage data - use heuristics
                dc.confidence = self.estimate_confidence(decl);
            }
        }

        dc
    }

    fn check_class_coverage(&self, decl: &Declaration, coverage: &CoverageData) -> CoverageStatus {
        // Build fully qualified name
        let fqn = self.build_class_fqn(decl);

        if coverage.covered_classes.contains(&fqn) {
            return CoverageStatus::Executed;
        }
        if coverage.uncovered_classes.contains(&fqn) {
            return CoverageStatus::NeverExecuted;
        }

        // Try variations of the name
        let simple_name = &decl.name;
        if coverage
            .covered_classes
            .iter()
            .any(|c| c.ends_with(simple_name))
        {
            return CoverageStatus::Executed;
        }
        if coverage
            .uncovered_classes
            .iter()
            .any(|c| c.ends_with(simple_name))
        {
            return CoverageStatus::NeverExecuted;
        }

        CoverageStatus::Unknown
    }

    fn check_method_coverage(&self, decl: &Declaration, coverage: &CoverageData) -> CoverageStatus {
        // Use fully qualified name if available
        if let Some(fqn) = &decl.fully_qualified_name {
            if coverage.covered_methods.contains(fqn) {
                return CoverageStatus::Executed;
            }
            if coverage.uncovered_methods.contains(fqn) {
                return CoverageStatus::NeverExecuted;
            }
        }

        // Try just the method name for top-level functions or partial matches
        let method_name = &decl.name;
        if coverage
            .covered_methods
            .iter()
            .any(|m| m.ends_with(&format!(".{}", method_name)))
        {
            return CoverageStatus::Executed;
        }
        if coverage
            .uncovered_methods
            .iter()
            .any(|m| m.ends_with(&format!(".{}", method_name)))
        {
            return CoverageStatus::NeverExecuted;
        }

        // Also try the simple name (for top-level functions like in LCOV)
        if coverage.covered_methods.contains(method_name) {
            return CoverageStatus::Executed;
        }
        if coverage.uncovered_methods.contains(method_name) {
            return CoverageStatus::NeverExecuted;
        }

        CoverageStatus::Unknown
    }

    fn check_line_coverage(&self, decl: &Declaration, coverage: &CoverageData) -> CoverageStatus {
        let file_path = &decl.location.file;
        let line = decl.location.line as u32;

        match coverage.is_line_covered(file_path, line) {
            Some(true) => CoverageStatus::Executed,
            Some(false) => CoverageStatus::NeverExecuted,
            None => CoverageStatus::Unknown,
        }
    }

    fn build_class_fqn(&self, decl: &Declaration) -> String {
        // Use fully qualified name if available, otherwise just the name
        decl.fully_qualified_name
            .clone()
            .unwrap_or_else(|| decl.name.clone())
    }

    fn estimate_confidence(&self, decl: &Declaration) -> Confidence {
        // Heuristics for confidence when no coverage data available
        match decl.kind {
            // Private members are less likely to be dynamically called
            DeclarationKind::Function | DeclarationKind::Method
                if decl.visibility == Visibility::Private =>
            {
                Confidence::High
            }
            // Internal members somewhat less likely
            DeclarationKind::Function | DeclarationKind::Method
                if decl.visibility == Visibility::Internal =>
            {
                Confidence::Medium
            }
            // Public methods could be called via reflection/dynamic dispatch
            DeclarationKind::Function | DeclarationKind::Method => Confidence::Medium,
            // Properties with getters could be accessed via reflection
            DeclarationKind::Property => Confidence::Medium,
            // Classes could be instantiated via reflection
            DeclarationKind::Class | DeclarationKind::Object => Confidence::Medium,
            // Parameters, imports, etc. - high confidence
            DeclarationKind::Parameter | DeclarationKind::Import => Confidence::High,
            // Default
            _ => Confidence::Medium,
        }
    }

    /// Find code that is statically reachable but never executed at runtime
    ///
    /// This is the "dynamic unreachable" code - code that passes static analysis
    /// but is never actually used in practice.
    pub fn find_runtime_dead_code(
        &self,
        graph: &Graph,
        reachable: &HashSet<crate::graph::DeclarationId>,
    ) -> Vec<DeadCode> {
        let Some(ref coverage) = self.coverage else {
            return Vec::new();
        };

        let mut dead_code = Vec::new();

        for decl in graph.declarations() {
            // Skip if already found by static analysis
            if !reachable.contains(&decl.id) {
                continue;
            }

            let coverage_status = match decl.kind {
                DeclarationKind::Class | DeclarationKind::Object => {
                    self.check_class_coverage(decl, coverage)
                }
                DeclarationKind::Function | DeclarationKind::Method => {
                    self.check_method_coverage(decl, coverage)
                }
                _ => continue, // Only report classes and methods for runtime analysis
            };

            if coverage_status == CoverageStatus::NeverExecuted {
                let mut dc = DeadCode::new(decl.clone(), DeadCodeIssue::Unreferenced)
                    .with_confidence(Confidence::High)
                    .with_runtime_confirmed(true);

                dc.message = format!(
                    "{} '{}' is reachable but never executed at runtime",
                    decl.kind.display_name(),
                    decl.name
                );

                dead_code.push(dc);
            }
        }

        dead_code
    }
}

impl Default for HybridAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoverageStatus {
    Executed,
    NeverExecuted,
    PartiallyExecuted,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{DeclarationId, Language, Location};
    use std::path::PathBuf;

    fn make_test_decl(name: &str, kind: DeclarationKind) -> Declaration {
        Declaration::new(
            DeclarationId::new(PathBuf::from("test.kt"), 0, 10),
            name.to_string(),
            kind,
            Location::new(PathBuf::from("test.kt"), 1, 1, 0, 10),
            Language::Kotlin,
        )
    }

    #[test]
    fn test_no_coverage_defaults_to_medium() {
        let analyzer = HybridAnalyzer::new();
        let dead = vec![DeadCode::new(
            make_test_decl("MyClass", DeclarationKind::Class),
            DeadCodeIssue::Unreferenced,
        )];

        let enhanced = analyzer.enhance_findings(dead);
        assert_eq!(enhanced[0].confidence, Confidence::Medium);
    }

    #[test]
    fn test_coverage_confirms_dead() {
        let mut coverage = CoverageData::new();
        coverage.uncovered_classes.insert("MyClass".to_string());

        let analyzer = HybridAnalyzer::new().with_coverage(coverage);
        let dead = vec![DeadCode::new(
            make_test_decl("MyClass", DeclarationKind::Class),
            DeadCodeIssue::Unreferenced,
        )];

        let enhanced = analyzer.enhance_findings(dead);
        assert_eq!(enhanced[0].confidence, Confidence::Confirmed);
        assert!(enhanced[0].runtime_confirmed);
    }
}
