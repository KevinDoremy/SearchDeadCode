# Changelog

All notable changes to SearchDeadCode will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.15.1] - 2026-08-01

### Fixed

- **Orphan re-parenting survives raw strings and char literals.** Counting
  braces over raw text desynchronised on `"""say " hi"""`, on a raw string
  ending in a backslash, and on `'{'`: the scan ran to end of file, returned
  nothing, and silently abandoned re-parenting for the whole file. The brace
  scan now runs over a copy with comments and literals blanked, the same
  view the ERROR recovery already used.

- **The parallel builder drops cross-file same-name property edges like the
  serial one.** Two files declaring `count` linked to each other through the
  simple-name fallback, which made every write-only property look read. The
  serial builder has always had the guard; the two paths now agree.

- **Every island member carries its reason line.** A holder sharing the
  member's name was filtered out of the explanation, so an island grouped
  BY that homonymy printed with nothing to explain it. Same-name holders are
  now named by file and line: `'shared' kept alive only by helper,
  shared (B.kt:3)`.

### Changed

- **DC006 computes a file's module once per directory.** The module lookup
  ran a linear scan of the project's Gradle roots for every declaration and
  every reference, costing about 40 % on a 150-module tree.

## [0.15.0] - 2026-08-01

### Added

- **`--islands`: dead islands.** Groups of declarations that reference only
  each other and are referenced by nothing else — the mutual pair, the chain
  behind a dead entry point, the interface plus its only implementation
  nobody constructs. `is_referenced` cannot see them (each member HAS an
  incoming edge, from its dead sibling), and reachability-from-blessed-roots
  overshoots the other way. The island model fixes the error direction:
  every mention that cannot be placed is a root and roots mean life — entry
  points, string literals (attributed by byte extent, so a class logging its
  own name does not resurrect itself), every token of every XML, guarded
  declarations' contents through their eligible ancestors. Liveness is a
  least fixpoint from those roots; islands are the connected components of
  the complement, computed by the same machinery as `--clusters`. Each
  member's report names who held it: `'X' kept alive only by Y — themselves
  dead`. An island referenced from tests is labeled `[test-only]` and left
  as a human call. Algorithm notes in `docs/dead-islands-algorithm.md`.

### Fixed

- **DC006 tells nested Gradle modules apart.** The module of a file was its
  first path component under the shared root, so a monorepo laid out as
  `shared/core` + `shared/ui` read as one module — and the report advised
  making a symbol `internal` that a sibling module consumes, which does not
  compile (`internal` is scoped to the Gradle module). A file now belongs to
  its deepest enclosing directory holding a `build.gradle`/`build.gradle.kts`;
  projects with no build script keep the path-component fallback.

- **A Java getter read from Kotlin as a synthetic property is alive.** Kotlin
  sees a Java class's `getX()`/`setX()` as the property `x`, so
  `button.interactionCount` IS a call to `getInteractionCount()`, and a Java
  getter used solely from Kotlin read as dead. The bridge lives in the
  parser, where the syntax says the access goes through a receiver: by
  resolution time a bare `count` local looks the same as `widget.count`, and
  bridging there resurrected every Java getter of that name in the corpus.
  Two of the three false positives in a fifty-finding audit of the reference
  corpus came from this gap.

- **Declarations a tree-sitter ERROR swallowed are rebuilt from the source
  text.** Trailing commas in named-argument calls can make the grammar
  produce an ERROR that eats member declarations whole — sometimes the
  enclosing `object` itself (one real file kept 15 of 26 declarations, all
  orphaned, and the builder's file-level fallback attributed every
  reference after the ERROR to the FIRST declaration of the file,
  manufacturing an island). The orphan fix now synthesizes the lost
  enclosing type from its column-0 header, rebuilds the member functions
  and nested classes inside any type that had to adopt orphans, and adopts
  members whose ERROR-inflated node runs to the type's own last byte.
  Underneath it, the brace scanner learned to skip comments: a KDoc
  apostrophe ("the section's first page") used to open a phantom char
  literal that swallowed every brace to end of file and silently aborted
  re-parenting. The recovery runs only when the parse actually carries an
  ERROR, and reads a copy of the source with comments and string literals
  blanked out: a file of top-level functions has no type by design, and
  text-scanning one turned a commented-out class into a declaration with a
  real fully qualified name, competing with the live class of that name for
  its manifest entry point.

- **Every carrier of a fully qualified name is rooted as an entry point.**
  Two product flavors declare the same class and the manifest names it once;
  rooting the first carrier alone reported the other as dead code.

- **Two overloads sharing a fully qualified name are both reachable through
  an import.** The FQN index kept one declaration per name, last write wins:
  a cross-module call to an overloaded top-level function bound only to the
  collision winner, so the public overload lost every imported call and read
  as dead while the same call resolved to ALL overloads when made without an
  import. The index now keeps every carrier; imported calls link them all,
  marked ambiguous. This undercounted references everywhere, not just in
  islands — expect finding counts to move.

- **A Java homonym no longer masks the Kotlin property behind its JVM
  accessor.** `getScreenContentWidth()` resolved to any unrelated Java
  method carrying that name and the bridge to the Kotlin property never
  ran — the property read as dead while Java called it on every launch.
  The accessor bridge is a union with the name matches now, not a fallback,
  and it only targets Kotlin declarations: a Java bean's own field must not
  catch its setter's calls, that would erase write-only findings.

- **An identifier under `indexing_suffix` emits a reference.**
  `uriParts[ACTION_COMMAND_INDEX]` referenced the receiver and dropped the
  index constant. Eleven more `simple_identifier` parents join the match —
  `when` subjects, loop headers and conditions, `catch` clauses, range
  tests, annotation collection literals, property and supertype delegation —
  each of which silently dropped its mention before.

- **Java `super_types` no longer carry the literal `extends ` prefix.**
  The extractor took the whole `superclass` node's text; 631 of 3196
  supertype arrays in the reference corpus said `extends Foo`. Every
  consumer happened to survive by substring matching — exact-name matching
  against a supertype was impossible.

- **Islands: a declaration under a `@Module` class is DI convention, not a
  corpse.** The processor generates factories from a module companion's
  `@Provides` members into `build/`; the companion's own name never appears
  in source, which read as an island of three `*ProvideModule` companions
  whose deletion breaks the object graph at compile time.

- **Islands: a Java `@Override` member leaves the population like a Kotlin
  `override`.** Java stores the annotation in the modifiers list as
  `"@Override"`; the guard compared against `"override"` exactly, so a
  framework callback and the members only it used read as an island.

- **Islands: a Java method under a class with any supertype is out of the
  population.** `@Override` being optional in Java, `onPreDraw` under
  `implements ViewTreeObserver.OnPreDrawListener` — an interface the corpus
  never declares — is called by the framework with nothing for the graph to
  see. Only supertype-free Java classes keep their methods as island
  candidates; the real dead methods this silences fall back to the
  per-symbol detectors, the accepted cost of never deleting a live callback.

- **A `-keepclassmembers` rule no longer retains classes.** Otto's idiom
  `-keepclassmembers class ** { @Subscribe public *; }` keeps annotated
  MEMBERS, never classes — parsed as a keep-all it turned every declaration
  of a real corpus into a retention root, which silently blinded the deep
  analysis (`--why-alive` answered "retention root" for classes with zero
  incoming references). Member-scoped keep variants now yield no class
  pattern; class-keeping variants (`-keep`, `-keepnames`,
  `-keepclasseswithmembers`) are untouched.

## [0.14.1] - 2026-07-31

### Fixed

- **DC005 no longer flags cases of an enum iterated without its type prefix.**
  A companion `from()` helper writes `values().first { … }`, not
  `Status.values()`; the guard only knew the qualified spellings. Bare
  `values()` / `valueOf(` / `entries` in the enum's own file now protect it,
  and `enumEntries<T>()` joins the qualified needle list. Measured against a
  large Android corpus: nine false positives gone.

- **DC005 stays out of test source sets.** An enum declared under `test/`,
  `androidTest/`, `sharedTest/` or in a `*Test` file is the test's business;
  its cases are no longer reported, on the detector path and on the
  reachability retain alike.

- **A class that implements `Runnable` is no longer a bus event.** The guard
  matched on the `*Runnable` name suffix only; `uiThread.post(AdRefreshTask(…))`
  slipped through. Posts now walk the supertype chain (Kotlin interface lists
  and Java `implements` included) before calling something an event.

- **A Java `record` implementing `Runnable` is no longer a bus event.** The
  supertype regex asked for the literal `class`, so a record never reached the
  thread-dispatch guard: `UIThread.post(new HandleRefreshDone(this))` read as a
  posted event. Two more false positives gone on the same corpus.

- **Inline subscriptions are seen.** `@Subscribe fun` on one line,
  `bus.subscribe<FooEvent> { … }` and `bus.register(TapEvent::class) { … }`
  all count as subscriptions; their events no longer read as
  posted-never-subscribed.

- **Dynamic posts resolve through variables and factories.** `bus.post(pending)`
  looks up `pending`'s declared type, `bus.post(buildEvent())` looks up the
  callee's return type; resolved types satisfy subscriptions (never reported
  as orphans — name-level resolution stays approximate) and only truly
  unresolvable posts feed the caveat count.

- **DC001 leaves the fields of a `Serializable` class alone.** Java
  serialization reads private fields reflectively; the serialization guard now
  walks the parent's supertypes and covers `writeObject`/`readObject`-style
  hooks, on both the unreachable and the unused-member paths.

- **DC010 says "unprovable" instead of guessing.** A preference wrapper with a
  parameterized key (`fun read(key: String) = prefs.getString(key, null)`)
  makes the read side impossible to enumerate: write-only candidates are
  withheld with an explicit caveat instead of reported. Constant keys resolve
  to their literal (`KEY_TOKEN` = `"auth_token"`) so a write through the
  constant meets its read through the literal, and qualified constant
  references (`PrefKeys.KEY_SESSION`) match their unqualified spelling.

## [0.14.0] - 2026-07-30

### Fixed

- **Nested types referenced through their parent no longer read as dead.**
  `is Action.Toggled ->` left `Toggled` with no incoming reference, because the
  parser emitted `Action.Toggled` while resolution matches declared names. The
  qualified form is what the compiler requires when a variant lives inside its
  sealed class, so this was the common spelling, not an edge case. Measured on a
  large Android corpus: five fewer false positives, no finding lost.

- **A cache written by another build is no longer reused.** The key held only
  the crate version, which separates releases but not two builds of one version.
  Rebuilding after a parser change silently served the old parser's results.

- **Config parsing moved off the archived `serde_yaml`** to `serde_yaml_bw`,
  picking up its fix for a memory leak on crafted YAML and for over-reads on
  malformed streams.

- **`--export-graph` lands through a rename.** It used to truncate in place, so
  a reader could see a half-written export and a failed write destroyed the
  previous one.

### Changed

- **The analysis cache is roughly a quarter of its former size.** Every
  reference stored a copy of its file's import list; on one corpus that was 880
  MB of a 943 MB cache. The list is written once per file now. Same findings,
  verified identical down to rule, name, file and line.

- Dependencies refreshed: quick-xml 0.41, ratatui 0.30, crossterm 0.29, clap
  4.6, plus the lockfile. `tree-sitter` stays at 0.22, capped by
  `tree-sitter-kotlin`.

### Added

- **A VS Code extension**, on the Marketplace and Open VSX. Platform builds ship
  the analyzer inside them, so nothing else needs installing.

- **Six detectors documented** that existed but appeared in no reference:
  unused resources (DC017), unused layouts (DC018), unused Intent extras
  (DC019), dead Remote Config keys (DC020), dead DTO fields (DC021), orphan
  translations (DC022). `DETECTORS.md` is now the single reference and covers
  all 22.

- Releases now publish to crates.io and update the Homebrew tap themselves.
  Both had drifted three versions behind, so `cargo install` and `brew install`
  were handing out 0.10.2 with false positives that were already fixed.


## [Unreleased]

## [0.10.2] - 2026-07-26

### Changed
- Test fixtures and doc examples use neutral package names.

## [0.10.1] - 2026-07-26

### Fixed
- Source sets wired via `java.srcDir("src/x/...")`/`srcDirs` count as declared
  and are no longer reported as phantom (found on a real 49-module monorepo).
- The analysis phase shows a spinner instead of up to a minute of silence on
  large repositories.

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

[Unreleased]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.15.1...HEAD
[0.15.1]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.15.0...v0.15.1
[0.15.0]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.14.1...v0.15.0
[0.4.0]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/KevinDoremy/SearchDeadCode/compare/v0.0.1...v0.1.0
[0.0.1]: https://github.com/KevinDoremy/SearchDeadCode/releases/tag/v0.0.1
