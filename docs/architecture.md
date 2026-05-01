# Architecture

## Pipeline overview

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
│  • Edges = usages / references between declarations              │
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
│  DFS / BFS from entry points → mark reachable nodes              │
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

## Technology stack

| Crate | Purpose | Why |
|---|---|---|
| `tree-sitter` | Core parsing | Incremental, error-tolerant parsing |
| `tree-sitter-kotlin` (v0.3.6) | Kotlin grammar | Official tree-sitter grammar |
| `tree-sitter-java` (v0.21) | Java grammar | Official tree-sitter grammar |
| `petgraph` | Graph data structure | Fast graph algorithms (DFS / BFS / Tarjan SCC) |
| `ignore` | File discovery | Same as ripgrep, respects `.gitignore` |
| `rayon` | Parallelism | Parse files in parallel |
| `clap` | CLI parsing | Industry standard, derive macros |
| `serde` | Config parsing | YAML + TOML support |
| `quick-xml` | XML parsing | Fast AndroidManifest / layout parsing |
| `indicatif` | Progress bars | User feedback for large codebases |
| `colored` | Terminal colors | Readable output |
| `miette` | Error reporting | Beautiful diagnostics with code snippets |
| `dialoguer` | Interactive prompts | Safe delete confirmations |

## Project structure

```
searchdeadcode/
├── Cargo.toml
├── src/
│   ├── main.rs                  # CLI entry point
│   ├── lib.rs                   # Library exports
│   │
│   ├── config/
│   │   ├── mod.rs
│   │   └── loader.rs            # YAML / TOML config loading
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
│   │   ├── reachability.rs      # DFS / BFS traversal
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

## Performance targets (achieved)

| Codebase Size | Parse Time | Analysis Time |
|---|---|---|
| 1 000 files | < 1 s | < 0.5 s |
| 10 000 files | < 5 s | < 2 s |
| 100 000 files | < 30 s | < 10 s |

Benchmarks run on M-series Apple Silicon. Full benchmark code: [`benches/parsing_bench.rs`](../benches/parsing_bench.rs).

## Implementation status

### Core analysis

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
- [x] All 9 detection types

### Hybrid analysis

- [x] JaCoCo XML coverage parsing
- [x] Kover XML coverage parsing
- [x] LCOV coverage parsing
- [x] ProGuard / R8 usage.txt parsing
- [x] Confidence scoring (low / medium / high / confirmed)
- [x] Runtime-dead code detection (reachable but never executed)
- [x] Zombie code cycle detection (Tarjan's algorithm)

### Deep analysis mode

- [x] Individual member analysis within classes
- [x] Interface implementation tracking
- [x] Sealed class subtype tracking
- [x] Suspend function detection
- [x] Flow / StateFlow / SharedFlow pattern detection
- [x] Companion object member analysis
- [x] Lazy / delegated property detection
- [x] Generic type argument tracking
- [x] Class delegation pattern detection
- [x] `const val` skip (compile-time inlining)
- [x] Data class generated method skip
- [x] DI annotation support (Dagger, Hilt, Koin, Room, Retrofit)

### Output & refactoring

- [x] Terminal reporter (colored with confidence indicators)
- [x] JSON reporter (v1.1 with confidence data)
- [x] SARIF reporter
- [x] Interactive deletion mode
- [x] Batch deletion mode
- [x] Dry-run mode
- [x] Undo script generation

## References

- [Periphery](https://github.com/peripheryapp/periphery) — Swift dead code detector (architecture inspiration).
- [tree-sitter](https://tree-sitter.github.io/) — incremental parsing library.
- [ripgrep](https://github.com/BurntSushi/ripgrep) — fast file search (the `ignore` crate).
- [ast-grep](https://ast-grep.github.io/) — structural code search.
- [rust-code-analysis](https://github.com/mozilla/rust-code-analysis) — Mozilla's code analysis library.
