use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use colored::Colorize;
use miette::Result;
use std::path::PathBuf;
use tracing::info;

mod analysis;
mod baseline;
mod cache;
mod config;
mod coverage;
mod discovery;
mod graph;
mod interactive;
mod parser;
mod proguard;
mod refactor;
mod report;
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
    RedundantOverrideDetector,
    RedundantPublicDetector,
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
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Path to configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Target directories to analyze (can be specified multiple times)
    #[arg(short, long)]
    target: Vec<PathBuf>,

    /// Patterns to exclude (can be specified multiple times)
    #[arg(short, long)]
    exclude: Vec<String>,

    /// Patterns to retain - never report as dead (can be specified multiple times)
    #[arg(short, long)]
    retain: Vec<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "terminal")]
    format: OutputFormat,

    /// Output file (for json/sarif formats)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Enable safe delete mode
    #[arg(long)]
    delete: bool,

    /// Interactive mode for deletions (confirm each)
    #[arg(long)]
    interactive: bool,

    /// Dry run - show what would be deleted without making changes
    #[arg(long)]
    dry_run: bool,

    /// Generate undo script
    #[arg(long)]
    undo_script: Option<PathBuf>,

    /// Detection types to run (comma-separated)
    #[arg(long)]
    detect: Option<String>,

    /// Explain why a symbol (simple name or FQN) is considered dead or alive
    #[arg(long, value_name = "SYMBOL")]
    explain: Option<String>,

    /// Show everything that falls if this symbol is deleted (exclusive dependents)
    #[arg(long, value_name = "SYMBOL")]
    kill_list: Option<String>,

    /// Group dead code findings into connected, deletable clusters
    #[arg(long)]
    clusters: bool,

    /// Only the findings safe to delete blind: whole cluster dead, every
    /// member low risk
    #[arg(long)]
    quick_wins: bool,

    /// Apply zero-risk fixes automatically (unused imports). Combine with
    /// --dry-run to preview. Always writes an undo script.
    #[arg(long)]
    fix: bool,

    /// PR-scoped analysis: judge only files changed since this git ref,
    /// reporting only symbols provably unreferenced project-wide
    #[arg(long, value_name = "REF")]
    changed_since: Option<String>,

    /// Migration diff: OLD=NEW worlds (package prefix or path fragment).
    /// Lists old-world symbols deletable at the flip and the blockers.
    #[arg(long, value_name = "OLD=NEW")]
    compare: Option<String>,

    /// Attribute a shared module's symbols to their real consumers:
    /// unreferenced, internal-only, or used by which directories
    #[arg(long, value_name = "MODULE")]
    module_usage: Option<String>,

    /// Generate a commented .deadcode.yml matching the project's shape
    /// (source sets, DI framework, exclusions) and exit
    #[arg(long)]
    init: bool,

    /// Feature flag cleanup: name (or key) of the flag being settled
    #[arg(long, value_name = "NAME")]
    flag: Option<String>,

    /// Assumed final behavior of --flag
    #[arg(long, value_enum, default_value = "enabled")]
    behavior: FlagBehavior,

    /// Coverage files (JaCoCo XML, Kover XML, or LCOV format)
    /// Can be specified multiple times for merged coverage
    #[arg(long, value_name = "FILE")]
    coverage: Vec<PathBuf>,

    /// Minimum confidence level to report (low, medium, high, confirmed)
    #[arg(long, default_value = "medium")]
    min_confidence: String,

    /// Only show findings confirmed by runtime coverage
    #[arg(long)]
    runtime_only: bool,

    /// Include runtime-dead code (reachable but never executed)
    #[arg(long)]
    include_runtime_dead: bool,

    /// Detect and report zombie code cycles (mutually dependent dead code)
    #[arg(long)]
    detect_cycles: bool,

    /// ProGuard/R8 usage.txt file for enhanced detection
    /// This file lists code that R8 determined is unused
    #[arg(long, value_name = "FILE")]
    proguard_usage: Option<PathBuf>,

    /// Generate a filtered dead code report from ProGuard usage.txt
    /// Filters out generated code (Dagger, Hilt, _Factory, _Impl, etc.)
    #[arg(long, value_name = "FILE")]
    generate_report: Option<PathBuf>,

    /// Package prefix to include in report (e.g., "com.example")
    /// Only classes matching this prefix will be included
    #[arg(long, value_name = "PREFIX")]
    report_package: Option<String>,

    /// Enable parallel processing for faster analysis (enabled by default)
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    parallel: bool,

    /// Enable enhanced detection mode with ProGuard cross-validation
    #[arg(long)]
    enhanced: bool,

    /// Enable deep analysis mode - more aggressive detection (enabled by default)
    /// Does not auto-mark class members as reachable
    /// Detects unused members even in reachable classes
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    deep: bool,

    /// Enable unused parameter detection (enabled by default)
    /// Finds function parameters that are declared but never used
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    unused_params: bool,

    /// Enable unused resource detection (off by default - slower)
    /// Finds Android resources (strings, colors, etc.) that are never referenced
    #[arg(long)]
    unused_resources: bool,

    /// Enable write-only variable detection (enabled by default)
    /// Finds variables that are assigned but never read
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    write_only: bool,

    /// Enable unused sealed variant detection (enabled by default)
    /// Finds sealed class variants that are never instantiated
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    sealed_variants: bool,

    /// Enable redundant override detection (off by default - can be intentional)
    /// Finds method overrides that only call super
    #[arg(long)]
    redundant_overrides: bool,

    /// Enable unused Intent extra detection (enabled by default)
    /// Finds putExtra() keys that are never retrieved via getXxxExtra()
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    unused_extras: bool,

    /// Enable write-only SharedPreferences detection (enabled by default)
    /// Finds SharedPreferences keys that are written but never read
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    write_only_prefs: bool,

    /// Enable write-only Room DAO detection (enabled by default)
    /// Finds Room DAOs that have @Insert but no @Query methods
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    write_only_dao: bool,

    /// Enable all anti-pattern detectors (AP001-AP034)
    /// Includes: architecture, performance, Kotlin, Android, and Compose patterns
    #[arg(long)]
    anti_patterns: bool,

    /// Enable architecture anti-pattern detectors (AP001-AP006)
    /// Detects: deep inheritance, EventBus, global mutable state, single-impl interfaces
    #[arg(long)]
    architecture_patterns: bool,

    /// Enable Kotlin anti-pattern detectors (AP007-AP010, AP021-AP025)
    /// Detects: GlobalScope, heavy ViewModel, lateinit abuse, scope function chaining,
    /// nullability overload, reflection overuse, long parameter lists, complex conditions
    #[arg(long)]
    kotlin_patterns: bool,

    /// Enable performance anti-pattern detectors (AP011-AP015)
    /// Detects: memory leaks, long methods, large classes, collection inefficiencies, loop allocations
    #[arg(long)]
    performance_patterns: bool,

    /// Enable Android-specific anti-pattern detectors (AP016-AP020, AP026-AP030)
    /// Detects: mutable state exposure, view logic in ViewModel, missing UseCase,
    /// nested callbacks, hardcoded dispatchers, unclosed resources, main thread DB,
    /// WakeLock abuse, AsyncTask usage, onDraw allocations
    #[arg(long)]
    android_patterns: bool,

    /// Enable Compose-specific anti-pattern detectors (AP031-AP034)
    /// Detects: state without remember, LaunchedEffect without key, business logic in composables,
    /// NavController passing to children
    #[arg(long)]
    compose_patterns: bool,

    /// Enable incremental analysis with caching (enabled by default)
    /// Skips re-parsing unchanged files for faster subsequent runs
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    incremental: bool,

    /// Clear the analysis cache before running
    #[arg(long)]
    clear_cache: bool,

    /// Custom cache file path (default: .searchdeadcode-cache.json)
    #[arg(long, value_name = "FILE")]
    cache_path: Option<PathBuf>,

    /// Baseline file for ignoring existing issues
    /// New issues not in baseline will be reported
    #[arg(long, value_name = "FILE")]
    baseline: Option<PathBuf>,

    /// Generate a baseline file from current results
    #[arg(long, value_name = "FILE")]
    generate_baseline: Option<PathBuf>,

    /// Watch mode - continuously monitor for changes
    #[arg(long)]
    watch: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Quiet mode - only output results
    #[arg(short, long)]
    quiet: bool,

    /// Generate shell completions
    #[arg(long, value_name = "SHELL")]
    completions: Option<Shell>,

    /// Summary output - show statistics and top issues only
    #[arg(long)]
    summary: bool,

    /// Compact output - one line per issue
    #[arg(long)]
    compact: bool,

    /// Group results by: rule, category, severity, file
    #[arg(long, value_name = "MODE")]
    group_by: Option<String>,

    /// Expand all collapsed groups (show every issue)
    #[arg(long)]
    expand: bool,

    /// Expand a specific rule's issues (e.g., --expand-rule AP017)
    #[arg(long, value_name = "RULE")]
    expand_rule: Option<String>,

    /// Number of top issues to show in summary mode
    #[arg(long, default_value = "10")]
    top: usize,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
