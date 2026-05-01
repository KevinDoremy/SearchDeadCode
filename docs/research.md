# Dead code detection: paradigms & research

This document collects the techniques used in the field, with the trade-offs and which one SearchDeadCode borrows from each.

## Two main approaches

According to systematic literature reviews, dead code detection splits into two families:

| Approach | Description | Tools |
|---|---|---|
| **Accessibility analysis** | Build dependency graph, traverse from entry points, mark unreachable as dead | Periphery, SearchDeadCode, R8 / ProGuard |
| **Data flow analysis** | Track how data flows through program, identify unused computations | Compilers (DCE), static analyzers |

## 1. Graph-based reachability

This is the approach SearchDeadCode uses, inspired by [Periphery](https://github.com/peripheryapp/periphery):

```
Entry Points → Build Dependency Graph → DFS / BFS Traversal → Mark Reachable → Report Unreachable
```

How Periphery works:
1. Build project to generate the "index store" with declaration / reference info.
2. Build in-memory graph of relational structure.
3. Mutate graph to mark entry points.
4. Traverse graph from roots to identify unreferenced declarations.

**Key insight**: the index store contains detailed information about declarations and their references, enabling accurate cross-file analysis.

## 2. Static + dynamic hybrid (Meta's SCARF)

[Meta's SCARF system](https://engineering.fb.com/2023/10/24/data-infrastructure/automating-dead-code-cleanup/) combines multiple analysis techniques:

- **Multi-language**: Java, Objective-C, JavaScript, Hack, Python.
- **Symbol-level analysis**: individual variables, not just files / classes.
- **Static analysis via Glean**: indexed, standardized format for static facts.
- **Runtime monitoring**: observes actual code execution in production.
- **Cycle detection**: finds mutually dependent dead code subgraphs.

Impact at Meta: 104+ million lines of code deleted, petabytes of deprecated data removed, 370 000+ automated change requests.

**Key technique**: SCARF tracks two metrics — static usage (code that appears to use data) and runtime usage (actual access patterns in production).

SearchDeadCode borrows the hybrid model via `--coverage` (JaCoCo / Kover / LCOV) and `--proguard-usage` (R8) for the "runtime" half, plus Tarjan's algorithm for cycles.

## 3. Tree shaking (JavaScript bundlers)

[Webpack](https://webpack.js.org/guides/tree-shaking/) and [Rollup](https://rollupjs.org/) popularized tree shaking:

> "Start with what you need, and work outwards" vs "Start with everything, and work backwards"

Algorithm:
1. Build dependency graph from entry points.
2. Identify all exports in modules.
3. Trace which exports are actually imported / used.
4. Eliminate code not reached during traversal.

Requirements:
- ES6 module syntax (`import` / `export`) — static structure required.
- CommonJS (`require`) cannot be tree-shaken because of dynamic resolution.

Webpack's implementation uses `usedExports` optimization marks unused exports; Terser performs final dead code elimination at the module boundary level.

## 4. Compiler-based dead code elimination

[R8 / ProGuard](https://blog.logrocket.com/r8-code-shrinking-android-guide/) for Android:

1. Entry points declared in ProGuard config.
2. Search for all reachable code from entry points.
3. Build list of reachable tokens.
4. Strip anything not in the list.

R8 advantages over ProGuard: faster (single-pass: shrink + optimize + dex), better Kotlin support, more aggressive inlining and class merging, ~10% size reduction vs ProGuard's ~8.5%.

SearchDeadCode reads R8's `usage.txt` output via `--proguard-usage` to mark findings as `● Confirmed`.

## 5. Scope & namespace tracking

Tools like [ReSharper](https://www.jetbrains.com/help/resharper/Code_Analysis__Solution-Wide_Analysis__Solution-Wide_Code_Inspections.html) use solution-wide analysis:

- Detect unused non-private members (requires whole-solution analysis).
- Track namespace imports across files.
- Identify redundant type casts and unused variables.
- Real-time analysis during development.

**Key insight**: some dead code can only be detected at solution / project scope, not file scope.

## 6. Transitive dependency analysis

Tools like [deptry](https://github.com/fpgmaas/deptry) (Python) and [Knip](https://knip.dev/) (TypeScript) detect:

- Unused dependencies (declared but not imported).
- Missing dependencies (imported but not declared).
- Transitive dependencies (used but only available through other packages).

Multi-module support: analyze relationships between workspaces, understand monorepo dependency structure, detect cross-module dead code.

## 7. Compiler optimization techniques

From compiler theory ([Wikipedia — Dead Code Elimination](https://en.wikipedia.org/wiki/Dead-code_elimination)):

**Data flow analysis**:
- Build Control Flow Graph (CFG).
- Perform liveness analysis.
- Identify variables written but never read.
- Remove unreachable basic blocks.

**Escape analysis**:
- Determine dynamic scope of pointers.
- Enable stack allocation for non-escaping objects.
- Remove synchronization for thread-local objects.

**SSA-based DCE**:
- Static Single Assignment form simplifies analysis.
- Each variable assigned exactly once.
- Dead assignments easily identified.

## 8. Incremental analysis (large codebases)

For large codebases, incremental analysis is essential. Techniques:

- **Caching**: store cryptographic hashes of analysis results.
- **Memoization**: reuse unchanged computation results.
- **Dependency tracking**: only re-analyze affected code.
- **Index stores**: pre-computed declaration / reference indexes.

Tools using incremental analysis:
- [Glean](https://glean.software/) (Meta) — incremental indexing.
- [Roslyn](https://github.com/dotnet/roslyn) — incremental generators with aggressive caching.
- Periphery — index store from compiler.

SearchDeadCode supports incremental analysis via the `--incremental` flag, caching parsed ASTs across runs.

## Comparison of approaches

| Paradigm | Accuracy | Speed | Scope | Best for |
|---|---|---|---|---|
| Graph reachability | High | Fast | Project | General dead code |
| Static + dynamic | Highest | Slow | Organization | Production code |
| Tree shaking | High | Fast | Bundle | JavaScript modules |
| Compiler DCE | Highest | Build-time | Binary | Release builds |
| Scope analysis | Medium | Real-time | IDE | Development feedback |
| Coverage-based | Medium | Requires runtime | Executed paths | Test coverage gaps |

## Challenges & limitations

1. **Halting problem**: theoretically impossible to find ALL dead code deterministically.
2. **Reflection**: dynamically invoked code cannot be detected statically.
3. **Polymorphism**: must know all possible types for method resolution.
4. **Configuration**: code referenced in XML, properties files, etc.
5. **Dynamic languages**: less static structure means harder analysis.

## Future improvements for SearchDeadCode

| Feature | Description | Inspiration | Status |
|---|---|---|---|
| Symbol-level analysis | Track individual variables, not just declarations | Meta SCARF | ✅ Done (v0.3.0 deep mode) |
| Cycle detection | Find mutually dependent dead code | Meta SCARF | ✅ Done (v0.2.0) |
| Coverage integration | Augment static analysis with runtime data | Hybrid tools | ✅ Done (v0.2.0) |
| Incremental mode | Cache results, only re-analyze changes | Glean, Roslyn | ✅ Done (v0.4.0) |
| Transitive tracking | Track full reference chains | deptry, Knip | Partial |
| Cross-module analysis | Analyze multi-module projects holistically | Knip | Planned |

## Research sources

- [Meta — Automating Dead Code Cleanup](https://engineering.fb.com/2023/10/24/data-infrastructure/automating-dead-code-cleanup/)
- [Periphery — Swift Dead Code Detection](https://github.com/peripheryapp/periphery)
- [Webpack — Tree Shaking Guide](https://webpack.js.org/guides/tree-shaking/)
- [Tree Shaking Reference Guide — Smashing Magazine](https://www.smashingmagazine.com/2021/05/tree-shaking-reference-guide/)
- [Vulture — Python Dead Code](https://github.com/jendrikseipp/vulture)
- [R8 Code Shrinking — LogRocket](https://blog.logrocket.com/r8-code-shrinking-android-guide/)
- [ReSharper Solution-Wide Analysis](https://www.jetbrains.com/help/resharper/Code_Analysis__Solution-Wide_Analysis__Solution-Wide_Code_Inspections.html)
- [deptry — Python Dependencies](https://github.com/fpgmaas/deptry)
- [Knip — TypeScript Unused Dependencies](https://knip.dev/typescript/unused-dependencies)
- [Dead Code Detection Techniques — Aivosto](https://www.aivosto.com/articles/deadcode.html)
- [Call Graphs — Wikipedia](https://en.wikipedia.org/wiki/Call_graph)
- [Dead Code Elimination — Wikipedia](https://en.wikipedia.org/wiki/Dead-code_elimination)
- [Dead Code Removal at Meta — ACM](https://dl.acm.org/doi/10.1145/3611643.3613871)
