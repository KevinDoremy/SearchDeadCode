# CLI reference

```
searchdeadcode [OPTIONS] [PATH]

Arguments:
  [PATH]  Path to the project directory to analyze [default: .]

Options:
  -c, --config <FILE>      Path to configuration file
  -t, --target <DIR>       Target directories to analyze (can be repeated)
  -e, --exclude <PATTERN>  Patterns to exclude (can be repeated)
  -r, --retain <PATTERN>   Patterns to retain as entry points (can be repeated)
  -f, --format <FORMAT>    Output format [default: terminal]
                           [possible values: terminal, json, sarif, csv]
  -o, --output <FILE>      Output file for json/sarif formats
      --delete             Enable safe delete mode
      --interactive        Interactive deletion (confirm each item)
      --dry-run            Preview deletions without making changes
      --undo-script <FILE> Generate undo / restore script
      --detect <TYPES>     Detection types (comma-separated)

  Analysis Options:
      --deep                  Deep analysis: individual members within classes
      --unused-params         Detect unused function parameters
      --unused-resources      Detect unused Android resources
      --write-only            Detect write-only variables
      --write-only-prefs      Detect write-only SharedPreferences
      --write-only-dao        Detect write-only DAO @Insert without @Query
      --sealed-variants       Detect unused sealed class variants
      --redundant-overrides   Detect overrides that only call super
      --unused-extras         Detect putExtra without getExtra

  Hybrid Analysis Options:
      --coverage <FILE>       Coverage file (JaCoCo XML, Kover XML, or LCOV)
                              Can be specified multiple times for merged coverage
      --proguard-usage <FILE> ProGuard / R8 usage.txt file
      --min-confidence        Minimum confidence level
                              [possible values: low, medium, high, confirmed]
      --runtime-only          Only show findings confirmed by runtime coverage
      --include-runtime-dead  Include reachable but never-executed code
      --detect-cycles         Detect zombie code cycles

  Performance Options:
      --incremental           Enable incremental analysis with caching
      --clear-cache           Clear the analysis cache
      --cache-path <FILE>     Custom cache file path
      --baseline <FILE>       Use baseline to filter existing issues
      --generate-baseline <FILE>  Generate baseline from current results
      --watch                 Watch mode for continuous monitoring

  Output Options:
      --completions <SHELL>   Generate shell completions (bash, zsh, fish)

  -v, --verbose            Verbose output
  -q, --quiet              Quiet mode - only output results
  -h, --help               Print help
  -V, --version            Print version
```

## Common command examples

### Basic

```bash
# Analyze current directory
searchdeadcode .

# Analyze specific Android project
searchdeadcode ./my-android-app

# Verbose output
searchdeadcode ./app --verbose

# Quiet mode (results only)
searchdeadcode ./app --quiet
```

### Output formats

```bash
# Terminal (default) - colored, grouped output
searchdeadcode ./app

# JSON for programmatic use
searchdeadcode ./app --format json --output report.json

# SARIF for GitHub Code Scanning
searchdeadcode ./app --format sarif --output report.sarif
```

### Filtering

```bash
# Exclude patterns (glob syntax)
searchdeadcode ./app \
  --exclude "**/test/**" \
  --exclude "**/generated/**"

# Retain patterns (never report as dead)
searchdeadcode ./app \
  --retain "*Activity" \
  --retain "*ViewModel"

# Combine filters
searchdeadcode ./app \
  --exclude "**/build/**" \
  --exclude "**/*Test.kt" \
  --retain "*Repository" \
  --retain "*UseCase"
```

### Hybrid analysis

```bash
# With JaCoCo coverage
searchdeadcode ./app --coverage build/reports/jacoco/test/jacocoTestReport.xml

# With Kover coverage
searchdeadcode ./app --coverage build/reports/kover/report.xml

# With LCOV
searchdeadcode ./app --coverage coverage/lcov.info

# Multiple coverage files (merged)
searchdeadcode ./app \
  --coverage build/reports/unit-test.xml \
  --coverage build/reports/integration-test.xml

# With ProGuard / R8 usage.txt
searchdeadcode ./app --proguard-usage app/build/outputs/mapping/release/usage.txt

# Full hybrid (static + dynamic + R8)
searchdeadcode ./app \
  --deep \
  --coverage build/reports/jacoco.xml \
  --proguard-usage usage.txt \
  --detect-cycles \
  --min-confidence high
```

### Safe delete

