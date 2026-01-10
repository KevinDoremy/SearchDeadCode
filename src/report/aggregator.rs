//! Smart aggregation and deduplication of issues
//!
//! Groups similar issues to reduce noise in output

use crate::analysis::{DeadCode, DeadCodeIssue, Severity};
use std::collections::HashMap;
use std::path::PathBuf;

/// Aggregated group of similar issues
#[derive(Debug, Clone)]
pub struct IssueGroup {
    /// The issue type/rule
    pub issue: DeadCodeIssue,
    /// Severity level
    pub severity: Severity,
    /// All items in this group
    pub items: Vec<DeadCode>,
    /// Short description for the group
    pub description: String,
}

impl IssueGroup {
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Get items grouped by file
    pub fn by_file(&self) -> HashMap<PathBuf, Vec<&DeadCode>> {
        let mut map: HashMap<PathBuf, Vec<&DeadCode>> = HashMap::new();
        for item in &self.items {
            map.entry(item.declaration.location.file.clone())
                .or_default()
                .push(item);
        }
        map
    }
}

/// Aggregation result
#[derive(Debug)]
pub struct AggregatedResults {
    /// Issues grouped by rule
    pub by_rule: Vec<IssueGroup>,
    /// Issues grouped by category
    pub by_category: HashMap<String, Vec<IssueGroup>>,
    /// Total count
    pub total: usize,
}

/// Aggregator for grouping and deduplicating issues
pub struct Aggregator;

impl Aggregator {
    pub fn new() -> Self {
        Self
    }

    /// Aggregate issues into groups
    pub fn aggregate(&self, dead_code: Vec<DeadCode>) -> AggregatedResults {
        let total = dead_code.len();

        // Group by rule
        let mut rule_map: HashMap<String, Vec<DeadCode>> = HashMap::new();
        for item in dead_code.clone() {
            rule_map
                .entry(item.issue.code().to_string())
                .or_default()
                .push(item);
        }

        // Convert to IssueGroups
        let mut by_rule: Vec<IssueGroup> = rule_map
            .into_iter()
            .map(|(_code, items)| {
                let first = items.first().unwrap();
                IssueGroup {
                    issue: first.issue.clone(),
                    severity: first.severity.clone(),
                    description: Self::group_description(&first.issue),
                    items,
                }
            })
            .collect();

        // Sort by count descending
        by_rule.sort_by(|a, b| b.count().cmp(&a.count()));

        // Group by category
        let by_category = self.group_by_category(&by_rule);

        AggregatedResults {
            by_rule,
            by_category,
            total,
        }
    }

