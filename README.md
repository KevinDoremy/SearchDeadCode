<div align="center">

<img src="assets/logo.svg" alt="SearchDeadCode Logo" width="120"/>

# SearchDeadCode

**Find and eliminate dead code in Android projects**

[English](README.md) · [简体中文](docs/README.zh-CN.md) · [日本語](docs/README.ja.md) · [한국어](docs/README.ko.md)

[![CI](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml/badge.svg?style=flat-square)](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/searchdeadcode.svg?style=flat-square)](https://crates.io/crates/searchdeadcode)
[![Cargo downloads](https://img.shields.io/crates/d/searchdeadcode.svg?style=flat-square&label=cargo%20downloads)](https://crates.io/crates/searchdeadcode)
[![GitHub downloads](https://img.shields.io/github/downloads/KevinDoremy/SearchDeadCode/total?style=flat-square&label=binary%20downloads)](https://github.com/KevinDoremy/SearchDeadCode/releases)
[![Clones](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FKevinDoremy%2FSearchDeadCode%2Fstats%2Fstats%2Fclones-badge.json&style=flat-square)](https://github.com/KevinDoremy/SearchDeadCode/tree/stats/stats)
[![Total clones](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FKevinDoremy%2FSearchDeadCode%2Fstats%2Fstats%2Fcumulative-badge.json&style=flat-square)](https://github.com/KevinDoremy/SearchDeadCode/tree/stats/stats)
[![MSRV](https://img.shields.io/badge/MSRV-1.80-blue.svg?style=flat-square)](https://blog.rust-lang.org/2024/07/25/Rust-1.80.0.html)
[![Homebrew](https://img.shields.io/badge/Homebrew-available-FBB040?logo=homebrew&logoColor=white&style=flat-square)](https://github.com/KevinDoremy/homebrew-tap)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)

A fast Rust CLI to detect and safely remove dead code in Android projects (Kotlin & Java). Inspired by [Periphery](https://github.com/peripheryapp/periphery) for Swift.

```bash
brew install KevinDoremy/tap/searchdeadcode  # macOS / Linux
cargo install searchdeadcode                  # via Cargo
```

<img src="assets/demo.svg" alt="SearchDeadCode Demo" width="600"/>

</div>

## Why SearchDeadCode

- **Fast.** Parse 1 000 files in under 1 second; 10 000 files in under 5 seconds.
- **Android-aware.** Activities, Fragments, Compose, AndroidManifest, layout XMLs, DI annotations all auto-retained as entry points.
- **Hybrid analysis.** Combine static analysis with JaCoCo / Kover / LCOV coverage and R8 `usage.txt` for confirmed findings.
- **Safe delete.** Interactive, batch, and dry-run modes, with restore script generation.
- Pairs well with [kotlin-jump](https://github.com/elumine-dev/kotlin-jump) for editor-side navigation.

## Comparison with alternatives

| Feature | SearchDeadCode | Android Lint | R8 / ProGuard | Detekt | IntelliJ |
|---|:---:|:---:|:---:|:---:|:---:|
| Speed | <1s/1k files | Slow | Build-time | Medium | Medium |
| Kotlin-first | ✅ | Partial | ✅ | ✅ | ✅ |
| Java support | ✅ | ✅ | ✅ | ❌ | ✅ |
| Safe delete | ✅ Interactive | ❌ | ❌ | ❌ | IDE only |
| CI / CD ready | ✅ SARIF, JSON | ✅ XML | ❌ | ✅ SARIF | ❌ |
| Coverage integration | ✅ JaCoCo, Kover, LCOV | ❌ | ❌ | ❌ | ❌ |
| Cycle detection | ✅ Zombie code | ❌ | ❌ | ❌ | ❌ |
| Resource detection | ✅ | ✅ | ❌ | ❌ | ✅ |
| Standalone (no build) | ✅ | ❌ | ❌ | ❌ | ❌ |
| License | MIT | Apache | Proprietary | Apache | Proprietary |

**When to reach for each**: SearchDeadCode for fast CI feedback and project audits. Android Lint for broader Android-specific checks. R8 for production-build accuracy. Detekt for style and complexity. IntelliJ for interactive refactoring inside the IDE.

## Quick start

```bash
# Analyze your Android project
searchdeadcode ./my-android-app

# Preview what would be deleted (no changes)
searchdeadcode ./my-android-app --delete --dry-run

# High-confidence findings only
searchdeadcode ./my-android-app --min-confidence high
```

Every command with its real output: [`docs/cli-tour.md`](docs/cli-tour.md).

### Sample output

```
$ searchdeadcode ./my-app --min-confidence high

SearchDeadCode v0.4.0
Found 247 files to analyze
Reachability: 1 847 reachable, 2 103 total

Found 12 dead code issues:

Confidence Legend:
  ✓ Confirmed (runtime)  ! High  ? Medium  ~ Low

app/src/main/java/com/example/data/OldApiClient.kt
  ! 15:1 warning [DC001] class 'LegacyApiClient' is never used

app/src/main/java/com/example/utils/StringUtils.kt
  ! 42:5 warning [DC001] function 'formatLegacyDate' is never used
  ! 67:5 warning [DC001] function 'parseOldFormat' is never used

Summary: 12 issues in 4 files (3 classes, 5 functions, 4 properties)
Estimated removable lines: ~340
```

## Detection capabilities

| Category | Detected |
|---|---|
| Core | Unused classes, interfaces, methods, functions, properties, fields, imports |
| Advanced | Unused parameters, enum cases, type aliases |
| Smart | Assign-only properties, dead branches, redundant public modifiers |
| Android | Activities, Fragments, XML layouts, AndroidManifest entries (auto-retained) |
| Resources | Unused strings, colors, dimens, styles, attrs |

Full reference and code examples for each detector: [`DETECTORS.md`](DETECTORS.md).

## Installation

### One line (CI, containers, anywhere)

```bash
curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sh
```

Downloads the binary for your platform, checks its published SHA-256, installs
it in `/usr/local/bin`. `SDC_INSTALL_DIR` and `SDC_VERSION` override where and
which.

### Homebrew (macOS / Linux)

```bash
brew tap KevinDoremy/tap
brew install searchdeadcode
```

### Windows (Scoop / winget)

```powershell
scoop bucket add kevindoremy https://github.com/KevinDoremy/scoop-bucket
scoop install searchdeadcode
# or, once the first winget submission clears Microsoft's review:
winget install KevinDoremy.SearchDeadCode
```

### Android Studio / IntelliJ IDEA

Search for **SearchDeadCode** in `Settings > Plugins > Marketplace`. The
plugin greys out dead declarations, lists findings in a tool window, and
offers delete / ignore / baseline quick fixes — details in
[docs/android-studio.md](docs/android-studio.md).

### Cargo

```bash
cargo install searchdeadcode
```

Compiles from source — fine on a workstation, several minutes on every CI
build. Prefer the one-liner there.

### Pre-built binaries

Download from [GitHub Releases](https://github.com/KevinDoremy/SearchDeadCode/releases). Available for Linux x86_64/aarch64, macOS Intel/Apple Silicon, Windows x86_64.

> macOS may quarantine the binary. Run `xattr -d com.apple.quarantine ~/Downloads/searchdeadcode-macos-*` then `chmod +x` it. More options in [`docs/troubleshooting.md`](docs/troubleshooting.md).

### From source

```bash
git clone https://github.com/KevinDoremy/SearchDeadCode
cd SearchDeadCode
cargo install --path .
```

## Usage essentials

The default run is the whole product: it detects what's going on in your
repo and prints the specialized command to dig in, parameters included.
Mid-migration codebases get this for free:

```text
Next steps
  ⚠ Deux arborescences similaires détectées (app/main / app/mainV2) — migration en cours ?
    searchdeadcode . --compare "app/main=app/mainV2"    vieux monde: supprimable au flip + bloqueurs
  ⚠ 12 paire(s) de classes V1/V2 (ex. HomeScreen / HomeScreenV2)
    searchdeadcode . --twins    les paires côte à côte avec leurs références
```

No flag to memorize: run `searchdeadcode .`, copy the suggestion.

```bash
# Basic analysis
searchdeadcode ./app

# JSON output for programmatic use
searchdeadcode ./app --format json --output report.json

# SARIF for GitHub Code Scanning
searchdeadcode ./app --format sarif --output report.sarif

# Hybrid analysis with coverage + R8 usage
searchdeadcode ./app \
  --coverage build/reports/jacoco/test/jacocoTestReport.xml \
  --proguard-usage app/build/outputs/mapping/release/usage.txt \
  --detect-cycles \
  --min-confidence high

# Safe delete with dry-run
searchdeadcode ./app --delete --dry-run
```

Power features: hybrid coverage analysis, R8 / ProGuard integration, zombie code detection, watch mode, baseline support, unused resources, unused params. See [`docs/cli-reference.md`](docs/cli-reference.md) for the full CLI reference and [`docs/hybrid-analysis.md`](docs/hybrid-analysis.md) for coverage + R8 workflows.

## CI integration

Two lines, on any platform:

```sh
curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sh
searchdeadcode . --profile ci
```

`--profile ci` is the whole pipeline setup in one flag: exit 1 on findings, no
cache file left in the workspace, and `.deadcode-baseline.json` picked up if
your project committed one.

On a codebase that has never run this, freeze the existing debt first so the
build only breaks on what a branch adds:

```sh
searchdeadcode . --generate-baseline .deadcode-baseline.json   # commit this
```

On GitHub Actions, the published action does the install for you:

```yaml
# .github/workflows/dead-code.yml
name: Dead code
on: [push, pull_request]

jobs:
  dead-code:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: KevinDoremy/SearchDeadCode@v0
        with:
          args: '--profile ci'
```

Jenkins, GitLab, CircleCI, Azure, Bitbucket, TeamCity, Buildkite and
Woodpecker, plus SARIF, Checkstyle and inline pull-request comments:
[`docs/ci-integration.md`](docs/ci-integration.md).

> The job needs no JDK, no Gradle and no build — `**/build/**` and
> `**/generated/**` are excluded by default, so a fresh checkout gives the same
> answer as your machine. And do not cache anything: the incremental cache
> halves the run but weighs 221 MB on a 9000-file project, which costs more to
> ship than it saves. `--profile ci` turns it off; elsewhere, put
> `.searchdeadcode-cache.json` in your `.gitignore`.

## Configuration

SearchDeadCode looks for `.deadcode.yml`, `.deadcode.toml`, or a path passed via `--config`. Minimal example:

```yaml
# .deadcode.yml
targets:
  - "app/src/main/kotlin"
  - "app/src/main/java"

exclude:
  - "**/generated/**"
  - "**/build/**"
  - "**/test/**"

retain_patterns:
  - "*Adapter"
  - "*ViewHolder"
  - "*Binding"

android:
  parse_manifest: true
  parse_layouts: true
  auto_retain_components: true
```

Full schema (YAML + TOML) and Android-specific options: [`docs/configuration.md`](docs/configuration.md).

## When NOT to use SearchDeadCode

Being honest about limits helps you pick the right tool. Skip SearchDeadCode if:

- **You need 100% accuracy.** Static analysis cannot catch reflection or runtime-only references. Validate against R8 `usage.txt` instead, or pass it via `--proguard-usage`.
- **Heavy reflection.** Code accessed via `Class.forName()` looks unused. Workaround: add reflection targets to `retain_patterns`.
- **Pure Java projects.** SearchDeadCode is Kotlin-first. Java works but [UCDetector](https://ucdetector.org/) or IntelliJ inspections may fit better.
- **You want IDE integration.** This is a CLI. Use IntelliJ / Android Studio's "Unused declaration" inspection, or run SearchDeadCode in `--watch` mode alongside.
- **Dynamic targets (KMP JS).** We analyze JVM bytecode patterns. JavaScript and other dynamic targets are out of scope.

But you'll likely want SearchDeadCode if you need: speed, CI integration, safe deletion with undo, hybrid coverage analysis, or no-build-required audits.

## Documentation

- [`DETECTORS.md`](DETECTORS.md) — every detector, with code examples and the flag that enables it
- [`docs/cli-reference.md`](docs/cli-reference.md) — full CLI reference and command examples
- [`docs/configuration.md`](docs/configuration.md) — YAML and TOML schemas
- [`docs/hybrid-analysis.md`](docs/hybrid-analysis.md) — coverage, R8 / ProGuard, zombie code
- [`docs/ci-integration.md`](docs/ci-integration.md) — eight CI platforms, baselines, exit codes, pre-commit hooks
- [`docs/troubleshooting.md`](docs/troubleshooting.md) — Gatekeeper, FAQ, known limitations
- [`docs/architecture.md`](docs/architecture.md) — pipeline, tech stack, project structure, performance targets
- [`docs/research.md`](docs/research.md) — dead code detection paradigms (Periphery, Meta SCARF, R8, tree shaking)
- [`docs/roadmap.md`](docs/roadmap.md) — 40 advanced patterns prioritized for future detectors
- [`CHANGELOG.md`](CHANGELOG.md) — full version history

## Contributing

Contributions welcome. See [`AGENTS.md`](AGENTS.md) for the full contributor guide and [`CONTRIBUTING.md`](CONTRIBUTING.md) for the dev setup.

Good first issues: add new annotation patterns to `entry_points.rs`, improve XML parsing for additional attributes, write fixtures for edge cases.

## Companion tools

- [kotlin-jump](https://github.com/elumine-dev/kotlin-jump) — VS Code Kotlin/Java navigation, no JVM (9,639 installs).
- [detekt-lsp](https://github.com/elumine-dev/detekt-lsp) — Live Detekt diagnostics for any LSP editor (pre-alpha).
- SearchDeadCode — this project.

Maintained alongside [elumine-dev](https://github.com/elumine-dev) by [Kevin Doremy](https://kevindoremy.com).

## License

[MIT](LICENSE) © Kevin Doremy Laferrière