```bash
# Interactive (confirm each item)
searchdeadcode ./app --delete --interactive

# Batch (select from list, confirm once)
searchdeadcode ./app --delete

# Dry run (no changes)
searchdeadcode ./app --delete --dry-run

# Generate undo script
searchdeadcode ./app --delete --undo-script restore.sh
```

### Performance / CI

```bash
# Incremental with cache
searchdeadcode ./app --incremental

# Watch mode
searchdeadcode ./app --watch

# Generate baseline (gradual adoption)
searchdeadcode ./app --generate-baseline .deadcode-baseline.json

# Use baseline (only new issues)
searchdeadcode ./app --baseline .deadcode-baseline.json
```

### Shell completions

```bash
# Bash
searchdeadcode --completions bash > ~/.local/share/bash-completion/completions/searchdeadcode

# Zsh
searchdeadcode --completions zsh > ~/.zfunc/_searchdeadcode

# Fish
searchdeadcode --completions fish > ~/.config/fish/completions/searchdeadcode.fish
```

## JSON output schema (v1.1)

```json
{
  "version": "1.1",
  "total_issues": 21,
  "issues": [
    {
      "code": "DC001",
      "severity": "warning",
      "confidence": "confirmed",
      "confidence_score": 1.0,
      "runtime_confirmed": true,
      "message": "class 'DeadHelper' is never used (confirmed by R8/ProGuard)",
      "file": "com/example/app/utils/DeadHelper.kt",
      "line": 5,
      "column": 1,
      "declaration": {
        "name": "DeadHelper",
        "kind": "class",
        "fully_qualified_name": "com.example.app.utils.DeadHelper"
      }
    }
  ],
  "summary": {
    "errors": 0,
    "warnings": 21,
    "infos": 0,
    "by_confidence": {
      "confirmed": 8,
      "high": 0,
      "medium": 13,
      "low": 0
    },
    "runtime_confirmed_count": 8
  }
}
```

| Field | Description |
|---|---|
| `code` | Issue code (DC001-DC007) |
| `confidence` | low / medium / high / confirmed |
| `confidence_score` | 0.25 to 1.0 for sorting |
| `runtime_confirmed` | true if coverage data confirms unused |
| `fully_qualified_name` | Package path when available |

## Complete flag list

Generated from `--help`; every flag the binary accepts, alphabetically.

