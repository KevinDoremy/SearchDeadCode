use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use colored::Colorize;
use miette::Result;
use std::path::{Path, PathBuf};
use tracing::info;

mod analysis;
mod baseline;
mod cache;
mod config;
mod coverage;
mod discovery;
mod graph;
mod interactive;
mod lsp;
mod mcp;
mod parser;
mod proguard;
mod refactor;
mod report;
mod tui;
mod watch;

use proguard::{ProguardUsage, ReportGenerator};

use analysis::detectors::{
    // Phase 5: Android-Specific (AP026-AP030)
    AsyncTaskUsageDetector,
    // Phase 6: Compose-Specific (AP031-AP034)
    BusinessLogicInComposableDetector,
    // Phase 2: Performance & Memory (AP011-AP015)
    CollectionWithoutSequenceDetector,
    // Phase 4: Kotlin-Specific (AP021-AP025)
    ComplexConditionDetector,
    DeadBranchDetector,
    // Anti-pattern detectors (AP001-AP006)
    DeepInheritanceDetector,
    // Core detectors
    Detector,
    DuplicateImportDetector,
    EventBusPatternDetector,
    GlobalMutableStateDetector,
    // Phase 1: Kotlin patterns (AP007-AP010)
    GlobalScopeUsageDetector,
    // Phase 3: Architecture & Design (AP016-AP020)
    HardcodedDispatcherDetector,
    HeavyViewModelDetector,
    InitOnDrawDetector,
    LargeClassDetector,
    LateinitAbuseDetector,
    LaunchedEffectWithoutKeyDetector,
    LongMethodDetector,
    LongParameterListDetector,
    MainThreadDatabaseDetector,
    MemoryLeakRiskDetector,
    MissingUseCaseDetector,
    MutableStateExposedDetector,
    NavControllerPassingDetector,
    NestedCallbackDetector,
    NullabilityOverloadDetector,
    ObjectAllocationInLoopDetector,
    PreferIsEmptyDetector,
    RedundantNullInitDetector,
    RedundantOverrideDetector,
    RedundantParenthesesDetector,
    RedundantPublicDetector,
    RedundantThisDetector,
    ReflectionOveruseDetector,
    ScopeFunctionChainingDetector,
    SingleImplInterfaceDetector,
    StateWithoutRememberDetector,
    StringLiteralDuplicationDetector,
    UnclosedResourceDetector,
    UnusedIntentExtraDetector,
    UnusedParamDetector,
    UnusedSealedVariantDetector,
    ViewLogicInViewModelDetector,
    WakeLockAbuseDetector,
    WriteOnlyDetector,
};
use analysis::{
    Confidence, CycleDetector, DeepAnalyzer, EnhancedAnalyzer, EntryPointDetector, HybridAnalyzer,
    ReachabilityAnalyzer, ResourceDetector,
};
use config::Config;
use coverage::parse_coverage_files;
use discovery::FileFinder;
use graph::{GraphBuilder, ParallelGraphBuilder};
use report::Reporter;

/// SearchDeadCode - Fast dead code detection for Android (Kotlin/Java)
#[derive(Parser, Debug)]
#[command(name = "searchdeadcode")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the project directory to analyze
    #[arg(help_heading = "Target & config", default_value = ".")]
    path: PathBuf,

    /// Path to configuration file
    #[arg(help_heading = "Target & config", short, long)]
    config: Option<PathBuf>,

    /// Target directories to analyze (can be specified multiple times)
    #[arg(help_heading = "Target & config", short, long)]
    target: Vec<PathBuf>,

    /// Patterns to exclude (can be specified multiple times)
    #[arg(help_heading = "Target & config", short, long)]
    exclude: Vec<String>,

    /// Patterns to retain - never report as dead (can be specified multiple times)
    #[arg(help_heading = "Target & config", short, long)]
    retain: Vec<String>,

    /// Output format (defaults to report.format from .deadcode.yml, else terminal)
    #[arg(help_heading = "Output & formats", short, long, value_enum)]
    format: Option<OutputFormat>,

    /// Output file (for json/sarif formats)
    #[arg(help_heading = "Output & formats", short, long)]
    output: Option<PathBuf>,

    /// Enable safe delete mode
    #[arg(help_heading = "Refactoring & writing", long)]
    delete: bool,

    /// Interactive mode for deletions (confirm each)
    #[arg(help_heading = "Refactoring & writing", long)]
    interactive: bool,

    /// Dry run - show what would be deleted without making changes
    #[arg(help_heading = "Refactoring & writing", long)]
    dry_run: bool,

    /// Generate undo script
    #[arg(help_heading = "Refactoring & writing", long)]
    undo_script: Option<PathBuf>,

    /// Detection types to run (comma-separated)
    #[arg(help_heading = "Detectors", long)]
    detect: Option<String>,

    /// Explain why a symbol (simple name or FQN) is considered dead or alive
    #[arg(help_heading = "Specialized views", long, value_name = "SYMBOL")]
    explain: Option<String>,

    /// Show the retention chain keeping a symbol alive (inverse of --explain)
    #[arg(help_heading = "Specialized views", long, value_name = "SYMBOL")]
    why_alive: Option<String>,

    /// Show everything that falls if this symbol is deleted (exclusive dependents)
    #[arg(help_heading = "Specialized views", long, value_name = "SYMBOL")]
    kill_list: Option<String>,

    /// Group dead code findings into connected, deletable clusters
    #[arg(help_heading = "Specialized views", long)]
    clusters: bool,

    /// Dead islands: groups of declarations that reference only each other
    /// and are referenced by nothing else — invisible to per-symbol counting
    #[arg(help_heading = "Specialized views", long)]
    islands: bool,

    /// Only the findings safe to delete blind: whole cluster dead, every
    /// member low risk
    #[arg(help_heading = "Specialized views", long)]
    quick_wins: bool,

    /// Apply zero-risk fixes automatically (unused imports). Combine with
    /// --dry-run to preview. Always writes an undo script.
    #[arg(help_heading = "Refactoring & writing", long)]
    fix: bool,

    /// PR-scoped analysis: judge only files changed since this git ref,
    /// reporting only symbols provably unreferenced project-wide
    #[arg(help_heading = "Filtering & confidence", long, value_name = "REF")]
    changed_since: Option<String>,

    /// Migration diff: OLD=NEW worlds (package prefix or path fragment).
    /// Lists old-world symbols deletable at the flip and the blockers.
    #[arg(help_heading = "Specialized views", long, value_name = "OLD=NEW")]
    compare: Option<String>,

    /// Attribute a shared module's symbols to their real consumers:
    /// unreferenced, internal-only, or used by which directories
    #[arg(help_heading = "Specialized views", long, value_name = "MODULE")]
    module_usage: Option<String>,

    /// Generate a commented .deadcode.yml matching the project's shape
    /// (source sets, DI framework, exclusions) and exit
    #[arg(help_heading = "Target & config", long)]
    init: bool,

    /// Feature flag cleanup: name (or key) of the flag being settled
    #[arg(help_heading = "Specialized views", long, value_name = "NAME")]
    flag: Option<String>,

    /// Assumed final behavior of --flag
    #[arg(
        help_heading = "Specialized views",
        long,
        value_enum,
        default_value = "enabled"
    )]
    behavior: FlagBehavior,

    /// Coverage files (JaCoCo XML, Kover XML, or LCOV format)
    /// Can be specified multiple times for merged coverage
    #[arg(help_heading = "Filtering & confidence", long, value_name = "FILE")]
    coverage: Vec<PathBuf>,

    /// Minimum confidence level to report (low, medium, high, confirmed).
    /// Defaults to medium, or to the --profile choice
    #[arg(help_heading = "Filtering & confidence", long)]
    min_confidence: Option<String>,

    /// Preset for an audience: ci (the whole pipeline setup in one flag) or
    /// explore (everything down to low confidence)
    #[arg(help_heading = "Filtering & confidence", long, value_enum)]
    profile: Option<Profile>,

    /// Only show findings confirmed by runtime coverage
    #[arg(help_heading = "Detectors", long)]
    runtime_only: bool,

    /// Include runtime-dead code (reachable but never executed)
    #[arg(help_heading = "Detectors", long)]
    include_runtime_dead: bool,

    /// Detect and report zombie code cycles (mutually dependent dead code)
    #[arg(help_heading = "Detectors", long)]
    detect_cycles: bool,

    /// ProGuard/R8 usage.txt file for enhanced detection
    /// This file lists code that R8 determined is unused
    #[arg(help_heading = "Filtering & confidence", long, value_name = "FILE")]
    proguard_usage: Option<PathBuf>,

    /// Generate a filtered dead code report from ProGuard usage.txt
    /// Filters out generated code (Dagger, Hilt, _Factory, _Impl, etc.)
    #[arg(help_heading = "Specialized views", long, value_name = "FILE")]
    generate_report: Option<PathBuf>,

    /// Package prefix to include in report (e.g., "com.example")
    /// Only classes matching this prefix will be included
    #[arg(help_heading = "Specialized views", long, value_name = "PREFIX")]
    report_package: Option<String>,

    /// Enable parallel processing for faster analysis (enabled by default)
    #[arg(help_heading = "Detectors", long, default_value = "true", default_missing_value = "true", num_args = 0..=1, action = clap::ArgAction::Set)]
    parallel: bool,

    /// Enable enhanced detection mode with ProGuard cross-validation
    #[arg(help_heading = "Detectors", long)]
    enhanced: bool,

    /// Enable deep analysis mode - more aggressive detection (enabled by default)
    /// Does not auto-mark class members as reachable
    /// Detects unused members even in reachable classes
    #[arg(help_heading = "Detectors", long, default_value = "true", default_missing_value = "true", num_args = 0..=1, action = clap::ArgAction::Set)]
    deep: bool,

    /// Enable unused parameter detection (enabled by default)
    /// Finds function parameters that are declared but never used
    #[arg(help_heading = "Detectors", long, default_value = "true", action = clap::ArgAction::Set)]
    unused_params: bool,

    /// Enable unused resource detection (off by default - slower)
    /// Finds Android resources (strings, colors, etc.) that are never referenced
    #[arg(help_heading = "Detectors", long)]
    unused_resources: bool,

    /// Enable write-only variable detection (enabled by default)
    /// Finds variables that are assigned but never read
    #[arg(help_heading = "Detectors", long, default_value = "true", action = clap::ArgAction::Set)]
    write_only: bool,

    /// Enable unused sealed variant detection (enabled by default)
    /// Finds sealed class variants that are never instantiated
    #[arg(help_heading = "Detectors", long, default_value = "true", action = clap::ArgAction::Set)]
    sealed_variants: bool,

    /// Enable redundant override detection (off by default - can be intentional)
    /// Finds method overrides that only call super
    #[arg(help_heading = "Detectors", long)]
    redundant_overrides: bool,

    /// Style lints: redundant this, doubled parentheses, size==0 (DC014-16)
    #[arg(help_heading = "Detectors", long)]
    style: bool,

    /// Annotate each finding with its last author and date (one git call per finding)
    #[arg(help_heading = "Filtering & confidence", long)]
    blame: bool,

    /// With --baseline: fail on new issues (exit 3) and rewrite the
    /// baseline downward on progress — the count can only decrease
    #[arg(help_heading = "Filtering & confidence", long)]
    ratchet: bool,

    /// Rank files by deletable lines instead of reporting findings
    #[arg(help_heading = "Output & formats", long, value_name = "N")]
    top_files: Option<usize>,

    /// Rank findings by deletability: lines x confidence / risk
    #[arg(help_heading = "Output & formats", long)]
    score: bool,

    /// Treat the public API as alive (its consumers live outside this
    /// repo) — report internal deadness only
    #[arg(help_heading = "Filtering & confidence", long)]
    library_mode: bool,

    /// Check the config against the repo's reality (dead globs,
    /// unknown entry points, missing targets) and exit
    #[arg(help_heading = "Specialized views", long)]
    doctor: bool,

    /// List Gradle modules nobody depends on (whole-module deletion
    /// candidates) and exit
    #[arg(help_heading = "Specialized views", long)]
    dead_modules: bool,

    /// List Gradle dependencies declared in build files but never
    /// imported by any source file, then exit
    #[arg(help_heading = "Specialized views", long)]
    unused_deps: bool,

    /// Show how many declarations each retention annotation keeps
    /// alive, broadest first, and exit
    #[arg(help_heading = "Specialized views", long)]
    retention_audit: bool,

    /// List boolean Remote Config flags with their defaults and the
    /// ready-made --flag probe for each, then exit
    #[arg(help_heading = "Specialized views", long)]
    stale_flags: bool,

    /// Show Xxx/XxxV2-style pairs side by side with reference counts,
    /// then exit
    #[arg(help_heading = "Specialized views", long)]
    twins: bool,

    /// Split @Deprecated symbols into ready-to-delete (no references)
    /// and unfinished migrations (still referenced), then exit
    #[arg(help_heading = "Specialized views", long)]
    deprecated: bool,

    /// List exposed LiveData/StateFlow/SharedFlow properties nobody
    /// collects or observes, then exit
    #[arg(help_heading = "Specialized views", long)]
    unobserved: bool,

    /// List src/main symbols referenced only from other source sets
    /// (debug, flavors, tests) — dead in the release build — then exit
    #[arg(help_heading = "Specialized views", long)]
    debug_only: bool,

    /// List src/main symbols kept alive only by test source sets
    /// (delete symbol and tests together), then exit
    #[arg(help_heading = "Specialized views", long)]
    test_only: bool,

    /// Install a pre-commit hook running the fast diff mode, then exit
    #[arg(help_heading = "Specialized views", long)]
    install_hook: bool,

    /// List exact -keep rules naming project classes that no longer
    /// exist, then exit
    #[arg(help_heading = "Specialized views", long)]
    dead_keep_rules: bool,

    /// List assets/ files whose path or name appears nowhere in the
    /// sources, then exit
    #[arg(help_heading = "Specialized views", long)]
    unused_assets: bool,

    /// List string values declared in several modules (centralization
    /// candidates), then exit
    #[arg(help_heading = "Specialized views", long)]
    duplicate_strings: bool,

    /// List JavaBean properties whose getter nobody calls (write-only
    /// or fully dead groups), then exit
    #[arg(help_heading = "Specialized views", long)]
    dead_accessors: bool,

    /// List manifest permissions whose API family never appears in the
    /// code, then exit
    #[arg(help_heading = "Specialized views", long)]
    unused_permissions: bool,

    /// List classes whose every method forwards to the same delegate,
    /// then exit
    #[arg(help_heading = "Specialized views", long)]
    middlemen: bool,

    /// Replace the report with a per-module summary (count, top rule)
    #[arg(help_heading = "Output & formats", long)]
    by_module: bool,

    /// Replace the report with an A-F health grade per module
    #[arg(help_heading = "Output & formats", long)]
    health: bool,

    /// With --health: exit 3 when any module grades below this letter
    #[arg(help_heading = "Output & formats", long, value_name = "GRADE", value_parser = ["A", "B", "C", "D"], requires = "health")]
    min_grade: Option<String>,

    /// Replace the report with a paste-ready cleanup-PR description
    /// (stats, proof of death, residual risks)
    #[arg(help_heading = "Output & formats", long)]
    pr_description: bool,

    /// Fail when code references a symbol the --baseline judged dead
    /// (someone is resurrecting legacy), then exit
    #[arg(help_heading = "Specialized views", long)]
    necromancy: bool,

    /// Write a shields-style SVG badge with the dead-code percentage
    #[arg(help_heading = "Output & formats", long, value_name = "FILE")]
    badge: Option<PathBuf>,

    /// List DI modules whose every binding produces an unconsumed type,
    /// then exit
    #[arg(help_heading = "Specialized views", long)]
    dead_di_modules: bool,

    /// List @Serializable classes with zero incoming references (kept
    /// only by their annotation), then exit
    #[arg(help_heading = "Specialized views", long)]
    dead_serializables: bool,

    /// Write the reference graph to this file (.json or .dot), then exit
    #[arg(help_heading = "Specialized views", long, value_name = "FILE")]
    export_graph: Option<PathBuf>,

    /// A saved --export-graph JSON to answer queries from (no re-scan)
    #[arg(help_heading = "Baseline & cache", long, value_name = "FILE")]
    graph_file: Option<PathBuf>,

    /// Print who references this symbol, answered from --graph-file
    #[arg(help_heading = "Specialized views", long, value_name = "NAME")]
    refs_of: Option<String>,

    /// Serve MCP tools (refs_of, is_dead) over stdio from --graph-file
    #[arg(help_heading = "Specialized views", long)]
    mcp_serve: bool,

    /// Serve LSP diagnostics over stdio from --graph-file
    #[arg(help_heading = "Specialized views", long)]
    lsp_serve: bool,

    /// Full-screen findings triage (needs a terminal)
    #[arg(help_heading = "Specialized views", long)]
    tui: bool,

    /// Cross TODO remove / FIXME delete comments with the symbol's
    /// real reference count, then exit
    #[arg(help_heading = "Specialized views", long)]
    promises: bool,

    /// Exit 1 when findings remain after filtering (baseline included)
    /// — the scriptable CI gate. Implied by --profile ci
    ///
    /// `require_equals`: sans lui, `--fail-on-findings .` avale le chemin
    /// comme valeur du drapeau et le parsing échoue — et c'est exactement
    /// l'ordre dans lequel l'action GitHub construit sa commande. La forme
    /// nue reste valide, la désactivation s'écrit `--fail-on-findings=false`.
    #[arg(help_heading = "Filtering & confidence", long, num_args = 0..=1, default_missing_value = "true", require_equals = true, action = clap::ArgAction::Set)]
    fail_on_findings: Option<bool>,

    /// List Worker/JobService classes nobody ever enqueues, then exit
    #[arg(help_heading = "Specialized views", long)]
    unscheduled_workers: bool,

    /// Convert @Suppress("unused") annotations into entries of this
    /// baseline file (migration from Detekt-style triage), then exit
    #[arg(help_heading = "Specialized views", long, value_name = "FILE")]
    import_suppressions: Option<PathBuf>,

    /// Convert the Unused* entries of a detekt-baseline.xml into the
    /// baseline given by --baseline, then exit
    #[arg(
        help_heading = "Specialized views",
        long,
        value_name = "XML",
        requires = "baseline"
    )]
    import_detekt_baseline: Option<PathBuf>,

    /// List cache keys written but never read back, then exit
    #[arg(help_heading = "Specialized views", long)]
    write_only_caches: bool,

    /// List same-named functions with near-identical bodies across
    /// files (migration copy-paste; survives local renames), then exit
    #[arg(help_heading = "Specialized views", long)]
    near_twins: bool,

    /// After --delete: run this command (a compile, a test suite) and
    /// restore every touched file automatically when it fails
    #[arg(help_heading = "Refactoring & writing", long, value_name = "CMD")]
    verify_cmd: Option<String>,

    /// With --delete: skip confirmation prompts (the CI path)
    #[arg(help_heading = "Refactoring & writing", long)]
    yes: bool,

    /// With --delete --dry-run: write the would-be deletion as a
    /// unified diff, reviewable and applicable with git apply
    #[arg(help_heading = "Refactoring & writing", long, value_name = "FILE")]
    patch: Option<std::path::PathBuf>,

    /// Report only symbols that became dead since this git reference
    #[arg(help_heading = "Filtering & confidence", long, value_name = "REF")]
    diff_base: Option<String>,

    /// Enable unused Intent extra detection (enabled by default)
    /// Finds putExtra() keys that are never retrieved via getXxxExtra()
    #[arg(help_heading = "Detectors", long, default_value = "true", action = clap::ArgAction::Set)]
    unused_extras: bool,

    /// Enable write-only SharedPreferences detection (enabled by default)
    /// Finds SharedPreferences keys that are written but never read
    #[arg(help_heading = "Detectors", long, default_value = "true", action = clap::ArgAction::Set)]
    write_only_prefs: bool,

    /// Enable write-only Room DAO detection (enabled by default)
    /// Finds Room DAOs that have @Insert but no @Query methods
    #[arg(help_heading = "Detectors", long, default_value = "true", action = clap::ArgAction::Set)]
    write_only_dao: bool,

    /// Enable all anti-pattern detectors (AP001-AP034)
    /// Includes: architecture, performance, Kotlin, Android, and Compose patterns
    #[arg(help_heading = "Detectors", long)]
    anti_patterns: bool,

    /// Enable architecture anti-pattern detectors (AP001-AP006)
    /// Detects: deep inheritance, EventBus, global mutable state, single-impl interfaces
    #[arg(help_heading = "Detectors", long)]
    architecture_patterns: bool,

    /// Enable Kotlin anti-pattern detectors (AP007-AP010, AP021-AP025)
    /// Detects: GlobalScope, heavy ViewModel, lateinit abuse, scope function chaining,
    /// nullability overload, reflection overuse, long parameter lists, complex conditions
    #[arg(help_heading = "Detectors", long)]
    kotlin_patterns: bool,

    /// Enable performance anti-pattern detectors (AP011-AP015)
    /// Detects: memory leaks, long methods, large classes, collection inefficiencies, loop allocations
    #[arg(help_heading = "Detectors", long)]
    performance_patterns: bool,

    /// Enable Android-specific anti-pattern detectors (AP016-AP020, AP026-AP030)
    /// Detects: mutable state exposure, view logic in ViewModel, missing UseCase,
    /// nested callbacks, hardcoded dispatchers, unclosed resources, main thread DB,
    /// WakeLock abuse, AsyncTask usage, onDraw allocations
    #[arg(help_heading = "Detectors", long)]
    android_patterns: bool,

    /// Enable Compose-specific anti-pattern detectors (AP031-AP034)
    /// Detects: state without remember, LaunchedEffect without key, business logic in composables,
    /// NavController passing to children
    #[arg(help_heading = "Detectors", long)]
    compose_patterns: bool,

    /// Enable incremental analysis with caching (enabled by default, off under
    /// --profile ci). Skips re-parsing unchanged files for faster subsequent
    /// runs, at the cost of a cache file next to the project — 221 MB on a
    /// 9000-file repository, which is why a pipeline does not want it
    #[arg(help_heading = "Baseline & cache", long, default_missing_value = "true", num_args = 0..=1, action = clap::ArgAction::Set)]
    incremental: Option<bool>,

    /// Clear the analysis cache before running
    #[arg(help_heading = "Baseline & cache", long)]
    clear_cache: bool,

    /// Custom cache file path (default: .searchdeadcode-cache.json)
    #[arg(help_heading = "Baseline & cache", long, value_name = "FILE")]
    cache_path: Option<PathBuf>,

    /// Baseline file for ignoring existing issues
    /// New issues not in baseline will be reported
    #[arg(help_heading = "Baseline & cache", long, value_name = "FILE")]
    baseline: Option<PathBuf>,

    /// Generate a baseline file from current results
    #[arg(help_heading = "Baseline & cache", long, value_name = "FILE")]
    generate_baseline: Option<PathBuf>,

    /// List the entries of the --baseline file, then exit
    #[arg(help_heading = "Baseline & cache", long)]
    baseline_show: bool,

    /// Remove entries matching this name (or FQN) from the --baseline
    /// file, then exit
    #[arg(help_heading = "Baseline & cache", long, value_name = "NAME")]
    baseline_rm: Option<String>,

    /// Drop baseline entries whose finding no longer exists (resolved),
    /// rewriting the --baseline file
    #[arg(help_heading = "Baseline & cache", long)]
    baseline_prune: bool,

    /// Show baseline entries counted per rule (where the tool cries
    /// wolf the most), then exit
    #[arg(help_heading = "Baseline & cache", long)]
    baseline_stats: bool,

    /// Create one local git branch per dead top-level class, each with
    /// one commit carrying the proof of death, then exit
    #[arg(help_heading = "Specialized views", long)]
    batch_branches: bool,

    /// Watch mode - continuously monitor for changes
    #[arg(help_heading = "Specialized views", long)]
    watch: bool,

    /// Verbose output
    #[arg(help_heading = "Output & formats", short, long)]
    verbose: bool,

    /// Quiet mode - only output results
    #[arg(help_heading = "Output & formats", short, long)]
    quiet: bool,

    /// Generate shell completions
    #[arg(help_heading = "Target & config", long, value_name = "SHELL")]
    completions: Option<Shell>,

    /// Summary output - show statistics and top issues only
    #[arg(help_heading = "Output & formats", long)]
    summary: bool,

    /// Compact output - one line per issue
    #[arg(help_heading = "Output & formats", long)]
    compact: bool,

    /// Group results by: rule, category, severity, file
    #[arg(help_heading = "Output & formats", long, value_name = "MODE")]
    group_by: Option<String>,

    /// Expand all collapsed groups (show every issue)
    #[arg(help_heading = "Output & formats", long)]
    expand: bool,

    /// Expand a specific rule's issues (e.g., --expand-rule AP017)
    #[arg(help_heading = "Output & formats", long, value_name = "RULE", value_parser = ["DC001", "DC002", "DC003", "DC004", "DC005", "DC006", "DC007", "DC008", "DC009", "DC010", "DC011", "DC012", "DC013", "DC014", "DC015", "DC016", "DC017", "DC018", "DC019", "DC020", "DC021", "DC022", "AP001", "AP002", "AP003", "AP004", "AP005", "AP006", "AP007", "AP008", "AP009", "AP010", "AP011", "AP012", "AP013", "AP014", "AP015", "AP016", "AP017", "AP018", "AP019", "AP020", "AP021", "AP022", "AP023", "AP024", "AP025", "AP026", "AP027", "AP028", "AP029", "AP030", "AP031", "AP032", "AP033", "AP034"])]
    expand_rule: Option<String>,

    /// Number of top issues to show in summary mode
    #[arg(help_heading = "Output & formats", long, default_value = "10")]
    top: usize,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum Profile {
    /// Pipeline preset: exit 1 on findings, no cache file left in the
    /// workspace, and .deadcode-baseline.json picked up if the project
    /// committed one
    Ci,
    /// Everything down to low confidence — for humans digging
    Explore,
}

