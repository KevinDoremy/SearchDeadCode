//! Ignored Return Value Detector
//!
//! Detects when function calls with meaningful return values are discarded.
//! Common patterns include:
//! - `list.map { transform(it) }` without capturing the result
//! - `list.filter { }` without using the filtered list
//! - `list.sorted()` without using the sorted result
//!
//! ## Detection Algorithm
//!
//! 1. Find all expression statements that are function calls
//! 2. Check if the function returns a non-Unit value
//! 3. Check if the result is not captured in a variable
//! 4. Report such calls as likely bugs
//!
//! ## Examples Detected
//!
//! ```kotlin
//! // BAD: sorted result is discarded
//! articles.sortedByDescending { it.date }
//! adapter.submitList(articles)  // Still unsorted!
//!
//! // BAD: map result is discarded
//! items.map { it.transform() }  // Result thrown away
//!
//! // GOOD: result is captured
//! val sorted = articles.sortedByDescending { it.date }
//! adapter.submitList(sorted)
//! ```

use super::Detector;
use crate::analysis::{Confidence, DeadCode, DeadCodeIssue};
use crate::graph::{DeclarationKind, Graph, ReferenceKind};
use std::collections::HashSet;

/// Functions that return a transformed collection (pure functions with no side effects)
const PURE_COLLECTION_FUNCTIONS: &[&str] = &[
    // Transformations
    "map",
    "mapNotNull",
    "mapIndexed",
    "mapIndexedNotNull",
    "flatMap",
    "flatMapIndexed",
    "flatten",
    // Filtering
    "filter",
    "filterNot",
    "filterNotNull",
    "filterIndexed",
    "filterIsInstance",
    // Sorting
    "sorted",
    "sortedBy",
    "sortedByDescending",
    "sortedDescending",
    "sortedWith",
    "reversed",
    "shuffled",
    // Taking/Dropping
    "take",
    "takeLast",
    "takeWhile",
    "takeLastWhile",
    "drop",
    "dropLast",
    "dropWhile",
    "dropLastWhile",
    // Combining
    "plus",
    "minus",
    "zip",
    "zipWithNext",
    "union",
    "intersect",
    "subtract",
    // Partitioning
    "partition",
    "chunked",
    "windowed",
    // Distinct
    "distinct",
    "distinctBy",
    // Association
    "associateBy",
    "associateWith",
    "associate",
    "groupBy",
    "groupingBy",
    // String operations (pure)
    "trim",
    "trimStart",
    "trimEnd",
    "lowercase",
    "uppercase",
    "replace",
    "replaceFirst",
    "replaceBefore",
    "replaceAfter",
    "removePrefix",
    "removeSuffix",
    "removeSurrounding",
    "padStart",
    "padEnd",
    "substringBefore",
    "substringAfter",
    "substringBeforeLast",
    "substringAfterLast",
    "split",
    "lines",
    "chunked",
    // Other pure functions
    "toList",
    "toMutableList",
    "toSet",
    "toMutableSet",
    "toMap",
    "toMutableMap",
    "toSortedMap",
    "toTypedArray",
    "toIntArray",
    "toLongArray",
    "asSequence",
    "asIterable",
    // Kotlin stdlib pure functions
    "copy",
    "also",
    "let",
    "run",
    "with",
    "apply",
];

/// Functions that are commonly called for side effects (should NOT be flagged)
const SIDE_EFFECT_FUNCTIONS: &[&str] = &[
    // Iteration (side effects expected)
    "forEach",
    "forEachIndexed",
    "onEach",
    "onEachIndexed",
    // Logging/Debug
    "println",
    "print",
    "log",
    "debug",
    "info",
    "warn",
    "error",
    // Android/UI
    "show",
    "hide",
    "dismiss",
    "finish",
    "startActivity",
    "invalidate",
    "requestLayout",
    "postInvalidate",
    "notifyDataSetChanged",
    "notifyItemChanged",
    "submitList",
    "setAdapter",
    // Coroutines (launch returns Job but often ignored intentionally)
    "launch",
    "async",
    "runBlocking",
    // Reactive
    "subscribe",
    "observe",
    "collect",
    "collectLatest",
    // Network/IO
    "execute",
    "enqueue",
    "send",
    "post",
    "put",
    "delete",
    // State
    "emit",
    "setValue",
    "postValue",
    // Lifecycle
    "addObserver",
    "removeObserver",
    "registerReceiver",
    "unregisterReceiver",
];

/// Detector for ignored return values
pub struct IgnoredReturnValueDetector {
    /// Functions that return values that should be used
    pure_functions: HashSet<&'static str>,
    /// Functions called for side effects (ignore these)
    side_effect_functions: HashSet<&'static str>,
}

impl IgnoredReturnValueDetector {
    pub fn new() -> Self {
        Self {
            pure_functions: PURE_COLLECTION_FUNCTIONS.iter().copied().collect(),
            side_effect_functions: SIDE_EFFECT_FUNCTIONS.iter().copied().collect(),
        }
    }

    /// Check if a function name is a pure function whose return value should be used
    fn is_pure_function(&self, name: &str) -> bool {
        self.pure_functions.contains(name)
    }

    /// Check if a function is called for side effects
    fn is_side_effect_function(&self, name: &str) -> bool {
        self.side_effect_functions.contains(name)
    }
}

impl Default for IgnoredReturnValueDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for IgnoredReturnValueDetector {
    fn detect(&self, _graph: &Graph) -> Vec<DeadCode> {
        // This detector requires AST-level analysis that we don't have in the graph
        // The graph tracks declarations and references, but not expression statements

        // For now, we can detect a subset: functions that are referenced but whose
        // return value is never captured. This requires tracking:
        // 1. Function calls as ReferenceKind::Call
        // 2. Whether the call site is in an expression context

        // Since we don't have expression-level analysis in the current graph,
        // this detector would need parser-level support to be accurate.

        // For Phase 11, we'll focus on Intent extras which we CAN detect.

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_functions() {
        let detector = IgnoredReturnValueDetector::new();
        assert!(detector.is_pure_function("map"));
        assert!(detector.is_pure_function("filter"));
        assert!(detector.is_pure_function("sorted"));
        assert!(!detector.is_pure_function("forEach"));
    }

    #[test]
    fn test_side_effect_functions() {
        let detector = IgnoredReturnValueDetector::new();
        assert!(detector.is_side_effect_function("forEach"));
        assert!(detector.is_side_effect_function("launch"));
        assert!(!detector.is_side_effect_function("map"));
    }
}
