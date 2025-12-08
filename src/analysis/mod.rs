// Analysis module - some types and variants reserved for future use
#![allow(dead_code)]

mod cycles;
mod deep;
pub mod detectors;
mod enhanced;
mod entry_points;
mod hybrid;
mod reachability;
pub mod resources;

pub use cycles::CycleDetector;
pub use deep::DeepAnalyzer;
pub use enhanced::EnhancedAnalyzer;
pub use entry_points::EntryPointDetector;
pub use hybrid::HybridAnalyzer;
pub use reachability::ReachabilityAnalyzer;
pub use resources::ResourceDetector;

use crate::graph::Declaration;

/// Confidence level for dead code detection
///
/// Combines static analysis with optional runtime coverage data
/// to provide confidence scores for dead code findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    /// Low confidence - static analysis only, may have dynamic dispatch
    Low,
    /// Medium confidence - static analysis with some supporting evidence
    Medium,
    /// High confidence - both static and dynamic analysis confirm
    High,
    /// Confirmed - runtime coverage explicitly shows never executed
    Confirmed,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::Low => "low",
            Confidence::Medium => "medium",
            Confidence::High => "high",
            Confidence::Confirmed => "confirmed",
        }
    }

    /// Score from 0.0 to 1.0 for sorting/filtering
    pub fn score(&self) -> f64 {
        match self {
            Confidence::Low => 0.25,
            Confidence::Medium => 0.50,
            Confidence::High => 0.75,
            Confidence::Confirmed => 1.0,
        }
    }
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Represents a piece of dead code detected by analysis
#[derive(Debug, Clone)]
pub struct DeadCode {
    /// The declaration that is dead/unused
    pub declaration: Declaration,

    /// The kind of dead code issue
    pub issue: DeadCodeIssue,

    /// Severity level
    pub severity: Severity,

    /// Confidence level based on analysis type
    pub confidence: Confidence,

    /// Additional context or suggestions
    pub message: String,

    /// Whether runtime coverage data confirmed this is unused
    pub runtime_confirmed: bool,
}

impl DeadCode {
    pub fn new(declaration: Declaration, issue: DeadCodeIssue) -> Self {
        let severity = issue.default_severity();
        let message = issue.default_message(&declaration);

        Self {
            declaration,
            issue,
            severity,
            confidence: Confidence::Medium, // Default for static-only analysis
            message,
            runtime_confirmed: false,
        }
    }

