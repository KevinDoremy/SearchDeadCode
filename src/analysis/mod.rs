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