    /// Get a short description for a rule
    fn group_description(issue: &DeadCodeIssue) -> String {
        match issue {
            // Dead code issues
            DeadCodeIssue::Unreferenced => "Unreferenced declarations".to_string(),
            DeadCodeIssue::UnusedImport => "Unused imports".to_string(),
            DeadCodeIssue::UnusedParameter => "Unused parameters".to_string(),
            DeadCodeIssue::AssignOnly => "Assign-only variables".to_string(),
            DeadCodeIssue::DeadBranch => "Dead branches".to_string(),
            DeadCodeIssue::RedundantOverride => "Redundant overrides".to_string(),
            DeadCodeIssue::RedundantPublic => "Redundant public modifiers".to_string(),
            DeadCodeIssue::UnusedEnumCase => "Unused enum cases".to_string(),
            DeadCodeIssue::UnusedSealedVariant => "Unused sealed variants".to_string(),
            DeadCodeIssue::WriteOnlyPreference => "Write-only preferences".to_string(),
            DeadCodeIssue::WriteOnlyDao => "Write-only DAOs".to_string(),
            DeadCodeIssue::DuplicateImport => "Duplicate imports".to_string(),
            DeadCodeIssue::RedundantNullInit => "Redundant null init".to_string(),
            DeadCodeIssue::RedundantThis => "Redundant this".to_string(),
            DeadCodeIssue::RedundantParentheses => "Redundant parentheses".to_string(),
            DeadCodeIssue::PreferIsEmpty => "Prefer isEmpty()".to_string(),

            // Architecture patterns
            DeadCodeIssue::DeepInheritance => "Deep inheritance hierarchies".to_string(),
            DeadCodeIssue::EventBusPattern => "EventBus @Subscribe usage".to_string(),
            DeadCodeIssue::GlobalMutableState => "Global mutable state".to_string(),
            DeadCodeIssue::SingleImplInterface => "Single-implementation interfaces".to_string(),
            DeadCodeIssue::LegacyDependency => "Legacy dependencies".to_string(),
            DeadCodeIssue::ExcessiveFeatureToggles => "Excessive feature toggles".to_string(),

            // Kotlin patterns
            DeadCodeIssue::HeavyViewModel => "Heavy ViewModels".to_string(),
            DeadCodeIssue::GlobalScopeUsage => "GlobalScope usage".to_string(),
            DeadCodeIssue::LateinitAbuse => "Excessive lateinit".to_string(),
            DeadCodeIssue::ScopeFunctionChaining => "Scope function chaining".to_string(),
            DeadCodeIssue::NullabilityOverload => "Excessive null handling".to_string(),
            DeadCodeIssue::ReflectionOveruse => "Reflection overuse".to_string(),
            DeadCodeIssue::LongParameterList => "Long parameter lists".to_string(),
            DeadCodeIssue::ComplexCondition => "Complex conditions".to_string(),
            DeadCodeIssue::StringLiteralDuplication => "Duplicated string literals".to_string(),

            // Performance patterns
            DeadCodeIssue::MemoryLeakRisk => "Memory leak risks".to_string(),
            DeadCodeIssue::LongMethod => "Long methods".to_string(),
            DeadCodeIssue::LargeClass => "Large classes".to_string(),
            DeadCodeIssue::CollectionWithoutSequence => "Collections without asSequence()".to_string(),
            DeadCodeIssue::ObjectAllocationInLoop => "Object allocation in loops".to_string(),

            // Android patterns
            DeadCodeIssue::MutableStateExposed => "Exposed mutable state".to_string(),
            DeadCodeIssue::ViewLogicInViewModel => "View/Context in ViewModel".to_string(),
            DeadCodeIssue::MissingUseCase => "Missing UseCase layer".to_string(),
            DeadCodeIssue::NestedCallback => "Nested callbacks".to_string(),
            DeadCodeIssue::HardcodedDispatcher => "Hardcoded Dispatchers".to_string(),
            DeadCodeIssue::UnclosedResource => "Unclosed resources".to_string(),
            DeadCodeIssue::MainThreadDatabase => "Main thread database access".to_string(),
            DeadCodeIssue::WakeLockAbuse => "WakeLock issues".to_string(),
            DeadCodeIssue::AsyncTaskUsage => "AsyncTask usage (deprecated)".to_string(),
            DeadCodeIssue::InitOnDraw => "Allocations in onDraw()".to_string(),

            // Compose patterns
            DeadCodeIssue::StateWithoutRemember => "State without remember".to_string(),
            DeadCodeIssue::LaunchedEffectWithoutKey => "LaunchedEffect without key".to_string(),
            DeadCodeIssue::BusinessLogicInComposable => "Business logic in Composable".to_string(),
            DeadCodeIssue::NavControllerPassing => "NavController passing".to_string(),
        }
    }

