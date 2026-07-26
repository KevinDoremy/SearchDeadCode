# Changelog

All notable changes to SearchDeadCode will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.10.0] - 2026-07-26

### Added
- **`--interactive` triage mode** (fzf-style): fuzzy-filter the findings by
  typing, act from the keyboard — Explain, Kill-list, or Delete with a diff
  preview and confirmation. Deletions land in an undo script rewritten after
  every removal; exclusive dependents of deleted symbols are marked ↯.
  Requires a real terminal; piped runs fall back to the standard report.
  `--delete --interactive` keeps its historical confirm-each behavior.

## [0.9.0] - 2026-07-26

### Changed
- A healthy project reports on a single checked line — no summary block.
- `--delete --dry-run` previews the exact lines a deletion would remove as a
  red, line-numbered diff instead of a list of names.

## [0.8.0] - 2026-07-26

### Changed
- Progress renders as aligned checked phase lines (`✓ parsed`, `✓ analysis`)
  with counts and timing, replacing the emoji banners. Lines print when a
  phase completes — a checkmark never lies.

## [0.7.1] - 2026-07-26

### Changed
- Report file headers are relative to the analyzed root.
- Annotations only render on digestible reports (≤ 20 findings); big reports
  keep one line per finding.

### Added
- `docs/cli-tour.md`: every important command with its real output, linked
  from the README.

## [0.7.0] - 2026-07-26

### Added
- **Rustc-style annotated findings**: the default terminal report shows each
  finding's source line with the symbol underlined and a per-finding
  `= help:` pointing at `--explain`. The dense one-line view lives on
  `--compact`.

## [0.6.0] - 2026-07-26

### Changed
- **Clean output streams**: logs go to stderr at warn level by default
  (`--verbose` restores the detail). stdout carries results only, so
  `searchdeadcode . --format json | jq` finally works.
- Reports with findings end on a **Next steps** footer pointing at
  `--clusters`, `--explain` and `--delete --dry-run`, replacing the old
  static summary tips.

### Added
- First-contact guidance: a project without `.deadcode.yml` gets pointed at
  `--init`; an empty run shows which path was searched.

## [0.5.0] - 2026-07-25

### Added
- **Incremental cache wired in**: `--incremental`, `--clear-cache` and `--cache-path`
  now work. The cache stores full parse results (v2 format) so a cache hit
  rebuilds the exact same graph, and it self-invalidates on tool version changes.
- **Phantom source set detection**: a `src/` directory no build file accounts for
  is reported and excluded — its references no longer keep dead code alive.
- **`--explain SYMBOL`**: why is this symbol dead (or alive)? Incoming references,
  every root source checked, and the verdict.
- **`--kill-list SYMBOL`**: "if I delete X, what else falls?" — the transitive
  closure of exclusive dependents, with an estimated line count.
- **`--clusters`**: dead code grouped into connected, deletable clusters sorted
  by size, instead of a flat per-file list.
- **Per-finding deletion risk**: names found in string literals, serialization
  annotations or reflection/event-bus neighborhoods are tagged medium/high in
  the terminal and in JSON output.
- **DI binding resolution**: `@Provides`/`@Binds` methods are roots only when
  their produced type is actually consumed. Orphan modules now show up as dead.
- **`--compare OLD=NEW`**: migration diff — old-world symbols deletable at the
  flip vs blockers still referenced from outside, each with a referencer.
- **`--init`**: generates a commented `.deadcode.yml` matching the project
  (phantom source sets pre-excluded, DI framework detected).
- **`--flag NAME --behavior enabled|disabled`**: feature-flag cleanup preview —
  what dies once the flag is burned in.

### Fixed
- Kotlin parser now extracts function return types.
- Deep analysis no longer follows dead method edges out of reachable classes.
- Ambiguous simple-name resolutions are marked on references and ignored where
  precision matters (migration blockers).
- Analyzing a single file no longer tries to create the cache under it.

### Previously unreleased
- OpenSSF Scorecard badge
- Downloads badge
- MSRV (Minimum Supported Rust Version) policy: 1.80+ (bumped from 1.70)
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
