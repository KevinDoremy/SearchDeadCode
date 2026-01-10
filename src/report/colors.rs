//! Centralized color scheme for consistent output formatting
//!
//! Based on Rust compiler diagnostics design (RFC 1644)

use colored::{ColoredString, Colorize};

/// Confidence level indicators and colors
pub struct ConfidenceIndicator;

impl ConfidenceIndicator {
    /// Confirmed - safe to act on
    pub fn confirmed() -> ColoredString {
        "✓".green().bold()
    }

    /// High confidence - very likely correct
    pub fn high() -> ColoredString {
        "!".yellow().bold()
    }

    /// Medium confidence - review recommended
    pub fn medium() -> ColoredString {
        "?".dimmed()
    }

    /// Low confidence - may be false positive
    pub fn low() -> ColoredString {
        "~".dimmed().italic()
    }

    /// Get indicator for confidence level
    pub fn for_level(confidence: &crate::analysis::Confidence, runtime_confirmed: bool) -> ColoredString {
        if runtime_confirmed {
            return Self::confirmed();
        }
        match confidence {
            crate::analysis::Confidence::Confirmed => Self::confirmed(),
            crate::analysis::Confidence::High => Self::high(),
            crate::analysis::Confidence::Medium => Self::medium(),
            crate::analysis::Confidence::Low => Self::low(),
        }
    }
}

/// Structural element colors
pub struct StructureColors;

impl StructureColors {
    /// File path header
    pub fn file_path(text: &str) -> ColoredString {
        text.cyan().bold()
    }

    /// Line/column numbers
    pub fn location(text: &str) -> ColoredString {
        text.dimmed()
    }

    /// Rule/issue code (e.g., AP001, DC001)
    pub fn rule_code(text: &str) -> ColoredString {
        text.magenta()
    }

    /// Declaration/symbol name
    pub fn symbol_name(text: &str) -> ColoredString {
        text.white().bold()
    }

    /// Category headers
    pub fn category(text: &str) -> ColoredString {
        text.cyan().bold()
    }

    /// Count/statistics numbers
    pub fn count(text: &str) -> ColoredString {
        text.white().bold()
    }
}

/// Severity symbols for compact display
pub struct SeveritySymbol;

impl SeveritySymbol {
    pub fn error() -> &'static str {
        "✖"
    }

    pub fn warning() -> &'static str {
        "⚠"
    }

    pub fn info() -> &'static str {
        "ℹ"
    }

    pub fn colored(severity: &crate::analysis::Severity) -> ColoredString {
        match severity {
            crate::analysis::Severity::Error => Self::error().red().bold(),
            crate::analysis::Severity::Warning => Self::warning().yellow(),
            crate::analysis::Severity::Info => Self::info().blue(),
        }
    }
}

/// Bar chart characters for summary display
pub struct ChartChars;

impl ChartChars {
    pub const FILLED: char = '█';
    pub const EMPTY: char = '░';

    /// Create a progress bar string
    pub fn bar(percentage: f64, width: usize) -> String {
        let filled = ((percentage / 100.0) * width as f64).round() as usize;
        let empty = width.saturating_sub(filled);
        format!(
            "{}{}",
            Self::FILLED.to_string().repeat(filled),
            Self::EMPTY.to_string().repeat(empty)
        )
    }
}

/// Box drawing characters for verbose mode
pub struct BoxChars;

impl BoxChars {
    /// Heavy separator line
    pub fn heavy_line(width: usize) -> String {
        "━".repeat(width)
    }

    /// Light separator line
    pub fn light_line(width: usize) -> String {
        "─".repeat(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bar_chart() {
        assert_eq!(ChartChars::bar(50.0, 10), "█████░░░░░");
        assert_eq!(ChartChars::bar(100.0, 10), "██████████");
        assert_eq!(ChartChars::bar(0.0, 10), "░░░░░░░░░░");
        assert_eq!(ChartChars::bar(25.0, 20), "█████░░░░░░░░░░░░░░░");
    }

    #[test]
    fn test_heavy_line() {
        assert_eq!(BoxChars::heavy_line(5), "━━━━━");
    }
}