    /// Get category for a rule
    pub fn category_for_issue(issue: &DeadCodeIssue) -> &'static str {
        match issue {
            DeadCodeIssue::Unreferenced
            | DeadCodeIssue::UnusedImport
            | DeadCodeIssue::UnusedParameter
            | DeadCodeIssue::AssignOnly
            | DeadCodeIssue::DeadBranch
            | DeadCodeIssue::RedundantOverride
            | DeadCodeIssue::RedundantPublic
            | DeadCodeIssue::UnusedEnumCase
            | DeadCodeIssue::UnusedSealedVariant
            | DeadCodeIssue::WriteOnlyPreference
            | DeadCodeIssue::WriteOnlyDao
            | DeadCodeIssue::DuplicateImport
            | DeadCodeIssue::RedundantNullInit
            | DeadCodeIssue::RedundantThis
            | DeadCodeIssue::RedundantParentheses
            | DeadCodeIssue::PreferIsEmpty => "Dead Code",

            DeadCodeIssue::DeepInheritance
            | DeadCodeIssue::EventBusPattern
            | DeadCodeIssue::GlobalMutableState
            | DeadCodeIssue::SingleImplInterface
            | DeadCodeIssue::LegacyDependency
            | DeadCodeIssue::ExcessiveFeatureToggles => "Architecture",

            DeadCodeIssue::HeavyViewModel
            | DeadCodeIssue::GlobalScopeUsage
            | DeadCodeIssue::LateinitAbuse
            | DeadCodeIssue::ScopeFunctionChaining
            | DeadCodeIssue::NullabilityOverload
            | DeadCodeIssue::ReflectionOveruse
            | DeadCodeIssue::LongParameterList
            | DeadCodeIssue::ComplexCondition
            | DeadCodeIssue::StringLiteralDuplication => "Kotlin",

            DeadCodeIssue::MemoryLeakRisk
            | DeadCodeIssue::LongMethod
            | DeadCodeIssue::LargeClass
            | DeadCodeIssue::CollectionWithoutSequence
            | DeadCodeIssue::ObjectAllocationInLoop => "Performance",

            DeadCodeIssue::MutableStateExposed
            | DeadCodeIssue::ViewLogicInViewModel
            | DeadCodeIssue::MissingUseCase
            | DeadCodeIssue::NestedCallback
            | DeadCodeIssue::HardcodedDispatcher
            | DeadCodeIssue::UnclosedResource
            | DeadCodeIssue::MainThreadDatabase
            | DeadCodeIssue::WakeLockAbuse
            | DeadCodeIssue::AsyncTaskUsage
            | DeadCodeIssue::InitOnDraw => "Android",

            DeadCodeIssue::StateWithoutRemember
            | DeadCodeIssue::LaunchedEffectWithoutKey
            | DeadCodeIssue::BusinessLogicInComposable
            | DeadCodeIssue::NavControllerPassing => "Compose",
        }
    }

    fn group_by_category(&self, by_rule: &[IssueGroup]) -> HashMap<String, Vec<IssueGroup>> {
        let mut map: HashMap<String, Vec<IssueGroup>> = HashMap::new();

        for group in by_rule {
            let category = Self::category_for_issue(&group.issue).to_string();
            map.entry(category).or_default().push(group.clone());
        }

        map
    }
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the analysis results
#[derive(Debug, Default)]
pub struct ResultStats {
    pub total_issues: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub confirmed: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub by_category: HashMap<String, usize>,
    pub by_rule: HashMap<String, usize>,
    pub files_affected: usize,
}

impl ResultStats {
    pub fn from_dead_code(dead_code: &[DeadCode]) -> Self {
        use crate::analysis::{Confidence, Severity};

        let mut stats = Self::default();
        stats.total_issues = dead_code.len();

        let mut files = std::collections::HashSet::new();

        for item in dead_code {
            // Severity
            match item.severity {
                Severity::Error => stats.errors += 1,
                Severity::Warning => stats.warnings += 1,
                Severity::Info => stats.infos += 1,
            }

            // Confidence
            match item.confidence {
                Confidence::Confirmed => stats.confirmed += 1,
                Confidence::High => stats.high += 1,
                Confidence::Medium => stats.medium += 1,
                Confidence::Low => stats.low += 1,
            }

            // Category
            let category = Aggregator::category_for_issue(&item.issue);
            *stats.by_category.entry(category.to_string()).or_default() += 1;

            // Rule
            *stats
                .by_rule
                .entry(item.issue.code().to_string())
                .or_default() += 1;

            // Files
            files.insert(item.declaration.location.file.clone());
        }

        stats.files_affected = files.len();
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_mapping() {
        assert_eq!(
            Aggregator::category_for_issue(&DeadCodeIssue::Unreferenced),
            "Dead Code"
        );
        assert_eq!(
            Aggregator::category_for_issue(&DeadCodeIssue::GlobalMutableState),
            "Architecture"
        );
        assert_eq!(
            Aggregator::category_for_issue(&DeadCodeIssue::ViewLogicInViewModel),
            "Android"
        );
    }
}
