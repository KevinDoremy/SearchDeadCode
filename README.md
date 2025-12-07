# SearchDeadCode

[![CI](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml/badge.svg)](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/searchdeadcode.svg)](https://crates.io/crates/searchdeadcode)
[![GitHub Action](https://img.shields.io/badge/GitHub_Action-available-2088FF?logo=github-actions&logoColor=white)](https://github.com/marketplace/actions/searchdeadcode)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

A blazingly fast CLI tool written in Rust to detect and safely remove dead/unused code in Android projects (Kotlin & Java).

Inspired by [Periphery](https://github.com/peripheryapp/periphery) for Swift.

## Features

### Detection Capabilities

| Category | Detection |
|----------|-----------|
| **Core** | Unused classes, interfaces, methods, functions, properties, fields, imports |
| **Advanced** | Unused parameters, enum cases, type aliases |
| **Smart** | Assign-only properties (written but never read), dead branches, redundant public modifiers |
| **Android-Aware** | Respects Activities, Fragments, XML layouts, Manifest entries as entry points |
| **Resources** | Unused Android resources (strings, colors, dimens, styles, attrs) |

### Safe Delete

- **Interactive mode**: Confirm each deletion individually
- **Batch mode**: Review all candidates, confirm once
- **Dry-run**: Preview what would be deleted
- **Undo support**: Generate restore scripts

## Quick Start

```bash
# Install via cargo
cargo install searchdeadcode

# Analyze an Android project
searchdeadcode /path/to/android/project

# Dry-run deletion preview
searchdeadcode /path/to/project --delete --dry-run
```

### Example Output

```
$ searchdeadcode ./my-app --min-confidence high

SearchDeadCode v0.4.0
Discovering files...
Found 247 files to analyze
Parsing files...
Detecting entry points...
Found 89 entry points
Running reachability analysis...
Reachability: 1,847 reachable, 2,103 total

Found 12 dead code issues:

Confidence Legend:
  ● Confirmed (runtime) ◉ High ○ Medium ◌ Low

app/src/main/java/com/example/data/OldApiClient.kt
  ◉ 15:1 warning [DC001] class 'LegacyApiClient' is never used
    → class 'LegacyApiClient'

app/src/main/java/com/example/utils/StringUtils.kt
  ◉ 42:5 warning [DC001] function 'formatLegacyDate' is never used
    → function 'formatLegacyDate'
  ◉ 67:5 warning [DC001] function 'parseOldFormat' is never used
    → function 'parseOldFormat'

app/src/main/java/com/example/models/User.kt
  ◉ 23:5 warning [DC004] property 'middleName' is never used
    → property 'middleName'

Summary: 12 issues in 4 files (3 classes, 5 functions, 4 properties)
Estimated removable lines: ~340
```

## Installation

### Via Cargo (Recommended)

```bash
cargo install searchdeadcode
```

### Pre-built Binaries

Download the latest release from [GitHub Releases](https://github.com/KevinDoremy/SearchDeadCode/releases).

Available binaries:
- `searchdeadcode-linux-x86_64` - Linux (Intel/AMD 64-bit)
- `searchdeadcode-linux-aarch64` - Linux (ARM 64-bit)
- `searchdeadcode-macos-x86_64` - macOS (Intel)
- `searchdeadcode-macos-aarch64` - macOS (Apple Silicon)
- `searchdeadcode-windows-x86_64.exe` - Windows (64-bit)

#### macOS: Bypass Gatekeeper Warning

macOS may show a security warning because the binary isn't code-signed. To run it:

**Option 1: Remove quarantine attribute (recommended)**
```bash
xattr -d com.apple.quarantine ~/Downloads/searchdeadcode-macos-*
chmod +x ~/Downloads/searchdeadcode-macos-*
```

**Option 2: Right-click → Open**
- Right-click the binary in Finder
- Select "Open" from the context menu
- Click "Open" in the dialog

**Option 3: System Preferences**
- Go to System Preferences → Privacy & Security
- Click "Open Anyway" next to the blocked app message

### From Source

```bash
git clone https://github.com/KevinDoremy/SearchDeadCode
cd SearchDeadCode
cargo install --path .
```

### GitHub Action

Add dead code detection to your CI pipeline:

```yaml
# .github/workflows/dead-code.yml
name: Dead Code Detection

on: [push, pull_request]

jobs:
  dead-code:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Detect Dead Code
        uses: KevinDoremy/SearchDeadCode@v0
        with:
          path: '.'
          min-confidence: 'medium'
```

#### Action Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `path` | Path to analyze | `.` |
| `version` | SearchDeadCode version | `latest` |
| `format` | Output format: `terminal`, `json`, `sarif` | `terminal` |
| `output` | Output file path | - |
| `args` | Additional CLI arguments | - |
| `fail-on-findings` | Fail if dead code found | `false` |
| `min-confidence` | Minimum confidence: `low`, `medium`, `high`, `confirmed` | `medium` |

#### Advanced Examples

**Fail CI on dead code:**
```yaml
- uses: KevinDoremy/SearchDeadCode@v0
  with:
    fail-on-findings: 'true'
    min-confidence: 'high'
```

**SARIF output for GitHub Security:**
```yaml
- uses: KevinDoremy/SearchDeadCode@v0
  with:
    format: 'sarif'
    output: 'dead-code.sarif'
```

**Deep analysis with all detectors:**
```yaml
- uses: KevinDoremy/SearchDeadCode@v0
  with:
    args: '--deep --unused-params --write-only --sealed-variants'
```

## Usage

### Basic Analysis

```bash
# Analyze current directory
searchdeadcode .

# Analyze specific Android project
searchdeadcode ./my-android-app

# Analyze with verbose output
searchdeadcode ./app --verbose

# Quiet mode (only results)
searchdeadcode ./app --quiet
```

### Output Formats

```bash
# Terminal (default) - colored, grouped output
searchdeadcode ./app

# JSON - for programmatic use
searchdeadcode ./app --format json --output report.json

# SARIF - for GitHub Actions / CI integration
searchdeadcode ./app --format sarif --output report.sarif
```

### Hybrid Analysis (Static + Dynamic)

SearchDeadCode supports hybrid analysis by combining static code analysis with runtime coverage data. This significantly increases confidence in dead code findings and reduces false positives.

#### Using Runtime Coverage

```bash
# With JaCoCo coverage from CI tests
searchdeadcode ./app --coverage build/reports/jacoco/test/jacocoTestReport.xml

# With Kover coverage (Kotlin)
searchdeadcode ./app --coverage build/reports/kover/report.xml

# With LCOV coverage
searchdeadcode ./app --coverage coverage/lcov.info

# Multiple coverage files (merged)
searchdeadcode ./app \
  --coverage build/reports/unit-test.xml \
  --coverage build/reports/integration-test.xml
```

#### Confidence Levels

Each finding is assigned a confidence level:

| Level | Indicator | Description |
|-------|-----------|-------------|
| **Confirmed** | ● (green) | Runtime coverage confirms code is never executed |
| **High** | ◉ (bright green) | Private/internal code with no static references |
| **Medium** | ○ (yellow) | Default for static-only analysis |
| **Low** | ◌ (red) | May be false positive (reflection, dynamic dispatch) |

```bash
# Only show high-confidence and confirmed findings
searchdeadcode ./app --min-confidence high

# Only show runtime-confirmed findings (safest)
searchdeadcode ./app --coverage coverage.xml --runtime-only
```

#### Runtime-Dead Code Detection

Find code that passes static analysis but is never executed in practice:

```bash
# Include reachable but never-executed code
searchdeadcode ./app --coverage coverage.xml --include-runtime-dead
```

This detects "zombie code" - code that exists in your codebase and appears to be used (passes static analysis) but is never actually executed during test runs.

### ProGuard/R8 Integration

Leverage ProGuard/R8's `usage.txt` for **confirmed** dead code detection. R8 performs whole-program analysis during release builds and identifies code it will remove.

#### Generating usage.txt

Add to your `proguard-rules.pro`:
```
-printusage usage.txt
```

Then build your release APK:
```bash
./gradlew assembleRelease
```

The file will be at: `app/build/outputs/mapping/release/usage.txt`

#### Using with SearchDeadCode

```bash
# Analyze with ProGuard data
searchdeadcode ./app --proguard-usage path/to/usage.txt

# Combine with other options
searchdeadcode ./app \
  --proguard-usage usage.txt \
  --coverage coverage.xml \
  --detect-cycles
```

#### Real-World Example

```bash
# Full analysis with R8 usage.txt
./target/release/searchdeadcode /path/to/your/android-project \
  --exclude "**/build/**" \
  --exclude "**/test/**" \
  --exclude "**/Color.kt" \
  --exclude "**/Theme.kt" \
  --proguard-usage /path/to/your/android-project/app/usage.txt \
  --detect-cycles

# Output:
# 📋 ProGuard usage.txt: 106329 unused items (24593 classes, 55479 methods)
# 🧟 Zombie Code Detected: 1 dead cycle (2 declarations)
# Found 21 dead code issues:
#   ● 8 confirmed (matched with R8/ProGuard)
#   ○ 13 medium confidence
```

#### Sample Output with ProGuard Integration

```
📋 ProGuard usage.txt: 106329 unused items (24593 classes, 55479 methods)

Found 21 dead code issues:

Confidence Legend:
  ● Confirmed (runtime) ◉ High
  ○ Medium ◌ Low

/app/src/main/java/com/example/app/admin/ui/SingleLiveEvent.kt
  ● 22:1 warning [DC001] class 'SingleLiveEvent' is never used (confirmed by R8/ProGuard)
    → class 'SingleLiveEvent'

/base/src/main/java/com/example/common/text/HtmlFormatterHelper.kt
  ● 7:1 warning [DC001] class 'HtmlFormatterHelper' is never used (confirmed by R8/ProGuard)
    → class 'HtmlFormatterHelper'

────────────────────────────────────────────────────────────
Summary: 21 warnings

By Confidence:
  ● 8 confirmed (0 runtime-confirmed)
  ○ 13 medium confidence
```

#### What This Provides

| Benefit | Description |
|---------|-------------|
| **Confirmed findings** | Items in usage.txt are marked as `● Confirmed` |
| **Cross-validation** | Static analysis + R8 agreement = high confidence |
| **Library dead code** | R8 sees unused library code we can't analyze |
| **False positive detection** | `const val` objects may appear unused but are inlined |

#### Important Notes

- **`const val` inlining**: Kotlin constants are inlined at compile time. The `Events` object may show as "unused" in usage.txt because only its values (not the object) are accessed at runtime. This is NOT dead code.
- **Build variants**: usage.txt is specific to release builds. Debug-only code won't appear.
- **Generated code**: Filter out `_Factory`, `_Impl`, `Dagger*`, `Hilt_*` classes.

### Zombie Code / Cycle Detection

Detect mutually dependent dead code - code that only references itself:

```bash
# Enable zombie code cycle detection
searchdeadcode ./app --detect-cycles
```

This finds patterns like:
- Class A uses Class B
- Class B uses Class A
- Neither A nor B is used by anything else

Example output:
```
🧟 Zombie Code Detected:
  2 dead cycles found (15 declarations)
  Largest cycle: 8 mutually dependent declarations
  3 zombie pairs (A↔B mutual references)

  Cycle #1 (8 items):
    • class 'LegacyHelper'
    • class 'LegacyProcessor'
    • method 'process'
    • method 'handle'
    ... and 4 more
```

### Unused Function Parameters Detection

Detect function parameters that are declared but never used within the function body:

```bash
# Enable unused parameter detection
searchdeadcode ./app --unused-params
```

This detector is conservative to minimize false positives:
- **Skips underscore-prefixed parameters** (`_unused`) - Kotlin convention for intentionally unused params
- **Skips override methods** - Parameters may be required by the interface
- **Skips abstract/interface methods** - No body to analyze
- **Skips @Composable functions** - Parameters used for recomposition
- **Skips constructors** - Parameters often used for property initialization
- **Skips callbacks/listeners** - `onXxx`, `*Listener`, `*Callback` patterns

### Unused Android Resources Detection

Detect Android resources (strings, colors, dimensions, styles, etc.) that are defined but never referenced in code or XML:

```bash
# Enable unused resource detection
searchdeadcode ./app --unused-resources
```

#### How It Works

1. **Parses resource definitions** from all `res/values/*.xml` files:
   - `strings.xml` → `R.string.*`
   - `colors.xml` → `R.color.*`
   - `dimens.xml` → `R.dimen.*`
   - `styles.xml` → `R.style.*`
   - `attrs.xml` → `R.attr.*`

2. **Scans for references** in all Kotlin, Java, and XML files:
   - Code references: `R.string.app_name`, `R.color.primary`
   - XML references: `@string/app_name`, `@color/primary`

3. **Reports unused resources** with file location and resource type

#### Example Output

```bash
$ searchdeadcode ./my-android-app --unused-resources

📦 Unused Android Resources:
  ○ app/src/main/res/values/strings.xml:21 - string 'unused_feature_text'
  ○ app/src/main/res/values/strings.xml:45 - string 'legacy_error_message'
  ○ app/src/main/res/values/colors.xml:12 - color 'deprecated_accent'
  ○ app/src/main/res/values/dimens.xml:8 - dimen 'old_margin_large'
  ○ app/src/main/res/values/styles.xml:15 - style 'LegacyButton'
  ○ base/src/main/res/values/attrs.xml:3 - attr 'customAttribute'

Found 6 unused resources (150 total defined, 320 referenced)
```

#### Real-World Results

```bash
$ searchdeadcode /path/to/android-project --unused-resources

📦 Unused Android Resources:
  ○ app/src/main/res/values/admin_strings.xml:21 - string 'admin_apiMockAddressSaved'
  ○ app/src/main/res/values/appboy.xml:3 - string 'com_braze_api_key'
  ○ app/src/main/res/values/dimens.xml:8 - dimen 'card_sticky_audio_bottom_margin'
  ○ app/src/main/res/values/styles.xml:2 - style 'AppTheme.AppBarOverlay'
  ○ base/src/main/res/values/base_strings.xml:46 - string 'donation_button_text'
  ○ component-feed/src/main/res/values/feed_colors.xml:31 - color 'dates_light'
  ... and 47 more

Found 53 unused resources (672 total defined, 1142 referenced)
```

#### Common False Positives to Ignore

Some resources may appear unused but are actually required:
- **Braze/Firebase SDK configs** (`com_braze_*`, `google_*`) - Read via reflection
- **Theme attributes** - May be referenced by parent themes
- **Build variant resources** - Only used in specific flavors

Use `--exclude` patterns or add to your config file:
```yaml
exclude:
  - "**/appboy.xml"
  - "**/google-services.xml"
```

### Deep Analysis Mode

For more aggressive dead code detection that analyzes individual members within classes:

```bash
# Enable deep analysis
searchdeadcode ./app --deep
```

#### Terminal Output Example

```
Dead Code Analysis Results
==========================

com/example/app/utils/DeadHelper.kt
  ├─ class DeadHelper (line 5)
  │  Never instantiated or referenced
  └─ function unusedFunction (line 12)
     Never called

com/example/app/models/LegacyModel.kt
  └─ property debugFlag (line 8)
     Assigned but never read

Summary: 3 issues found
  - 1 unused class
  - 1 unused function
  - 1 assign-only property
```

#### JSON Output Format

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
|-------|-------------|
| `code` | Issue code (DC001-DC007) |
| `confidence` | low, medium, high, confirmed |
| `confidence_score` | 0.25 to 1.0 for sorting |
| `runtime_confirmed` | True if coverage data confirms unused |
| `fully_qualified_name` | Package path when available |

### Filtering

```bash
# Exclude patterns (glob syntax)
searchdeadcode ./app --exclude "**/test/**" --exclude "**/generated/**"

# Retain patterns (never report as dead)
searchdeadcode ./app --retain "*Activity" --retain "*ViewModel"

# Combine multiple filters
searchdeadcode ./app \
  --exclude "**/build/**" \
  --exclude "**/*Test.kt" \
  --retain "*Repository" \
  --retain "*UseCase"
```

### Safe Delete

```bash
# Interactive deletion (confirm each item)
searchdeadcode ./app --delete --interactive

# Batch deletion (select from list, confirm once)
searchdeadcode ./app --delete

# Dry run (preview only, no changes)
searchdeadcode ./app --delete --dry-run

# Generate undo script for recovery
searchdeadcode ./app --delete --undo-script restore.sh
```

#### Dry-Run Output Example

```
Dry run - would delete:
  class DeadHelper at com/example/utils/DeadHelper.kt:5
  function unusedMethod at com/example/Service.kt:42
  property debugFlag at com/example/Config.kt:8

Total: 3 items would be deleted
```

## Configuration

### Configuration File

SearchDeadCode looks for configuration in these locations (in order):

1. Path specified via `--config` flag
2. `.deadcode.yml` / `.deadcode.yaml` in project root
3. `.deadcode.toml` in project root
4. `deadcode.yml` / `deadcode.yaml` / `deadcode.toml` in project root

### YAML Configuration Example

```yaml
# .deadcode.yml

# Directories to analyze (relative to project root)
targets:
  - "app/src/main/kotlin"
  - "app/src/main/java"
  - "feature/src/main/kotlin"
  - "core/src/main/kotlin"

# Patterns to exclude from analysis (glob syntax)
exclude:
  - "**/generated/**"      # Generated code
  - "**/build/**"          # Build outputs
  - "**/.gradle/**"        # Gradle cache
  - "**/.idea/**"          # IDE files
  - "**/test/**"           # Test files (see note below)
  - "**/*Test.kt"          # Test classes
  - "**/*Spec.kt"          # Spec classes

# Patterns to retain - never report as dead (glob syntax)
# Use for code accessed via reflection, external libraries, etc.
retain_patterns:
  - "*Adapter"             # RecyclerView adapters
  - "*ViewHolder"          # ViewHolders
  - "*Callback"            # Callback interfaces
  - "*Listener"            # Event listeners
  - "*Binding"             # View bindings

# Explicit entry points (fully qualified class names)
entry_points:
  - "com.example.app.MainActivity"
  - "com.example.app.MyApplication"
  - "com.example.api.PublicApi"

# Report configuration
report:
  format: "terminal"       # terminal | json | sarif
  group_by: "file"         # file | type | severity
  show_code: true          # Show code snippets in output

# Detection configuration - enable/disable specific detectors
detection:
  unused_class: true       # Unused classes and interfaces
  unused_method: true      # Unused methods and functions
  unused_property: true    # Unused properties and fields
  unused_import: true      # Unused import statements
  unused_param: true       # Unused function parameters
  unused_enum_case: true   # Unused enum values
  assign_only: true        # Write-only properties
  dead_branch: true        # Unreachable code branches
  redundant_public: true   # Public members only used internally

# Android-specific configuration
android:
  parse_manifest: true           # Parse AndroidManifest.xml for entry points
  parse_layouts: true            # Parse layout XMLs for class references
  auto_retain_components: true   # Auto-retain Android lifecycle components
  component_patterns:            # Additional patterns to auto-retain
    - "*Activity"
    - "*Fragment"
    - "*Service"
    - "*BroadcastReceiver"
    - "*ContentProvider"
    - "*ViewModel"
    - "*Application"
    - "*Worker"                  # WorkManager workers
```

### TOML Configuration Example

```toml
# .deadcode.toml

targets = [
  "app/src/main/kotlin",
  "app/src/main/java",
]

exclude = [
  "**/generated/**",
  "**/build/**",
  "**/test/**",
]

retain_patterns = [
  "*Adapter",
  "*ViewHolder",
]

entry_points = [
  "com.example.app.MainActivity",
]

[report]
format = "terminal"
group_by = "file"
show_code = true

[detection]
unused_class = true
unused_method = true
unused_property = true
unused_import = true
unused_param = true
unused_enum_case = true
assign_only = true
dead_branch = true
redundant_public = true

[android]
parse_manifest = true
parse_layouts = true
auto_retain_components = true
component_patterns = [
  "*Activity",
  "*Fragment",
  "*ViewModel",
]
```

## CLI Reference

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
      --undo-script <FILE> Generate undo/restore script
      --detect <TYPES>     Detection types (comma-separated)

  Analysis Options:
      --deep                  Deep analysis mode - analyzes individual members
                              within classes for more aggressive detection
      --unused-params         Detect unused function parameters
      --unused-resources      Detect unused Android resources (strings, colors, etc.)

  Hybrid Analysis Options:
      --coverage <FILE>       Coverage file (JaCoCo XML, Kover XML, or LCOV)
                              Can be specified multiple times for merged coverage
      --proguard-usage <FILE> ProGuard/R8 usage.txt file for enhanced detection
      --min-confidence        Minimum confidence level to report
                              [possible values: low, medium, high, confirmed]
      --runtime-only          Only show findings confirmed by runtime coverage
      --include-runtime-dead  Include reachable but never-executed code
      --detect-cycles         Detect zombie code cycles (mutually dependent dead code)

  -v, --verbose            Verbose output
  -q, --quiet              Quiet mode - only output results
  -h, --help               Print help
  -V, --version            Print version
```

### Complete Command Examples

```bash
# Basic analysis
searchdeadcode /path/to/android/project

# Deep analysis (more aggressive, analyzes individual members)
searchdeadcode ./app --deep

# With exclusions
searchdeadcode ./app \
  --exclude "**/build/**" \
  --exclude "**/test/**" \
  --exclude "**/generated/**"

# Full hybrid analysis (static + dynamic + R8)
searchdeadcode ./app \
  --deep \
  --coverage build/reports/jacoco.xml \
  --proguard-usage app/build/outputs/mapping/release/usage.txt \
  --detect-cycles \
  --min-confidence high

# JSON output for CI/CD
searchdeadcode ./app \
  --format json \
  --output dead-code-report.json

# SARIF for GitHub Code Scanning
searchdeadcode ./app \
  --format sarif \
  --output results.sarif

# Safe delete with dry-run preview
searchdeadcode ./app --delete --dry-run

# Detect unused Android resources
searchdeadcode ./app --unused-resources

# Detect unused function parameters
searchdeadcode ./app --unused-params

# Full analysis with all enhanced detection
searchdeadcode ./app \
  --deep \
  --unused-params \
  --unused-resources \
  --detect-cycles

# Interactive deletion with undo script
searchdeadcode ./app \
  --delete \
  --interactive \
  --undo-script restore.sh

# Only show confirmed dead code (highest confidence)
searchdeadcode ./app \
  --coverage coverage.xml \
  --proguard-usage usage.txt \
  --runtime-only \
  --min-confidence confirmed
```

## Detection Types

### 1. Unused Classes/Interfaces

Classes or interfaces that are never instantiated, extended, or referenced.

```kotlin
// DEAD: Never used anywhere
class OrphanHelper {
    fun doSomething() {}
}
```

### 2. Unused Methods/Functions

Methods that are never called, including extension functions.

```kotlin
class UserService {
    fun getUser(id: String) = // used

    // DEAD: Never called
    fun legacyGetUser(id: Int) = // ...
}

// Extension functions are also detected
fun String.deadExtension(): String = this  // DEAD: Never called
```

### 3. Unused Properties/Fields

Properties declared but never read.

```kotlin
class Config {
    val apiUrl = "https://api.example.com"  // used
    val debugMode = true                     // DEAD: never read
}
```

### 4. Assign-Only Properties

Properties that are written to but never read.

```kotlin
class Analytics {
    var lastEventTime: Long = 0  // DEAD: assigned but never read

    fun track(event: Event) {
        lastEventTime = System.currentTimeMillis()  // write-only
        send(event)
    }
}
```

### 5. Unused Parameters

Function parameters that are never used in the body.

```kotlin
// DEAD: 'context' parameter never used
fun formatDate(date: Date, context: Context): String {
    return SimpleDateFormat("yyyy-MM-dd").format(date)
}
```

### 6. Unused Imports

Import statements with no corresponding usage.

```kotlin
import com.example.utils.StringUtils  // DEAD: never used
import com.example.models.User        // used

class UserProfile {
    fun display(user: User) {}
}
```

### 7. Unused Enum Cases

Individual enum values that are never referenced.

```kotlin
enum class Status {
    ACTIVE,     // used
    INACTIVE,   // used
    LEGACY,     // DEAD: never referenced
    DEPRECATED  // DEAD: never referenced
}
```

### 8. Redundant Public Modifiers

Public declarations only used within the same module.

```kotlin
// DEAD visibility: only used internally, could be internal/private
public class InternalHelper {
    public fun process() {}  // only called within this module
}
```

### 9. Dead Branches

Code paths that can never be executed.

```kotlin
fun process(value: Int) {
    if (value > 0) {
        // reachable
    } else if (value <= 0) {
        // reachable
    } else {
        // DEAD: impossible to reach
        handleImpossible()
    }
}
```

## Android-Specific Handling

### Auto-Retained Entry Points

The tool automatically retains (never reports as dead):

| Category | Patterns / Annotations |
|----------|----------------------|
| **Lifecycle Components** | `*Activity`, `*Fragment`, `*Service`, `*BroadcastReceiver`, `*ContentProvider`, `*Application` |
| **Jetpack Compose** | `@Composable`, `@Preview` |
| **ViewModels** | `*ViewModel`, `@HiltViewModel` |
| **Dependency Injection** | `@Inject`, `@Provides`, `@Binds`, `@BindsOptionalOf`, `@BindsInstance`, `@IntoMap`, `@IntoSet`, `@Module`, `@Component`, `@HiltAndroidApp`, `@AndroidEntryPoint`, `@AssistedInject`, `@AssistedFactory` |
| **Serialization** | `@Serializable`, `@Parcelize`, `@JsonClass`, `@Entity`, `@SerializedName`, `@SerialName` |
| **Data Binding** | `@BindingAdapter`, `@InverseBindingAdapter`, `@BindingMethod`, `@BindingMethods`, `@BindingConversion` |
| **Room Database** | `@Dao`, `@Database`, `@Query`, `@Insert`, `@Update`, `@Delete`, `@RawQuery`, `@Transaction`, `@TypeConverter` |
| **Retrofit** | `@GET`, `@POST`, `@PUT`, `@DELETE`, `@PATCH`, `@HEAD`, `@OPTIONS`, `@HTTP`, `@Path`, `@Body`, `@Field`, `@Header` |
| **Testing** | `@Test`, `@Before`, `@After`, `@BeforeEach`, `@AfterEach`, `@BeforeAll`, `@AfterAll`, `@ParameterizedTest`, `@RunWith` |
| **Reflection** | `@JvmStatic`, `@JvmOverloads`, `@JvmField`, `@JvmName`, `@Keep` |
| **WorkManager** | `@HiltWorker` |
| **Lifecycle** | `@OnLifecycleEvent` |
| **Koin DI** | `@Factory`, `@Single`, `@KoinViewModel` |
| **Event Bus** | `@Subscribe` |
| **Coroutines** | `suspend` functions (in reachable classes), `@FlowPreview`, `@ExperimentalCoroutinesApi` |
| **Entry Functions** | `main()` functions |

### XML Parsing

The tool parses Android XML files to detect additional entry points:

**AndroidManifest.xml**
- `<activity android:name=".MainActivity">`
- `<service android:name=".MyService">`
- `<receiver>`, `<provider>`, `<application>` components

**Layout XMLs** (`res/layout/*.xml`)
- Custom views: `<com.example.CustomView>`
- Context references: `tools:context=".MyActivity"`
- Data binding: `app:viewModel="@{viewModel}"`

## Test Code Handling

Code that is **only** used in tests is considered dead code. This is intentional because:

1. Test-only utilities should be in test directories
2. Production code shouldn't exist solely for testing
3. Such code adds maintenance burden without production value

To exclude test files from analysis:
```yaml
exclude:
  - "**/test/**"
  - "**/androidTest/**"
  - "**/*Test.kt"
  - "**/*Spec.kt"
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLI Interface                             │
│                    (clap + YAML config)                          │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                      File Discovery                              │
│              (ignore crate, respects .gitignore)                 │
│                    .kt  .java  .xml files                        │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Parsing Phase (Parallel)                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ tree-sitter-    │  │ tree-sitter-    │  │   quick-xml     │  │
│  │     kotlin      │  │      java       │  │  (AndroidXML)   │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Declaration Registry                           │
│                                                                   │
│  HashMap<DeclarationId, Declaration>                             │
│  • Fully qualified names (com.example.MyClass.myMethod)          │
│  • Location: file:line:column                                    │
│  • Kind: class | method | property | function | enum | etc.      │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Reference Graph                              │
│                                                                   │
│  petgraph::DiGraph<Declaration, Reference>                       │
│  • Nodes = all declarations                                      │
│  • Edges = usages/references between declarations                │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Entry Point Detection                           │
│                                                                   │
│  Android Roots (auto-retained):                                  │
│  • Activity, Fragment, Service, BroadcastReceiver, Provider      │
│  • @Composable functions                                         │
│  • Classes in AndroidManifest.xml                                │
│  • Views referenced in layout XMLs                               │
│  • @Serializable, @Parcelize data classes                        │
│  • main() functions, @Test methods                               │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Reachability Analysis                           │
│                                                                   │
│  DFS/BFS from entry points → mark reachable nodes                │
│  Unreachable declarations = dead code candidates                 │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Output                                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐ │
│  │ Terminal │  │   JSON   │  │  SARIF   │  │   Safe Delete    │ │
│  │ (colored)│  │ (export) │  │  (CI)    │  │  (interactive)   │ │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Technology Stack

| Crate | Purpose | Why |
|-------|---------|-----|
| `tree-sitter` | Core parsing | Incremental, error-tolerant parsing |
| `tree-sitter-kotlin` (v0.3.6) | Kotlin grammar | Official tree-sitter grammar |
| `tree-sitter-java` (v0.21) | Java grammar | Official tree-sitter grammar |
| `petgraph` | Graph data structure | Fast graph algorithms (DFS/BFS) |
| `ignore` | File discovery | Same as ripgrep, respects .gitignore |
| `rayon` | Parallelism | Parse files in parallel |
| `clap` | CLI parsing | Industry standard, derive macros |
| `serde` | Config parsing | YAML/TOML support |
| `quick-xml` | XML parsing | Fast AndroidManifest/layout parsing |
| `indicatif` | Progress bars | User feedback for large codebases |
| `colored` | Terminal colors | Readable output |
| `miette` | Error reporting | Beautiful diagnostics with code snippets |
| `dialoguer` | Interactive prompts | Safe delete confirmations |

## Project Structure

```
searchdeadcode/
├── Cargo.toml
├── src/
│   ├── main.rs                  # CLI entry point
│   ├── lib.rs                   # Library exports
│   │
│   ├── config/
│   │   ├── mod.rs
│   │   └── loader.rs            # YAML/TOML config loading
│   │
│   ├── discovery/
│   │   ├── mod.rs
│   │   └── file_finder.rs       # Parallel file discovery
│   │
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── kotlin.rs            # Kotlin AST → declarations
│   │   ├── java.rs              # Java AST → declarations
│   │   ├── xml/
│   │   │   ├── mod.rs
│   │   │   ├── manifest.rs      # AndroidManifest.xml
│   │   │   └── layout.rs        # Layout XMLs
│   │   └── common.rs            # Shared types
│   │
│   ├── graph/
│   │   ├── mod.rs
│   │   ├── declaration.rs       # Declaration types
│   │   ├── reference.rs         # Reference types
│   │   └── builder.rs           # Graph construction
│   │
│   ├── analysis/
│   │   ├── mod.rs
│   │   ├── entry_points.rs      # Entry point detection
│   │   ├── reachability.rs      # DFS/BFS traversal
│   │   └── detectors/
│   │       ├── mod.rs
│   │       ├── unused_class.rs
│   │       ├── unused_method.rs
│   │       ├── unused_property.rs
│   │       ├── unused_import.rs
│   │       ├── unused_param.rs
│   │       ├── unused_enum_case.rs
│   │       ├── assign_only.rs
│   │       ├── dead_branch.rs
│   │       └── redundant_public.rs
│   │
│   ├── refactor/
│   │   ├── mod.rs
│   │   ├── safe_delete.rs       # Interactive deletion
│   │   ├── undo.rs              # Restore script generation
│   │   └── editor.rs            # File modification
│   │
│   └── report/
│       ├── mod.rs
│       ├── terminal.rs          # Colored CLI output
│       ├── json.rs              # JSON export
│       └── sarif.rs             # SARIF for CI
│
├── tests/
│   ├── fixtures/
│   │   ├── kotlin/              # Test Kotlin files
│   │   ├── java/                # Test Java files
│   │   └── android/             # Full Android project
│   └── integration/
│
└── benches/
    └── parsing_bench.rs         # Performance benchmarks
```

## Implementation Status

All major features are implemented and tested:

### Core Analysis
- [x] Project setup with Cargo
- [x] CLI with clap (all options)
- [x] Config file loading (YAML + TOML)
- [x] File discovery with ignore crate
- [x] tree-sitter-kotlin integration
- [x] tree-sitter-java integration
- [x] Declaration extraction (classes, methods, properties, extension functions)
- [x] Fully-qualified name resolution
- [x] Generic type handling (`Foo<T>` → `Foo`)
- [x] Declaration registry
- [x] Reference extraction (including navigation expressions)
- [x] Graph construction with petgraph
- [x] AndroidManifest.xml parsing
- [x] Layout XML parsing
- [x] Entry point detection (annotations, inheritance, XML references)
- [x] Reachability analysis (DFS)
- [x] All detection types (9 detectors)

### Hybrid Analysis (Static + Dynamic)
- [x] JaCoCo XML coverage parsing
- [x] Kover XML coverage parsing
- [x] LCOV coverage parsing
- [x] ProGuard/R8 usage.txt parsing
- [x] Confidence scoring (low/medium/high/confirmed)
- [x] Runtime-dead code detection (reachable but never executed)
- [x] Zombie code cycle detection (Tarjan's algorithm)

### Deep Analysis Mode
- [x] Individual member analysis within classes
- [x] Interface implementation tracking
- [x] Sealed class subtype tracking
- [x] Suspend function detection
- [x] Flow/StateFlow/SharedFlow pattern detection
- [x] Companion object member analysis
- [x] Lazy/delegated property detection
- [x] Generic type argument tracking
- [x] Class delegation pattern detection
- [x] Const val skip (compile-time inlining)
- [x] Data class generated method skip
- [x] Comprehensive DI annotation support (Dagger, Hilt, Koin, Room, Retrofit)

### Output & Refactoring
- [x] Terminal reporter (colored with confidence indicators)
- [x] JSON reporter (v1.1 with confidence data)
- [x] SARIF reporter
- [x] Interactive deletion mode
- [x] Batch deletion mode
- [x] Dry-run mode
- [x] Undo script generation

## Known Limitations

1. **Reflection**: Code accessed via reflection (e.g., `Class.forName()`) cannot be detected as used. Use `retain_patterns` for such cases.

2. **Multi-module Projects**: Each module is analyzed independently. Cross-module references work but require all modules to be in the analysis path.

3. **Annotation Processors**: Generated code (Dagger, Room, etc.) should be excluded as it may reference declarations in ways not visible to static analysis. However, the tool now properly recognizes most DI annotations (`@Provides`, `@Binds`, `@Query`, etc.) as entry points.

4. **`const val` Inlining**: Kotlin compile-time constants are inlined by the compiler. The tool now automatically skips `const val` properties to avoid false positives.

5. **ProGuard Keep Rules**: The tool doesn't parse ProGuard `-keep` rules. Use `retain_patterns` for kept classes, or verify against usage.txt output.

6. **R.* Resource References**: Android resource references (`R.drawable.*`, `R.string.*`, etc.) are compile-time constants and don't create trackable references in the code graph.

## Troubleshooting

### "No Kotlin or Java files found"

- Check that your target path is correct
- Ensure files aren't excluded by `.gitignore` or `--exclude` patterns
- Verify the project structure has `.kt` or `.java` files

### False Positives

If code is incorrectly reported as dead:

1. **Check entry points**: Add to `entry_points` in config
2. **Check patterns**: Add to `retain_patterns` for reflection/framework usage
3. **Check annotations**: Ensure framework annotations are recognized
4. **Check XML**: Verify AndroidManifest.xml and layouts are being parsed

```yaml
# Common false positive fixes
retain_patterns:
  - "*Adapter"           # RecyclerView adapters
  - "*ViewHolder"        # ViewHolders
  - "*Callback"          # Callback interfaces
  - "*Binding"           # Generated bindings
  - "Dagger*"            # Dagger components
```

### Extension Functions Named `<anonymous>`

This was fixed in v0.1.0. If you see this, ensure you're using the latest version.

### Generic Types Not Matching

Generic type references like `Foo<Bar>` now correctly match declarations `Foo`. This was fixed in v0.1.0.

### Glob Patterns Matching Wrong Paths

Patterns like `**/test/**` now only match complete directory names, not substrings. `/test/` matches, but `/testproject/` does not.

## Changelog

### v0.4.0 (Current)

**Enhanced Detection (Phase 6)**
- **`--unused-resources` flag**: Detect unused Android resources (strings, colors, dimens, styles, attrs)
  - Parses all `res/values/*.xml` files for resource definitions
  - Scans Kotlin, Java, and XML files for `R.type.name` and `@type/name` references
  - Real-world test: Found 53 unused resources in a 1800-file project
- **`--unused-params` flag**: Detect unused function parameters
  - Conservative detection to minimize false positives
  - Skips override methods, abstract methods, @Composable functions, constructors

**Performance & CI Features (Phase 5)**
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
- **Optimized reachability analysis**: ~8% faster on large codebases

**CLI Additions**
- `--unused-resources` - Detect unused Android resources
- `--unused-params` - Detect unused function parameters
- `--incremental` - Enable incremental analysis with caching
- `--clear-cache` - Clear the analysis cache
- `--cache-path <FILE>` - Custom cache file path
- `--baseline <FILE>` - Use baseline to filter existing issues
- `--generate-baseline <FILE>` - Generate baseline from current results
- `--watch` - Watch mode for continuous monitoring

### v0.3.0

**Deep Analysis Mode**
- **`--deep` flag**: More aggressive dead code detection that analyzes individual members within classes
- **Suspend function detection**: Properly handles Kotlin suspend functions and marks them as reachable when containing class is reachable
- **Flow pattern detection**: Recognizes Kotlin Flow, StateFlow, SharedFlow patterns
- **Interface implementation tracking**: Classes implementing reachable interfaces are now marked as reachable
- **Sealed class subtype tracking**: All subtypes of reachable sealed classes are marked as reachable

**Enhanced DI/Framework Support**
- Comprehensive annotation detection for Dagger, Hilt, Koin, Room, Retrofit
- Methods with `@Provides`, `@Binds`, `@Query`, `@GET`, etc. are properly recognized as entry points
- Skips DI entry points in deep analysis to avoid false positives

**Kotlin Language Features**
- **Companion object analysis**: Properly tracks companion objects and their members
- **Lazy/delegated property detection**: Properties using `by lazy`, `by Delegates.observable()`, etc.
- **Generic type argument tracking**: Properly extracts and tracks type arguments from `List<MyClass>`, `Map<K, V>`, etc.
- **Class delegation**: Detects `class Foo : Bar by delegate` patterns
- **Const val handling**: Skips `const val` properties (inlined at compile time)
- **Data class methods**: Skips auto-generated `copy()`, `componentN()`, `equals()`, `hashCode()`, `toString()`

**Results**
- ~23% reduction in false positives on real-world Android projects (deep mode)
- ~15% reduction in false positives (standard mode)

### v0.2.0

**New Features**
- **ProGuard/R8 Integration**: Use `--proguard-usage` to load R8's usage.txt for confirmed dead code detection
- **Hybrid Analysis**: Combine static analysis with runtime coverage (JaCoCo, Kover, LCOV)
- **Confidence Scoring**: Findings now have confidence levels (low/medium/high/confirmed)
- **Zombie Code Detection**: Find mutually dependent dead code cycles with `--detect-cycles`
- **Runtime-Dead Code**: Detect code that's reachable but never executed with `--include-runtime-dead`

**CLI Additions**
- `--proguard-usage <FILE>` - Load ProGuard/R8 usage.txt
- `--coverage <FILE>` - Load coverage data (can be repeated)
- `--min-confidence <LEVEL>` - Filter by confidence level
- `--runtime-only` - Only show runtime-confirmed findings
- `--include-runtime-dead` - Include reachable but never-executed code
- `--detect-cycles` - Enable zombie code cycle detection

**Output Improvements**
- Confidence indicators in terminal output: ● ◉ ○ ◌
- JSON schema v1.1 with confidence_score and runtime_confirmed fields
- Better grouping and summary statistics

### v0.1.0

**Bug Fixes**
- Fixed extension function name extraction (no longer reported as `<anonymous>`)
- Fixed generic type resolution (`Focusable<T>` now matches `Focusable`)
- Fixed navigation expression references (`obj.method()` calls now detected)
- Fixed ambiguous reference resolution (overloaded functions all marked as used)
- Fixed glob pattern matching (`**/test/**` no longer matches `/testproject/`)
- Fixed dry-run mode (no longer requires interactive terminal)

**Improvements**
- Reduced false positives by ~51% on real-world Android projects
- Better handling of Kotlin extension functions
- Improved method call detection via navigation_suffix nodes
- All CLI options working and tested

## Performance

Target performance goals (achieved):

| Codebase Size | Parse Time | Analysis Time |
|---------------|------------|---------------|
| 1,000 files   | < 1s       | < 0.5s        |
| 10,000 files  | < 5s       | < 2s          |
| 100,000 files | < 30s      | < 10s         |

## CI/CD Integration

### GitHub Actions

```yaml
name: Dead Code Check

on: [push, pull_request]

jobs:
  deadcode:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install SearchDeadCode
        run: cargo install searchdeadcode

      - name: Run Dead Code Analysis
        run: searchdeadcode . --format sarif --output deadcode.sarif

      - name: Upload SARIF
        uses: github/codeql-action/upload-sarif@v2
        with:
          sarif_file: deadcode.sarif
```

### GitLab CI

```yaml
deadcode:
  stage: analyze
  script:
    - cargo install searchdeadcode
    - searchdeadcode . --format json --output deadcode.json
  artifacts:
    paths:
      - deadcode.json
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Write tests for new functionality
4. Ensure all tests pass (`cargo test`)
5. Submit a pull request

See `AGENTS.md` for the full contributor guide covering module layout, workflows, and review expectations.

## References

- [Periphery](https://github.com/peripheryapp/periphery) - Swift dead code detector (architecture inspiration)
- [tree-sitter](https://tree-sitter.github.io/) - Incremental parsing library
- [ripgrep](https://github.com/BurntSushi/ripgrep) - Fast file search (ignore crate)
- [ast-grep](https://ast-grep.github.io/) - Structural code search
- [rust-code-analysis](https://github.com/mozilla/rust-code-analysis) - Mozilla's code analysis library

## Dead Code Detection Paradigms & Research

This section documents the various paradigms and techniques used for dead code detection, based on research across industry tools and academic literature.

### Overview of Detection Techniques

According to systematic literature reviews, there are two main approaches for automating dead code detection:

| Approach | Description | Tools |
|----------|-------------|-------|
| **Accessibility Analysis** | Build dependency graph, traverse from entry points, mark unreachable as dead | Periphery, SearchDeadCode, R8/ProGuard |
| **Data Flow Analysis** | Track how data flows through program, identify unused computations | Compilers (DCE), Static analyzers |

### 1. Graph-Based Reachability Analysis

This is the approach used by SearchDeadCode, inspired by [Periphery](https://github.com/peripheryapp/periphery):

```
Entry Points → Build Dependency Graph → DFS/BFS Traversal → Mark Reachable → Report Unreachable
```

**How Periphery works:**
1. Build project to generate the "index store" with declaration/reference info
2. Build in-memory graph of relational structure
3. Mutate graph to mark entry points
4. Traverse graph from roots to identify unreferenced declarations

**Key insight**: The index store contains detailed information about declarations and their references, enabling accurate cross-file analysis.

### 2. Static + Dynamic Hybrid Analysis (Meta's SCARF)

[Meta's SCARF system](https://engineering.fb.com/2023/10/24/data-infrastructure/automating-dead-code-cleanup/) combines multiple analysis techniques:

**Capabilities:**
- **Multi-language support**: Java, Objective-C, JavaScript, Hack, Python
- **Symbol-level analysis**: Analyzes individual variables, not just files/classes
- **Static analysis via Glean**: Indexed, standardized format for static facts
- **Runtime monitoring**: Observes actual code execution in production
- **Cycle detection**: Detects mutually dependent dead code subgraphs

**Impact at Meta:**
- Deleted 104+ million lines of code
- Removed petabytes of deprecated data
- 370,000+ automated change requests

**Key technique**: SCARF tracks two metrics - static usage (code that appears to use data) and runtime usage (actual access patterns in production).

### 3. Tree Shaking (JavaScript Bundlers)

[Webpack](https://webpack.js.org/guides/tree-shaking/) and [Rollup](https://rollupjs.org/) popularized tree shaking:

> "Start with what you need, and work outwards" vs "Start with everything, and work backwards"

**Algorithm:**
1. Build dependency graph from entry points
2. Identify all exports in modules
3. Trace which exports are actually imported/used
4. Eliminate code not reached during traversal

**Requirements:**
- ES6 module syntax (`import`/`export`) - static structure required
- CommonJS (`require`) cannot be tree-shaken due to dynamic nature

**Webpack's implementation:**
- `usedExports` optimization marks unused exports
- Terser performs final dead code elimination
- Works at module boundary level

### 4. Compiler-Based Dead Code Elimination

[R8/ProGuard](https://blog.logrocket.com/r8-code-shrinking-android-guide/) for Android:

**Process:**
1. Entry points declared in ProGuard config
2. Search for all reachable code from entry points
3. Build list of reachable tokens
4. Strip anything not in the list

**R8 advantages over ProGuard:**
- Faster (single-pass: shrink + optimize + dex)
- Better Kotlin support
- More aggressive inlining and class merging
- ~10% size reduction vs ProGuard's ~8.5%

### 5. Scope & Namespace Tracking

Tools like [ReSharper](https://www.jetbrains.com/help/resharper/Code_Analysis__Solution-Wide_Analysis__Solution-Wide_Code_Inspections.html) use solution-wide analysis:

**Capabilities:**
- Detect unused non-private members (requires whole-solution analysis)
- Track namespace imports across files
- Identify redundant type casts and unused variables
- Real-time analysis during development

**Key insight**: Some dead code can only be detected at solution/project scope, not file scope.

### 6. Transitive Dependency Analysis

Tools like [deptry](https://github.com/fpgmaas/deptry) (Python) and [Knip](https://knip.dev/) (TypeScript):

**Detects:**
- Unused dependencies (declared but not imported)
- Missing dependencies (imported but not declared)
- Transitive dependencies (used but only available through other packages)

**Multi-module support:**
- Analyze relationships between workspaces
- Understand monorepo dependency structure
- Detect cross-module dead code

### 7. Compiler Optimization Techniques

From compiler theory ([Wikipedia - Dead Code Elimination](https://en.wikipedia.org/wiki/Dead-code_elimination)):

**Data Flow Analysis:**
- Build Control Flow Graph (CFG)
- Perform liveness analysis
- Identify variables written but never read
- Remove unreachable basic blocks

**Escape Analysis:**
- Determine dynamic scope of pointers
- Enable stack allocation for non-escaping objects
- Remove synchronization for thread-local objects

**SSA-based DCE:**
- Static Single Assignment form simplifies analysis
- Each variable assigned exactly once
- Dead assignments easily identified

### 8. Incremental Analysis (Large Codebases)

For large codebases, incremental analysis is essential:

**Techniques:**
- **Caching**: Store cryptographic hashes of analysis results
- **Memoization**: Reuse unchanged computation results
- **Dependency tracking**: Only re-analyze affected code
- **Index stores**: Pre-computed declaration/reference indexes

**Tools using incremental analysis:**
- [Glean](https://glean.software/) (Meta) - Incremental indexing
- [Roslyn](https://github.com/dotnet/roslyn) - Incremental generators with aggressive caching
- Periphery - Index store from compiler

### Comparison of Approaches

| Paradigm | Accuracy | Speed | Scope | Best For |
|----------|----------|-------|-------|----------|
| Graph Reachability | High | Fast | Project | General dead code |
| Static + Dynamic | Highest | Slow | Organization | Production code |
| Tree Shaking | High | Fast | Bundle | JavaScript modules |
| Compiler DCE | Highest | Build-time | Binary | Release builds |
| Scope Analysis | Medium | Real-time | IDE | Development feedback |
| Coverage-based | Medium | Requires runtime | Executed paths | Test coverage gaps |

### Challenges & Limitations

1. **Halting Problem**: Theoretically impossible to find ALL dead code deterministically
2. **Reflection**: Dynamically invoked code cannot be detected statically
3. **Polymorphism**: Must know all possible types for method resolution
4. **Configuration**: Code referenced in XML, properties files, etc.
5. **Dynamic Languages**: Less static structure = harder analysis

### Future Improvements for SearchDeadCode

Based on this research, potential enhancements include:

| Feature | Description | Inspiration | Status |
|---------|-------------|-------------|--------|
| **Symbol-level analysis** | Track individual variables, not just declarations | Meta SCARF | ✅ Done (v0.3.0 deep mode) |
| **Cycle detection** | Find mutually dependent dead code | Meta SCARF | ✅ Done (v0.2.0) |
| **Coverage integration** | Augment static analysis with runtime data | Hybrid tools | ✅ Done (v0.2.0) |
| **Incremental mode** | Cache results, only re-analyze changes | Glean, Roslyn | Planned |
| **Transitive tracking** | Track full reference chains | deptry, Knip | Partial |
| **Cross-module analysis** | Analyze multi-module projects holistically | Knip | Planned |

## Advanced Dead Code Patterns - Prioritized Implementation Roadmap

This section documents advanced dead code patterns beyond traditional "unreferenced code" detection. These patterns represent **code that executes but serves no purpose** - a more insidious form of technical debt.

Based on analysis of real-world Android codebases (1800+ files), we've prioritized these patterns by:
- **Detectability**: How accurately can static analysis find this? (High/Medium/Low)
- **Frequency**: How common is this pattern? (Based on real-world codebase analysis)
- **Impact**: How much wasted code/resources? (High/Medium/Low)

### Priority Tier 1: High Impact, High Detectability ⭐⭐⭐

These patterns are common, easy to detect, and represent significant waste.

| # | Pattern | Detectability | Frequency | Description |
|---|---------|---------------|-----------|-------------|
| **1** | **Write-Only Variables** | High | 58+ occurrences | Variables assigned but never read (`private var x = 0` without reads) |
| **2** | **Unused Sealed Class Variants** | High | 73 sealed classes | Sealed class/interface cases that are never instantiated |
| **3** | **Override Methods That Only Call Super** | High | 284 overrides | `override fun onCreate() { super.onCreate() }` - adds no value |
| **4** | **Ignored Return Values** | High | Common | `list.map { transform(it) }` without using the result |
| **5** | **Empty Catch Blocks** | High | Common | `catch (e: Exception) { }` - swallowed errors |
| **6** | **Unused Intent Extras** | High | 90 putExtra calls | `intent.putExtra("key", value)` where "key" is never read |
| **7** | **Write-Only SharedPreferences** | High | Medium | `prefs.edit().putString("x", y).apply()` where "x" is never read |
| **8** | **Write-Only Database Tables** | High | 16 DAOs | `@Insert` without corresponding `@Query` usage |
| **9** | **Redundant Null Checks** | High | Common | `user?.let { if (it != null) }` - double null check |
| **10** | **Dead Feature Flags** | Medium | 388 isEnabled | `if (RemoteConfig.isFeatureEnabled())` where flag is always true/false |

### Priority Tier 2: Medium Impact, High Detectability ⭐⭐

Detectable patterns with moderate frequency.

| # | Pattern | Detectability | Frequency | Description |
|---|---------|---------------|-----------|-------------|
| **11** | **Unobserved LiveData/StateFlow** | Medium | 64 collectors | `_state.value = x` where `_state` is never observed in UI |
| **12** | **Unused Constructor Parameters** | High | Medium | Parameters passed to constructor but never used |
| **13** | **Middle-Man Classes** | Medium | Low | Classes that only delegate to other classes with no added logic |
| **14** | **Lazy Classes** | Medium | Low | Classes with minimal logic that could be inlined |
| **15** | **Invariants Always True/False** | High | Common | `if (list.size >= 0)` - always true |
| **16** | **Cache Write Without Read** | Medium | Medium | `cache.save(data)` but always fetching from network |
| **17** | **Analytics Events Never Analyzed** | Low | 253 log calls | Events tracked but no dashboard configured |
| **18** | **Unused Type Parameters** | High | Low | `class Foo<T>` where T is never used in the class |
| **19** | **Dead Migrations** | Medium | Low | Database migrations for versions no user has anymore |
| **20** | **Listeners Never Triggered** | Medium | Medium | `view.setOnClickListener { }` on views that can't be clicked |

### Priority Tier 3: High Impact, Lower Detectability ⭐

High-value patterns that require more sophisticated analysis.

| # | Pattern | Detectability | Frequency | Description |
|---|---------|---------------|-----------|-------------|
| **21** | **Dormant Code Reactivated** (Knight Capital Bug) | Low | Rare | Old code accidentally enabled by feature flags |
| **22** | **Defensive Copies Never Modified** | Medium | Low | `val copy = list.toMutableList()` but copy never mutated |
| **23** | **Calculations Overwritten Immediately** | Medium | Low | `var x = expensiveCalc(); x = otherValue` |
| **24** | **Partially Dead Code** | Medium | Medium | Code only used on some branches but computed on all |
| **25** | **Recalculation of Available Values** | Medium | Low | `val h1 = data.hash(); ... val h2 = data.hash()` |
| **26** | **Audit Logs Never Queried** | Low | Low | `auditDao.insert(log)` with no read methods |
| **27** | **Breadcrumbs Without Consumer** | Low | Low | Navigation history saved but never displayed |
| **28** | **Event Bus Without Subscribers** | Medium | Low | `eventBus.post(event)` with no `@Subscribe` for that event type |
| **29** | **Coroutines Launched Then Cancelled** | Low | Medium | Jobs cancelled before completing meaningful work |
| **30** | **Workers That Produce Unused Output** | Low | Low | WorkManager jobs whose results are never consumed |

### Priority Tier 4: Specialized Patterns ⭐

Domain-specific or less common patterns.

| # | Pattern | Detectability | Frequency | Description |
|---|---------|---------------|-----------|-------------|
| **31** | **Annotations Without Effect** | Medium | Low | `@Keep` when ProGuard isn't configured to use it |
| **32** | **Validation After The Fact** | Medium | Low | `db.insert(x); require(x.isValid)` - too late |
| **33** | **Unused Debug Logging** | High | 253 Timber calls | Logs in production that output to nowhere |
| **34** | **Semi-Dead Classes** | Medium | Low | Classes used as types but never instantiated |
| **35** | **Test-Only Code in Production** | High | Medium | Code only referenced by tests, never production |
| **36** | **Timestamps Never Used** | Medium | Low | `updatedAt` field maintained but never queried |
| **37** | **Serializable Without Serialization** | Medium | Low | `@Serializable` on classes never serialized |
| **38** | **Crashlytics Keys Never Filtered** | Low | Low | Custom keys set but never used in dashboard |
| **39** | **Threads Spawned Without Work** | Low | Rare | Executor pools with empty task queues |
| **40** | **Configuration Values Never Read** | Medium | Medium | Properties defined but never accessed |

### Implementation Phases

Based on the priority analysis, here's the recommended implementation order:

#### Phase 9: Write-Only Detection (Highest ROI)
```
Priority: ⭐⭐⭐⭐⭐
Patterns: #1, #7, #8, #26
Estimated dead code found: 15-25% increase
```

**Detectors to implement:**
- `WriteOnlyVariableDetector` - Variables assigned but never read
- `WriteOnlyPreferenceDetector` - SharedPreferences written but never read
- `WriteOnlyDatabaseDetector` - DAO methods with @Insert but no @Query callers

**Algorithm:**
1. For each variable/property, track all assignments (writes)
2. Track all reads (usages that don't assign)
3. If writes > 0 && reads == 0, report as write-only

#### Phase 10: Sealed Class & Override Analysis
```
Priority: ⭐⭐⭐⭐
Patterns: #2, #3
Estimated dead code found: 10-15% increase
```

**Detectors to implement:**
- `UnusedSealedVariantDetector` - Sealed subclasses never instantiated
- `RedundantOverrideDetector` - Overrides that only call super

**Algorithm for sealed variants:**
1. Find all sealed class/interface definitions
2. Find all subclasses/implementations
3. For each subclass, check if it's ever instantiated (constructor called)
4. Report never-instantiated subclasses

#### Phase 11: Intent & Data Flow
```
Priority: ⭐⭐⭐
Patterns: #4, #6, #9
Estimated dead code found: 5-10% increase
```

**Detectors to implement:**
- `IgnoredReturnValueDetector` - Function results not captured
- `UnusedIntentExtraDetector` - putExtra without getExtra
- `RedundantNullCheckDetector` - Double null checks

#### Phase 12: Observable State Analysis
```
Priority: ⭐⭐
Patterns: #10, #11, #16
Estimated dead code found: 5-8% increase
```

**Detectors to implement:**
- `DeadFeatureFlagDetector` - Flags always true/false
- `UnobservedStateDetector` - StateFlow/LiveData never collected
- `WriteOnlyCacheDetector` - Cache writes without reads

#### Phase 13: Advanced Flow Analysis
```
Priority: ⭐
Patterns: #21-30
Estimated dead code found: 2-5% increase
```

**Detectors to implement:**
- `PartiallyDeadCodeDetector` - Code used only on some paths
- `RecalculationDetector` - Redundant recomputation
- `EventBusOrphanDetector` - Events without subscribers

### Pattern Detection Examples

#### Write-Only Variable (#1)
```kotlin
class Analytics {
    private var lastEventTime: Long = 0  // DEAD: never read

    fun track(event: Event) {
        lastEventTime = System.currentTimeMillis()  // write-only
        send(event)
    }
}
```

#### Unused Sealed Variant (#2)
```kotlin
sealed class UiState {
    object Loading : UiState()          // Used
    data class Success(val data: Data) : UiState()  // Used
    data class Error(val msg: String) : UiState()   // Used
    object Empty : UiState()            // DEAD: never emitted
}
```

#### Override Only Calling Super (#3)
```kotlin
override fun onCreateView(...): View {
    return super.onCreateView(inflater, container, savedInstanceState)
    // DEAD: If this is all it does, the override is unnecessary
}
```

#### Write-Only Database (#8)
```kotlin
@Dao
interface ReadHistoryDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun saveReadArticle(history: ReadHistory)  // Called

    @Query("SELECT * FROM read_history ORDER BY timestamp DESC")
    fun getReadHistory(): Flow<List<ReadHistory>>  // DEAD: never called!
}
```

#### Ignored Return Value (#4)
```kotlin
// DEAD: The sorted list is never used
articles.sortedByDescending { it.date }
adapter.submitList(articles)  // Still the original unsorted list!
```

#### Dead Feature Flag (#10)
```kotlin
// The flag has been true for 2 years
if (RemoteConfig.isNewPlayerEnabled()) {  // Always true
    playWithExoPlayer()
} else {
    playWithMediaPlayer()  // DEAD: never executed
}
```

### Codebase Analysis Results

From our analysis of a real-world Android project (1806 files):

| Pattern Category | Occurrences | Potential Dead Code |
|------------------|-------------|---------------------|
| Timber/Log calls | 253 | ~50% may be production-silent |
| Override methods | 284 | ~10-20% may only call super |
| Intent extras (putExtra) | 90 | ~30% may be unread |
| Sealed classes | 73 | ~5-10% may have unused variants |
| Feature flags | 388 | ~20% may be dead branches |
| Flow collectors | 64 | ~10% may be unobserved |
| Map operations | 72 | ~5% may have ignored results |
| Private vars | 58 | ~20% may be write-only |
| DAO @Insert methods | 16 | ~10% may be write-only tables |
| DAO @Query methods | 49 | (Need cross-reference analysis) |

**Estimated additional dead code**: Using these advanced detectors could identify **30-50% more dead code** beyond current detection.

### Manual Investigation Results - Verified Examples

Through thorough manual investigation of a real-world Android codebase, we verified the following concrete examples:

#### Confirmed Write-Only Variables (Pattern #1)

**Example 1: `feedStartUpdatingTimestamp` in NewsToolbarController.kt:65**
```kotlin
private var feedStartUpdatingTimestamp = 0L  // Line 65

// Only written, never read:
feedStartUpdatingTimestamp = timeService.now().toInstant().toEpochMilli()  // Line 102
```
**File**: `feature-news/src/main/java/com/example/feed/news/toolbar/NewsToolbarController.kt`

**Example 2: Same pattern in ShowcaseToolbarController.kt:50**
```kotlin
private var feedStartUpdatingTimestamp = 0L  // Line 50

// Only written, never read:
feedStartUpdatingTimestamp = timeService.now().toInstant().toEpochMilli()  // Line 124
```
**File**: `feature-showcase/src/main/java/com/example/feed/showcase/ui/toolbar/ShowcaseToolbarController.kt`

**Impact**: 2 confirmed write-only variables that store timestamps but never use them.

#### Confirmed Empty Override Methods (Pattern #3)

Found 20+ empty override methods that add no value:

| File | Line | Method |
|------|------|--------|
| `ShowcaseToolbarController.kt` | 137 | `override fun onFragmentViewDestroyed() {}` |
| `ListViewsFactory.kt` | 30 | `override fun onCreate() {}` |
| `ListViewsFactory.kt` | 46 | `override fun onDestroy() {}` |
| `StartupAdController.kt` | 248 | `override fun onActivityStarted(activity: Activity) {}` |
| `StartupAdController.kt` | 249 | `override fun onActivityPaused(activity: Activity) {}` |
| `StartupAdController.kt` | 250 | `override fun onActivityStopped(activity: Activity) {}` |
| `StartupAdController.kt` | 251 | `override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) {}` |
| `TimeViewHolder.kt` | 76 | `override fun unbind() {}` |
| `MenuFeedDataSource.kt` | 27 | `override fun onAdapterViewBinded(position: Int) {}` |
| `SingleScrollDirectionEnforcer.kt` | 44 | `override fun onTouchEvent(rv: RecyclerView, e: MotionEvent) {}` |
| `SingleScrollDirectionEnforcer.kt` | 46 | `override fun onRequestDisallowInterceptTouchEvent(disallowIntercept: Boolean) {}` |
| `MultipleCardFragment.kt` | 139, 149, 151 | Empty animation listener methods |

**Impact**: These are interface requirements but represent code that does nothing.

#### Patterns NOT Found (False Positives Avoided)

During investigation, these patterns were verified as **properly used** (NOT dead code):

1. **GlucheStatusDao.get()** - Initially looked write-only but is called via `GlucheRepositoryImpl.getGluchePostStatus()`
2. **BannerDao.exists()** - Called via `BannerRepository.isDismissed()`
3. **beNotificationID/Secret preferences** - Both written and read in `BackEndNotificationService.kt`
4. **intervalCheckInMilliseconds** - Assigned in `init` and read in `scheduleVerifyIfServerHasNewPosts()`
5. **newDeepLinkIntent** - Both getter and setter are used across multiple files

This validates that our detection algorithm must follow the full call chain through repositories and services.

### Detection Algorithm Requirements

Based on the investigation, the Write-Only Variable detector must:

1. **Track all assignments** to private variables
2. **Track all reads** (usages that don't assign)
3. **Exclude reads inside the assignment expression** (`x = x + 1` counts `x` as read)
4. **Handle property delegates** (`by lazy`, `by BooleanPreferenceDelegate`)
5. **Handle backing fields** with custom getters/setters
6. **Report if**: writes > 0 && reads == 0

The Empty Override detector must:

1. **Find all `override fun`** declarations
2. **Check if body is empty** or only contains `super.method()`
3. **Exclude**: Abstract implementations where empty is intentional (e.g., `LifecycleObserver`)
4. **Report with confidence level** based on interface type

## What's Next

Planned features and improvements for future releases:

### Completed Phases

#### Phase 5: Performance & Scale ✅
- [x] **Incremental analysis** - Cache parsed ASTs and only re-analyze changed files (`--incremental`)
- [x] **Watch mode** - Continuous analysis during development (`--watch`)
- [x] **Optimized reachability** - ~8% faster analysis on large codebases
- [ ] **Parallel graph construction** - Parallelize reference resolution phase
- [ ] **Memory optimization** - Reduce memory footprint for very large codebases (100k+ files)

#### Phase 6: Enhanced Detection ✅
- [x] **Unused function parameters** - Detect parameters that are never used in function body (`--unused-params`)
- [x] **Dead string resources** - Cross-reference `R.string.*` usage with `strings.xml` (`--unused-resources`)
- [ ] **Redundant null checks** - Detect null checks on non-nullable types
- [ ] **Unused type parameters** - Detect generic type parameters that aren't used
- [ ] **Unused Gradle dependencies** - Detect declared but unused library dependencies

#### Phase 7: CI Integration ✅ (Partial)
- [x] **Baseline support** - Ignore existing dead code, only flag new issues (`--baseline`)
- [ ] **Language Server Protocol (LSP)** - Real-time dead code highlighting in editors
- [ ] **IntelliJ/Android Studio plugin** - Native IDE integration
- [x] **GitHub Action** - Pre-built action for easy CI setup (`uses: KevinDoremy/SearchDeadCode@v0`)
- [ ] **Pre-commit hook** - Block commits introducing dead code

#### Phase 9: Write-Only Detection ✅ (Mostly Complete)
- [x] **Write-only variables** - Variables assigned but never read (`--write-only`)
- [x] **Write-only SharedPreferences** - prefs.putString() without getString() (`--write-only-prefs`)
- [x] **Write-only database tables** - @Insert without @Query consumers (`--write-only-dao`)
- [ ] **Write-only cache** - Cache writes that are never read

#### Phase 10: Sealed Class & Override Analysis ✅
- [x] **Unused sealed variants** - Sealed class cases never instantiated (`--sealed-variants`)
- [x] **Redundant overrides** - Override methods that only call super (`--redundant-overrides`)

#### Phase 11: Intent & Data Flow ✅ (Partial)
- [ ] **Ignored return values** - `list.map{}` without capturing result
- [x] **Unused intent extras** - putExtra() without getExtra() (`--unused-extras`)
- [ ] **Redundant null checks** - Double null checks after safe calls

### Upcoming Phases

#### Phase 8: Multi-Platform
- [ ] **iOS/Swift support** - Extend to Swift/Objective-C projects
- [ ] **React Native** - Analyze both native and JavaScript layers
- [ ] **Flutter/Dart** - Support Dart language analysis
- [ ] **KMP (Kotlin Multiplatform)** - Proper shared code analysis

#### Phase 12: Observable State ⭐⭐
- [ ] **Dead feature flags** - Flags always true/false
- [ ] **Unobserved StateFlow/LiveData** - State never collected in UI

#### Phase 13: Advanced Flow Analysis ⭐
- [ ] **Partially dead code** - Code computed on all paths but used on some
- [ ] **Recalculation detection** - Redundant recomputation of available values
- [ ] **Event bus orphans** - Events posted without subscribers

### Contributing to Future Development

Want to help? Here are good first issues:

1. **Add new annotation support** - Easy: add annotation names to `entry_points.rs`
2. **Improve XML parsing** - Medium: add support for more XML attributes
3. **Write tests** - Medium: add test cases for edge cases
4. **Performance profiling** - Advanced: identify and fix bottlenecks
5. **LSP implementation** - Advanced: implement language server protocol

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

### Research Sources

- [Meta - Automating Dead Code Cleanup](https://engineering.fb.com/2023/10/24/data-infrastructure/automating-dead-code-cleanup/)
- [Periphery - Swift Dead Code Detection](https://github.com/peripheryapp/periphery)
- [Webpack - Tree Shaking Guide](https://webpack.js.org/guides/tree-shaking/)
- [Tree Shaking Reference Guide - Smashing Magazine](https://www.smashingmagazine.com/2021/05/tree-shaking-reference-guide/)
- [Vulture - Python Dead Code](https://github.com/jendrikseipp/vulture)
- [R8 Code Shrinking - LogRocket](https://blog.logrocket.com/r8-code-shrinking-android-guide/)
- [ReSharper Solution-Wide Analysis](https://www.jetbrains.com/help/resharper/Code_Analysis__Solution-Wide_Analysis__Solution-Wide_Code_Inspections.html)
- [deptry - Python Dependencies](https://github.com/fpgmaas/deptry)
- [Knip - TypeScript Unused Dependencies](https://knip.dev/typescript/unused-dependencies)
- [Dead Code Detection Techniques - Aivosto](https://www.aivosto.com/articles/deadcode.html)
- [Call Graphs - Wikipedia](https://en.wikipedia.org/wiki/Call_graph)
- [Dead Code Elimination - Wikipedia](https://en.wikipedia.org/wiki/Dead-code_elimination)
- [Dead Code Removal at Meta - ACM](https://dl.acm.org/doi/10.1145/3611643.3613871)

## License

MIT
