# How SearchDeadCode compares

Every tool in this table is good at something. This page says what, honestly,
so you can pick the right one for the job at hand.

| Feature | SearchDeadCode | Android Lint | R8 / ProGuard | Detekt | IntelliJ |
|---|:---:|:---:|:---:|:---:|:---:|
| Speed | ~1 s per module, no build | Slow | Build-time | Medium | Medium |
| Kotlin-first | ✅ | Partial | ✅ | ✅ | ✅ |
| Java support | ✅ | ✅ | ✅ | ❌ | ✅ |
| Safe delete | ✅ Interactive | ❌ | ❌ | ❌ | IDE only |
| CI / CD ready | ✅ SARIF, JSON, Checkstyle | ✅ XML | ❌ | ✅ SARIF | ❌ |
| Coverage integration | ✅ JaCoCo, Kover, LCOV | ❌ | ❌ | ❌ | ❌ |
| Cycle detection | ✅ Zombie code | ❌ | ❌ | ❌ | ❌ |
| Resource detection | ✅ | ✅ | ❌ | ❌ | ✅ |
| Standalone (no build) | ✅ | ❌ | ❌ | ❌ | ❌ |
| License | MIT | Apache | Proprietary | Apache | Proprietary |

**When to reach for each**: SearchDeadCode for fast CI feedback and project
audits. Android Lint for broader Android-specific checks. R8 for
production-build accuracy. Detekt for style and complexity. IntelliJ for
interactive refactoring inside the IDE.

## When NOT to use SearchDeadCode

- **You need 100 % accuracy.** Static analysis cannot catch reflection or
  runtime-only references. Validate against R8 `usage.txt` instead, or pass
  it via `--proguard-usage`.
- **Heavy reflection.** Code accessed via `Class.forName()` looks unused.
  Workaround: add reflection targets to `retain_patterns`.
- **Pure Java projects.** SearchDeadCode is Kotlin-first. Java works but
  IntelliJ's own inspections may fit better.
- **Dynamic targets (KMP JS).** JavaScript and other dynamic targets are
  out of scope.

You will likely want SearchDeadCode when you need speed, CI integration,
safe deletion with undo, hybrid coverage analysis, or audits on a bare
checkout.

The intellectual lineage (Periphery, Meta's SCARF, R8 tree shaking) is in
[research.md](research.md).