| Flag | Description |
|---|---|
| `--android-patterns` | Enable Android-specific anti-pattern detectors (AP016-AP020, AP026-AP030) Detects: mutable state exposure, view logic in ViewModel, missing UseCase, nested callbacks, hardcoded dispatchers, unclosed resources, main thread DB, WakeLock abuse, AsyncTask usage, onDraw allocations |
| `--anti-patterns` | Enable all anti-pattern detectors (AP001-AP034) Includes: architecture, performance, Kotlin, Android, and Compose patterns |
| `--architecture-patterns` | Enable architecture anti-pattern detectors (AP001-AP006) Detects: deep inheritance, EventBus, global mutable state, single-impl interfaces |
| `--badge` | Write a shields-style SVG badge with the dead-code percentage |
| `--baseline` | Baseline file for ignoring existing issues New issues not in baseline will be reported |
| `--baseline-prune` | Drop baseline entries whose finding no longer exists (resolved), rewriting the --baseline file |
| `--baseline-rm` | Remove entries matching this name (or FQN) from the --baseline file, then exit |
| `--baseline-show` | List the entries of the --baseline file, then exit |
| `--baseline-stats` | Show baseline entries counted per rule (where the tool cries wolf the most), then exit |
| `--batch-branches` | Create one local git branch per dead top-level class, each with one commit carrying the proof of death, then exit |
| `--behavior` | Assumed final behavior of --flag [default: enabled] [possible values: enabled, disabled] |
| `--blame` | Annotate each finding with its last author and date (one git call per finding) |
| `--by-module` | Replace the report with a per-module summary (count, top rule) |
| `--cache-path` | Custom cache file path (default: .searchdeadcode-cache.json) |
| `--changed-since` | PR-scoped analysis: judge only files changed since this git ref, reporting only symbols provably unreferenced project-wide |
| `--clear-cache` | Clear the analysis cache before running |
| `--clusters` | Group dead code findings into connected, deletable clusters |
| `--compact` | Compact output - one line per issue |
| `--compare` | Migration diff: OLD=NEW worlds (package prefix or path fragment). Lists old-world symbols deletable at the flip and the blockers |
| `--completions` | Generate shell completions [possible values: bash, elvish, fish, powershell, zsh] |
| `--compose-patterns` | Enable Compose-specific anti-pattern detectors (AP031-AP034) Detects: state without remember, LaunchedEffect without key, business logic in composables, NavController passing to children |
| `--config` | Path to configuration file |
| `--coverage` | Coverage files (JaCoCo XML, Kover XML, or LCOV format) Can be specified multiple times for merged coverage |
| `--dead-accessors` | List JavaBean properties whose getter nobody calls (write-only or fully dead groups), then exit |
| `--dead-di-modules` | List DI modules whose every binding produces an unconsumed type, then exit |
| `--dead-keep-rules` | List exact -keep rules naming project classes that no longer exist, then exit |
| `--dead-modules` | List Gradle modules nobody depends on (whole-module deletion candidates) and exit |
| `--dead-serializables` | List @Serializable classes with zero incoming references (kept only by their annotation), then exit |
| `--debug-only` | List src/main symbols referenced only from other source sets (debug, flavors, tests) — dead in the release build — then exit |
| `--deep` | Enable deep analysis mode - more aggressive detection (enabled by default) Does not auto-mark class members as reachable Detects unused members even in reachable classes [default: true] [possible values: true, false] |
| `--delete` | Enable safe delete mode |
| `--deprecated` | Split @Deprecated symbols into ready-to-delete (no references) and unfinished migrations (still referenced), then exit |
| `--detect` | Detection types to run (comma-separated) |
| `--detect-cycles` | Detect and report zombie code cycles (mutually dependent dead code) |
| `--diff-base` | Report only symbols that became dead since this git reference |
| `--doctor` | Check the config against the repo's reality (dead globs, unknown entry points, missing targets) and exit |
| `--dry-run` | Dry run - show what would be deleted without making changes |
| `--duplicate-strings` | List string values declared in several modules (centralization candidates), then exit |
| `--enhanced` | Enable enhanced detection mode with ProGuard cross-validation |
| `--exclude` | Patterns to exclude (can be specified multiple times) |
| `--expand` | Expand all collapsed groups (show every issue) |
| `--expand-rule` | Expand a specific rule's issues (e.g., --expand-rule AP017) |
| `--explain` | Explain why a symbol (simple name or FQN) is considered dead or alive |
| `--export-graph` | Write the reference graph to this file (.json or .dot), then exit |
| `--fail-on-findings` | Exit 1 when findings remain after filtering (baseline included) — the scriptable CI gate |
| `--fix` | Apply zero-risk fixes automatically (unused imports). Combine with --dry-run to preview. Always writes an undo script |
| `--flag` | Feature flag cleanup: name (or key) of the flag being settled |
| `--format` | Output format (defaults to report.format from .deadcode.yml, else terminal) [possible values: terminal, compact, json, sarif, html, markdown, reviewdog] |
| `--generate-baseline` | Generate a baseline file from current results |
| `--generate-report` | Generate a filtered dead code report from ProGuard usage.txt Filters out generated code (Dagger, Hilt, _Factory, _Impl, etc.) |
| `--graph-file` | A saved --export-graph JSON to answer queries from (no re-scan) |
| `--group-by` | Group results by: rule, category, severity, file |
| `--include-runtime-dead` | Include runtime-dead code (reachable but never executed) |
| `--incremental` | Enable incremental analysis with caching (enabled by default) Skips re-parsing unchanged files for faster subsequent runs [default: true] [possible values: true, false] |
| `--import-suppressions` | Convert @Suppress(\"unused\") annotations into entries of this baseline file (migration from Detekt-style triage), then exit |
| `--init` | Generate a commented .deadcode.yml matching the project's shape (source sets, DI framework, exclusions) and exit |
| `--install-hook` | Install a pre-commit hook running the fast diff mode, then exit |
| `--interactive` | Interactive mode for deletions (confirm each) |
| `--kill-list` | Show everything that falls if this symbol is deleted (exclusive dependents) |
| `--kotlin-patterns` | Enable Kotlin anti-pattern detectors (AP007-AP010, AP021-AP025) Detects: GlobalScope, heavy ViewModel, lateinit abuse, scope function chaining, nullability overload, reflection overuse, long parameter lists, complex conditions |
| `--library-mode` | Treat the public API as alive (its consumers live outside this repo) — report internal deadness only |
| `--lsp-serve` | Serve LSP diagnostics over stdio from --graph-file |
| `--mcp-serve` | Serve MCP tools (refs_of, is_dead) over stdio from --graph-file |
| `--middlemen` | List classes whose every method forwards to the same delegate, then exit |
| `--min-confidence` | Minimum confidence level to report (low, medium, high, confirmed). Defaults to medium, or to the --profile choice |
| `--module-usage` | Attribute a shared module's symbols to their real consumers: unreferenced, internal-only, or used by which directories |
| `--necromancy` | Fail when code references a symbol the --baseline judged dead (someone is resurrecting legacy), then exit |
| `--output` | Output file (for json/sarif formats) |
| `--parallel` | Enable parallel processing for faster analysis (enabled by default) [default: true] [possible values: true, false] |
| `--patch` | With --delete --dry-run: write the would-be deletion as a unified diff, reviewable and applicable with git apply |
| `--performance-patterns` | Enable performance anti-pattern detectors (AP011-AP015) Detects: memory leaks, long methods, large classes, collection inefficiencies, loop allocations |
| `--profile` | Preset for an audience: ci (strict, high confidence only) or explore (everything down to low) Possible values: - ci:      Strict: high-confidence findings only — for pipelines - explore: Everything down to low confidence — for humans digging |
| `--proguard-usage` | ProGuard/R8 usage.txt file for enhanced detection This file lists code that R8 determined is unused |
| `--promises` | Cross TODO remove / FIXME delete comments with the symbol's real reference count, then exit |
| `--quick-wins` | Only the findings safe to delete blind: whole cluster dead, every member low risk |
| `--quiet` | Quiet mode - only output results |
| `--ratchet` | With --baseline: fail on new issues (exit 3) and rewrite the baseline downward on progress — the count can only decrease |
| `--redundant-overrides` | Enable redundant override detection (off by default - can be intentional) Finds method overrides that only call super |
| `--refs-of` | Print who references this symbol, answered from --graph-file |
| `--report-package` | Package prefix to include in report (e.g., "com.example") Only classes matching this prefix will be included |
| `--retain` | Patterns to retain - never report as dead (can be specified multiple times) |
| `--retention-audit` | Show how many declarations each retention annotation keeps alive, broadest first, and exit |
| `--runtime-only` | Only show findings confirmed by runtime coverage |
| `--score` | Rank findings by deletability: lines x confidence / risk |
| `--sealed-variants` | Enable unused sealed variant detection (enabled by default) Finds sealed class variants that are never instantiated [default: true] [possible values: true, false] |
| `--stale-flags` | List boolean Remote Config flags with their defaults and the ready-made --flag probe for each, then exit |
| `--style` | Style lints: redundant this, doubled parentheses, size==0 (DC014-16) |
| `--summary` | Summary output - show statistics and top issues only |
| `--target` | Target directories to analyze (can be specified multiple times) |
| `--top` | Number of top issues to show in summary mode [default: 10] |
| `--top-files` | Rank files by deletable lines instead of reporting findings |
| `--tui` | Full-screen findings triage (needs a terminal) |
| `--twins` | Show Xxx/XxxV2-style pairs side by side with reference counts, then exit |
| `--undo-script` | Generate undo script |
| `--unobserved` | List exposed LiveData/StateFlow/SharedFlow properties nobody collects or observes, then exit |
| `--unscheduled-workers` | List Worker/JobService classes nobody ever enqueues, then exit |
| `--unused-assets` | List assets/ files whose path or name appears nowhere in the sources, then exit |
| `--unused-deps` | List Gradle dependencies declared in build files but never imported by any source file, then exit |
| `--unused-extras` | Enable unused Intent extra detection (enabled by default) Finds putExtra() keys that are never retrieved via getXxxExtra() [default: true] [possible values: true, false] |
| `--unused-params` | Enable unused parameter detection (enabled by default) Finds function parameters that are declared but never used [default: true] [possible values: true, false] |
| `--unused-permissions` | List manifest permissions whose API family never appears in the code, then exit |
| `--unused-resources` | Enable unused resource detection (off by default - slower) Finds Android resources (strings, colors, etc.) that are never referenced |
| `--verbose` | Verbose output |
| `--verify-cmd` | After --delete: run this command (a compile, a test suite) and restore every touched file automatically when it fails |
| `--watch` | Watch mode - continuously monitor for changes |
| `--why-alive` | Show the retention chain keeping a symbol alive (inverse of --explain) |
| `--write-only` | Enable write-only variable detection (enabled by default) Finds variables that are assigned but never read [default: true] [possible values: true, false] |
| `--write-only-dao` | Enable write-only Room DAO detection (enabled by default) Finds Room DAOs that have @Insert but no @Query methods [default: true] [possible values: true, false] |
| `--write-only-prefs` | Enable write-only SharedPreferences detection (enabled by default) Finds SharedPreferences keys that are written but never read [default: true] [possible values: true, false] |
| `--yes` | With --delete: skip confirmation prompts (the CI path) |