enum OutputFormat {
    #[default]
    Terminal,
    Compact,
    Json,
    Sarif,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default)]
enum FlagBehavior {
    #[default]
    Enabled,
    Disabled,
}

/// Determine the report format from CLI options
fn determine_report_format(cli: &Cli) -> report::ReportFormat {
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

    // Fall back to --format option
    match cli.format {
        OutputFormat::Terminal => report::ReportFormat::Terminal,
        OutputFormat::Compact => report::ReportFormat::Compact,
        OutputFormat::Json => report::ReportFormat::Json,
        OutputFormat::Sarif => report::ReportFormat::Sarif,
    }
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
    let cli_format = cli.format.clone();
    let cli_output = cli.output.clone();
    let cli_verbose = cli.verbose;
    let cli_quiet = cli.quiet;
    let cli_deep = cli.deep;
    let cli_parallel = cli.parallel;
    let cli_enhanced = cli.enhanced;
    let cli_detect_cycles = cli.detect_cycles;
    let cli_min_confidence = cli.min_confidence.clone();
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
                    println!("{}", format!("📋 Baseline: {}", stats).cyan());
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
        println!("Symbol '{}' not found in the analyzed project.", symbol);
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
                let parse_result = entry.parse_result.clone();
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

    let graph = if cli.incremental {
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
    if cli.parallel && !cli.incremental {
        phase_line(
            cli.quiet,
            "parsed",
            &format!("{} files in {:.2}s", files.len(), parse_time.as_secs_f64()),
        );
    }

    // Step 3: Detect entry points
    info!("Detecting entry points...");
    let entry_detector = EntryPointDetector::new(config);
    let entry_points = entry_detector.detect(&graph, &cli.path)?;

    info!("Found {} entry points", entry_points.len());

    // --explain short-circuits the normal report
    if let Some(symbol) = cli.explain.as_deref() {
        let enhanced = EnhancedAnalyzer::new();
        let (_, reachable) = enhanced.analyze(&graph, &entry_points);
        explain_symbol(&graph, &entry_points, &reachable, symbol);
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
        let report = analysis::migration::compare(&graph, old_token);
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
    let proguard_data = if let Some(ref usage_path) = cli.proguard_usage {
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
        if !reflective.is_empty() {
            dead_code.retain(|dc| {
                !(matches!(dc.issue, analysis::DeadCodeIssue::UnusedEnumCase)
                    && dc
                        .declaration
                        .parent
                        .as_ref()
                        .is_some_and(|p| reflective.contains(p)))
            });
        }
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

    // Step 9f: Detect unused Android resources
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
            // Print unused resources directly (they're not part of the code graph)
            if !cli.quiet {
                use colored::Colorize;
                println!();
                println!("{}", "📦 Unused Android Resources:".yellow().bold());
                for resource in &resource_analysis.unused {
                    let rel_path = resource
                        .file
                        .strip_prefix(&cli.path)
                        .unwrap_or(&resource.file);
                    println!(
                        "  {} {}:{} - {} '{}'",
                        "○".dimmed(),
                        rel_path.display(),
                        resource.line,
                        resource.resource_type,
                        resource.name
                    );
                }
                println!();
            }
        }
    }

    // Step 9f2: Dead layouts (ViewBinding-aware, cheap string scan)
    {
        let dead_layouts = analysis::layouts::find_dead_layouts(&files);
        if !dead_layouts.is_empty() && !cli.quiet {
            println!();
            println!(
                "{}",
                "📐 Unused layouts (no Binding usage, no R.layout, no include):"
                    .yellow()
                    .bold()
            );
            for layout in &dead_layouts {
                let rel = layout.strip_prefix(&cli.path).unwrap_or(layout);
                println!("  {} {}", "○".dimmed(), rel.display());
            }
            println!(
                "{}",
                "  (check for getIdentifier()-style dynamic inflation before deleting)".dimmed()
            );
            println!();
        }
    }

    // Step 9f3: Event-bus orphans (cheap regex pass over the sources)
    {
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
        let bus_report = analysis::bus::analyze(&corpus);
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

    // Step 9g: Detect unused Intent extras (Phase 11)
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
            // Print unused extras directly
            if !cli.quiet {
                use colored::Colorize;
                println!();
                println!("{}", "🔑 Unused Intent Extras:".yellow().bold());
                for extra in &intent_analysis.unused_extras {
                    let rel_path = extra.file.strip_prefix(&cli.path).unwrap_or(&extra.file);
                    println!(
                        "  {} {}:{} - putExtra(\"{}\") never retrieved",
                        "○".dimmed(),
                        rel_path.display(),
                        extra.line,
                        extra.key
                    );
                }
                println!();
            }
        }
    }

    // Step 9h: Detect write-only SharedPreferences (Phase 9)
    if cli.write_only_prefs {
        use analysis::detectors::WriteOnlyPrefsDetector;
        use discovery::FileType;
        let prefs_detector = WriteOnlyPrefsDetector::new();

        // Analyze all Kotlin files for SharedPreferences usage
        let mut prefs_analysis = analysis::detectors::SharedPrefsAnalysis::new();
        for file in &files {
            if file.file_type == FileType::Kotlin {
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
                }
            }
        }

        let write_only_keys = prefs_analysis.get_write_only_keys();
        if !write_only_keys.is_empty() {
            info!(
                "Found {} write-only SharedPreferences keys",
                write_only_keys.len()
            );
            if !cli.quiet {
                use colored::Colorize;
                println!();
                println!("{}", "🔐 Write-Only SharedPreferences:".yellow().bold());
                for key in write_only_keys {
                    if let Some(locs) = prefs_analysis.writes.get(key) {
                        for loc in locs {
                            let rel_path = loc.file.strip_prefix(&cli.path).unwrap_or(&loc.file);
                            println!(
                                "  {} {}:{} - key \"{}\" written but never read",
                                "○".dimmed(),
                                rel_path.display(),
                                loc.line,
                                key
                            );
                        }
                    }
                }
                println!();
            }
        }
    }

    // Step 9i: Detect write-only Room DAOs (Phase 9)
    if cli.write_only_dao {
        use analysis::detectors::WriteOnlyDaoDetector;
        use discovery::FileType;
        let dao_detector = WriteOnlyDaoDetector::new();

        // Analyze all Kotlin files for DAO definitions
        let mut dao_analysis = analysis::detectors::DaoCollectionAnalysis::new();
        for file in &files {
            if file.file_type == FileType::Kotlin {
                if let Ok(content) = std::fs::read_to_string(&file.path) {
                    let file_analysis = dao_detector.analyze_source(&content, &file.path);
                    dao_analysis.daos.extend(file_analysis.daos);
                }
            }
        }

        let write_only_daos = dao_analysis.get_write_only_daos();
        if !write_only_daos.is_empty() {
            info!("Found {} write-only Room DAOs", write_only_daos.len());
            if !cli.quiet {
                use colored::Colorize;
                println!();
                println!("{}", "🗄️ Write-Only Room DAOs:".yellow().bold());
                for dao in write_only_daos {
                    let rel_path = dao.file.strip_prefix(&cli.path).unwrap_or(&dao.file);
                    println!(
                        "  {} {}:{} - DAO '{}' has @Insert but no @Query",
                        "○".dimmed(),
                        rel_path.display(),
                        dao.line,
                        dao.name
                    );
                    for method in dao.write_methods() {
                        let entity_info = method
                            .entity_type
                            .as_ref()
                            .map(|e| format!(" ({})", e))
                            .unwrap_or_default();
                        println!(
                            "    {} {}{}",
                            "└".dimmed(),
                            method.name,
                            entity_info.dimmed()
                        );
                    }
                }
                println!();
            }
        }
    }

    // Step 9j: Anti-pattern detectors
    let run_architecture = cli.anti_patterns || cli.architecture_patterns;
    let run_kotlin = cli.anti_patterns || cli.kotlin_patterns;
    let run_performance = cli.anti_patterns || cli.performance_patterns;
    let run_android = cli.anti_patterns || cli.android_patterns;
    let run_compose = cli.anti_patterns || cli.compose_patterns;

    // Architecture patterns (AP001-AP006)
    if run_architecture {
        let detectors: Vec<Box<dyn Detector>> = vec![
            Box::new(DeepInheritanceDetector::new()),
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
            Box::new(LongParameterListDetector::new()),
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
            Box::new(LongMethodDetector::new()),
            Box::new(LargeClassDetector::new()),
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

    // Step 10: Filter by confidence level
    let min_confidence = parse_confidence(&cli.min_confidence);
    let dead_code: Vec<_> = dead_code
        .into_iter()
        .filter(|dc| dc.confidence >= min_confidence)
        .filter(|dc| !cli.runtime_only || dc.runtime_confirmed)
        .collect();

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

    // Step 13: Filter by baseline if provided
    let dead_code = if let Some(ref baseline_path) = cli.baseline {
        match baseline::Baseline::load(baseline_path) {
            Ok(baseline) => {
                let stats = baseline.stats(&dead_code, &cli.path);
                println!("{}", format!("📋 Baseline: {}", stats).cyan());

                // Only report new issues not in baseline
                let new_issues: Vec<_> = baseline
                    .filter_new(&dead_code, &cli.path)
                    .into_iter()
                    .cloned()
                    .collect();

                if new_issues.is_empty() && stats.baselined_found > 0 {
                    println!("{}", "✓ No new dead code issues found!".green());
                }

                new_issues
            }
            Err(e) => {
                eprintln!("{}: Failed to load baseline: {}", "Warning".yellow(), e);
                dead_code
            }
        }
    } else {
        dead_code
    };

    // Step 13b: Assess deletion risk on the final findings
    let mut dead_code = dead_code;
    analysis::risk::assess(&mut dead_code, &files);

    // Step 14: Report results
    let report_format = determine_report_format(cli);
    let mut report_options = report::ReportOptions::new();
    report_options.output_path = cli.output.clone();
    report_options.base_path = Some(cli.path.clone());
    report_options.expand_all = cli.expand;
    report_options.expand_rule = cli.expand_rule.clone();
    report_options.top_n = cli.top;
    report_options.files_count = Some(files.len());
    report_options.declarations_count = Some(graph.declarations().count());

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

    if cli.clusters {
        let dead_ids: std::collections::HashSet<graph::DeclarationId> =
            dead_code.iter().map(|d| d.declaration.id.clone()).collect();
        let clusters = analysis::kill_list::dead_clusters(&graph, &dead_ids);
        print_clusters(&graph, clusters);
    } else {
        let reporter = Reporter::with_options(report_format, report_options);
        reporter.report(&dead_code)?;

        // Guide the next move — terminal output with findings only
        let is_terminal = matches!(cli.format, OutputFormat::Terminal | OutputFormat::Compact);
        if is_terminal && cli.output.is_none() && !cli.quiet && !dead_code.is_empty() {
            println!("\n{}", "Next steps".bold());
            println!("  searchdeadcode --interactive       triage findings from the keyboard");
            println!("  searchdeadcode --clusters          group findings into deletable units");
            println!("  searchdeadcode --explain <name>    see why a symbol is considered dead");
            println!("  searchdeadcode --delete --dry-run  preview the cleanup, touch nothing");
        }
    }

    // Print timing
    let elapsed = start_time.elapsed();
    info!("Analysis completed in {:.2}s", elapsed.as_secs_f64());

    // Step 15: Safe delete if requested
    if cli.delete && !dead_code.is_empty() {
        let deleter =
            refactor::SafeDeleter::new(cli.interactive, cli.dry_run, cli.undo_script.clone());
        deleter.delete(&dead_code)?;
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