/// The conventional baseline name, the one `--profile ci` looks for. Same
/// spirit as detekt's `baseline.xml`: the file is the project's ground truth,
/// so a pipeline should not have to name it on the command line.
const CONVENTIONAL_BASELINE: &str = ".deadcode-baseline.json";

fn is_ci_profile(cli: &Cli) -> bool {
    matches!(cli.profile, Some(Profile::Ci))
}

/// Explicit flag first, then the profile preset, then medium.
///
/// `ci` deliberately does NOT raise the bar to `high`, which it used to do.
/// Measured on a 9000-file project: `high` sees 126 findings out of 2058, and
/// 79 of those 126 come from DC013, a cosmetic rule. A dead class someone just
/// pushed is reported at `medium` — so the strict preset could not catch the
/// one thing a pipeline gate exists for. Noise is the baseline's job, not the
/// threshold's: the baseline freezes what is already there and the gate then
/// fires on what the branch added.
fn resolve_min_confidence(cli: &Cli) -> String {
    cli.min_confidence.clone().unwrap_or_else(|| {
        match cli.profile {
            Some(Profile::Explore) => "low",
            Some(Profile::Ci) | None => "medium",
        }
        .to_string()
    })
}

/// A flag named for pipelines has to gate the pipeline. Explicit wins, so
/// `--profile ci --fail-on-findings=false` is still a way to look without
/// breaking the build.
fn resolve_fail_on_findings(cli: &Cli) -> bool {
    cli.fail_on_findings.unwrap_or_else(|| is_ci_profile(cli))
}

/// A CI checkout is fresh: the cache can teach it nothing, and it would leave
/// a large file in the workspace on every run. Explicit wins.
fn resolve_incremental(cli: &Cli) -> bool {
    cli.incremental.unwrap_or_else(|| !is_ci_profile(cli))
}

/// Explicit `--baseline` wins. Under the CI profile only, fall back to the
/// conventional file when it exists — a pipeline that committed one means it.
/// Never invented outside that profile: a missing baseline must stay a loud
/// mistake for anyone who typed the flag.
fn resolve_baseline(cli: &Cli) -> Option<PathBuf> {
    if let Some(explicit) = &cli.baseline {
        return Some(explicit.clone());
    }
    if !is_ci_profile(cli) {
        return None;
    }
    let root = if cli.path.is_dir() {
        cli.path.clone()
    } else {
        cli.path.parent().map(PathBuf::from).unwrap_or_default()
    };
    let candidate = root.join(CONVENTIONAL_BASELINE);
    candidate.is_file().then_some(candidate)
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
enum OutputFormat {
    #[default]
    Terminal,
    Compact,
    Json,
    Sarif,
    Html,
    Markdown,
    Reviewdog,
    Csv,
    Gitlab,
    Checkstyle,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default)]
enum FlagBehavior {
    #[default]
    Enabled,
    Disabled,
}

/// CLI flag first, then report.format from .deadcode.yml, then terminal
fn resolve_output_format(cli: &Cli, config: &Config) -> OutputFormat {
    // La table couvrait trois formats sur neuf : `report.format: gitlab` (ou
    // html, markdown, reviewdog, csv, checkstyle) dans .deadcode.yml rendait
    // du terminal sans un mot. Un format inconnu le dit maintenant, plutôt
    // que de laisser croire que la config a été lue.
    cli.format
        .clone()
        .unwrap_or(match config.report.format.as_str() {
            "compact" => OutputFormat::Compact,
            "json" => OutputFormat::Json,
            "sarif" => OutputFormat::Sarif,
            "html" => OutputFormat::Html,
            "markdown" => OutputFormat::Markdown,
            "reviewdog" => OutputFormat::Reviewdog,
            "csv" => OutputFormat::Csv,
            "gitlab" => OutputFormat::Gitlab,
            "checkstyle" => OutputFormat::Checkstyle,
            "terminal" | "" => OutputFormat::Terminal,
            other => {
                eprintln!(
                    "{}: unknown report.format '{other}' in config, using terminal",
                    "Warning".yellow()
                );
                OutputFormat::Terminal
            }
        })
}

/// Determine the report format from CLI options
fn determine_report_format(cli: &Cli, config: &Config) -> report::ReportFormat {
    // Explicit format flags take precedence
    if cli.summary {
        return report::ReportFormat::Summary;
    }

    if cli.compact {
        return report::ReportFormat::Compact;
    }

    if let Some(group_by) = &cli.group_by {
        let mode = group_by
            .parse::<report::GroupBy>()
            .unwrap_or(report::GroupBy::Rule);
        return report::ReportFormat::Grouped(mode);
    }

    // Fall back to --format, then to the config file
    match resolve_output_format(cli, config) {
        OutputFormat::Terminal => report::ReportFormat::Terminal,
        OutputFormat::Compact => report::ReportFormat::Compact,
        OutputFormat::Json => report::ReportFormat::Json,
        OutputFormat::Sarif => report::ReportFormat::Sarif,
        OutputFormat::Html => report::ReportFormat::Html,
        OutputFormat::Markdown => report::ReportFormat::Markdown,
        OutputFormat::Reviewdog => report::ReportFormat::Reviewdog,
        OutputFormat::Csv => report::ReportFormat::Csv,
        OutputFormat::Gitlab => report::ReportFormat::Gitlab,
        OutputFormat::Checkstyle => report::ReportFormat::Checkstyle,
    }
}

const HOOK_MARKER: &str = "installed by searchdeadcode --install-hook";

/// Write .git/hooks/pre-commit running the fast diff mode. Refuses to
/// clobber a hook it did not write; reinstalling over our own is fine.
fn install_pre_commit_hook(root: &Path) -> std::result::Result<PathBuf, String> {
    let git_dir = root.join(".git");
    if !git_dir.is_dir() {
        return Err(format!(
            "{} is not a git repository — nowhere to hang the hook",
            root.display()
        ));
    }
    let hooks_dir = git_dir.join("hooks");
    let hook_path = hooks_dir.join("pre-commit");
    if hook_path.exists() {
        let existing = std::fs::read_to_string(&hook_path).unwrap_or_default();
        if !existing.contains(HOOK_MARKER) {
            return Err(format!(
                "{} already exists and was not written by us — remove it first",
                hook_path.display()
            ));
        }
    }
    // --fail-on-findings arme la porte : sans lui le hook listait le code
    // mort puis laissait passer le commit — un garde-fou qui ne garde rien.
    let script = format!(
        "#!/bin/sh\n# {HOOK_MARKER}\n# Fast diff mode: only files changed since HEAD are analyzed.\nsearchdeadcode . --changed-since HEAD --quiet --fail-on-findings\n"
    );
    std::fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;
    std::fs::write(&hook_path, script).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(hook_path)
}

