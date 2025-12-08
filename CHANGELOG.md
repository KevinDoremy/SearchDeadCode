# Changelog

All notable changes to SearchDeadCode will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- OpenSSF Scorecard badge
- Downloads badge
- MSRV (Minimum Supported Rust Version) policy: 1.70+
- This CHANGELOG.md file

## [0.4.0] - 2024-12-07

### Added - Enhanced Detection (Phase 6)
- **`--unused-resources` flag**: Detect unused Android resources (strings, colors, dimens, styles, attrs)
  - Parses all `res/values/*.xml` files for resource definitions
  - Scans Kotlin, Java, and XML files for `R.type.name` and `@type/name` references
  - Real-world test: Found 53 unused resources in a 1800-file project
- **`--unused-params` flag**: Detect unused function parameters
  - Conservative detection to minimize false positives
  - Skips override methods, abstract methods, @Composable functions, constructors

### Added - Performance & CI Features (Phase 5)
- **`--incremental` flag**: Incremental analysis with file caching
  - Caches parsed AST data to skip re-parsing unchanged files
  - Uses file hash + mtime for change detection
- **`--watch` flag**: Watch mode for continuous monitoring
  - Automatically re-runs analysis when source files change
  - Debounced to avoid excessive re-runs
- **`--baseline <FILE>` flag**: Baseline support for CI adoption
  - Generate baseline with `--generate-baseline <FILE>`
  - Only report new issues not in baseline
  - Perfect for gradual adoption in existing projects

### Changed
- Optimized reachability analysis: ~8% faster on large codebases

### New CLI Options
- `--unused-resources` - Detect unused Android resources
- `--unused-params` - Detect unused function parameters
- `--incremental` - Enable incremental analysis with caching
- `--clear-cache` - Clear the analysis cache
- `--cache-path <FILE>` - Custom cache file path
- `--baseline <FILE>` - Use baseline to filter existing issues
- `--generate-baseline <FILE>` - Generate baseline from current results
- `--watch` - Watch mode for continuous monitoring

## [0.3.0] - 2024-11-15

### Added - Deep Analysis Mode
- **`--deep` flag**: More aggressive dead code detection that analyzes individual members within classes
- **Suspend function detection**: Properly handles Kotlin suspend functions
- **Flow pattern detection**: Recognizes Kotlin Flow, StateFlow, SharedFlow patterns
- **Interface implementation tracking**: Classes implementing reachable interfaces are now marked as reachable
- **Sealed class subtype tracking**: All subtypes of reachable sealed classes are marked as reachable

### Added - Enhanced DI/Framework Support
- Comprehensive annotation detection for Dagger, Hilt, Koin, Room, Retrofit
- Methods with `@Provides`, `@Binds`, `@Query`, `@GET`, etc. are properly recognized as entry points
- Skips DI entry points in deep analysis to avoid false positives

### Added - Kotlin Language Features
- **Companion object analysis**: Properly tracks companion objects and their members
- **Lazy/delegated property detection**: Properties using `by lazy`, `by Delegates.observable()`, etc.
- **Generic type argument tracking**: Properly extracts and tracks type arguments
- **Class delegation**: Detects `class Foo : Bar by delegate` patterns
- **Const val handling**: Skips `const val` properties (inlined at compile time)
- **Data class methods**: Skips auto-generated `copy()`, `componentN()`, `equals()`, `hashCode()`, `toString()`

### Changed
- ~23% reduction in false positives on real-world Android projects (deep mode)
- ~15% reduction in false positives (standard mode)

## [0.2.0] - 2024-10-20

### Added - Hybrid Analysis
- **ProGuard/R8 Integration**: Use `--proguard-usage` to load R8's usage.txt for confirmed dead code detection
- **Coverage Integration**: Combine static analysis with runtime coverage (JaCoCo, Kover, LCOV)
- **Confidence Scoring**: Findings now have confidence levels (low/medium/high/confirmed)
- **Zombie Code Detection**: Find mutually dependent dead code cycles with `--detect-cycles`
- **Runtime-Dead Code**: Detect code that's reachable but never executed with `--include-runtime-dead`

### New CLI Options
- `--proguard-usage <FILE>` - Load ProGuard/R8 usage.txt
- `--coverage <FILE>` - Load coverage data (can be repeated)
- `--min-confidence <LEVEL>` - Filter by confidence level
- `--runtime-only` - Only show runtime-confirmed findings
- `--include-runtime-dead` - Include reachable but never-executed code
- `--detect-cycles` - Enable zombie code cycle detection

### Changed - Output Improvements
- Confidence indicators in terminal output: ● ◉ ○ ◌
- JSON schema v1.1 with confidence_score and runtime_confirmed fields
- Better grouping and summary statistics

## [0.1.0] - 2024-09-15

### Fixed
- Extension function name extraction (no longer reported as `<anonymous>`)
- Generic type resolution (`Focusable<T>` now matches `Focusable`)
- Navigation expression references (`obj.method()` calls now detected)
- Ambiguous reference resolution (overloaded functions all marked as used)
- Glob pattern matching (`**/test/**` no longer matches `/testproject/`)
- Dry-run mode (no longer requires interactive terminal)

### Changed
- Reduced false positives by ~51% on real-world Android projects
- Better handling of Kotlin extension functions
- Improved method call detection via navigation_suffix nodes
- All CLI options working and tested

## [0.0.1] - 2024-08-01

### Added - Initial Release
- Core dead code detection for Kotlin and Java
- Android-aware analysis (Activities, Fragments, ViewModels, etc.)
- Multiple output formats: terminal, JSON, SARIF
- Safe delete with interactive mode and dry-run
- Configuration via YAML/TOML files
- Homebrew tap for easy installation
- GitHub Action for CI integration

[Unreleased]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.0.1...v0.1.0
[0.0.1]: https://github.com/KevinDoremy/SearchDeadCode/releases/tag/v0.0.1
