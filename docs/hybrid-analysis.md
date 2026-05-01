# Hybrid analysis

SearchDeadCode combines static analysis with runtime data to increase confidence and reduce false positives.

## Coverage integration

Pass coverage from your test runs to confirm dead code with runtime evidence.

```bash
# JaCoCo (Java / Kotlin)
searchdeadcode ./app --coverage build/reports/jacoco/test/jacocoTestReport.xml

# Kover (Kotlin)
searchdeadcode ./app --coverage build/reports/kover/report.xml

# LCOV (generic)
searchdeadcode ./app --coverage coverage/lcov.info

# Multiple files merged
searchdeadcode ./app \
  --coverage build/reports/unit-test.xml \
  --coverage build/reports/integration-test.xml
```

### Confidence levels with coverage

| Level | Indicator | Meaning |
|---|---|---|
| Confirmed | ● green | Runtime coverage confirms code is never executed |
| High | ◉ bright green | Private / internal with no static references |
| Medium | ○ yellow | Default for static-only analysis |
| Low | ◌ red | May be a false positive (reflection, dynamic dispatch) |

```bash
# Only high-confidence and confirmed findings
searchdeadcode ./app --min-confidence high

# Only runtime-confirmed (safest)
searchdeadcode ./app --coverage coverage.xml --runtime-only
```

### Runtime-dead code (zombie code)

Code that passes static analysis but never executes in practice:

```bash
searchdeadcode ./app --coverage coverage.xml --include-runtime-dead
```

This finds "zombie code" — code that exists in your codebase, appears used (passes static analysis), but is never executed during test runs.

## ProGuard / R8 integration

R8 performs whole-program analysis during release builds and identifies code it removes. Use its `usage.txt` for confirmed findings.

### Generate usage.txt

Add to `proguard-rules.pro`:
```
-printusage usage.txt
```

Build the release APK:
```bash
./gradlew assembleRelease
```

The file lands at `app/build/outputs/mapping/release/usage.txt`.

### Use with SearchDeadCode

```bash
# Analyze with R8 data
searchdeadcode ./app --proguard-usage path/to/usage.txt

# Combine everything
searchdeadcode ./app \
  --proguard-usage usage.txt \
  --coverage coverage.xml \
  --detect-cycles
```

### What this provides

| Benefit | Description |
|---|---|
| Confirmed findings | Items in `usage.txt` are marked as `● Confirmed` |
| Cross-validation | Static analysis + R8 agreement = high confidence |
| Library dead code | R8 sees unused library code we cannot analyze |
| False positive detection | `const val` objects may appear unused but are inlined |

### Important notes

- **`const val` inlining.** Kotlin constants are inlined at compile time. The `Events` object may show as "unused" in `usage.txt` because only its values are accessed at runtime. This is **not** dead code.
- **Build variants.** `usage.txt` is specific to release builds. Debug-only code does not appear.
- **Generated code.** Filter out `_Factory`, `_Impl`, `Dagger*`, `Hilt_*` classes.

### Real-world example

```bash
./target/release/searchdeadcode /path/to/your/android-project \
  --exclude "**/build/**" \
  --exclude "**/test/**" \
  --proguard-usage /path/to/your/android-project/app/usage.txt \
  --detect-cycles

# Output:
# 📋 ProGuard usage.txt: 106329 unused items (24593 classes, 55479 methods)
# 🧟 Zombie Code Detected: 1 dead cycle (2 declarations)
# Found 21 dead code issues:
#   ● 8 confirmed (matched with R8/ProGuard)
#   ○ 13 medium confidence
```

## Zombie code (cycle detection)

Mutually dependent dead code: A uses B, B uses A, neither is used elsewhere. Enable with `--detect-cycles`.

```bash
searchdeadcode ./app --detect-cycles
```

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

The cycle algorithm uses Tarjan's strongly connected components on the reference graph.

## Recommended pipeline

For a production-grade dead code audit:

```bash
# 1. Generate baseline once
searchdeadcode ./app --generate-baseline .deadcode-baseline.json

# 2. Run unit tests with coverage
./gradlew testDebug jacocoTestReport

# 3. Build release with R8 usage
./gradlew assembleRelease

# 4. Hybrid analysis with both signals
searchdeadcode ./app \
  --baseline .deadcode-baseline.json \
  --coverage build/reports/jacoco/test/jacocoTestReport.xml \
  --proguard-usage app/build/outputs/mapping/release/usage.txt \
  --detect-cycles \
  --min-confidence high \
  --format sarif \
  --output deadcode.sarif
```

The baseline filters existing dead code, so only new issues are reported. Coverage and `usage.txt` together yield the highest confidence findings.