/// Find build/outputs/mapping/<variant>/usage.txt without descending
/// into build/ trees: walk shallow directories and probe the well-known
/// suffix from each. Release variants win (the shrunk build teams care
/// about), most-recent mtime breaks ties.
fn discover_usage_txt(root: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .max_depth(3)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !(name.starts_with('.') || name == "node_modules" || name == "build")
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
    {
        let mapping = entry.path().join("build/outputs/mapping");
        let Ok(variants) = std::fs::read_dir(&mapping) else {
            continue;
        };
        for variant in variants.filter_map(Result::ok) {
            let candidate = variant.path().join("usage.txt");
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort_by_key(|path| {
        let is_release = path
            .parent()
            .and_then(|v| v.file_name())
            .map(|n| n.to_string_lossy().to_lowercase().contains("release"))
            .unwrap_or(false);
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        (is_release, mtime)
    });
    candidates.pop()
}

/// The report-replacing flags the user passed, in dispatch order. Modifiers
/// (--behavior, --report-package), the stdio servers, --generate-report (falls
/// through to the normal report) and --watch (its own mode) are not conflicts,
/// so they are not listed.
fn active_wedges(cli: &Cli) -> Vec<&'static str> {
    let checks: [(bool, &'static str); 44] = [
        (cli.refs_of.is_some(), "--refs-of"),
        (cli.install_hook, "--install-hook"),
        (cli.unused_permissions, "--unused-permissions"),
        (cli.duplicate_strings, "--duplicate-strings"),
        (cli.unused_assets, "--unused-assets"),
        (cli.unused_deps, "--unused-deps"),
        (cli.dead_modules, "--dead-modules"),
        (cli.deprecated, "--deprecated"),
        (cli.unobserved, "--unobserved"),
        (cli.export_graph.is_some(), "--export-graph"),
        (cli.write_only_caches, "--write-only-caches"),
        (cli.import_suppressions.is_some(), "--import-suppressions"),
        (
            cli.import_detekt_baseline.is_some(),
            "--import-detekt-baseline",
        ),
        (cli.unscheduled_workers, "--unscheduled-workers"),
        (cli.promises, "--promises"),
        (cli.dead_serializables, "--dead-serializables"),
        (cli.dead_di_modules, "--dead-di-modules"),
        (cli.near_twins, "--near-twins"),
        (cli.middlemen, "--middlemen"),
        (cli.dead_accessors, "--dead-accessors"),
        (cli.necromancy, "--necromancy"),
        (cli.dead_keep_rules, "--dead-keep-rules"),
        (cli.test_only, "--test-only"),
        (cli.debug_only, "--debug-only"),
        (cli.twins, "--twins"),
        (cli.stale_flags, "--stale-flags"),
        (cli.retention_audit, "--retention-audit"),
        (cli.doctor, "--doctor"),
        (cli.explain.is_some(), "--explain"),
        (cli.why_alive.is_some(), "--why-alive"),
        (cli.flag.is_some(), "--flag"),
        (cli.module_usage.is_some(), "--module-usage"),
        (cli.compare.is_some(), "--compare"),
        (cli.kill_list.is_some(), "--kill-list"),
        (cli.top_files.is_some(), "--top-files"),
        (cli.by_module, "--by-module"),
        (cli.health, "--health"),
        (cli.pr_description, "--pr-description"),
        (cli.tui, "--tui"),
        (cli.batch_branches, "--batch-branches"),
        (cli.score, "--score"),
        (cli.quick_wins, "--quick-wins"),
        (cli.islands, "--islands"),
        (cli.clusters, "--clusters"),
    ];
    checks
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect()
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle shell completions
    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        let name = cmd.get_name().to_string();
        generate(shell, &mut cmd, name, &mut std::io::stdout());
        return Ok(());
    }

    // One wedge answers per run; without this warning the losers vanished
    // without a trace and looked like they had been scanned.
    let wedges = active_wedges(&cli);
    if wedges.len() > 1 && !cli.quiet {
        eprintln!(
            "{}: {} each replace the report; {} wins (dispatch order), the others are ignored",
            "Warning".yellow(),
            wedges.join(", "),
            wedges[0]
        );
    }

    // Generate a starter config and exit
    if cli.init {
        match config::generate_config(&cli.path) {
            Ok(path) => {
                println!("✅ Wrote {}", path);
                return Ok(());
            }
            Err(message) => {
                eprintln!("{}", message);
                std::process::exit(2);
            }
        }
    }

    // --lsp-serve: LSP stdio server over the saved graph — no scan
    if cli.lsp_serve {
        let Some(ref graph_path) = cli.graph_file else {
            eprintln!(
                "{}: --lsp-serve needs --graph-file <file> (from --export-graph)",
                "Error".red()
            );
            std::process::exit(2);
        };
        let saved = match report::graph_export::SavedGraph::load(graph_path) {
            Ok(saved) => saved,
            Err(e) => {
                eprintln!("{}: cannot read graph file: {}", "Error".red(), e);
                std::process::exit(2);
            }
        };
        lsp::serve(saved, graph_path, &cli.path).map_err(|e| miette::miette!(e))?;
        return Ok(());
    }

    // --mcp-serve: MCP stdio server over the saved graph — no scan
    if cli.mcp_serve {
        let Some(ref graph_path) = cli.graph_file else {
            eprintln!(
                "{}: --mcp-serve needs --graph-file <file> (from --export-graph)",
                "Error".red()
            );
            std::process::exit(2);
        };
        let saved = match report::graph_export::SavedGraph::load(graph_path) {
            Ok(saved) => saved,
            Err(e) => {
                eprintln!("{}: cannot read graph file: {}", "Error".red(), e);
                std::process::exit(2);
            }
        };
        mcp::serve(saved, graph_path, &cli.path).map_err(|e| miette::miette!(e))?;
        return Ok(());
    }

    // Graph queries answer from a saved file — no scan, no analysis
    if let Some(ref symbol) = cli.refs_of {
        let Some(ref graph_path) = cli.graph_file else {
            eprintln!(
                "{}: --refs-of needs --graph-file <file> (from --export-graph)",
                "Error".red()
            );
            std::process::exit(2);
        };
        let saved = match report::graph_export::SavedGraph::load(graph_path) {
            Ok(saved) => saved,
            Err(e) => {
                eprintln!("{}: cannot read graph file: {}", "Error".red(), e);
                std::process::exit(2);
            }
        };
        match saved.refs_of(symbol) {
            report::graph_export::QueryAnswer::UnknownSymbol => {
                println!("'{symbol}' is not in the graph");
            }
            report::graph_export::QueryAnswer::Referencers(refs) if refs.is_empty() => {
                println!("no references to '{symbol}'");
            }
            report::graph_export::QueryAnswer::Referencers(refs) => {
                println!("{}", format!("References to '{symbol}':").bold());
                for node in refs {
                    println!(
                        "  {} {:<25} {}  {}",
                        "○".dimmed(),
                        node.name,
                        node.kind,
                        format!("{}:{}", node.file, node.line).dimmed()
                    );
                }
            }
        }
        return Ok(());
    }

    // Install the packaged pre-commit hook and exit
    if cli.install_hook {
        match install_pre_commit_hook(&cli.path) {
            Ok(path) => {
                println!("✅ Installed pre-commit hook: {}", path.display());
                return Ok(());
            }
            Err(message) => {
                eprintln!("{}: {}", "Error".red(), message);
                std::process::exit(2);
            }
        }
    }

    // `resolve_baseline`, pas `cli.baseline` : sous --profile ci le
    // .deadcode-baseline.json conventionnel satisfait déjà ces deux modes —
    // exiger le drapeau explicite faisait échouer un job qui allait charger
    // ce fichier exact deux étapes plus loin.
    if cli.baseline_prune && resolve_baseline(&cli).is_none() {
        eprintln!(
            "{}: --baseline-prune needs --baseline <file> (or --profile ci with a committed .deadcode-baseline.json)",
            "Error".red()
        );
        std::process::exit(2);
    }

    if cli.necromancy && resolve_baseline(&cli).is_none() {
        eprintln!(
            "{}: --necromancy needs --baseline <file> — the corpses are recorded there",
            "Error".red()
        );
        std::process::exit(2);
    }

    // Baseline management (show/rm/stats) needs only the file, not an analysis
    if cli.baseline_show || cli.baseline_rm.is_some() || cli.baseline_stats {
        let Some(ref baseline_path) = cli.baseline else {
            eprintln!(
                "{}: baseline management needs --baseline <file>",
                "Error".red()
            );
            std::process::exit(2);
        };
        let mut loaded = match baseline::Baseline::load(baseline_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{}: cannot read baseline: {}", "Error".red(), e);
                std::process::exit(2);
            }
        };
        if let Some(ref target) = cli.baseline_rm {
            let before = loaded.issues.len();
            loaded
                .issues
                .retain(|fp| fp.name != *target && fp.fqn.as_deref() != Some(target.as_str()));
            let removed = before - loaded.issues.len();
            if removed == 0 {
                println!("no entry named '{target}' in the baseline — file untouched");
            } else {
                loaded.save(baseline_path).map_err(|e| miette::miette!(e))?;
                println!("removed {removed} entrie(s) named '{target}'");
            }
            return Ok(());
        }
        if cli.baseline_stats {
            let mut per_rule: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for fp in &loaded.issues {
                let rule = fp.rule.clone().unwrap_or_else(|| "unknown".to_string());
                *per_rule.entry(rule).or_default() += 1;
            }
            let mut rows: Vec<(String, usize)> = per_rule.into_iter().collect();
            rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            println!(
                "{}",
                format!("Baseline entries per rule ({} total):", loaded.issues.len()).bold()
            );
            for (rule, count) in rows {
                println!("  {:<10} {:>4}", rule, count);
            }
            println!(
                "{}",
                "  (the rules people baseline the most are where the tool cries wolf)".dimmed()
            );
            return Ok(());
        }
        println!(
            "{}",
            format!("Baseline: {} entrie(s)", loaded.issues.len()).bold()
        );
        for fp in &loaded.issues {
            println!(
                "  {} {:<30} {:<10} {}",
                "○".dimmed(),
                fp.name,
                fp.kind,
                format!("{}:{}", fp.file, fp.line).dimmed()
            );
        }
        return Ok(());
    }

    // Initialize logging
    init_logging(cli.verbose, cli.quiet);

    info!("SearchDeadCode v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = load_config(&cli)?;

    // Watch mode
    if cli.watch {
        run_watch_mode(&config, &cli)?;
    } else {
        // Run analysis once
        run_analysis(&config, &cli)?;
    }

    Ok(())
}

fn run_watch_mode(config: &Config, cli: &Cli) -> Result<()> {
    use watch::FileWatcher;

    let watcher = FileWatcher::new();

    // Clone what we need for the closure
    let config = config.clone();
    let cli_path = cli.path.clone();
    let cli_format = resolve_output_format(cli, &config);
    let cli_output = cli.output.clone();
    let cli_verbose = cli.verbose;
    let cli_quiet = cli.quiet;
    let cli_deep = cli.deep;
    let cli_parallel = cli.parallel;
    let cli_enhanced = cli.enhanced;
    let cli_detect_cycles = cli.detect_cycles;
    let cli_min_confidence = resolve_min_confidence(cli);
    let cli_baseline = cli.baseline.clone();
    let cli_coverage = cli.coverage.clone();
    let cli_proguard_usage = cli.proguard_usage.clone();

    watcher
        .watch(&cli.path, move || {
            // Suppress output for repeated runs except results
            if !cli_verbose {
                // Temporarily change log level
            }

            // Re-run analysis
            match run_analysis_internal(
                &config,
                &cli_path,
                cli_format.clone(),
                cli_output.clone(),
                cli_deep,
                cli_parallel,
                cli_enhanced,
                cli_detect_cycles,
                &cli_min_confidence,
                &cli_baseline,
                &cli_coverage,
                &cli_proguard_usage,
                cli_quiet,
            ) {
                Ok(_) => {
                    println!();
                    println!("{}", "✓ Analysis complete. Waiting for changes...".green());
                    true
                }
                Err(e) => {
                    eprintln!("{}: {}", "Analysis error".red(), e);
                    true // Continue watching
                }
            }
        })
        .map_err(|e| miette::miette!("Watch error: {}", e))?;

    Ok(())
}

/// Internal analysis function for watch mode
#[allow(clippy::too_many_arguments)]
fn run_analysis_internal(
    config: &Config,
    path: &std::path::Path,
    format: OutputFormat,
    output: Option<PathBuf>,
    deep: bool,
    parallel: bool,
    enhanced: bool,
    detect_cycles: bool,
    min_confidence: &str,
    baseline_path: &Option<PathBuf>,
    coverage_files: &[PathBuf],
    proguard_usage: &Option<PathBuf>,
    quiet: bool,
) -> Result<()> {
    use colored::Colorize;
    use std::time::Instant;

    let start_time = Instant::now();

    // Discover files
    let finder = FileFinder::new(config);
    let files = finder.find_files(path)?;

    if files.is_empty() {
        if !quiet {
            println!("{}", "No Kotlin or Java files found.".yellow());
        }
        return Ok(());
    }

    // Parse and build graph
    let graph = if parallel {
        let parallel_builder = ParallelGraphBuilder::new();
        parallel_builder.build_from_files(&files)?
    } else {
        let mut graph_builder = GraphBuilder::new();
        for file in &files {
            graph_builder.process_file(file)?;
        }
        graph_builder.build()
    };

    // Detect entry points
    let entry_detector = EntryPointDetector::new(config);
    let entry_points = entry_detector.detect(&graph, path)?;

    // Load ProGuard data if available
    let proguard_data = if let Some(ref usage_path) = proguard_usage {
        ProguardUsage::parse(usage_path).ok()
    } else {
        None
    };

    // Run reachability analysis
    let (dead_code, reachable) = if deep {
        let analyzer = DeepAnalyzer::new()
            .with_parallel(parallel)
            .with_unused_members(true);
        analyzer.analyze(&graph, &entry_points)
    } else if enhanced && proguard_data.is_some() {
        let mut analyzer = EnhancedAnalyzer::new();
        if let Some(pg) = proguard_data.clone() {
            analyzer = analyzer.with_proguard(pg);
        }
        analyzer.analyze(&graph, &entry_points)
    } else {
        let analyzer = ReachabilityAnalyzer::new();
        analyzer.find_unreachable_with_reachable(&graph, &entry_points)
    };

    // Load coverage data
    let coverage_data = if !coverage_files.is_empty() {
        parse_coverage_files(coverage_files).ok()
    } else {
        None
    };

    // Enhance findings
    let mut hybrid = HybridAnalyzer::new();
    if let Some(coverage) = coverage_data {
        hybrid = hybrid.with_coverage(coverage);
    }
    if let Some(proguard) = proguard_data {
        hybrid = hybrid.with_proguard(proguard);
    }

    let dead_code = hybrid.enhance_findings(dead_code);

    // Filter by confidence
    let min_conf = parse_confidence(min_confidence);
    let dead_code: Vec<_> = dead_code
        .into_iter()
        .filter(|dc| dc.confidence >= min_conf)
        .collect();

    // Apply baseline filter
    let dead_code = if let Some(ref bp) = baseline_path {
        match baseline::Baseline::load(bp) {
            Ok(baseline) => {
                let stats = baseline.stats(&dead_code, path);
                if !quiet {
                    eprintln!("{}", format!("📋 Baseline: {}", stats).cyan());
                }
                baseline
                    .filter_new(&dead_code, path)
                    .into_iter()
                    .cloned()
                    .collect()
            }
            Err(_) => dead_code,
        }
    } else {
        dead_code
    };

    // Detect cycles if requested
    if detect_cycles {
        let cycle_detector = CycleDetector::new();
        let cycle_stats = cycle_detector.get_cycle_stats(&graph, &reachable);
        if cycle_stats.has_cycles() && !quiet {
            println!(
                "{}",
                format!(
                    "🧟 {} dead cycles ({} declarations)",
                    cycle_stats.num_dead_cycles, cycle_stats.total_declarations_in_cycles
                )
                .yellow()
            );
        }
    }

    // Report results
    let report_format = match format {
        OutputFormat::Terminal => report::ReportFormat::Terminal,
        OutputFormat::Compact => report::ReportFormat::Compact,
        OutputFormat::Json => report::ReportFormat::Json,
        OutputFormat::Sarif => report::ReportFormat::Sarif,
        OutputFormat::Html => report::ReportFormat::Html,
        OutputFormat::Markdown => report::ReportFormat::Markdown,
        OutputFormat::Reviewdog => report::ReportFormat::Reviewdog,
        OutputFormat::Csv => report::ReportFormat::Csv,
        OutputFormat::Gitlab => report::ReportFormat::Gitlab,
        OutputFormat::Checkstyle => report::ReportFormat::Checkstyle,
    };
    let reporter = Reporter::new(report_format, output);
    reporter.report(&dead_code)?;

    // Print timing
    let elapsed = start_time.elapsed();
    if !quiet {
        println!(
            "{}",
            format!(
                "⏱  Analyzed {} files in {:.2}s",
                files.len(),
                elapsed.as_secs_f64()
            )
            .dimmed()
        );
    }

    Ok(())
}

fn init_logging(verbose: bool, quiet: bool) {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = if quiet {
        EnvFilter::new("error")
    } else if verbose {
        EnvFilter::new("debug")
    } else {
        // The styled progress lines already tell the story; raw logs are
        // developer detail, opt-in via --verbose
        EnvFilter::new("warn")
    };

    // stdout carries results only — logs go to stderr so JSON stays pipeable
    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}

fn load_config(cli: &Cli) -> Result<Config> {
    let mut config = if let Some(config_path) = &cli.config {
        Config::from_file(config_path)?
    } else {
        // Try to load from default locations
        Config::from_default_locations(&cli.path)?
    };

    // Module-level .deadcode.yml files merge their excludes, scoped to
    // their own directory — one root config never fits 49 modules
    config.merge_module_overrides(&cli.path);

    // Override with CLI arguments
    if !cli.target.is_empty() {
        config.targets = cli.target.clone();
    }
    if !cli.exclude.is_empty() {
        config.exclude.extend(cli.exclude.clone());
    }
    if !cli.retain.is_empty() {
        config.retain_patterns.extend(cli.retain.clone());
    }

    Ok(config)
}

/// Outermost entries of a declaration set with their estimated line total
/// A finding about something outside the code graph (a resource, an
/// intent extra, a prefs key): synthesize the declaration the reporter
/// needs so the finding flows through JSON/SARIF/baseline like any other.
#[allow(clippy::too_many_arguments)]
fn synthetic_finding(
    file: &std::path::Path,
    line: usize,
    name: &str,
    kind: graph::DeclarationKind,
    issue: analysis::DeadCodeIssue,
    message: String,
    confidence: analysis::Confidence,
) -> analysis::DeadCode {
    let decl = graph::Declaration::new(
        graph::DeclarationId::new(file.to_path_buf(), line * 1000, line * 1000 + name.len()),
        name.to_string(),
        kind,
        graph::Location::new(
            file.to_path_buf(),
            line,
            1,
            line * 1000,
            line * 1000 + name.len(),
        ),
        graph::Language::Kotlin,
    );
    analysis::DeadCode::new(decl, issue)
        .with_message(message)
        .with_confidence(confidence)
}

/// Platform shell for --verify-cmd
fn shell_command(cmd: &str) -> std::process::Command {
    #[cfg(windows)]
    {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", cmd]);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    }
}

/// Config checkup: a glob matching nothing or an unknown entry point
/// silently skews every run. Returns true when everything checks out.
fn run_doctor(config: &Config, graph: &graph::Graph, root: &std::path::Path) -> bool {
    if !root.join(".deadcode.yml").exists() {
        println!(
            "{}",
            "🩺 no .deadcode.yml — defaults in use, nothing to check".dimmed()
        );
        return true;
    }

    let mut problems = 0usize;

    for pattern in &config.exclude {
        match globset::Glob::new(pattern) {
            Ok(glob) => {
                let matcher = glob.compile_matcher();
                let hit = walkdir::WalkDir::new(root)
                    .into_iter()
                    .flatten()
                    .any(|entry| {
                        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
                        matcher.is_match(rel)
                    });
                if !hit {
                    println!("  ✗ exclude '{pattern}' matches nothing in the repo");
                    problems += 1;
                }
            }
            Err(e) => {
                println!("  ✗ exclude '{pattern}' is not a valid glob: {e}");
                problems += 1;
            }
        }
    }

    for name in &config.entry_points {
        if graph.find_by_fqn(name).is_none() && graph.find_by_name(name).is_empty() {
            println!("  ✗ entry point '{name}' is unknown to the graph");
            problems += 1;
        }
    }

    for target in &config.targets {
        let path = if target.is_absolute() {
            target.clone()
        } else {
            root.join(target)
        };
        if !path.exists() {
            println!("  ✗ target '{}' does not exist", target.display());
            problems += 1;
        }
    }

    if problems == 0 {
        println!("{}", "✓ .deadcode.yml matches the repo".green());
        true
    } else {
        println!(
            "{}",
            format!("🩺 {problems} problem(s) found — the analysis would run silently skewed")
                .yellow()
        );
        false
    }
}

/// One sortable number per finding: lines × confidence ÷ risk.
/// "Delete in this order."
fn print_score_ranking(dead_code: &[analysis::DeadCode], base: &std::path::Path) {
    if dead_code.is_empty() {
        println!("{}", "✓ Nothing to score — no findings".green());
        return;
    }

    let mut rows: Vec<(f64, usize, &analysis::DeadCode)> = dead_code
        .iter()
        .map(|dc| {
            let lines = std::fs::read(&dc.declaration.location.file)
                .ok()
                .map(|content| {
                    let end = dc.declaration.id.end.min(content.len());
                    let start = dc.declaration.id.start.min(end);
                    content[start..end].iter().filter(|b| **b == b'\n').count() + 1
                })
                .unwrap_or(1);
            let risk_divisor = match dc.risk {
                analysis::RiskLevel::Low => 1.0,
                analysis::RiskLevel::Medium => 2.0,
                analysis::RiskLevel::High => 4.0,
            };
            let score = lines as f64 * dc.confidence.score() / risk_divisor;
            (score, lines, dc)
        })
        .collect();
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    println!(
        "{}",
        "Deletability ranking (lines × confidence ÷ risk)".bold()
    );
    for (score, lines, dc) in rows {
        let rel = dc
            .declaration
            .location
            .file
            .strip_prefix(base)
            .unwrap_or(&dc.declaration.location.file);
        println!(
            "  {:>7.1}  {}  {}  {}",
            score,
            dc.declaration.name,
            format!("{}:{}", rel.display(), dc.declaration.location.line).dimmed(),
            format!("(~{lines}L, {}, risk {})", dc.confidence.as_str(), dc.risk).dimmed()
        );
    }
}

/// Rank files by deletable lines — the Monday-morning "where do I start"
/// One local branch per dead top-level class, one commit each, proof
/// in the message. Refuses on a dirty worktree, restores the starting
/// branch, never pushes anything.
fn run_batch_branches(
    root: &Path,
    dead_code: &[analysis::DeadCode],
) -> std::result::Result<usize, String> {
    let git = |args: &[&str]| -> std::result::Result<std::process::Output, String> {
        std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|e| e.to_string())
    };
    if !root.join(".git").is_dir() {
        return Err(format!("{} is not a git repository", root.display()));
    }
    // untracked files (the analysis cache among them) are harmless to
    // branch surgery — only tracked modifications block
    let status = git(&["status", "--porcelain", "--untracked-files=no"])?;
    if !String::from_utf8_lossy(&status.stdout).trim().is_empty() {
        return Err("the worktree has uncommitted changes — commit or stash first".to_string());
    }
    let head = git(&["rev-parse", "--abbrev-ref", "HEAD"])?;
    let original = String::from_utf8_lossy(&head.stdout).trim().to_string();

    let corpses: Vec<&analysis::DeadCode> = dead_code
        .iter()
        .filter(|dc| {
            dc.declaration.parent.is_none()
                && dc.declaration.kind.is_type()
                && dc.issue == analysis::DeadCodeIssue::Unreferenced
        })
        .collect();

    let mut created = 0usize;
    for corpse in corpses {
        let slug: String = corpse
            .declaration
            .name
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        if slug.is_empty() {
            continue;
        }
        let branch = format!("deadcode/{slug}");
        let checkout = git(&["checkout", "-q", "-b", &branch])?;
        if !checkout.status.success() {
            eprintln!(
                "{}: branch {branch} already exists — skipped",
                "Warning".yellow()
            );
            continue;
        }
        let deleter = refactor::SafeDeleter::new(false, false, None).with_assume_yes(true);
        let deleted = deleter.delete(std::slice::from_ref(corpse));
        if deleted.is_err() {
            let _ = git(&["checkout", "-q", &original]);
            let _ = git(&["branch", "-q", "-D", &branch]);
            continue;
        }
        let message = format!(
            "remove dead {} {}\n\nproof: 0 incoming references in the whole graph\nlocation: {}:{}\ngenerated by searchdeadcode --batch-branches",
            corpse.declaration.kind.display_name(),
            corpse.declaration.name,
            corpse.declaration.location.file.display(),
            corpse.declaration.location.line,
        );
        git(&["add", "-u"])?;
        let commit = git(&[
            "-c",
            "user.name=searchdeadcode",
            "-c",
            "user.email=searchdeadcode@localhost",
            "commit",
            "-q",
            "-m",
            &message,
        ])?;
        let back = git(&["checkout", "-q", &original])?;
        if !back.status.success() {
            return Err(format!(
                "could not return to branch {original} — resolve manually"
            ));
        }
        if commit.status.success() {
            println!("  {} {branch}  ({})", "⌥".dimmed(), corpse.declaration.name);
            created += 1;
        } else {
            let _ = git(&["branch", "-q", "-D", &branch]);
        }
    }
    Ok(created)
}

/// Shields-style flat badge: grey label, colored value. Zero is green,
/// a little rot is yellow, more is red.
fn write_badge(
    path: &Path,
    dead_code: &[analysis::DeadCode],
    total_declarations: usize,
) -> std::io::Result<u32> {
    let unique_dead: std::collections::HashSet<_> = dead_code
        .iter()
        .map(|dc| dc.declaration.id.clone())
        .collect();
    let percent = if total_declarations == 0 {
        0
    } else {
        ((unique_dead.len() as f64 / total_declarations as f64) * 100.0).round() as u32
    };
    let color = match percent {
        0 => "#4c1",
        1..=5 => "#dfb317",
        _ => "#e05d44",
    };
    let value = format!("{percent}%");
    // shields "flat" geometry: 6px per char is close enough for a badge
    let label = "dead code";
    let label_width = 6 * label.len() as u32 + 10;
    let value_width = 6 * value.len() as u32 + 10;
    let width = label_width + value_width;
    let svg = format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"20\" role=\"img\" aria-label=\"{label}: {value}\">",
            "<rect width=\"{lw}\" height=\"20\" fill=\"#555\"/>",
            "<rect x=\"{lw}\" width=\"{vw}\" height=\"20\" fill=\"{color}\"/>",
            "<g fill=\"#fff\" text-anchor=\"middle\" font-family=\"Verdana,DejaVu Sans,sans-serif\" font-size=\"11\">",
            "<text x=\"{lx}\" y=\"14\">{label}</text>",
            "<text x=\"{vx}\" y=\"14\">{value}</text>",
            "</g></svg>"
        ),
        w = width,
        lw = label_width,
        vw = value_width,
        color = color,
        lx = label_width / 2,
        vx = label_width + value_width / 2,
        label = label,
        value = value,
    );
    std::fs::write(path, svg)?;
    Ok(percent)
}

/// The cleanup PR body, ready to paste: stats, per-symbol proof of
/// death, and a residual-risks section for what static analysis
/// cannot fully vouch for.
fn pr_description_text(dead_code: &[analysis::DeadCode], root: &Path) -> String {
    use std::fmt::Write as _;
    let corpses: Vec<&analysis::DeadCode> = dead_code
        .iter()
        .filter(|dc| dc.severity != analysis::Severity::Info)
        .collect();
    if corpses.is_empty() {
        return "nothing to clean — no PR to describe\n".to_string();
    }
    let files: std::collections::BTreeSet<_> = corpses
        .iter()
        .map(|dc| dc.declaration.location.file.clone())
        .collect();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "## Remove dead code ({} finding(s) across {} file(s))\n",
        corpses.len(),
        files.len()
    );
    out.push_str("Every symbol below has no incoming references in the whole reference graph (0 incoming references), as verified by searchdeadcode.\n\n");
    out.push_str("## Findings\n\n");
    out.push_str("| Code | Symbol | Kind | Location |\n");
    out.push_str("|------|--------|------|----------|\n");
    for dc in &corpses {
        let rel = dc
            .declaration
            .location
            .file
            .strip_prefix(root)
            .unwrap_or(&dc.declaration.location.file);
        let _ = writeln!(
            out,
            "| {} | {} | {} | {}:{} |",
            dc.issue.code(),
            dc.declaration.name,
            dc.declaration.kind.display_name(),
            rel.display(),
            dc.declaration.location.line
        );
    }
    let risky: Vec<&&analysis::DeadCode> = corpses
        .iter()
        .filter(|dc| dc.risk != analysis::RiskLevel::Low)
        .collect();
    out.push_str("\n## Residual risks\n\n");
    if risky.is_empty() {
        out.push_str(
            "None flagged: no string references, reflection or bus signals on any finding.\n",
        );
    } else {
        for dc in risky {
            let _ = writeln!(
                out,
                "- **{}** ({} risk): {}",
                dc.declaration.name, dc.risk, dc.message
            );
        }
        out.push_str(
            "\nDouble-check these before merging — static analysis cannot fully vouch for them.\n",
        );
    }
    out
}

/// A-F report card per module from the dead/total declaration ratio —
/// the light gamification that lands in a team review.
fn health_rows(
    graph: &graph::Graph,
    dead_code: &[analysis::DeadCode],
    root: &Path,
) -> Vec<(String, usize, usize)> {
    use std::collections::{HashMap, HashSet};
    let mut totals: HashMap<String, usize> = HashMap::new();
    for decl in graph.declarations() {
        *totals
            .entry(analysis::strings_dup::module_of(root, &decl.location.file))
            .or_default() += 1;
    }
    let mut dead: HashMap<String, HashSet<String>> = HashMap::new();
    for dc in dead_code {
        // health measures death, not advice: info-level findings
        // (visibility hints, style) do not rot a module
        if dc.severity == analysis::Severity::Info {
            continue;
        }
        dead.entry(analysis::strings_dup::module_of(
            root,
            &dc.declaration.location.file,
        ))
        .or_default()
        .insert(dc.declaration.id.to_string());
    }
    let mut rows: Vec<(String, usize, usize)> = totals
        .into_iter()
        .map(|(module, total)| {
            let corpses = dead.get(&module).map(HashSet::len).unwrap_or(0);
            (module, corpses, total)
        })
        .collect();
    rows.sort_by(|a, b| {
        let ratio_a = a.1 as f64 / a.2.max(1) as f64;
        let ratio_b = b.1 as f64 / b.2.max(1) as f64;
        ratio_b.partial_cmp(&ratio_a).unwrap().then(a.0.cmp(&b.0))
    });
    rows
}

fn health_grade(percent: f64) -> &'static str {
    match percent {
        p if p <= 1.0 => "A",
        p if p <= 3.0 => "B",
        p if p <= 8.0 => "C",
        p if p <= 15.0 => "D",
        _ => "F",
    }
}

fn print_health_json(graph: &graph::Graph, dead_code: &[analysis::DeadCode], root: &Path) {
    let modules: Vec<serde_json::Value> = health_rows(graph, dead_code, root)
        .into_iter()
        .map(|(module, corpses, total)| {
            let percent = corpses as f64 * 100.0 / total.max(1) as f64;
            serde_json::json!({
                "module": module,
                "grade": health_grade(percent),
                "dead": corpses,
                "total": total,
                "percent": (percent * 10.0).round() / 10.0
            })
        })
        .collect();
    println!("{}", serde_json::json!({ "modules": modules }));
}

