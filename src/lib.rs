//! SearchDeadCode - Fast dead code detection for Android (Kotlin/Java)
//!
//! This library provides static analysis capabilities to detect unused code
//! in Android projects written in Kotlin and Java.
//!
//! # Architecture
//!
//! The analysis pipeline consists of:
//! 1. **File Discovery** - Find all .kt, .java, and .xml files
//! 2. **Parsing** - Parse source files using tree-sitter
//! 3. **Graph Building** - Build a reference graph of declarations
//! 4. **Entry Point Detection** - Identify Android entry points
//! 5. **Reachability Analysis** - Find unreachable code
//! 6. **Reporting** - Output results in various formats

pub mod analysis;
pub mod config;
pub mod coverage;
pub mod discovery;
pub mod graph;
pub mod parser;
pub mod proguard;
pub mod refactor;
pub mod report;

pub use analysis::{
    Confidence, DeadCode, EntryPointDetector, HybridAnalyzer, ReachabilityAnalyzer,
};
pub use config::Config;
pub use coverage::{parse_coverage_file, parse_coverage_files, CoverageData, CoverageParser};
pub use discovery::FileFinder;
pub use graph::{Declaration, DeclarationKind, Graph, Reference};
pub use proguard::{ProguardUsage, UsageEntryKind};
pub use refactor::SafeDeleter;
pub use report::{ReportFormat, Reporter};