    pub fn with_message(mut self, message: String) -> Self {
        self.message = message;
        self
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_runtime_confirmed(mut self, confirmed: bool) -> Self {
        self.runtime_confirmed = confirmed;
        if confirmed {
            self.confidence = Confidence::Confirmed;
        }
        self
    }
}

/// Types of dead code issues
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadCodeIssue {
    /// Declaration is never referenced
    Unreferenced,

    /// Property is assigned but never read
    AssignOnly,

    /// Parameter is never used
    UnusedParameter,

    /// Import is never used
    UnusedImport,

    /// Enum case is never used
    UnusedEnumCase,

    /// Public visibility is unnecessary (only used internally)
    RedundantPublic,

    /// Code branch can never be executed
    DeadBranch,

    /// Sealed class variant is never instantiated
    UnusedSealedVariant,

    /// Override only calls super (no additional behavior)
    RedundantOverride,

    /// SharedPreferences key is written but never read
    WriteOnlyPreference,

    /// Room DAO method writes data but the DAO has no read queries
    WriteOnlyDao,

    /// Import statement appears multiple times
    DuplicateImport,

    /// Nullable variable explicitly initialized to null (redundant)
    RedundantNullInit,

    /// Unnecessary 'this.' reference where not needed for disambiguation
    RedundantThis,

    /// Unnecessary parentheses around expression
    RedundantParentheses,

    /// Using size == 0 instead of isEmpty()
    PreferIsEmpty,

    // ==========================================================================
    // Anti-Pattern Detectors (inspired by common Android code smells)
    // ==========================================================================

    /// Kotlin object with mutable public properties (global mutable state)
    GlobalMutableState,

    /// Deep inheritance chain (e.g., 3+ levels of Base classes)
    DeepInheritance,

    /// Interface with only one implementation (unnecessary abstraction)
    SingleImplInterface,

    /// EventBus or similar global event pattern detected
    EventBusPattern,

    /// Legacy/deprecated dependency detected
    LegacyDependency,

    /// Excessive feature toggles (code smell)
    ExcessiveFeatureToggles,

    /// ViewModel with too many dependencies (God ViewModel)
    HeavyViewModel,

    /// GlobalScope usage in coroutines (memory leak risk)
    GlobalScopeUsage,

    /// Excessive lateinit properties (initialization smell)
    LateinitAbuse,

    /// Excessive scope function chaining (readability issue)
    ScopeFunctionChaining,
}

impl DeadCodeIssue {
    pub fn default_severity(&self) -> Severity {
        match self {
            DeadCodeIssue::Unreferenced => Severity::Warning,
            DeadCodeIssue::AssignOnly => Severity::Warning,
            DeadCodeIssue::UnusedParameter => Severity::Info,
            DeadCodeIssue::UnusedImport => Severity::Info,
            DeadCodeIssue::UnusedEnumCase => Severity::Warning,
            DeadCodeIssue::RedundantPublic => Severity::Info,
            DeadCodeIssue::DeadBranch => Severity::Warning,
            DeadCodeIssue::UnusedSealedVariant => Severity::Warning,
            DeadCodeIssue::RedundantOverride => Severity::Info,
            DeadCodeIssue::WriteOnlyPreference => Severity::Warning,
            DeadCodeIssue::WriteOnlyDao => Severity::Warning,
            DeadCodeIssue::DuplicateImport => Severity::Warning,
            DeadCodeIssue::RedundantNullInit => Severity::Info,
            DeadCodeIssue::RedundantThis => Severity::Info,
            DeadCodeIssue::RedundantParentheses => Severity::Info,
            DeadCodeIssue::PreferIsEmpty => Severity::Info,
            DeadCodeIssue::GlobalMutableState => Severity::Warning,
            DeadCodeIssue::DeepInheritance => Severity::Warning,
            DeadCodeIssue::SingleImplInterface => Severity::Info,
            DeadCodeIssue::EventBusPattern => Severity::Warning,
            DeadCodeIssue::LegacyDependency => Severity::Warning,
            DeadCodeIssue::ExcessiveFeatureToggles => Severity::Warning,
            DeadCodeIssue::HeavyViewModel => Severity::Warning,
            DeadCodeIssue::GlobalScopeUsage => Severity::Warning,
            DeadCodeIssue::LateinitAbuse => Severity::Info,
            DeadCodeIssue::ScopeFunctionChaining => Severity::Info,
        }
    }

