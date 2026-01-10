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
mod parser;
mod proguard;
mod refactor;
mod report;
mod watch;

use proguard::{ProguardUsage, ReportGenerator};

use analysis::detectors::{
    // Core detectors
    Detector, RedundantOverrideDetector, UnusedIntentExtraDetector, UnusedParamDetector,
    UnusedSealedVariantDetector, WriteOnlyDetector,
    // Anti-pattern detectors (AP001-AP006)
    DeepInheritanceDetector, EventBusPatternDetector, GlobalMutableStateDetector,
    SingleImplInterfaceDetector,
    // Phase 1: Kotlin patterns (AP007-AP010)
    GlobalScopeUsageDetector, HeavyViewModelDetector, LateinitAbuseDetector,
    ScopeFunctionChainingDetector,
    // Phase 2: Performance & Memory (AP011-AP015)
    CollectionWithoutSequenceDetector, LargeClassDetector, LongMethodDetector,
    MemoryLeakRiskDetector, ObjectAllocationInLoopDetector,
    // Phase 3: Architecture & Design (AP016-AP020)
    HardcodedDispatcherDetector, MissingUseCaseDetector, MutableStateExposedDetector,
    NestedCallbackDetector, ViewLogicInViewModelDetector,
    // Phase 4: Kotlin-Specific (AP021-AP025)
    ComplexConditionDetector, LongParameterListDetector, NullabilityOverloadDetector,
    ReflectionOveruseDetector, StringLiteralDuplicationDetector,
    // Phase 5: Android-Specific (AP026-AP030)
    AsyncTaskUsageDetector, InitOnDrawDetector, MainThreadDatabaseDetector,
    UnclosedResourceDetector, WakeLockAbuseDetector,
    // Phase 6: Compose-Specific (AP031-AP034)
    BusinessLogicInComposableDetector, LaunchedEffectWithoutKeyDetector,
    NavControllerPassingDetector, StateWithoutRememberDetector,
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

    /// Enable deep analysis mode - more aggressive detection
    /// Does not auto-mark class members as reachable
    /// Detects unused members even in reachable classes
    #[arg(long)]
    deep: bool,

    /// Enable unused parameter detection
    /// Finds function parameters that are declared but never used
    #[arg(long)]
    unused_params: bool,

    /// Enable unused resource detection
    /// Finds Android resources (strings, colors, etc.) that are never referenced
    #[arg(long)]
    unused_resources: bool,

    /// Enable write-only variable detection
    /// Finds variables that are assigned but never read (Phase 9)
    #[arg(long)]
    write_only: bool,

    /// Enable unused sealed variant detection
    /// Finds sealed class variants that are never instantiated (Phase 10)
    #[arg(long)]
    sealed_variants: bool,

    /// Enable redundant override detection
    /// Finds method overrides that only call super (Phase 10)
    #[arg(long)]
    redundant_overrides: bool,

    /// Enable unused Intent extra detection
    /// Finds putExtra() keys that are never retrieved via getXxxExtra() (Phase 11)
    #[arg(long)]
    unused_extras: bool,

    /// Enable write-only SharedPreferences detection
    /// Finds SharedPreferences keys that are written but never read (Phase 9)
    #[arg(long)]
    write_only_prefs: bool,

    /// Enable write-only Room DAO detection
    /// Finds Room DAOs that have @Insert but no @Query methods (Phase 9)
    #[arg(long)]
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
        let mode = group_by.parse::<report::GroupBy>().unwrap_or(report::GroupBy::Rule);
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
        EnvFilter::new("info")
    };

    fmt().with_env_filter(filter).with_target(false).init();
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

fn run_analysis(config: &Config, cli: &Cli) -> Result<()> {
    use colored::Colorize;
    use indicatif::{ProgressBar, ProgressStyle};
    use std::time::Instant;

    let start_time = Instant::now();

    // Step 1: Discover files
    info!("Discovering files...");
    let finder = FileFinder::new(config);
    let files = finder.find_files(&cli.path)?;

    info!("Found {} files to analyze", files.len());

    if files.is_empty() {
        println!("{}", "No Kotlin or Java files found.".yellow());
        return Ok(());
    }

    // Step 2: Parse files and build graph
    let graph = if cli.parallel {
        // Parallel parsing mode
        println!(
            "{}",
            format!("⚡ Parallel mode: parsing {} files...", files.len()).cyan()
        );
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
    if cli.parallel {
        println!(
            "{}",
            format!(
                "⚡ Parsed {} files in {:.2}s",
                files.len(),
                parse_time.as_secs_f64()
            )
            .green()
        );
    }

    // Step 3: Detect entry points
    info!("Detecting entry points...");
    let entry_detector = EntryPointDetector::new(config);
    let entry_points = entry_detector.detect(&graph, &cli.path)?;

    info!("Found {} entry points", entry_points.len());

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

    let (dead_code, reachable) = if cli.deep {
        // Deep analysis mode - most aggressive
        println!(
            "{}",
            "🔬 Deep mode: aggressive dead code detection...".cyan()
        );
        let deep = DeepAnalyzer::new()
            .with_parallel(cli.parallel)
            .with_unused_members(true);
        deep.analyze(&graph, &entry_points)
    } else if cli.enhanced && proguard_data.is_some() {
        // Enhanced mode with ProGuard cross-validation
        println!(
            "{}",
            "🔍 Enhanced mode: cross-validating with ProGuard data...".cyan()
        );
        let mut enhanced = EnhancedAnalyzer::new();
        if let Some(pg) = proguard_data.clone() {
            enhanced = enhanced.with_proguard(pg);
        }
        enhanced.analyze(&graph, &entry_points)
    } else if cli.parallel {
        // Standard analysis with parallel analyzer
        let enhanced = EnhancedAnalyzer::new();
        enhanced.analyze(&graph, &entry_points)
    } else {
        // Standard sequential analysis
        let analyzer = ReachabilityAnalyzer::new();
        analyzer.find_unreachable_with_reachable(&graph, &entry_points)
    };

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

    let reporter = Reporter::with_options(report_format, report_options);
    reporter.report(&dead_code)?;

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
