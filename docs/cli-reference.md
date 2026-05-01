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
                           [possible values: terminal, json, sarif]
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