    pub fn default_message(&self, decl: &Declaration) -> String {
        match self {
            DeadCodeIssue::Unreferenced => {
                format!("{} '{}' is never used", decl.kind.display_name(), decl.name)
            }
            DeadCodeIssue::AssignOnly => {
                format!(
                    "{} '{}' is assigned but never read",
                    decl.kind.display_name(),
                    decl.name
                )
            }
            DeadCodeIssue::UnusedParameter => {
                format!("Parameter '{}' is never used", decl.name)
            }
            DeadCodeIssue::UnusedImport => {
                format!("Import '{}' is never used", decl.name)
            }
            DeadCodeIssue::UnusedEnumCase => {
                format!("Enum case '{}' is never used", decl.name)
            }
            DeadCodeIssue::RedundantPublic => {
                format!(
                    "{} '{}' could be private (only used internally)",
                    decl.kind.display_name(),
                    decl.name
                )
            }
            DeadCodeIssue::DeadBranch => "This code branch can never be executed".to_string(),
            DeadCodeIssue::UnusedSealedVariant => {
                format!("Sealed variant '{}' is never instantiated", decl.name)
            }
            DeadCodeIssue::RedundantOverride => {
                format!(
                    "Override '{}' may be redundant (only calls super)",
                    decl.name
                )
            }
            DeadCodeIssue::WriteOnlyPreference => {
                format!(
                    "SharedPreferences key '{}' is written but never read",
                    decl.name
                )
            }
            DeadCodeIssue::WriteOnlyDao => {
                format!(
                    "DAO method '{}' writes data but the DAO has no read queries",
                    decl.name
                )
            }
            DeadCodeIssue::DuplicateImport => {
                format!("Import '{}' is duplicated", decl.name)
            }
            DeadCodeIssue::RedundantNullInit => {
                format!(
                    "Nullable {} '{}' is explicitly initialized to null (default value)",
                    decl.kind.display_name(),
                    decl.name
                )
            }
            DeadCodeIssue::RedundantThis => {
                format!(
                    "Unnecessary 'this.' reference for '{}' (no disambiguation needed)",
                    decl.name
                )
            }
            DeadCodeIssue::RedundantParentheses => {
                "Redundant parentheses around expression".to_string()
            }
            DeadCodeIssue::PreferIsEmpty => {
                format!(
                    "Prefer isEmpty()/isNotEmpty() instead of size/length comparison for '{}'",
                    decl.name
                )
            }
            DeadCodeIssue::GlobalMutableState => {
                format!(
                    "Object '{}' has mutable public properties (global mutable state is an anti-pattern)",
                    decl.name
                )
            }
            DeadCodeIssue::DeepInheritance => {
                format!(
                    "Class '{}' has deep inheritance chain (prefer composition over inheritance)",
                    decl.name
                )
            }
            DeadCodeIssue::SingleImplInterface => {
                format!(
                    "Interface '{}' has only one implementation (consider removing the interface)",
                    decl.name
                )
            }
            DeadCodeIssue::EventBusPattern => {
                format!(
                    "'{}' uses EventBus pattern (consider more structured communication)",
                    decl.name
                )
            }
            DeadCodeIssue::LegacyDependency => {
                format!(
                    "'{}' is a legacy/deprecated dependency (consider migrating)",
                    decl.name
                )
            }
            DeadCodeIssue::ExcessiveFeatureToggles => {
                format!(
                    "'{}' has excessive feature toggles (simplify branching logic)",
                    decl.name
                )
            }
            DeadCodeIssue::HeavyViewModel => {
                format!(
                    "ViewModel '{}' has too many dependencies (consider splitting responsibilities)",
                    decl.name
                )
            }
            DeadCodeIssue::GlobalScopeUsage => {
                format!(
                    "'{}' uses GlobalScope (use viewModelScope or lifecycleScope instead)",
                    decl.name
                )
            }
            DeadCodeIssue::LateinitAbuse => {
                format!(
                    "'{}' has excessive lateinit properties (consider constructor injection or lazy)",
                    decl.name
                )
            }
            DeadCodeIssue::ScopeFunctionChaining => {
                format!(
                    "'{}' has excessive scope function chaining (simplify for readability)",
                    decl.name
                )
            }
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            DeadCodeIssue::Unreferenced => "DC001",
            DeadCodeIssue::AssignOnly => "DC002",
            DeadCodeIssue::UnusedParameter => "DC003",
            DeadCodeIssue::UnusedImport => "DC004",
            DeadCodeIssue::UnusedEnumCase => "DC005",
            DeadCodeIssue::RedundantPublic => "DC006",
            DeadCodeIssue::DeadBranch => "DC007",
            DeadCodeIssue::UnusedSealedVariant => "DC008",
            DeadCodeIssue::RedundantOverride => "DC009",
            DeadCodeIssue::WriteOnlyPreference => "DC010",
            DeadCodeIssue::WriteOnlyDao => "DC011",
            DeadCodeIssue::DuplicateImport => "DC012",
            DeadCodeIssue::RedundantNullInit => "DC013",
            DeadCodeIssue::RedundantThis => "DC014",
            DeadCodeIssue::RedundantParentheses => "DC015",
            DeadCodeIssue::PreferIsEmpty => "DC016",
            DeadCodeIssue::GlobalMutableState => "AP001",
            DeadCodeIssue::DeepInheritance => "AP002",
            DeadCodeIssue::SingleImplInterface => "AP003",
            DeadCodeIssue::EventBusPattern => "AP004",
            DeadCodeIssue::LegacyDependency => "AP005",
            DeadCodeIssue::ExcessiveFeatureToggles => "AP006",
            DeadCodeIssue::HeavyViewModel => "AP007",
            DeadCodeIssue::GlobalScopeUsage => "AP008",
            DeadCodeIssue::LateinitAbuse => "AP009",
            DeadCodeIssue::ScopeFunctionChaining => "AP010",
        }
    }
}

/// Severity levels for dead code issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