fn print_health(graph: &graph::Graph, dead_code: &[analysis::DeadCode], root: &Path) {
    println!("{}", "Module health (dead/total declarations):".bold());
    for (module, corpses, total) in health_rows(graph, dead_code, root) {
        let percent = corpses as f64 * 100.0 / total.max(1) as f64;
        let grade = health_grade(percent);
        let colored_grade = match grade {
            "A" => grade.green().bold().to_string(),
            "B" | "C" => grade.yellow().bold().to_string(),
            _ => grade.red().bold().to_string(),
        };
        println!(
            "  {colored_grade} {:<30} {corpses}/{total} dead ({percent:.1}%)",
            module
        );
    }
}

/// Findings aggregated per Gradle module: count and dominant rule —
/// the view a lead of a many-module repo actually looks at.
fn print_by_module(dead_code: &[analysis::DeadCode], root: &Path) {
    use std::collections::HashMap;
    if dead_code.is_empty() {
        println!("{}", "✓ no findings — nothing to aggregate".green());
        return;
    }
    let mut per_module: HashMap<String, (usize, HashMap<&'static str, usize>)> = HashMap::new();
    for dc in dead_code {
        let module = analysis::strings_dup::module_of(root, &dc.declaration.location.file);
        let entry = per_module.entry(module).or_default();
        entry.0 += 1;
        *entry.1.entry(dc.issue.code()).or_default() += 1;
    }
    let mut rows: Vec<(String, usize, &'static str, usize)> = per_module
        .into_iter()
        .map(|(module, (count, rules))| {
            let (top_rule, top_count) = rules
                .into_iter()
                .max_by_key(|(code, n)| (*n, std::cmp::Reverse(*code)))
                .unwrap_or(("", 0));
            (module, count, top_rule, top_count)
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!("{}", "Findings per module:".bold());
    for (module, count, top_rule, top_count) in rows {
        println!(
            "  {:<30} {:>4} finding(s)  top: {} ({})",
            module, count, top_rule, top_count
        );
    }
}

fn print_top_files(
    graph: &graph::Graph,
    dead_code: &[analysis::DeadCode],
    base: &std::path::Path,
    limit: usize,
) {
    use std::collections::BTreeMap;

    let mut by_file: BTreeMap<std::path::PathBuf, Vec<graph::DeclarationId>> = BTreeMap::new();
    for dc in dead_code {
        by_file
            .entry(dc.declaration.location.file.clone())
            .or_default()
            .push(dc.declaration.id.clone());
    }

    let mut rows: Vec<(usize, usize, std::path::PathBuf)> = by_file
        .into_iter()
        .map(|(file, ids)| {
            let (_, lines) = outermost_entries(graph, &ids);
            (lines, ids.len(), file)
        })
        .collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.0));

    if rows.is_empty() {
        println!("{}", "✓ No deletable lines — nothing to rank".green());
        return;
    }

    println!("{}", "Top files by deletable lines".bold());
    for (lines, findings, file) in rows.into_iter().take(limit) {
        let rel = file.strip_prefix(base).unwrap_or(&file);
        println!(
            "  {:>6}  {}  {}",
            format!("~{lines}L").yellow(),
            rel.display(),
            format!("({findings} finding(s))").dimmed()
        );
    }
}

fn outermost_entries(graph: &graph::Graph, ids: &[graph::DeclarationId]) -> (Vec<String>, usize) {
    let in_list: std::collections::HashSet<&graph::DeclarationId> = ids.iter().collect();
    let mut estimated_lines = 0usize;
    let mut entries = Vec::new();

    for id in ids {
        let Some(decl) = graph.get_declaration(id) else {
            continue;
        };
        let is_outermost = decl
            .parent
            .as_ref()
            .map(|p| !in_list.contains(p))
            .unwrap_or(true);
        if !is_outermost {
            continue;
        }
        if let Ok(content) = std::fs::read(&id.file) {
            let end = id.end.min(content.len());
            let start = id.start.min(end);
            estimated_lines += content[start..end].iter().filter(|b| **b == b'\n').count() + 1;
        }
        entries.push(format!(
            "   - {} — {}:{}",
            decl.name,
            decl.location.file.display(),
            decl.location.line
        ));
    }

    (entries, estimated_lines)
}

/// Print only the findings safe to delete blind: their whole connected
/// cluster is dead and every clustered finding carries low risk — one risky
/// member poisons its cluster, since deleting the root drags it down too.
fn print_quick_wins(graph: &graph::Graph, dead_code: &[analysis::DeadCode]) {
    use std::collections::HashMap;

    let dead_ids: std::collections::HashSet<graph::DeclarationId> =
        dead_code.iter().map(|d| d.declaration.id.clone()).collect();
    let risk_by_id: HashMap<&graph::DeclarationId, analysis::RiskLevel> = dead_code
        .iter()
        .map(|d| (&d.declaration.id, d.risk))
        .collect();

    let clusters = analysis::kill_list::dead_clusters(graph, &dead_ids);
    let mut wins: Vec<(Vec<String>, usize, usize)> = clusters
        .into_iter()
        .filter(|cluster| {
            cluster.iter().all(|id| {
                risk_by_id
                    .get(id)
                    .map(|r| *r == analysis::RiskLevel::Low)
                    .unwrap_or(true) // expanded members without findings carry no signal
            })
        })
        .map(|ids| {
            let (entries, lines) = outermost_entries(graph, &ids);
            (entries, lines, ids.len())
        })
        .filter(|(entries, _, _)| !entries.is_empty())
        .collect();
    wins.sort_by_key(|win| std::cmp::Reverse(win.1));

    if wins.is_empty() {
        println!("No quick wins — every dead cluster carries some risk. Try --clusters for the full picture.");
        return;
    }

    println!(
        "⚡ {} quick win(s) — whole cluster dead, all low risk:",
        wins.len()
    );
    for (entries, lines, count) in &wins {
        println!("\n{} declaration(s), ~{} lines:", count, lines);
        for entry in entries {
            println!("{entry}");
        }
    }
    println!("\nDelete them: searchdeadcode --delete --dry-run  (preview first)");
}

/// Print dead code grouped into connected clusters, biggest first
fn print_clusters(graph: &graph::Graph, clusters: Vec<Vec<graph::DeclarationId>>) {
    let mut rendered: Vec<(Vec<String>, usize, usize)> = clusters
        .into_iter()
        .map(|ids| {
            let (entries, lines) = outermost_entries(graph, &ids);
            (entries, lines, ids.len())
        })
        .filter(|(entries, _, _)| !entries.is_empty())
        .collect();
    rendered.sort_by_key(|cluster| std::cmp::Reverse(cluster.1));

    println!("🧩 {} deletable cluster(s), biggest first", rendered.len());
    for (index, (entries, lines, count)) in rendered.iter().enumerate() {
        println!(
            "\nCluster {}: {} declaration(s), ~{} lines",
            index + 1,
            count,
            lines
        );
        for entry in entries {
            println!("{entry}");
        }
    }
}

/// Print dead islands: groups of declarations holding only each other.
fn print_islands(islands: Vec<analysis::islands::DeadIsland>) {
    if islands.is_empty() {
        println!("No dead islands: nothing is kept alive only by dead code.");
        return;
    }
    println!(
        "🏝️  {} dead island(s) — declarations that reference only each other, biggest first",
        islands.len()
    );
    for (index, island) in islands.iter().enumerate() {
        let mut marker = String::new();
        if island.test_only {
            marker.push_str(
                "  [test-only: its tests still reference it — deleting both is a human call]",
            );
        }
        if island.keep_covered {
            marker.push_str(
                "  [matches a wildcard -keep rule — retire the rule together with the island]",
            );
        }
        println!(
            "\nIsland {}: {} declaration(s), ~{} lines{}",
            index + 1,
            island.total_declarations,
            island.estimated_lines,
            marker
        );
        for member in &island.members {
            println!(
                "   - {} — {}:{}",
                member.name,
                member.file.display(),
                member.line
            );
        }
        for member in &island.members {
            if let Some(chain) = analysis::islands::chain_of(member) {
                println!("     {chain}");
            }
        }
    }
    println!("\nEvery mention that cannot be placed inside the island itself counts as life:");
    println!("XML, ProGuard, string literals, annotations and entry points all root. Verify");
    println!("a member with: searchdeadcode . --deep --why-alive <name>");
}

/// Print the module usage attribution report
fn print_module_usage(graph: &graph::Graph, module: &str, root: &std::path::Path) {
    use analysis::module_usage::Usage;

    let usages = analysis::module_usage::module_usage(graph, module, root);
    if usages.is_empty() {
        println!("No symbols found in module '{}'.", module);
        return;
    }

    let entry = |id: &graph::DeclarationId| -> String {
        graph
            .get_declaration(id)
            .map(|d| {
                let rel = d
                    .location
                    .file
                    .strip_prefix(root)
                    .unwrap_or(&d.location.file);
                format!("{} — {}:{}", d.name, rel.display(), d.location.line)
            })
            .unwrap_or_default()
    };

    println!("📊 Module usage: {}", module);

    let consumed: Vec<_> = usages
        .iter()
        .filter_map(|u| match &u.usage {
            Usage::UsedBy(dirs) => Some((u, dirs)),
            _ => None,
        })
        .collect();
    if !consumed.is_empty() {
        println!("\nUsed from outside ({}):", consumed.len());
        for (u, dirs) in consumed {
            let dirs: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();
            println!("   - {}  ← {}", entry(&u.id), dirs.join(", "));
        }
    }

    let internal: Vec<_> = usages
        .iter()
        .filter(|u| matches!(u.usage, Usage::InternalOnly))
        .collect();
    if !internal.is_empty() {
        println!(
            "\nUsed inside the module only ({}) — candidates for internal visibility:",
            internal.len()
        );
        for u in internal {
            println!("   - {}", entry(&u.id));
        }
    }

    let unreferenced: Vec<_> = usages
        .iter()
        .filter(|u| matches!(u.usage, Usage::Unreferenced))
        .collect();
    if !unreferenced.is_empty() {
        println!("\nUnreferenced ({}):", unreferenced.len());
        for u in unreferenced {
            println!("   - {}", entry(&u.id));
        }
    }
}

/// Print the migration diff between the old world and everything else
fn print_migration_report(
    graph: &graph::Graph,
    old_token: &str,
    new_token: &str,
    report: &analysis::migration::MigrationReport,
) {
    println!("🔀 Migration compare: {} → {}", old_token, new_token);

    let deletable_ids: Vec<graph::DeclarationId> =
        report.deletable.iter().map(|e| e.id.clone()).collect();
    let (entries, lines) = outermost_entries(graph, &deletable_ids);
    println!(
        "\nDeletable at the flip ({} declarations, ~{} lines):",
        deletable_ids.len(),
        lines
    );
    for entry in entries {
        println!("{entry}");
    }

    println!(
        "\nStill referenced from outside ({} blockers):",
        report.blockers.len()
    );
    for blocker in &report.blockers {
        let Some(decl) = graph.get_declaration(&blocker.id) else {
            continue;
        };
        let used_by = blocker
            .blocked_by
            .as_ref()
            .and_then(|id| graph.get_declaration(id))
            .map(|r| {
                format!(
                    ", used by {}:{} ({})",
                    r.location.file.display(),
                    r.location.line,
                    r.name
                )
            })
            .unwrap_or_default();
        println!(
            "   - {} — {}:{}{}",
            decl.name,
            decl.location.file.display(),
            decl.location.line,
            used_by
        );
    }
}

/// Print a kill-list: the target plus everything that only it kept alive
fn print_kill_list(graph: &graph::Graph, symbol: &str, ids: &[graph::DeclarationId]) {
    let (entries, estimated_lines) = outermost_entries(graph, ids);

    println!(
        "💀 Kill-list for {}: {} declarations, ~{} lines",
        symbol,
        ids.len(),
        estimated_lines
    );
    for entry in entries {
        println!("{entry}");
    }
}

/// Print why a symbol is considered dead or alive
/// The inverse of --explain: walk backwards from a living symbol through
/// incoming references (and the member-of link) until a root is reached,
/// then print the chain root-first.
fn why_alive_symbol(
    graph: &graph::Graph,
    entry_points: &std::collections::HashSet<graph::DeclarationId>,
    reachable: &std::collections::HashSet<graph::DeclarationId>,
    symbol: &str,
) {
    use std::collections::{HashMap, HashSet, VecDeque};

    let candidates: Vec<&graph::Declaration> = match graph.find_by_fqn(symbol) {
        Some(decl) => vec![decl],
        None => graph.find_by_name(symbol),
    };
    if candidates.is_empty() {
        println!("Symbol '{}' not found in the analyzed project.", symbol);
        return;
    }

    let display = |id: &graph::DeclarationId| -> String {
        let Some(decl) = graph.get_declaration(id) else {
            return "<unknown>".to_string();
        };
        let name = match decl.parent.as_ref().and_then(|p| graph.get_declaration(p)) {
            Some(parent) => format!("{}.{}", parent.name, decl.name),
            None => decl.name.clone(),
        };
        format!(
            "{} ({}:{})",
            name,
            decl.location.file.display(),
            decl.location.line
        )
    };

    for decl in candidates.iter().take(3) {
        println!("\n🌿 Why alive: {}", display(&decl.id));

        if entry_points.contains(&decl.id) {
            match analysis::EntryPointDetector::entry_annotation_reason(graph, decl) {
                Some(reason) => println!(
                    "   It is itself an entry point — a retention root ({reason})."
                ),
                None => println!(
                    "   It is itself an entry point — a retention root (manifest, layout, inheritance or config rule)."
                ),
            }
            continue;
        }
        if !reachable.contains(&decl.id) {
            println!(
                "   It is not alive — this symbol is dead. See: searchdeadcode --explain {}",
                symbol
            );
            continue;
        }

        // BFS backwards: target ← referencing declaration, plus the
        // member-of link (a live parent retains its members)
        let mut preds: HashMap<graph::DeclarationId, (graph::DeclarationId, &'static str)> =
            HashMap::new();
        let mut visited: HashSet<graph::DeclarationId> = HashSet::new();
        let mut queue = VecDeque::from([decl.id.clone()]);
        visited.insert(decl.id.clone());
        let mut root: Option<graph::DeclarationId> = None;

        'search: while let Some(current) = queue.pop_front() {
            let mut steps: Vec<(graph::DeclarationId, &'static str)> = graph
                .get_references_to(&current)
                .into_iter()
                .map(|(source, _)| (source.id.clone(), "referenced by"))
                .collect();
            if let Some(parent) = graph
                .get_declaration(&current)
                .and_then(|d| d.parent.clone())
            {
                steps.push((parent, "member of"));
            }
            for (next, how) in steps {
                if !visited.insert(next.clone()) {
                    continue;
                }
                preds.insert(next.clone(), (current.clone(), how));
                if entry_points.contains(&next) {
                    root = Some(next);
                    break 'search;
                }
                queue.push_back(next);
            }
        }

        match root {
            Some(root_id) => {
                // rebuild root → … → symbol
                let mut chain = vec![(root_id.clone(), "entry point")];
                let mut cursor = root_id;
                while let Some((towards_symbol, how)) = preds.get(&cursor) {
                    chain.push((towards_symbol.clone(), *how));
                    cursor = towards_symbol.clone();
                }
                for (i, (id, how)) in chain.iter().enumerate() {
                    if i == 0 {
                        println!("   {} — {}", display(id), how);
                    } else {
                        println!("   → {} ({})", display(id), how);
                    }
                }
            }
            None => {
                println!(
                    "   Alive without a reference chain from a root — retained by \
                     analyzer policy (member retention, config, or keep rules)."
                );
            }
        }
    }
}

fn explain_symbol(
    graph: &graph::Graph,
    entry_points: &std::collections::HashSet<graph::DeclarationId>,
    reachable: &std::collections::HashSet<graph::DeclarationId>,
    symbol: &str,
) {
    let candidates: Vec<&graph::Declaration> = match graph.find_by_fqn(symbol) {
        Some(decl) => vec![decl],
        None => graph.find_by_name(symbol),
    };

    if candidates.is_empty() {
        // detector findings (prefs keys, flags, resources) are string
        // literals, not graph nodes — locate them instead of shrugging
        let quoted = format!("\"{symbol}\"");
        let files: std::collections::BTreeSet<_> = graph
            .declarations()
            .map(|d| d.location.file.clone())
            .collect();
        let mut sites: Vec<(std::path::PathBuf, usize)> = Vec::new();
        for file in files {
            let Ok(content) = std::fs::read_to_string(&file) else {
                continue;
            };
            for (index, line) in content.lines().enumerate() {
                if line.contains(&quoted) {
                    sites.push((file.clone(), index + 1));
                }
            }
        }
        if sites.is_empty() {
            println!("Symbol '{}' not found in the analyzed project.", symbol);
            return;
        }
        println!(
            "'{}' is not a declaration in the reference graph, but appears as a string literal at {} site(s):",
            symbol,
            sites.len()
        );
        for (file, line) in sites.iter().take(5) {
            println!("  - {}:{}", file.display(), line);
        }
        println!(
            "String keys are judged by detectors (write-only prefs, stale flags, caches), not by graph reachability."
        );
        return;
    }

    for decl in candidates.iter().take(3) {
        let display_name = decl.fully_qualified_name.as_deref().unwrap_or(&decl.name);
        println!(
            "\n🔎 Explain: {} ({:?}) — {}:{}",
            display_name,
            decl.kind,
            decl.location.file.display(),
            decl.location.line
        );

        let incoming = graph.get_references_to(&decl.id);
        println!("   Incoming references: {}", incoming.len());
        for (from, _) in incoming.iter().take(5) {
            println!(
                "     - referenced by {}:{} ({})",
                from.location.file.display(),
                from.location.line,
                from.name
            );
        }

        let is_entry = entry_points.contains(&decl.id);
        let is_reachable = reachable.contains(&decl.id);
        println!("   Roots checked:");
        println!(
            "     - entry point (manifest, layouts, navigation, menus, annotations, inheritance, config): {}",
            if is_entry { "yes" } else { "no" }
        );
        println!(
            "     - reachable from an entry point: {}",
            if is_reachable { "yes" } else { "no" }
        );

        if is_entry || is_reachable {
            println!("   Verdict: ALIVE");
        } else {
            println!("   Verdict: DEAD — no root retains this symbol");
        }
    }
}

/// PR-scoped analysis: parse only the files changed since `base_ref` and
/// report a symbol only when its name appears nowhere else in the project —
/// the one verdict a partial subgraph can make honestly. Any other mention
/// (even a comment) means silence.
fn run_changed_since(files: &[discovery::SourceFile], cli: &Cli, base_ref: &str) -> Result<()> {
    use discovery::FileType;
    use miette::IntoDiagnostic;
    use std::collections::HashSet;

    let diff = std::process::Command::new("git")
        .arg("-C")
        .arg(&cli.path)
        .args(["diff", "--name-only", base_ref])
        .output()
        .into_diagnostic()?;
    if !diff.status.success() {
        return Err(miette::miette!(
            "git diff against '{}' failed — is {} a git repository with that ref?",
            base_ref,
            cli.path.display()
        ));
    }
    let repo_root = std::process::Command::new("git")
        .arg("-C")
        .arg(&cli.path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .into_diagnostic()?;
    let repo_root = PathBuf::from(String::from_utf8_lossy(&repo_root.stdout).trim());

    // Canonicalize BOTH sides: Windows canonical paths carry a \\?\ prefix
    // and macOS resolves /var symlinks — mixed forms never compare equal
    let changed: HashSet<PathBuf> = String::from_utf8_lossy(&diff.stdout)
        .lines()
        .map(|l| repo_root.join(l.trim()))
        .map(|p| p.canonicalize().unwrap_or(p))
        .collect();

    let changed_sources: Vec<&discovery::SourceFile> = files
        .iter()
        .filter(|f| matches!(f.file_type, FileType::Kotlin | FileType::Java))
        .filter(|f| {
            let canonical = f.path.canonicalize().unwrap_or_else(|_| f.path.clone());
            changed.contains(&canonical) || changed.contains(&f.path)
        })
        .collect();

    if changed_sources.is_empty() {
        println!("No changed source files since {}.", base_ref);
        return Ok(());
    }
    phase_line(
        cli.quiet,
        "pr-scope",
        &format!(
            "{} changed file(s) since {}",
            changed_sources.len(),
            base_ref
        ),
    );

    // Parse only the changed files
    let mut builder = GraphBuilder::new();
    for file in &changed_sources {
        builder.process_file(file)?;
    }
    let graph = builder.build();

    // Keep rules still apply in PR scope
    let keep_patterns = analysis::keep_rules::collect_keep_patterns(&cli.path);

    // One project-wide text corpus per changed file (its own text excluded)
    let mut findings = Vec::new();
    for decl in graph.declarations() {
        if decl.parent.is_some() {
            continue; // judge outermost symbols only: members follow their owner
        }
        if decl.is_android_entry_point() {
            continue;
        }
        if let Some(fqn) = &decl.fully_qualified_name {
            if keep_patterns.iter().any(|p| p.matches(fqn)) {
                continue;
            }
        }
        let Ok(name_re) = regex::Regex::new(&format!(r"\b{}\b", regex::escape(&decl.name))) else {
            continue;
        };
        let mentioned_elsewhere = files.iter().any(|f| {
            if f.path == decl.location.file {
                return false;
            }
            // Cheap prefilter then regex: read only when the name might be there
            std::fs::read_to_string(&f.path)
                .map(|content| name_re.is_match(&content))
                .unwrap_or(false)
        });
        if !mentioned_elsewhere {
            findings.push(decl.clone());
        }
    }

    if findings.is_empty() {
        println!(
            "✓ No stable findings — nothing in the diff is provably unreferenced project-wide."
        );
        return Ok(());
    }

    println!(
        "\n{} stable finding(s) in the diff (name absent from the rest of the project):",
        findings.len()
    );
    for decl in &findings {
        let rel = decl
            .location
            .file
            .strip_prefix(&cli.path)
            .unwrap_or(&decl.location.file);
        println!(
            "  ⚠ {} '{}' — {}:{}",
            decl.kind.display_name(),
            decl.name,
            rel.display(),
            decl.location.line
        );
    }
    println!("\nPR scope stays silent on anything mentioned elsewhere — run a full analysis for the complete picture.");
    // Le mode PR sortait toujours 0 : il imprimait ses trouvailles puis
    // rendait Ok(()), sans jamais consulter la porte — `--profile ci
    // --changed-since` ne pouvait donc pas faire échouer un pipeline, et le
    // hook écrit par --install-hook ne bloquait aucun commit. Pas de garde
    // is_empty : le chemin vide a déjà retourné plus haut.
    if resolve_fail_on_findings(cli) && cli.generate_baseline.is_none() {
        std::process::exit(1);
    }
    Ok(())
}

/// Apply zero-risk fixes: remove unused imports across the project.
/// --dry-run lists without touching; otherwise an undo script is written.
fn run_fix(files: &[discovery::SourceFile], cli: &Cli) -> Result<()> {
    use discovery::FileType;

    let undo_path = cli
        .undo_script
        .clone()
        .unwrap_or_else(|| cli.path.join(".searchdeadcode-undo.sh"));
    let mut undo = refactor::UndoScript::new();
    let mut removed = 0usize;
    let mut touched_files = 0usize;

    for file in files {
        if !matches!(file.file_type, FileType::Kotlin | FileType::Java) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&file.path) else {
            continue;
        };
        let unused = refactor::fix_imports::unused_imports(&content);
        if unused.is_empty() {
            continue;
        }

        let rel = file.path.strip_prefix(&cli.path).unwrap_or(&file.path);
        for import in &unused {
            println!(
                "  {} {}:{} {}",
                if cli.dry_run { "○" } else { "✂" },
                rel.display(),
                import.line_index + 1,
                import.line.trim()
            );
        }

        if !cli.dry_run {
            undo.record_file_state(&file.path, &content);
            let new_content = refactor::fix_imports::remove_imports(&content, &unused);
            std::fs::write(&file.path, new_content)
                .map_err(|e| miette::miette!("write {}: {e}", file.path.display()))?;
        }
        removed += unused.len();
        touched_files += 1;
    }

    if removed == 0 {
        println!("Nothing to fix — no unused imports found.");
        return Ok(());
    }

    if cli.dry_run {
        println!(
            "\nDry run: {} unused import(s) in {} file(s) would be removed.",
            removed, touched_files
        );
    } else {
        undo.write(&undo_path)?;
        println!(
            "\n🔧 Fixed {} unused import(s) in {} file(s) — undo: {}",
            removed,
            touched_files,
            undo_path.display()
        );
    }
    Ok(())
}

/// Build the graph incrementally: parse changed files, load unchanged ones from cache
fn build_graph_incremental(
    files: &[discovery::SourceFile],
    project_root: &std::path::Path,
    cache_file: &std::path::Path,
    cli: &Cli,
) -> Result<graph::Graph> {
    use cache::{FileCacheEntry, FileMetadata, IncrementalAnalyzer};
    use discovery::FileType;
    use miette::IntoDiagnostic;
    use std::collections::HashSet;

    let mut analyzer =
        IncrementalAnalyzer::with_cache_path(project_root.to_path_buf(), cache_file.to_path_buf());
    analyzer.prune();

    let source_paths: Vec<PathBuf> = files
        .iter()
        .filter(|f| matches!(f.file_type, FileType::Kotlin | FileType::Java))
        .map(|f| f.path.clone())
        .collect();
    let (to_parse, _) = analyzer.get_files_to_parse(&source_paths);
    let to_parse: HashSet<PathBuf> = to_parse.into_iter().cloned().collect();

    let mut builder = GraphBuilder::new();
    let mut parsed_count = 0usize;
    let mut cached_count = 0usize;

    for file in files {
        if !matches!(file.file_type, FileType::Kotlin | FileType::Java) {
            continue; // XML files are handled later in the pipeline
        }

        if !to_parse.contains(&file.path) {
            if let Some(entry) = analyzer.get_cached(&file.path) {
                let mut parse_result = entry.parse_result.clone();
                // The cache does not store per-reference imports, they are a
                // copy of the file's list. Put them back before resolution,
                // which reads them off the reference.
                parse_result.restore_reference_imports();
                builder.add_parse_result(parse_result);
                cached_count += 1;
                continue;
            }
        }

        if let Some(parse_result) = builder.parse_source(file)? {
            let metadata = FileMetadata::from_path(&file.path).into_diagnostic()?;
            analyzer.update_cache(
                &file.path,
                FileCacheEntry {
                    metadata,
                    parse_result: parse_result.clone(),
                },
            );
            builder.add_parse_result(parse_result);
            parsed_count += 1;
        }
    }

    analyzer.save().into_diagnostic()?;

    phase_line(
        cli.quiet,
        "parsed",
        &format!("{} parsed, {} from cache", parsed_count, cached_count),
    );

    Ok(builder.build())
}

/// One aligned, checked progress line per phase (pnpm-style)
fn phase_line(quiet: bool, label: &str, detail: &str) {
    if quiet {
        return;
    }
    eprintln!(
        " {} {:<10} {}",
        "✓".green().bold(),
        label.bold(),
        detail.dimmed()
    );
}

fn run_analysis(config: &Config, cli: &Cli) -> Result<()> {
    use colored::Colorize;
    use indicatif::{ProgressBar, ProgressStyle};
    use std::time::Instant;

    let start_time = Instant::now();

    // First contact: a project without config deserves a pointer
    if cli.config.is_none() && !cli.quiet {
        let has_config = [
            ".deadcode.yml",
            ".deadcode.yaml",
            ".deadcode.toml",
            "deadcode.yml",
            "deadcode.yaml",
            "deadcode.toml",
        ]
        .iter()
        .any(|name| cli.path.join(name).is_file());
        if !has_config {
            eprintln!(
                "{}",
                "💡 No .deadcode.yml found — `searchdeadcode --init` writes one matched to this project".dimmed()
            );
        }
    }

    // Step 1: Discover files
    //
    // Un chemin qui n'existe pas est une erreur d'OUTILLAGE, pas un projet
    // sain : sortir 0 ici faisait dire à une CI « aucun code mort » sur un
    // checkout raté ou une faute de frappe — exactement ce que le contrat
    // 0/1/2 promet d'empêcher. Un dossier existant sans sources, lui, reste
    // un rapport vide légitime.
    if !cli.path.exists() {
        eprintln!(
            "{}: path does not exist: {}",
            "Error".red(),
            cli.path.display()
        );
        std::process::exit(2);
    }
    info!("Discovering files...");
    let finder = FileFinder::new(config);
    let files = finder.find_files(&cli.path)?;

    info!("Found {} files to analyze", files.len());

    // Step 1b: Drop phantom source sets (src/ dirs no build file accounts for)
    let audit = discovery::detect_phantom_source_sets(&cli.path);
    let files = if audit.phantom_dirs.is_empty() {
        files
    } else {
        if !cli.quiet {
            for dir in &audit.phantom_dirs {
                eprintln!(
                    "{}",
                    format!(
                        "⚠ Phantom source set (not part of the build), excluded: {}",
                        dir.display()
                    )
                    .yellow()
                );
            }
        }
        files
            .into_iter()
            .filter(|f| !audit.phantom_dirs.iter().any(|d| f.path.starts_with(d)))
            .collect()
    };

    if files.is_empty() {
        println!(
            "{}",
            format!("No Kotlin or Java files found in {}.", cli.path.display()).yellow()
        );
        println!("Check the path, or point the tool at your sources: searchdeadcode <path>");
        return Ok(());
    }

    // --fix: zero-risk cleanup (unused imports), no graph needed
    if cli.fix {
        return run_fix(&files, cli);
    }

    // --changed-since: PR scope, stable verdicts only
    if let Some(base_ref) = cli.changed_since.as_deref() {
        return run_changed_since(&files, cli, base_ref);
    }

    // Step 2: Parse files and build graph
    let cache_root = if cli.path.is_dir() {
        cli.path.clone()
    } else {
        cli.path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    };
    let cache_file = cli
        .cache_path
        .clone()
        .unwrap_or_else(|| cache::AnalysisCache::default_cache_path(&cache_root));
    if cli.clear_cache {
        let _ = std::fs::remove_file(&cache_file);
        if !cli.quiet {
            eprintln!("{}", "🧹 Cache cleared".cyan());
        }
    }

    let graph = if resolve_incremental(cli) {
        build_graph_incremental(&files, &cache_root, &cache_file, cli)?
    } else if cli.parallel {
        // Parallel parsing mode
        let parallel_builder = ParallelGraphBuilder::new();
        parallel_builder.build_from_files(&files)?
    } else {
        // Sequential parsing mode
        let pb = ProgressBar::new(files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("#>-"),
        );

        info!("Parsing files...");
        let mut graph_builder = GraphBuilder::new();

        for file in &files {
            graph_builder.process_file(file)?;
            pb.inc(1);
        }
        pb.finish_with_message("Parsing complete");

        graph_builder.build()
    };

    let parse_time = start_time.elapsed();
    if cli.parallel && !resolve_incremental(cli) {
        phase_line(
            cli.quiet,
            "parsed",
            &format!("{} files in {:.2}s", files.len(), parse_time.as_secs_f64()),
        );
    }

    // Step 3: Detect entry points
    info!("Detecting entry points...");
    let entry_detector = EntryPointDetector::new(config);
    let mut entry_points = entry_detector.detect(&graph, &cli.path)?;

    // Library mode: the public surface is consumed outside this repo,
    // so every public top-level declaration becomes a root
    if cli.library_mode {
        let mut public_roots = 0usize;
        for decl in graph.declarations() {
            if decl.parent.is_none()
                && decl.visibility == graph::Visibility::Public
                && entry_points.insert(decl.id.clone())
            {
                public_roots += 1;
            }
        }
        info!(
            "Library mode: {} public declarations kept as roots",
            public_roots
        );
    }
    let entry_points = entry_points;

    info!("Found {} entry points", entry_points.len());

    // --unused-permissions short-circuits everything: manifest + strings
    if cli.unused_permissions {
        match analysis::permissions::unused_permissions(&cli.path) {
            None => println!("{}", "no manifest found — nothing to check".dimmed()),
            Some(unused) if unused.is_empty() => {
                println!(
                    "{}",
                    "✓ every checkable permission has a matching API in use".green()
                )
            }
            Some(unused) => {
                println!("{}", "Permissions with no matching API in the code:".bold());
                for permission in unused {
                    let rel = permission
                        .manifest
                        .strip_prefix(&cli.path)
                        .unwrap_or(&permission.manifest);
                    println!(
                        "  {} {}  {}",
                        "○".dimmed(),
                        permission.name,
                        rel.display().to_string().dimmed()
                    );
                }
                println!(
                    "{}",
                    "  (candidates only — libraries may use the capability without these tokens)"
                        .dimmed()
                );
            }
        }
        return Ok(());
    }

    // --duplicate-strings short-circuits everything: values files only
    if cli.duplicate_strings {
        match analysis::strings_dup::duplicate_strings(&cli.path) {
            None => println!(
                "{}",
                "no string resources found — nothing to compare".dimmed()
            ),
            Some(dups) if dups.is_empty() => {
                println!("{}", "✓ no duplicate strings across modules".green())
            }
            Some(dups) => {
                println!("{}", "String values declared in several modules:".bold());
                for dup in dups {
                    println!("  \"{}\"", dup.value);
                    for (module, name) in dup.declarations {
                        println!("    {} {}  {}", "○".dimmed(), name, module.dimmed());
                    }
                }
                println!(
                    "{}",
                    "  (one shared resource is cheaper — each copy drifts and gets translated alone)"
                        .dimmed()
                );
            }
        }
        return Ok(());
    }

    // --unused-assets short-circuits everything: file paths + strings only
    if cli.unused_assets {
        match analysis::assets::unused_assets(&cli.path) {
            None => println!(
                "{}",
                "no assets directory found — nothing to check".dimmed()
            ),
            Some(unused) if unused.is_empty() => {
                println!("{}", "✓ every asset is referenced somewhere".green())
            }
            Some(unused) => {
                println!("{}", "Assets nothing references:".bold());
                for asset in unused {
                    let rel = asset.file.strip_prefix(&cli.path).unwrap_or(&asset.file);
                    println!(
                        "  {} {:<40} {}",
                        "○".dimmed(),
                        asset.rel_path,
                        rel.display().to_string().dimmed()
                    );
                }
                println!(
                    "{}",
                    "  (candidates only — server-driven or reflective paths cannot be seen)"
                        .dimmed()
                );
            }
        }
        return Ok(());
    }

    // --unused-deps short-circuits everything: build files + imports only
    if cli.unused_deps {
        match analysis::gradle_deps::unused_dependencies(&cli.path) {
            None => println!(
                "{}",
                "no gradle build files found — nothing to check".dimmed()
            ),
            Some(unused) if unused.is_empty() => {
                println!(
                    "{}",
                    "✓ every declared dependency is imported somewhere".green()
                )
            }
            Some(unused) => {
                println!("{}", "Declared but never imported:".bold());
                for dep in unused {
                    let rel = dep
                        .build_file
                        .strip_prefix(&cli.path)
                        .unwrap_or(&dep.build_file);
                    println!(
                        "  {} {}  {}",
                        "○".dimmed(),
                        dep.coordinate,
                        rel.display().to_string().dimmed()
                    );
                }
                println!(
                    "{}",
                    "  (candidates only — check reflection, resources and transitive needs before removing)"
                        .dimmed()
                );
            }
        }
        return Ok(());
    }

    // --dead-modules short-circuits everything: build files only
    if cli.dead_modules {
        match analysis::dead_modules::dead_modules(&cli.path) {
            None => println!(
                "{}",
                "no settings.gradle(.kts) — single-module repo, nothing to check".dimmed()
            ),
            Some(dead) if dead.is_empty() => {
                println!("{}", "✓ every included module has a consumer".green())
            }
            Some(dead) => {
                println!(
                    "{}",
                    "Dead module candidates (no incoming dependency):".bold()
                );
                for module in dead {
                    println!("  {} {}", "○".dimmed(), module.gradle_path);
                }
                println!(
                    "{}",
                    "  (check reflection/manifest usage before deleting a whole module)".dimmed()
                );
            }
        }
        return Ok(());
    }

    // --deprecated short-circuits everything after the graph
    if cli.deprecated {
        let mut ready: Vec<String> = Vec::new();
        let mut lingering: Vec<String> = Vec::new();
        for decl in graph.declarations() {
            if !decl.annotations.iter().any(|a| a.contains("Deprecated")) {
                continue;
            }
            let refs = graph.get_references_to(&decl.id).len();
            let rel = decl
                .location
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&decl.location.file);
            if refs == 0 {
                ready.push(format!(
                    "  {} {}  {}",
                    "○".dimmed(),
                    decl.name,
                    format!("{}:{}", rel.display(), decl.location.line).dimmed()
                ));
            } else {
                lingering.push(format!(
                    "  {} {}  {} ref(s)  {}",
                    "○".dimmed(),
                    decl.name,
                    refs,
                    format!("{}:{}", rel.display(), decl.location.line).dimmed()
                ));
            }
        }
        if ready.is_empty() && lingering.is_empty() {
            println!("{}", "✓ no deprecated symbols".green());
            return Ok(());
        }
        if !ready.is_empty() {
            println!(
                "{}",
                "Deprecated, ready to delete (no references left):".bold()
            );
            for line in ready {
                println!("{line}");
            }
        }
        if !lingering.is_empty() {
            println!(
                "{}",
                "Deprecated, still referenced (migration unfinished):".bold()
            );
            for line in lingering {
                println!("{line}");
            }
        }
        return Ok(());
    }

    // --unobserved short-circuits everything: streams nobody collects
    if cli.unobserved {
        let findings = analysis::unobserved::unobserved_streams(&cli.path);
        if findings.is_empty() {
            println!("{}", "✓ no unobserved streams found".green());
            return Ok(());
        }
        println!("{}", "Exposed streams nobody collects or observes:".bold());
        for finding in findings {
            let rel = finding
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&finding.file);
            println!(
                "  {} {:<30} {}  {}",
                "○".dimmed(),
                finding.name,
                finding.stream_type,
                format!("{}:{}", rel.display(), finding.line).dimmed()
            );
        }
        println!(
            "{}",
            "  (the upstream computation runs for nobody — wire a collector or delete the chain)"
                .dimmed()
        );
        return Ok(());
    }

    // --export-graph short-circuits everything after the graph
    if let Some(ref export_path) = cli.export_graph {
        let extension = export_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let result = match extension.as_str() {
            "json" => report::graph_export::export_json(&graph, &entry_points, export_path),
            "dot" | "gv" => report::graph_export::export_dot(&graph, &entry_points, export_path),
            other => {
                eprintln!(
                    "{}: unknown graph format '.{other}' — use .json or .dot",
                    "Error".red()
                );
                std::process::exit(2);
            }
        };
        match result {
            Ok(_) => {
                println!("✅ Wrote reference graph: {}", export_path.display());
                return Ok(());
            }
            Err(e) => {
                eprintln!("{}: failed to write graph: {}", "Error".red(), e);
                std::process::exit(2);
            }
        }
    }

    // --write-only-caches short-circuits everything: text scan only
    if cli.write_only_caches {
        match analysis::caches::write_only_cache_keys(&cli.path) {
            None => println!("{}", "no cache writes found — nothing to check".dimmed()),
            Some(keys) if keys.is_empty() => {
                println!("{}", "✓ no write-only cache keys found".green())
            }
            Some(keys) => {
                println!("{}", "Cache keys written but never read back:".bold());
                for entry in keys {
                    let rel = entry.file.strip_prefix(&cli.path).unwrap_or(&entry.file);
                    println!(
                        "  {} \"{}\"  {}",
                        "○".dimmed(),
                        entry.key,
                        format!("{}:{}", rel.display(), entry.line).dimmed()
                    );
                }
                println!(
                    "{}",
                    "  (the compute-and-store pipeline behind each runs for nothing)".dimmed()
                );
            }
        }
        return Ok(());
    }

    // --import-suppressions short-circuits everything after the graph
    if let Some(ref baseline_path) = cli.import_suppressions {
        let mut baseline = if baseline_path.exists() {
            match baseline::Baseline::load(baseline_path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{}: cannot read baseline: {}", "Error".red(), e);
                    std::process::exit(2);
                }
            }
        } else {
            baseline::Baseline::from_findings(&[], &cli.path)
        };
        let mut imported = 0usize;
        for decl in graph.declarations() {
            // Only a suppression that NAMES this report imports the symbol.
            // `to_lowercase().contains("unused")` also matched
            // `@Suppress("UNUSED_PARAMETER")` on a class, which says nothing
            // about whether the class itself is reachable.
            let suppressed_unused = analysis::suppress::annotations_suppress(
                &decl.annotations,
                analysis::suppress::UNUSED_DECLARATION,
            );
            if !suppressed_unused {
                continue;
            }
            let synthetic =
                analysis::DeadCode::new(decl.clone(), analysis::DeadCodeIssue::Unreferenced);
            if baseline.is_baselined(&synthetic, &cli.path) {
                continue;
            }
            baseline
                .issues
                .push(baseline::IssueFingerprint::from_dead_code(
                    &synthetic, &cli.path,
                ));
            imported += 1;
        }
        if imported > 0 {
            if let Err(e) = baseline.save(baseline_path) {
                eprintln!("{}: cannot write baseline: {}", "Error".red(), e);
                std::process::exit(2);
            }
        }
        println!(
            "✅ Imported {imported} suppression(s) into {} ({} total entries)",
            baseline_path.display(),
            baseline.issues.len()
        );
        return Ok(());
    }

    // --import-detekt-baseline short-circuits everything after the graph
    if let Some(ref xml_path) = cli.import_detekt_baseline {
        let baseline_path = cli.baseline.as_ref().expect("clap requires --baseline");
        let xml = match std::fs::read_to_string(xml_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!(
                    "{}: cannot read {}: {}",
                    "Error".red(),
                    xml_path.display(),
                    e
                );
                std::process::exit(2);
            }
        };
        let mut baseline = if baseline_path.exists() {
            match baseline::Baseline::load(baseline_path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{}: cannot read baseline: {}", "Error".red(), e);
                    std::process::exit(2);
                }
            }
        } else {
            baseline::Baseline::from_findings(&[], &cli.path)
        };
        let mut imported = 0usize;
        let mut skipped = 0usize;
        for id in xml
            .split("<ID>")
            .skip(1)
            .filter_map(|chunk| chunk.split("</ID>").next())
        {
            // Detekt IDs read RuleId:FileName.kt$Signature
            let Some((rule, rest)) = id.split_once(':') else {
                continue;
            };
            if !rule.starts_with("Unused") {
                continue;
            }
            let Some((file_name, signature)) = rest.split_once('$') else {
                skipped += 1;
                continue;
            };
            // the signature quotes the declaration; its last keyword-led
            // identifier is the symbol name
            let symbol = signature
                .split_whitespace()
                .collect::<Vec<_>>()
                .windows(2)
                .rev()
                .find(|w| {
                    matches!(
                        w[0],
                        "fun" | "class" | "object" | "interface" | "val" | "var"
                    )
                })
                .map(|w| {
                    w[1].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_')
                        .split('(')
                        .next()
                        .unwrap_or("")
                        .to_string()
                });
            let Some(symbol) = symbol.filter(|s| !s.is_empty()) else {
                skipped += 1;
                continue;
            };
            let matched = graph.declarations().find(|decl| {
                decl.name == symbol
                    && decl
                        .id
                        .file
                        .file_name()
                        .map(|f| f.to_string_lossy() == file_name)
                        .unwrap_or(false)
            });
            let Some(decl) = matched else {
                skipped += 1;
                continue;
            };
            let synthetic =
                analysis::DeadCode::new(decl.clone(), analysis::DeadCodeIssue::Unreferenced);
            if baseline.is_baselined(&synthetic, &cli.path) {
                continue;
            }
            baseline
                .issues
                .push(baseline::IssueFingerprint::from_dead_code(
                    &synthetic, &cli.path,
                ));
            imported += 1;
        }
        if imported > 0 {
            if let Err(e) = baseline.save(baseline_path) {
                eprintln!("{}: cannot write baseline: {}", "Error".red(), e);
                std::process::exit(2);
            }
        }
        println!(
            "✅ Imported {imported} Detekt entr{} into {} ({} total entries, {skipped} skipped)",
            if imported == 1 { "y" } else { "ies" },
            baseline_path.display(),
            baseline.issues.len()
        );
        return Ok(());
    }

    // --unscheduled-workers short-circuits everything after the graph
    if cli.unscheduled_workers {
        const WORKER_BASES: &[&str] = &[
            "Worker",
            "CoroutineWorker",
            "ListenableWorker",
            "RxWorker",
            "JobService",
            "JobIntentService",
        ];
        let mut orphans: Vec<&graph::Declaration> = graph
            .declarations()
            .filter(|decl| {
                decl.kind.is_type()
                    && decl.super_types.iter().any(|s| {
                        let base = s.split('(').next().unwrap_or(s).trim();
                        let simple = base.rsplit('.').next().unwrap_or(base);
                        WORKER_BASES.contains(&simple)
                    })
                    && graph.get_references_to(&decl.id).is_empty()
            })
            .collect();
        if orphans.is_empty() {
            println!("{}", "✓ no unscheduled workers found".green());
            return Ok(());
        }
        orphans.sort_by(|a, b| {
            a.location
                .file
                .cmp(&b.location.file)
                .then(a.location.line.cmp(&b.location.line))
        });
        println!("{}", "Workers nobody ever enqueues:".bold());
        for decl in orphans {
            let rel = decl
                .location
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&decl.location.file);
            println!(
                "  {} {:<25} {}",
                "○".dimmed(),
                decl.name,
                format!("{}:{}", rel.display(), decl.location.line).dimmed()
            );
        }
        println!(
            "{}",
            "  (no WorkRequest, no enqueue, no schedule — background code that never runs)"
                .dimmed()
        );
        return Ok(());
    }

    // --promises short-circuits everything after the graph
    if cli.promises {
        let promises = analysis::promises::deletion_promises(&graph);
        if promises.is_empty() {
            println!(
                "{}",
                "no deletion promises found (TODO remove / FIXME delete)".dimmed()
            );
            return Ok(());
        }
        println!("{}", "Written deletion promises:".bold());
        for promise in promises {
            let verdict = match promise.state {
                analysis::promises::PromiseState::Ready => {
                    "ready to honor (0 references)".green().to_string()
                }
                analysis::promises::PromiseState::StillReferenced(n) => {
                    format!("still referenced ({n})").yellow().to_string()
                }
            };
            let rel = promise
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&promise.file);
            println!(
                "  {} {:<25} {}  {}",
                "○".dimmed(),
                promise.symbol,
                verdict,
                format!("{}:{}", rel.display(), promise.line).dimmed()
            );
        }
        return Ok(());
    }

    // --dead-serializables short-circuits everything after the graph:
    // kotlinx.serialization needs a static reference, so a DTO with
    // zero incoming references survives only through its annotation
    if cli.dead_serializables {
        let mut corpses: Vec<&graph::Declaration> = graph
            .declarations()
            .filter(|decl| {
                decl.kind.is_type()
                    && decl.annotations.iter().any(|a| {
                        a.contains("Serializable")
                            || a.contains("JsonClass")
                            || a.contains("SerialName")
                    })
                    && graph.get_references_to(&decl.id).is_empty()
            })
            .collect();
        if corpses.is_empty() {
            println!("{}", "✓ no dead serializables found".green());
            return Ok(());
        }
        corpses.sort_by(|a, b| {
            a.location
                .file
                .cmp(&b.location.file)
                .then(a.location.line.cmp(&b.location.line))
        });
        println!(
            "{}",
            "Serializable classes kept alive only by their annotation:".bold()
        );
        for decl in corpses {
            let rel = decl
                .location
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&decl.location.file);
            println!(
                "  {} {:<25} {}",
                "○".dimmed(),
                decl.name,
                format!("{}:{}", rel.display(), decl.location.line).dimmed()
            );
        }
        println!(
            "{}",
            "  (kotlinx.serialization needs a static reference — zero references means nothing (de)serializes it)"
                .dimmed()
        );
        return Ok(());
    }

    // --dead-di-modules short-circuits everything after the graph
    if cli.dead_di_modules {
        let findings = analysis::di_modules::dead_di_modules(&graph);
        if findings.is_empty() {
            println!("{}", "✓ no dead DI modules found".green());
            return Ok(());
        }
        println!("{}", "DI modules whose bindings nobody consumes:".bold());
        for finding in findings {
            let rel = finding
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&finding.file);
            println!(
                "  {} {:<25} {} unconsumed binding(s)  {}",
                "○".dimmed(),
                finding.name,
                finding.bindings,
                format!("{}:{}", rel.display(), finding.line).dimmed()
            );
        }
        println!(
            "{}",
            "  (delete the module and its providers together — a whole DI cluster)".dimmed()
        );
        return Ok(());
    }

    // --near-twins short-circuits everything after the graph
    if cli.near_twins {
        let findings = analysis::near_twins::near_twins(&graph);
        if findings.is_empty() {
            println!("{}", "✓ no near twins found".green());
            return Ok(());
        }
        println!(
            "{}",
            "Same-named functions with near-identical bodies:".bold()
        );
        for twin in findings {
            let left = twin.left.0.strip_prefix(&cli.path).unwrap_or(&twin.left.0);
            let right = twin
                .right
                .0
                .strip_prefix(&cli.path)
                .unwrap_or(&twin.right.0);
            println!(
                "  {} {:<20} {:.0}% shared  {} <-> {}",
                "○".dimmed(),
                twin.name,
                twin.similarity * 100.0,
                format!("{}:{}", left.display(), twin.left.1).dimmed(),
                format!("{}:{}", right.display(), twin.right.1).dimmed()
            );
        }
        println!(
            "{}",
            "  (migration copy-paste — once one side dies, deduplicate the survivor)".dimmed()
        );
        return Ok(());
    }

    // --middlemen short-circuits everything after the graph
    if cli.middlemen {
        let findings = analysis::middlemen::middlemen(&graph);
        if findings.is_empty() {
            println!("{}", "✓ no middleman classes found".green());
            return Ok(());
        }
        println!("{}", "Classes that only forward to a delegate:".bold());
        for finding in findings {
            let rel = finding
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&finding.file);
            println!(
                "  {} {:<25} {} method(s), all → {}  {}",
                "○".dimmed(),
                finding.class,
                finding.methods,
                finding.receiver,
                format!("{}:{}", rel.display(), finding.line).dimmed()
            );
        }
        println!(
            "{}",
            "  (callers can talk to the delegate directly — inline the façade)".dimmed()
        );
        return Ok(());
    }

    // --dead-accessors short-circuits everything after the graph
    if cli.dead_accessors {
        let findings = analysis::accessors::dead_accessors(&graph);
        if findings.is_empty() {
            println!("{}", "✓ no dead accessor groups found".green());
            return Ok(());
        }
        println!("{}", "Bean properties nobody reads:".bold());
        for finding in findings {
            let verdict = match finding.verdict {
                analysis::accessors::AccessorVerdict::WriteOnly => {
                    "write-only (setter still called)".yellow().to_string()
                }
                analysis::accessors::AccessorVerdict::Dead => {
                    "dead (field + accessors can go)".red().to_string()
                }
            };
            let rel = finding
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&finding.file);
            println!(
                "  {} {}.{:<20} {}  {}",
                "○".dimmed(),
                finding.class,
                finding.field,
                verdict,
                format!("{}:{}", rel.display(), finding.line).dimmed()
            );
        }
        return Ok(());
    }

    // --necromancy short-circuits everything after the graph: does any
    // live code reference a symbol the baseline judged dead?
    if cli.necromancy {
        let baseline_path = resolve_baseline(cli).expect("guarded at startup");
        let loaded = match baseline::Baseline::load(&baseline_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{}: cannot read baseline: {}", "Error".red(), e);
                std::process::exit(2);
            }
        };
        let mut resurrections: Vec<(String, String, String)> = Vec::new();
        for entry in &loaded.issues {
            for decl in graph.find_by_name(&entry.name) {
                if let (Some(declared), Some(recorded)) =
                    (decl.fully_qualified_name.as_deref(), entry.fqn.as_deref())
                {
                    if declared != recorded {
                        continue;
                    }
                }
                for (referencer, _) in graph.get_references_to(&decl.id) {
                    let site = referencer
                        .location
                        .file
                        .strip_prefix(&cli.path)
                        .unwrap_or(&referencer.location.file);
                    resurrections.push((
                        entry.name.clone(),
                        referencer.name.clone(),
                        format!("{}:{}", site.display(), referencer.location.line),
                    ));
                }
            }
        }
        if resurrections.is_empty() {
            println!("{}", "✓ no resurrections — the dead stay dead".green());
            return Ok(());
        }
        println!("{}", "Baselined symbols being referenced again:".bold());
        resurrections.sort();
        resurrections.dedup();
        for (corpse, necromancer, site) in resurrections {
            println!(
                "  {} {}  raised by {}  {}",
                "☠".red(),
                corpse,
                necromancer,
                site.dimmed()
            );
        }
        println!(
            "{}",
            "  (use the replacement instead, or remove the baseline entry if the symbol is back for good)"
                .dimmed()
        );
        std::process::exit(3);
    }

    // --dead-keep-rules short-circuits everything after the graph
    if cli.dead_keep_rules {
        match analysis::keep_rules::dead_keep_rules(&cli.path, &graph) {
            None => println!("{}", "no proguard rules files (*.pro) found".dimmed()),
            Some(dead) if dead.is_empty() => {
                println!(
                    "{}",
                    "✓ every verifiable -keep rule still keeps something".green()
                )
            }
            Some(dead) => {
                println!("{}", "-keep rules pointing at vanished classes:".bold());
                for rule in dead {
                    let rel = rule.file.strip_prefix(&cli.path).unwrap_or(&rule.file);
                    println!(
                        "  {} {}  {}",
                        "○".dimmed(),
                        rule.spec,
                        rel.display().to_string().dimmed()
                    );
                }
                println!(
                    "{}",
                    "  (wildcard and library rules are skipped — unverifiable from sources alone)"
                        .dimmed()
                );
            }
        }
        return Ok(());
    }

    // --test-only short-circuits everything after the graph: the
    // subset of variant lifelines where every keeper is a test set
    if cli.test_only {
        const TEST_SETS: &[&str] = &["test", "androidTest", "testFixtures", "sharedTest"];
        let findings: Vec<_> = analysis::variant_scope::debug_only_symbols(&graph)
            .into_iter()
            .filter(|f| f.sets.iter().all(|s| TEST_SETS.contains(&s.as_str())))
            .collect();
        if findings.is_empty() {
            println!("{}", "✓ no test-only production symbols found".green());
            return Ok(());
        }
        println!("{}", "src/main symbols only the tests keep alive:".bold());
        for finding in findings {
            let rel = finding
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&finding.file);
            println!(
                "  {} {:<30} kept by: {}  {}",
                "○".dimmed(),
                finding.name,
                finding.sets.join(", "),
                format!("{}:{}", rel.display(), finding.line).dimmed()
            );
        }
        println!(
            "{}",
            "  (production ships them for nothing — delete the symbol and its tests together)"
                .dimmed()
        );
        return Ok(());
    }

    // --debug-only short-circuits everything after the graph
    if cli.debug_only {
        let findings = analysis::variant_scope::debug_only_symbols(&graph);
        if findings.is_empty() {
            println!("{}", "✓ no debug-only lifelines found".green());
            return Ok(());
        }
        println!(
            "{}",
            "src/main symbols alive only through another source set:".bold()
        );
        for finding in findings {
            let rel = finding
                .file
                .strip_prefix(&cli.path)
                .unwrap_or(&finding.file);
            println!(
                "  {} {:<30} kept by: {}  {}",
                "○".dimmed(),
                finding.name,
                finding.sets.join(", "),
                format!("{}:{}", rel.display(), finding.line).dimmed()
            );
        }
        println!(
            "{}",
            "  (dead in the release build — move the symbol into that source set or delete it)"
                .dimmed()
        );
        return Ok(());
    }

    // --twins short-circuits everything after the graph
    if cli.twins {
        let pairs = analysis::twins::version_twins(&graph);
        if pairs.is_empty() {
            println!("{}", "✓ no version twins found".green());
            return Ok(());
        }
        println!("{}", "Version twins (side by side)".bold());
        for pair in pairs {
            for side in [&pair.base, &pair.variant] {
                let verdict = if side.refs == 0 {
                    "  ← unreferenced".yellow().to_string()
                } else {
                    String::new()
                };
                println!(
                    "  {:<30} {:>3} ref(s)  {}{verdict}",
                    side.name,
                    side.refs,
                    format!(
                        "{}:{}",
                        side.id
                            .file
                            .strip_prefix(&cli.path)
                            .unwrap_or(&side.id.file)
                            .display(),
                        graph
                            .get_declaration(&side.id)
                            .map(|d| d.location.line)
                            .unwrap_or(0)
                    )
                    .dimmed()
                );
            }
            println!();
        }
        return Ok(());
    }

    // --stale-flags short-circuits everything: the Piranha inventory
    if cli.stale_flags {
        match analysis::remote_config::boolean_flags(&cli.path) {
            None => println!(
                "{}",
                "no remote_config_defaults.xml — nothing to inventory".dimmed()
            ),
            Some(flags) if flags.is_empty() => {
                println!("{}", "✓ defaults exist but hold no boolean flags".green())
            }
            Some(flags) => {
                println!("{}", "Feature flags in defaults".bold());
                for (key, default) in flags {
                    println!(
                        "  {} = {:<5}  {}",
                        key,
                        default,
                        format!("searchdeadcode --flag {key} --behavior enabled").dimmed()
                    );
                }
                println!(
                    "{}",
                    "  (run the suggested command to see what dies under each flag)".dimmed()
                );
            }
        }
        return Ok(());
    }

    // --retention-audit short-circuits everything after the graph
    if cli.retention_audit {
        let counts = entry_detector.annotation_retention_counts(&graph);
        if counts.is_empty() {
            println!("{}", "✓ no annotation-retained declarations".green());
            return Ok(());
        }
        let total = graph.declarations().count().max(1);
        println!(
            "{}",
            "Retention audit (declarations kept per annotation)".bold()
        );
        for (name, count) in counts {
            let share = count * 100 / total;
            let broad = if share >= 20 {
                " ⚠ broad — consider refining".yellow().to_string()
            } else {
                String::new()
            };
            println!(
                "  {count:>5}  @{name}  {}{broad}",
                format!("({share}% of declarations)").dimmed()
            );
        }
        return Ok(());
    }

    // --doctor short-circuits everything: checkup, verdict, exit
    if cli.doctor {
        if run_doctor(config, &graph, &cli.path) {
            return Ok(());
        }
        std::process::exit(2);
    }

    // --explain short-circuits the normal report
    if let Some(symbol) = cli.explain.as_deref() {
        let enhanced = EnhancedAnalyzer::new();
        let (_, reachable) = enhanced.analyze(&graph, &entry_points);
        explain_symbol(&graph, &entry_points, &reachable, symbol);
        return Ok(());
    }

    // --why-alive short-circuits the normal report
    if let Some(symbol) = cli.why_alive.as_deref() {
        let enhanced = EnhancedAnalyzer::new();
        let (_, reachable) = enhanced.analyze(&graph, &entry_points);
        why_alive_symbol(&graph, &entry_points, &reachable, symbol);
        return Ok(());
    }

    // --flag short-circuits the normal report
    if let Some(flag_name) = cli.flag.as_deref() {
        let enabled = matches!(cli.behavior, FlagBehavior::Enabled);
        let report = analysis::flags::dead_under_flag(&files, flag_name, enabled);
        println!(
            "🚩 Flag cleanup: {} = {} ({} gate site(s))",
            flag_name,
            if enabled { "enabled" } else { "disabled" },
            report.gate_count
        );
        if report.dead_symbols.is_empty() {
            println!("Nothing dies with this flag.");
        } else {
            println!("Dead once the flag is burned in:");
            for name in &report.dead_symbols {
                match graph.find_by_name(name).first() {
                    Some(decl) => println!(
                        "   - {} — {}:{}",
                        name,
                        decl.location.file.display(),
                        decl.location.line
                    ),
                    None => println!("   - {}", name),
                }
            }
        }
        return Ok(());
    }

    // --module-usage short-circuits the normal report
    if let Some(module) = cli.module_usage.as_deref() {
        print_module_usage(&graph, module, &cli.path);
        return Ok(());
    }

    // --compare short-circuits the normal report
    if let Some(spec) = cli.compare.as_deref() {
        let (old_token, new_token) = spec.split_once('=').unwrap_or((spec, ""));
        let report = analysis::migration::compare(&graph, old_token, new_token);
        print_migration_report(&graph, old_token, new_token, &report);
        return Ok(());
    }

    // --kill-list short-circuits the normal report
    if let Some(symbol) = cli.kill_list.as_deref() {
        let targets: std::collections::HashSet<graph::DeclarationId> =
            match graph.find_by_fqn(symbol) {
                Some(decl) => std::iter::once(decl.id.clone()).collect(),
                None => graph
                    .find_by_name(symbol)
                    .iter()
                    .map(|d| d.id.clone())
                    .collect(),
            };
        if targets.is_empty() {
            println!("Symbol '{}' not found in the analyzed project.", symbol);
            return Ok(());
        }
        let list = analysis::kill_list::kill_list(&graph, &entry_points, &targets);
        print_kill_list(&graph, symbol, &list);
        return Ok(());
    }

    // Step 4: Load ProGuard data early if available (needed for enhanced mode)
    // R8 writes usage.txt at a well-known path; when the flag is absent,
    // go look there instead of making every run pass it by hand
    let proguard_usage_path = cli.proguard_usage.clone().or_else(|| {
        let found = discover_usage_txt(&cli.path);
        if let Some(ref path) = found {
            println!(
                "{}",
                format!("📋 Auto-discovered R8 usage.txt: {}", path.display()).cyan()
            );
        }
        found
    });
    let proguard_data = if let Some(ref usage_path) = proguard_usage_path {
        info!("Loading ProGuard usage.txt from {:?}...", usage_path);
        match ProguardUsage::parse(usage_path) {
            Ok(data) => {
                let stats = data.stats();
                info!("ProGuard usage: {}", stats);
                println!(
                    "{}",
                    format!(
                        "📋 ProGuard usage.txt: {} unused items ({} classes, {} methods)",
                        stats.total, stats.classes, stats.methods
                    )
                    .cyan()
                );
                Some(data)
            }
            Err(e) => {
                eprintln!("{}: Failed to load usage.txt: {}", "Warning".yellow(), e);
                None
            }
        }
    } else {
        None
    };

    // Step 5: Run reachability analysis (deep, enhanced, or standard)
    info!("Running reachability analysis...");

    let analysis_start = std::time::Instant::now();
    // Never frozen: the analysis phase can run for a minute on big repos
    let spinner = if cli.quiet {
        None
    } else {
        let sp = ProgressBar::new_spinner();
        sp.set_message("analysis…");
        sp.enable_steady_tick(std::time::Duration::from_millis(120));
        Some(sp)
    };
    let (mode_label, (dead_code, reachable)) = if cli.deep {
        // Deep analysis mode - most aggressive
        let deep = DeepAnalyzer::new()
            .with_parallel(cli.parallel)
            .with_unused_members(true);
        ("deep mode", deep.analyze(&graph, &entry_points))
    } else if cli.enhanced && proguard_data.is_some() {
        // Enhanced mode with ProGuard cross-validation
        let mut enhanced = EnhancedAnalyzer::new();
        if let Some(pg) = proguard_data.clone() {
            enhanced = enhanced.with_proguard(pg);
        }
        (
            "enhanced with ProGuard",
            enhanced.analyze(&graph, &entry_points),
        )
    } else if cli.parallel {
        // Standard analysis with parallel analyzer
        let enhanced = EnhancedAnalyzer::new();
        ("standard", enhanced.analyze(&graph, &entry_points))
    } else {
        // Standard sequential analysis
        let analyzer = ReachabilityAnalyzer::new();
        (
            "standard",
            analyzer.find_unreachable_with_reachable(&graph, &entry_points),
        )
    };
    if let Some(sp) = spinner {
        sp.finish_and_clear();
    }
    phase_line(
        cli.quiet,
        "analysis",
        &format!(
            "{}, {} reachable of {} in {:.2}s",
            mode_label,
            reachable.len(),
            graph.declarations().count(),
            analysis_start.elapsed().as_secs_f64()
        ),
    );

    info!(
        "Reachability: {} reachable, {} total",
        reachable.len(),
        graph.declarations().count()
    );

    // Step 6: Load coverage data if provided
    let coverage_data = if !cli.coverage.is_empty() {
        info!(
            "Loading coverage data from {} file(s)...",
            cli.coverage.len()
        );
        match parse_coverage_files(&cli.coverage) {
            Ok(data) => {
                let stats = data.stats();
                info!(
                    "Coverage: {} files, {} classes ({:.1}% covered), {} methods ({:.1}% covered)",
                    stats.total_files,
                    stats.total_classes,
                    stats.class_coverage_percent(),
                    stats.total_methods,
                    stats.method_coverage_percent()
                );
                Some(data)
            }
            Err(e) => {
                eprintln!("{}: Failed to load coverage: {}", "Warning".yellow(), e);
                None
            }
        }
    } else {
        None
    };

    // Step 7: Generate filtered report if requested
    if let Some(ref report_path) = cli.generate_report {
        if let Some(ref proguard) = proguard_data {
            info!("Generating filtered dead code report...");
            let generator = ReportGenerator::new().with_package_filter(cli.report_package.clone());

            match generator.generate(proguard, report_path) {
                Ok(stats) => {
                    println!(
                        "{}",
                        format!(
                            "📝 Report generated: {} ({} classes, {} filtered)",
                            report_path.display(),
                            stats.classes,
                            stats.filtered_generated
                        )
                        .green()
                    );
                }
                Err(e) => {
                    eprintln!("{}: Failed to generate report: {}", "Error".red(), e);
                }
            }
        } else {
            eprintln!(
                "{}",
                "Error: --generate-report requires --proguard-usage".red()
            );
        }
    }

    // Step 8: Enhance findings with hybrid analysis
    let mut hybrid = HybridAnalyzer::new();
    if let Some(coverage) = coverage_data {
        hybrid = hybrid.with_coverage(coverage);
    }
    if let Some(proguard) = proguard_data.clone() {
        hybrid = hybrid.with_proguard(proguard);
    }

    let mut dead_code = hybrid.enhance_findings(dead_code);

    // Step 9: Find runtime-dead code (reachable but never executed)
    if cli.include_runtime_dead {
        let runtime_dead = hybrid.find_runtime_dead_code(&graph, &reachable);
        if !runtime_dead.is_empty() {
            info!(
                "Found {} additional runtime-dead code items",
                runtime_dead.len()
            );
            dead_code.extend(runtime_dead);
        }
    }

    // Step 9b: Detect unused parameters
    if cli.unused_params {
        let param_detector = UnusedParamDetector::new();
        let unused_params = param_detector.detect(&graph);
        if !unused_params.is_empty() {
            info!("Found {} unused parameters", unused_params.len());
            dead_code.extend(unused_params);
        }
    }

    // Step 9c: Detect write-only variables (Phase 9)
    if cli.write_only {
        let write_only_detector = WriteOnlyDetector::new();
        let write_only_vars = write_only_detector.detect(&graph);
        if !write_only_vars.is_empty() {
            info!("Found {} write-only variables", write_only_vars.len());
            dead_code.extend(write_only_vars);
        }
    }

    // Step 9d: Detect unused sealed variants (Phase 10)
    if cli.sealed_variants {
        let sealed_detector = UnusedSealedVariantDetector::new();
        let sealed_issues = sealed_detector.detect(&graph);
        if !sealed_issues.is_empty() {
            info!("Found {} unused sealed variants", sealed_issues.len());
            dead_code.extend(sealed_issues);
        }
    }

    // Step 9e: Detect redundant overrides (Phase 10)
    if cli.redundant_overrides {
        let override_detector = RedundantOverrideDetector::new();
        let override_issues = override_detector.detect(&graph);
        if !override_issues.is_empty() {
            info!("Found {} redundant overrides", override_issues.len());
            dead_code.extend(override_issues);
        }
    }

    // Step 9e2: DC005 guard and DC007 literal-false branches.
    // The reachability analyzers already emit DC005 now that the parser
    // extracts enum entries; an enum iterated reflectively keeps its cases.
    {
        let reflective = analysis::detectors::reflectively_iterated_enum_ids(&graph);
        dead_code.retain(|dc| {
            if !matches!(dc.issue, analysis::DeadCodeIssue::UnusedEnumCase) {
                return true;
            }
            // An enum declared in a test source set is the test's business.
            if analysis::test_refs::is_test_file(&dc.declaration.location.file) {
                return false;
            }
            !dc.declaration
                .parent
                .as_ref()
                .is_some_and(|p| reflective.contains(p))
        });
        let branch_issues = DeadBranchDetector::new().detect(&graph);
        if !branch_issues.is_empty() {
            info!("Found {} dead branches", branch_issues.len());
            dead_code.extend(branch_issues);
        }
        // DC006: public but only used in its own module. Entry points stay
        // public — the framework reaches them from outside the graph.
        let public_issues: Vec<_> = RedundantPublicDetector::new()
            .detect(&graph)
            .into_iter()
            .filter(|dc| !entry_points.contains(&dc.declaration.id))
            .collect();
        if !public_issues.is_empty() {
            info!(
                "Found {} redundant public declarations",
                public_issues.len()
            );
            dead_code.extend(public_issues);
        }
    }

    // Step 9e3: opt-in style lints (DC014-16) — style, not deadness
    if cli.style {
        let style_issues: Vec<_> = RedundantThisDetector::new()
            .detect(&graph)
            .into_iter()
            .chain(RedundantParenthesesDetector::new().detect(&graph))
            .chain(PreferIsEmptyDetector::new().detect(&graph))
            .collect();
        if !style_issues.is_empty() {
            info!("Found {} style findings", style_issues.len());
            dead_code.extend(style_issues);
        }
    }

    // Step 9e4: DC012 duplicate imports and DC013 redundant Java '= null'
    {
        let import_issues = DuplicateImportDetector::new().detect(&graph);
        if !import_issues.is_empty() {
            info!("Found {} duplicate imports", import_issues.len());
            dead_code.extend(import_issues);
        }
        let null_issues = RedundantNullInitDetector::new().detect(&graph);
        if !null_issues.is_empty() {
            info!("Found {} redundant null initializations", null_issues.len());
            dead_code.extend(null_issues);
        }
    }

    // Step 9f: Unused Android resources — findings flow through the
    // standard report so JSON/SARIF/baseline see them (DC017)
    if cli.unused_resources {
        let resource_detector = ResourceDetector::new();
        let resource_analysis = resource_detector.analyze(&cli.path);
        if !resource_analysis.unused.is_empty() {
            info!(
                "Found {} unused resources ({} total defined, {} referenced)",
                resource_analysis.unused.len(),
                resource_analysis
                    .defined
                    .values()
                    .map(|m| m.len())
                    .sum::<usize>(),
                resource_analysis.referenced.len()
            );
            // getIdentifier() resolves resources by runtime-built names:
            // findings of a reachable type stay reported but high-risk
            let dynamic_probe = analysis::resources::dynamic_resource_probe(&cli.path);
            for resource in &resource_analysis.unused {
                let at_risk = dynamic_probe
                    .as_ref()
                    .map(|p| p.puts_at_risk(&resource.resource_type))
                    .unwrap_or(false);
                let message = if at_risk {
                    format!(
                        "{} '{}' is defined but never referenced — getIdentifier() in this codebase may resolve it dynamically",
                        resource.resource_type, resource.name
                    )
                } else {
                    format!(
                        "{} '{}' is defined but never referenced",
                        resource.resource_type, resource.name
                    )
                };
                let mut finding = synthetic_finding(
                    &resource.file,
                    resource.line,
                    &resource.name,
                    graph::DeclarationKind::Property,
                    analysis::DeadCodeIssue::UnusedResource,
                    message,
                    // Medium, not Low: dropping below the default
                    // confidence floor would hide the finding instead
                    // of flagging it risky
                    if at_risk {
                        analysis::Confidence::Medium
                    } else {
                        analysis::Confidence::High
                    },
                );
                if at_risk {
                    finding.risk = analysis::RiskLevel::High;
                }
                dead_code.push(finding);
            }
        }
    }

    // Step 9f2: Dead layouts — same route, DC018. The dynamic-inflation
    // caveat lives in the message.
    {
        let dead_layouts = analysis::layouts::find_dead_layouts(&files);
        for layout in &dead_layouts {
            let stem = layout
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| layout.display().to_string());
            dead_code.push(synthetic_finding(
                layout,
                1,
                &stem,
                graph::DeclarationKind::File,
                analysis::DeadCodeIssue::UnusedLayout,
                format!(
                    "Layout '{stem}' has no Binding usage, no R.layout and no include \
                     (check for getIdentifier()-style dynamic inflation before deleting)"
                ),
                analysis::Confidence::Medium,
            ));
        }
    }

    // Step 9f3: Event-bus orphans (cheap regex pass over the sources).
    // Hissé hors du bloc : les hints de fin de rapport le réutilisent.
    let bus_report = {
        let mut corpus = String::new();
        for file in &files {
            if matches!(
                file.file_type,
                discovery::FileType::Kotlin | discovery::FileType::Java
            ) {
                if let Ok(content) = std::fs::read_to_string(&file.path) {
                    corpus.push_str(&content);
                    corpus.push('\n');
                }
            }
        }
        analysis::bus::analyze(&corpus)
    };
    {
        if !bus_report.is_empty() && !cli.quiet {
            println!();
            println!("{}", "🚌 Event bus orphans:".yellow().bold());
            if !bus_report.posted_never_subscribed.is_empty() {
                println!("  posted but never subscribed:");
                for event in &bus_report.posted_never_subscribed {
                    println!("    {} {}", "○".dimmed(), event);
                }
            }
            if !bus_report.subscribed_never_posted.is_empty() {
                println!("  subscribed but never posted:");
                for event in &bus_report.subscribed_never_posted {
                    println!("    {} {}", "○".dimmed(), event);
                }
                if bus_report.dynamic_posts > 0 {
                    println!(
                        "{}",
                        format!(
                            "    ({} dynamic post(s) found — some of these may be posted through variables)",
                            bus_report.dynamic_posts
                        )
                        .dimmed()
                    );
                }
            }
            println!();
        }
    }

    // Step 9f4: Remote Config keys declared in defaults but never read
    {
        // A sweep over every key (an admin panel reading `getInstance().all`)
        // is not a behavioural read: nothing acts on that key. Keep the
        // finding, but the caveat says where to look before deleting, and
        // confidence drops since removal changes what a screen displays.
        let sweep = analysis::remote_config::key_sweep_site(&files);
        for (key, file, line) in analysis::remote_config::dead_keys(&cli.path, &files) {
            let (message, confidence) = match &sweep {
                Some(site) => (
                    format!(
                        "Remote Config key \"{key}\" is declared in defaults and read by no name — a whole-key sweep at {site} still surfaces it"
                    ),
                    analysis::Confidence::Medium,
                ),
                None => (
                    format!("Remote Config key \"{key}\" is declared in defaults but never read"),
                    analysis::Confidence::High,
                ),
            };
            dead_code.push(synthetic_finding(
                &file,
                line,
                &key,
                graph::DeclarationKind::Property,
                analysis::DeadCodeIssue::DeadConfigKey,
                message,
                confidence,
            ));
        }
    }

    // Step 9f5: serialized DTO fields nobody reads (DC021)
    {
        for (name, file, line) in analysis::dto_fields::dead_fields(&files) {
            dead_code.push(synthetic_finding(
                &file,
                line,
                &name,
                graph::DeclarationKind::Property,
                analysis::DeadCodeIssue::DeadDtoField,
                format!("DTO field '{name}' is deserialized but never read by the code"),
                analysis::Confidence::Medium,
            ));
        }
    }

    // Step 9f6: locale strings whose base key was removed (DC022)
    {
        for (key, file, line) in analysis::translations::orphan_translations(&cli.path) {
            dead_code.push(synthetic_finding(
                &file,
                line,
                &key,
                graph::DeclarationKind::Property,
                analysis::DeadCodeIssue::OrphanTranslation,
                format!(
                    "Translation '{key}' has no base entry in values/ — it can never be resolved"
                ),
                analysis::Confidence::High,
            ));
        }
    }

    // Step 9g: Unused Intent extras — through the standard report (DC019)
    if cli.unused_extras {
        let intent_detector = UnusedIntentExtraDetector::new();
        let intent_analysis = intent_detector.analyze(&cli.path);
        if !intent_analysis.unused_extras.is_empty() {
            info!(
                "Found {} unused Intent extras ({} total put, {} retrieved)",
                intent_analysis.unused_extras.len(),
                intent_analysis.total_put,
                intent_analysis.total_get
            );
            for extra in &intent_analysis.unused_extras {
                dead_code.push(synthetic_finding(
                    &extra.file,
                    extra.line,
                    &extra.key,
                    graph::DeclarationKind::Property,
                    analysis::DeadCodeIssue::UnusedIntentExtra,
                    format!("putExtra(\"{}\") is never retrieved", extra.key),
                    analysis::Confidence::Medium,
                ));
            }
        }
    }

    // Step 9h: Detect write-only SharedPreferences (Phase 9)
    if cli.write_only_prefs {
        use analysis::detectors::WriteOnlyPrefsDetector;
        use discovery::FileType;
        let prefs_detector = WriteOnlyPrefsDetector::new();

        // Analyze Kotlin AND Java files for SharedPreferences usage —
        // put/get patterns read the same in both languages
        let mut prefs_analysis = analysis::detectors::SharedPrefsAnalysis::new();
        let mut prefs_corpus = String::new();
        for file in &files {
            if matches!(file.file_type, FileType::Kotlin | FileType::Java) {
                if let Ok(content) = std::fs::read_to_string(&file.path) {
                    let file_analysis = prefs_detector.analyze_source(&content, &file.path);
                    // Merge results
                    for (key, locs) in file_analysis.writes {
                        for loc in locs {
                            prefs_analysis.add_write(key.clone(), loc.file, loc.line);
                        }
                    }
                    for (key, locs) in file_analysis.reads {
                        for loc in locs {
                            prefs_analysis.add_read(key.clone(), loc.file, loc.line);
                        }
                    }
                    prefs_analysis.dynamic_reads += file_analysis.dynamic_reads;
                    prefs_corpus.push_str(&content);
                    prefs_corpus.push('\n');
                }
            }
        }

        // Une écriture via constante et une lecture via littéral (ou une
        // référence qualifiée) désignent la même clé une fois résolues
        analysis::detectors::resolve_constant_keys(&mut prefs_analysis, &prefs_corpus);

        let write_only_keys = prefs_analysis.get_write_only_keys();
        if prefs_analysis.dynamic_reads > 0 {
            // Un wrapper à clés paramétrées lit des clés qu'aucun scan ne
            // peut énumérer : le verdict write-only n'est pas prouvable.
            if !write_only_keys.is_empty() && !cli.quiet {
                println!();
                println!(
                    "{}",
                    format!(
                        "🔑 {} preference read(s) go through a variable key — write-only verdicts are unprovable, {} candidate key(s) not reported",
                        prefs_analysis.dynamic_reads,
                        write_only_keys.len()
                    )
                    .dimmed()
                );
            }
        } else if !write_only_keys.is_empty() {
            info!(
                "Found {} write-only SharedPreferences keys",
                write_only_keys.len()
            );
            for key in write_only_keys {
                if let Some(locs) = prefs_analysis.writes.get(key) {
                    for loc in locs {
                        dead_code.push(synthetic_finding(
                            &loc.file,
                            loc.line,
                            key,
                            graph::DeclarationKind::Property,
                            analysis::DeadCodeIssue::WriteOnlyPreference,
                            format!("SharedPreferences key \"{key}\" is written but never read"),
                            analysis::Confidence::Medium,
                        ));
                    }
                }
            }
        }
    }

    // Step 9i: Detect write-only Room DAOs (Phase 9)
    if cli.write_only_dao {
        use analysis::detectors::WriteOnlyDaoDetector;
        use discovery::FileType;
        let dao_detector = WriteOnlyDaoDetector::new();

        // Analyze Kotlin AND Java files for DAO definitions — Room
        // annotations are identical in both languages
        let mut dao_analysis = analysis::detectors::DaoCollectionAnalysis::new();
        for file in &files {
            if matches!(file.file_type, FileType::Kotlin | FileType::Java) {
                if let Ok(content) = std::fs::read_to_string(&file.path) {
                    let file_analysis = dao_detector.analyze_source(&content, &file.path);
                    dao_analysis.daos.extend(file_analysis.daos);
                }
            }
        }

        let write_only_daos = dao_analysis.get_write_only_daos();
        if !write_only_daos.is_empty() {
            info!("Found {} write-only Room DAOs", write_only_daos.len());
            for dao in write_only_daos {
                let writers: Vec<String> =
                    dao.write_methods().iter().map(|m| m.name.clone()).collect();
                dead_code.push(synthetic_finding(
                    &dao.file,
                    dao.line,
                    &dao.name,
                    graph::DeclarationKind::Interface,
                    analysis::DeadCodeIssue::WriteOnlyDao,
                    format!(
                        "DAO '{}' has @Insert but no @Query (writers: {})",
                        dao.name,
                        writers.join(", ")
                    ),
                    analysis::Confidence::Medium,
                ));
            }
        }
    }

    // Step 9j: Anti-pattern detectors — CLI flags or config groups
    let ap_config = &config.detection.anti_patterns;
    let limits = &config.detection.thresholds;
    let run_architecture = cli.anti_patterns
        || cli.architecture_patterns
        || ap_config.enabled
        || ap_config.architecture;
    let run_kotlin =
        cli.anti_patterns || cli.kotlin_patterns || ap_config.enabled || ap_config.kotlin;
    let run_performance =
        cli.anti_patterns || cli.performance_patterns || ap_config.enabled || ap_config.performance;
    let run_android =
        cli.anti_patterns || cli.android_patterns || ap_config.enabled || ap_config.android;
    let run_compose =
        cli.anti_patterns || cli.compose_patterns || ap_config.enabled || ap_config.compose;

    // Architecture patterns (AP001-AP006)
    if run_architecture {
        let detectors: Vec<Box<dyn Detector>> = vec![
            Box::new(DeepInheritanceDetector::new().with_max_depth(limits.deep_inheritance_depth)),
            Box::new(EventBusPatternDetector::new()),
            Box::new(GlobalMutableStateDetector::new()),
            Box::new(SingleImplInterfaceDetector::new()),
        ];
        for detector in detectors {
            let issues = detector.detect(&graph);
            if !issues.is_empty() {
                dead_code.extend(issues);
            }
        }
        info!("Architecture pattern analysis complete");
    }

    // Kotlin patterns (AP007-AP010, AP021-AP025)
    if run_kotlin {
        let detectors: Vec<Box<dyn Detector>> = vec![
            // Phase 1
            Box::new(GlobalScopeUsageDetector::new()),
            Box::new(HeavyViewModelDetector::new()),
            Box::new(LateinitAbuseDetector::new()),
            Box::new(ScopeFunctionChainingDetector::new()),
            // Phase 4
            Box::new(ComplexConditionDetector::new()),
            Box::new(
                LongParameterListDetector::new().with_max_parameters(limits.long_parameter_list),
            ),
            Box::new(NullabilityOverloadDetector::new()),
            Box::new(ReflectionOveruseDetector::new()),
            Box::new(StringLiteralDuplicationDetector::new()),
        ];
        for detector in detectors {
            let issues = detector.detect(&graph);
            if !issues.is_empty() {
                dead_code.extend(issues);
            }
        }
        info!("Kotlin pattern analysis complete");
    }

    // Performance patterns (AP011-AP015)
    if run_performance {
        let detectors: Vec<Box<dyn Detector>> = vec![
            Box::new(MemoryLeakRiskDetector::new()),
            Box::new(LongMethodDetector::new().with_max_lines(limits.long_method_lines)),
            Box::new(
                LargeClassDetector::new()
                    .with_max_methods(limits.large_class_methods)
                    .with_max_properties(limits.large_class_properties),
            ),
            Box::new(CollectionWithoutSequenceDetector::new()),
            Box::new(ObjectAllocationInLoopDetector::new()),
        ];
        for detector in detectors {
            let issues = detector.detect(&graph);
            if !issues.is_empty() {
                dead_code.extend(issues);
            }
        }
        info!("Performance pattern analysis complete");
    }

    // Android patterns (AP016-AP020, AP026-AP030)
    if run_android {
        let detectors: Vec<Box<dyn Detector>> = vec![
            // Phase 3
            Box::new(MutableStateExposedDetector::new()),
            Box::new(ViewLogicInViewModelDetector::new()),
            Box::new(MissingUseCaseDetector::new()),
            Box::new(NestedCallbackDetector::new()),
            Box::new(HardcodedDispatcherDetector::new()),
            // Phase 5
            Box::new(UnclosedResourceDetector::new()),
            Box::new(MainThreadDatabaseDetector::new()),
            Box::new(WakeLockAbuseDetector::new()),
            Box::new(AsyncTaskUsageDetector::new()),
            Box::new(InitOnDrawDetector::new()),
        ];
        for detector in detectors {
            let issues = detector.detect(&graph);
            if !issues.is_empty() {
                dead_code.extend(issues);
            }
        }
        info!("Android pattern analysis complete");
    }

    // Compose patterns (AP031-AP034)
    if run_compose {
        let detectors: Vec<Box<dyn Detector>> = vec![
            Box::new(StateWithoutRememberDetector::new()),
            Box::new(LaunchedEffectWithoutKeyDetector::new()),
            Box::new(BusinessLogicInComposableDetector::new()),
            Box::new(NavControllerPassingDetector::new()),
        ];
        for detector in detectors {
            let issues = detector.detect(&graph);
            if !issues.is_empty() {
                dead_code.extend(issues);
            }
        }
        info!("Compose pattern analysis complete");
    }

    // Step 9z: honor config.detection.* — deserialized-and-ignored until now
    {
        use analysis::DeadCodeIssue as I;
        use graph::DeclarationKind as K;
        let det = &config.detection;
        dead_code.retain(|dc| match dc.issue {
            I::Unreferenced => match dc.declaration.kind {
                K::Class | K::Interface | K::Object | K::Enum | K::TypeAlias | K::Annotation => {
                    det.unused_class
                }
                K::Function | K::Method | K::Constructor => det.unused_method,
                K::Property | K::Field => det.unused_property,
                _ => true,
            },
            I::UnusedImport | I::DuplicateImport => det.unused_import,
            I::UnusedParameter => det.unused_param,
            I::UnusedEnumCase => det.unused_enum_case,
            I::AssignOnly => det.assign_only,
            I::DeadBranch => det.dead_branch,
            I::RedundantPublic => det.redundant_public,
            _ => true,
        });
    }

    // Step 10: Filter by confidence level
    let min_confidence = parse_confidence(&resolve_min_confidence(cli));
    let mut dead_code: Vec<_> = dead_code
        .into_iter()
        .filter(|dc| dc.confidence >= min_confidence)
        .filter(|dc| !cli.runtime_only || dc.runtime_confirmed)
        .collect();

    // Step 10a: inline `deadcode:ignore(reason)` directives
    {
        let outcome = analysis::ignore::apply(&mut dead_code);
        if !outcome.ignored.is_empty() && !cli.quiet {
            let reasons: Vec<String> = outcome
                .ignored
                .iter()
                .map(|(name, reason)| format!("{name} ({reason})"))
                .collect();
            println!(
                "{}",
                format!(
                    "🤫 {} ignored inline: {}",
                    outcome.ignored.len(),
                    reasons.join(", ")
                )
                .dimmed()
            );
        }
        if outcome.missing_reason > 0 {
            eprintln!(
                "{}",
                format!(
                    "{} deadcode:ignore directive(s) refused — a reason is mandatory: deadcode:ignore(<why>)",
                    outcome.missing_reason
                )
                .yellow()
            );
        }
    }

    // Step 10a2: a dead symbol still named by a test file — the test
    // outlived its target, delete them together
    analysis::test_refs::annotate(&mut dead_code, &cli.path);

    // Step 10a3: `@file:Suppress` — the author already declined this report
    // for the whole file. The finding STAYS: a file-wide opt-out is not
    // evidence of life, and hiding it is how a temporary silence becomes
    // permanent. It carries the reservation and drops one notch of
    // confidence. Deliberately after the Step 10 confidence filter, so the
    // downgrade can never remove anything from the report.
    let reserved = analysis::suppress::annotate(&mut dead_code);
    if reserved > 0 && !cli.quiet {
        println!(
            "{}",
            format!(
                "{reserved} finding(s) in files marked @file:Suppress — kept, confidence lowered"
            )
            .dimmed()
        );
    }

    // Step 10b: ownership — after the filters, one git call per survivor
    if cli.blame {
        analysis::blame::annotate(&mut dead_code, &cli.path);
    }

    // Step 10c: --diff-base — drop everything already dead at the reference
    if let Some(ref base_ref) = cli.diff_base {
        match analysis::diff_base::reference_fingerprints(&cli.path, base_ref) {
            Ok(old) => {
                let before = dead_code.len();
                dead_code.retain(|dc| {
                    !old.contains(&analysis::diff_base::fingerprint_of(dc, &cli.path))
                });
                println!(
                    "{}",
                    format!(
                        "🔀 Since {base_ref}: {} new finding(s) ({} already dead there)",
                        dead_code.len(),
                        before - dead_code.len()
                    )
                    .cyan()
                );
            }
            Err(e) => {
                eprintln!("{}: --diff-base {base_ref}: {e}", "Error".red());
                std::process::exit(2);
            }
        }
    }
    // Les analyseurs trient leur propre sortie, mais une vingtaine de
    // détecteurs ont poussé la leur après coup, chacun dans son ordre. Un
    // dernier tri total ici est le seul endroit qui voit tout le rapport :
    // sans lui, deux exécutions identiques rendaient des sorties différentes,
    // et le diff de `check-corpus.sh` devenait du bruit.
    dead_code.sort_by(analysis::report_order);
    let dead_code = dead_code;

    info!("Found {} dead code candidates", dead_code.len());

    // Step 11: Detect zombie code cycles if requested
    if cli.detect_cycles {
        let cycle_detector = CycleDetector::new();
        let cycle_stats = cycle_detector.get_cycle_stats(&graph, &reachable);

        if cycle_stats.has_cycles() {
            println!();
            println!("{}", "🧟 Zombie Code Detected:".to_string().yellow().bold());
            println!(
                "  {} dead cycles found ({} declarations)",
                cycle_stats.num_dead_cycles, cycle_stats.total_declarations_in_cycles
            );
            if cycle_stats.largest_cycle_size > 2 {
                println!(
                    "  Largest cycle: {} mutually dependent declarations",
                    cycle_stats.largest_cycle_size
                );
            }
            if cycle_stats.num_zombie_pairs > 0 {
                println!(
                    "  {} zombie pairs (A↔B mutual references)",
                    cycle_stats.num_zombie_pairs
                );
            }

            // Print cycle details
            let dead_cycles = cycle_detector.find_dead_cycles(&graph, &reachable);
            for (i, cycle) in dead_cycles.iter().take(5).enumerate() {
                println!();
                println!(
                    "  {}",
                    format!("Cycle #{} ({} items):", i + 1, cycle.size).dimmed()
                );
                for name in cycle.names.iter().take(5) {
                    println!("    • {}", name);
                }
                if cycle.names.len() > 5 {
                    println!("    ... and {} more", cycle.names.len() - 5);
                }
            }
            if dead_cycles.len() > 5 {
                println!();
                println!("  ... and {} more cycles", dead_cycles.len() - 5);
            }
            println!();
        }
    }

    // Step 12: Generate baseline if requested
    if let Some(ref baseline_path) = cli.generate_baseline {
        info!("Generating baseline file...");
        let baseline = baseline::Baseline::from_findings(&dead_code, &cli.path);
        match baseline.save(baseline_path) {
            Ok(_) => {
                println!(
                    "{}",
                    format!(
                        "📋 Baseline generated: {} ({} issues)",
                        baseline_path.display(),
                        dead_code.len()
                    )
                    .green()
                );
            }
            Err(e) => {
                eprintln!("{}: Failed to generate baseline: {}", "Error".red(), e);
            }
        }
    }

    // Step 13: Filter by baseline if provided — or, under --profile ci, if the
    // project committed one under its conventional name
    let mut ratchet_failed = false;
    let effective_baseline = resolve_baseline(cli);
    let dead_code = if let Some(ref baseline_path) = effective_baseline {
        match baseline::Baseline::load(baseline_path) {
            Ok(baseline) => {
                // --baseline-prune: entries no finding matches anymore are
                // resolved — drop them and rewrite the file
                let baseline = if cli.baseline_prune {
                    let mut pruned = baseline;
                    let before = pruned.issues.len();
                    pruned
                        .issues
                        .retain(|fp| dead_code.iter().any(|dc| fp.matches(dc, &cli.path)));
                    let dropped = before - pruned.issues.len();
                    if dropped > 0 {
                        match pruned.save(baseline_path) {
                            Ok(_) => eprintln!(
                                "{}",
                                format!("🧹 Pruned {dropped} resolved entrie(s) from the baseline")
                                    .green()
                            ),
                            Err(e) => {
                                eprintln!(
                                    "{}: failed to rewrite baseline: {}",
                                    "Warning".yellow(),
                                    e
                                )
                            }
                        }
                    } else {
                        eprintln!(
                            "nothing to prune — every baseline entry still matches a finding"
                        );
                    }
                    pruned
                } else {
                    baseline
                };
                let stats = baseline.stats(&dead_code, &cli.path);
                eprintln!("{}", format!("📋 Baseline: {}", stats).cyan());

                // The ratchet only accepts decrease: new issues fail the
                // run, progress rewrites the ceiling downward.
                if cli.ratchet {
                    if stats.new_issues > 0 {
                        eprintln!(
                            "{}",
                            format!(
                                "Ratchet: {} new issue(s) over the baseline ceiling",
                                stats.new_issues
                            )
                            .red()
                        );
                        ratchet_failed = true;
                    } else if stats.baselined_found < stats.total_in_baseline {
                        let tightened = baseline::Baseline::from_findings(&dead_code, &cli.path);
                        match tightened.save(baseline_path) {
                            Ok(_) => eprintln!(
                                "{}",
                                format!(
                                    "📉 Ratchet tightened: {} → {} issues",
                                    stats.total_in_baseline, stats.baselined_found
                                )
                                .green()
                            ),
                            Err(e) => eprintln!(
                                "{}: Failed to tighten baseline: {}",
                                "Warning".yellow(),
                                e
                            ),
                        }
                    }
                }

                // Only report new issues not in baseline
                let new_issues: Vec<_> = baseline
                    .filter_new(&dead_code, &cli.path)
                    .into_iter()
                    .cloned()
                    .collect();

                if new_issues.is_empty() && stats.baselined_found > 0 {
                    eprintln!("{}", "✓ No new dead code issues found!".green());
                }

                new_issues
            }
            Err(e) => {
                // Toujours exit 2, ratchet ou pas : le contrat documenté dit
                // « baseline corrompu = l'outil n'a pas pu travailler ». Le
                // demi-mode d'avant (avertir et rendre le rapport NON filtré)
                // faisait sortir 1 sous la porte — la CI disait « du code
                // mort » quand le vrai problème était le fichier. Un baseline
                // résolu existe forcément (resolve_baseline vérifie is_file),
                // donc l'échec signifie présent-mais-inutilisable.
                eprintln!(
                    "{}: cannot load baseline {}: {}",
                    "Error".red(),
                    baseline_path.display(),
                    e
                );
                std::process::exit(2);
            }
        }
    } else {
        if cli.ratchet {
            eprintln!(
                "{}: --ratchet needs --baseline <file> — a ratchet without a ceiling guards nothing",
                "Error".red()
            );
            std::process::exit(2);
        }
        dead_code
    };

    // Step 13b: Assess deletion risk on the final findings
    let mut dead_code = dead_code;
    analysis::risk::assess(&mut dead_code, &files);

    // Step 14: Report results
    let report_format = determine_report_format(cli, config);
    let mut report_options = report::ReportOptions::new();
    report_options.output_path = cli.output.clone();
    report_options.base_path = Some(cli.path.clone());
    report_options.expand_all = cli.expand;
    report_options.expand_rule = cli.expand_rule.clone();
    report_options.top_n = cli.top;
    report_options.files_count = Some(files.len());
    report_options.declarations_count = Some(graph.declarations().count());

    // --top-files replaces the report with a per-file impact ranking
    if let Some(limit) = cli.top_files {
        print_top_files(&graph, &dead_code, &cli.path, limit.max(1));
        return Ok(());
    }

    // --by-module replaces the report with a per-module summary
    if cli.by_module {
        print_by_module(&dead_code, &cli.path);
        return Ok(());
    }

    // --health replaces the report with the module report card
    if cli.health {
        if matches!(cli.format, Some(OutputFormat::Json)) {
            print_health_json(&graph, &dead_code, &cli.path);
        } else {
            print_health(&graph, &dead_code, &cli.path);
        }
        if let Some(ref floor) = cli.min_grade {
            let failing: Vec<String> = health_rows(&graph, &dead_code, &cli.path)
                .into_iter()
                .filter_map(|(module, corpses, total)| {
                    let percent = corpses as f64 * 100.0 / total.max(1) as f64;
                    // later letters are worse: a simple ordinal compare
                    (health_grade(percent) > floor.as_str()).then_some(module)
                })
                .collect();
            if !failing.is_empty() {
                println!(
                    "❌ {} module(s) below the min-grade {floor}: {}",
                    failing.len(),
                    failing.join(", ")
                );
                std::process::exit(3);
            }
        }
        return Ok(());
    }

    // --pr-description replaces the report with a paste-ready PR body
    if cli.pr_description {
        let body = pr_description_text(&dead_code, &cli.path);
        match &cli.output {
            Some(path) => {
                if let Err(e) = std::fs::write(path, &body) {
                    eprintln!("{}: cannot write {}: {}", "Error".red(), path.display(), e);
                    std::process::exit(2);
                }
                println!("✅ PR description written to {}", path.display());
            }
            None => print!("{body}"),
        }
        return Ok(());
    }

    // --tui replaces the report with the full-screen triage
    if cli.tui {
        use std::io::IsTerminal;
        if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
            let exit = tui::run(&dead_code, &cli.path, cli.baseline.as_deref())
                .map_err(|e| miette::miette!(e))?;
            if exit == tui::Exit::Refresh {
                // the analysis pipeline lives above this wedge: re-exec
                // the binary (incremental cache makes the rerun cheap)
                let exe = std::env::current_exe().map_err(|e| miette::miette!(e))?;
                let status = std::process::Command::new(exe)
                    .args(std::env::args_os().skip(1))
                    .status()
                    .map_err(|e| miette::miette!(e))?;
                std::process::exit(status.code().unwrap_or(0));
            }
            return Ok(());
        }
        eprintln!("--tui requires a terminal; printing the standard report instead.");
        // fall through to the normal report
    }

    // --batch-branches replaces the report with branch surgery
    if cli.batch_branches {
        match run_batch_branches(&cli.path, &dead_code) {
            Ok(created) => {
                if created == 0 {
                    println!(
                        "{}",
                        "✓ no dead top-level classes — no branches to cut".green()
                    );
                } else {
                    println!(
                        "{}",
                        format!("✅ Created {created} branch(es) under deadcode/ — CI validates, you merge")
                            .green()
                    );
                }
                return Ok(());
            }
            Err(message) => {
                eprintln!("{}: {}", "Error".red(), message);
                std::process::exit(2);
            }
        }
    }

    // --badge replaces the report with an SVG for the README
    if let Some(ref badge_path) = cli.badge {
        let total = graph.declarations().count();
        match write_badge(badge_path, &dead_code, total) {
            Ok(percent) => {
                println!("✅ Wrote badge ({percent}% dead): {}", badge_path.display());
                return Ok(());
            }
            Err(e) => {
                eprintln!("{}: failed to write badge: {}", "Error".red(), e);
                std::process::exit(2);
            }
        }
    }

    // --score replaces the report with a per-finding deletability ranking
    if cli.score {
        print_score_ranking(&dead_code, &cli.path);
        return Ok(());
    }

    // Interactive triage: fzf-style filtering with keyboard actions.
    // --delete --interactive keeps its historical confirm-each semantics.
    if cli.interactive && !cli.delete {
        use std::io::IsTerminal;
        if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
            interactive::run_triage(
                &graph,
                &entry_points,
                &reachable,
                dead_code,
                &cli.path,
                cli.undo_script.clone(),
            )?;
            return Ok(());
        }
        eprintln!("--interactive requires a terminal; printing the standard report instead.");
    }

    if cli.quick_wins {
        print_quick_wins(&graph, &dead_code);
        return Ok(());
    }

    if cli.islands {
        // Wildcard -keep rules retain shrinker bytes, not source: the island
        // analysis re-detects entry points without them and labels instead.
        let island_config = load_config(cli)?;
        let island_entries = EntryPointDetector::new(&island_config)
            .detect_with_keep_rules(&graph, &cli.path, false)?;
        let islands =
            analysis::islands::find_islands(&graph, &island_entries, &files, &cli.path, 8);
        print_islands(islands);
    } else if cli.clusters {
        let dead_ids: std::collections::HashSet<graph::DeclarationId> =
            dead_code.iter().map(|d| d.declaration.id.clone()).collect();
        let clusters = analysis::kill_list::dead_clusters(&graph, &dead_ids);
        print_clusters(&graph, clusters);
    } else {
        let reporter = Reporter::with_options(report_format, report_options);
        reporter.report(&dead_code)?;

        // Guide the next move — situations détectées + commandes prêtes à
        // copier, uniquement en sortie humaine (des jumeaux méritent un
        // hint même à zéro finding)
        let is_terminal = matches!(
            resolve_output_format(cli, config),
            OutputFormat::Terminal | OutputFormat::Compact
        );
        if is_terminal && cli.output.is_none() && !cli.quiet {
            let hints =
                analysis::situations::detect(&graph, &cli.path, &bus_report, dead_code.len());
            if !hints.is_empty() {
                println!("\n{}", "Next steps".bold());
                for hint in hints {
                    if !hint.message.is_empty() {
                        println!("  {} {}", "⚠".yellow(), hint.message);
                    }
                    println!("    {}", hint.command.dimmed());
                }
            }
        }
    }

    // Print timing
    let elapsed = start_time.elapsed();
    info!("Analysis completed in {:.2}s", elapsed.as_secs_f64());

    // --patch describes a dry-run: it needs one
    if cli.patch.is_some() && !(cli.delete && cli.dry_run) {
        eprintln!("{}: --patch requires --delete --dry-run", "Error".red());
        std::process::exit(2);
    }
    if let Some(ref patch_path) = cli.patch {
        if cli.delete && cli.dry_run && !dead_code.is_empty() {
            let patch = refactor::patch::unified_patch(&dead_code, &cli.path);
            if let Err(e) = std::fs::write(patch_path, &patch) {
                eprintln!("{}: cannot write patch: {e}", "Error".red());
                std::process::exit(2);
            }
            println!(
                "{}",
                format!(
                    "📄 Patch written to {} — review, then git apply",
                    patch_path.display()
                )
                .green()
            );
        }
    }

    // A check command with nothing to check is a config mistake
    if cli.verify_cmd.is_some() && !cli.delete {
        eprintln!(
            "{}: --verify-cmd only makes sense with --delete",
            "Error".red()
        );
        std::process::exit(2);
    }

    // Step 15: Safe delete if requested
    if cli.delete && !dead_code.is_empty() {
        // Snapshot before touching anything: restoring in Rust is
        // portable, the undo shell script is not
        let snapshots: Option<Vec<(std::path::PathBuf, String)>> =
            if cli.verify_cmd.is_some() && !cli.dry_run {
                let files_to_touch: std::collections::BTreeSet<std::path::PathBuf> = dead_code
                    .iter()
                    .map(|dc| dc.declaration.location.file.clone())
                    .collect();
                Some(
                    files_to_touch
                        .into_iter()
                        .filter_map(|p| std::fs::read_to_string(&p).ok().map(|c| (p, c)))
                        .collect(),
                )
            } else {
                None
            };

        let deleter =
            refactor::SafeDeleter::new(cli.interactive, cli.dry_run, cli.undo_script.clone())
                .with_assume_yes(cli.yes);
        deleter.delete(&dead_code)?;

        if let (Some(cmd), Some(snaps)) = (cli.verify_cmd.as_deref(), snapshots) {
            println!("{}", format!("🔧 Verifying: {cmd}").dimmed());
            let status = shell_command(cmd).current_dir(&cli.path).status();
            let passed = matches!(status, Ok(s) if s.success());
            if passed {
                println!("{}", "✓ Verification passed — deletion kept".green());
            } else {
                for (path, content) in &snaps {
                    let _ = std::fs::write(path, content);
                }
                eprintln!(
                    "{}",
                    format!(
                        "✗ Verification failed — {} file(s) restored byte-for-byte",
                        snaps.len()
                    )
                    .red()
                );
                std::process::exit(3);
            }
        }
    }

    // The ratchet verdict comes last: the report above already told the
    // user what crossed the ceiling
    if ratchet_failed {
        std::process::exit(3);
    }

    // the scriptable gate: 1 = findings, after the report printed and
    // the baseline filtered — new findings only.
    //
    // `--generate-baseline` désarme la porte pour CE run : geler la dette est
    // un acte d'acceptation, et sortir 1 dans la seconde qui suit ferait
    // échouer l'étape d'adoption que la doc CI recommande en premier.
    //
    // Une suppression qui a EU LIEU désarme aussi : les trouvailles sont
    // résolues, échouer bloquerait l'étape commit d'un pipeline d'auto-delete.
    // `--delete --dry-run` ne touche à rien et gate normalement.
    let deletion_ran = cli.delete && !cli.dry_run;
    if resolve_fail_on_findings(cli)
        && cli.generate_baseline.is_none()
        && !deletion_ran
        && !dead_code.is_empty()
    {
        std::process::exit(1);
    }

    Ok(())
}

fn parse_confidence(s: &str) -> Confidence {
    match s.to_lowercase().as_str() {
        "low" => Confidence::Low,
        "medium" => Confidence::Medium,
        "high" => Confidence::High,
        "confirmed" => Confidence::Confirmed,
        _ => Confidence::Low,
    }
}
