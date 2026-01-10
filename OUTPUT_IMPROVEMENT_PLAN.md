# Output Improvement Plan for SearchDeadCode

## Research Summary

Based on analysis of industry-leading tools:
- [Rust Compiler Diagnostics](https://rustc-dev-guide.rust-lang.org/diagnostics.html) - Best-in-class error formatting
- [ESLint Formatters](https://eslint.org/docs/latest/use/formatters/) - Multiple output modes
- [annotate-snippets](https://docs.rs/annotate-snippets) - Code snippet annotation library
- [SARIF Format](https://www.sonarsource.com/resources/library/sarif/) - Deduplication & aggregation
- [eslint-formatter-pretty](https://github.com/sindresorhus/eslint-formatter-pretty) - Enhanced formatting

---

## Current Issues

### 1. Repetitive Output
- Same issue type repeated many times without aggregation
- No grouping by rule/detector
- Hard to see patterns across codebase

### 2. Color Usage
- Too many colors competing for attention
- Low confidence (red) is confusing - red usually means "error"
- Confidence indicators (●◉○◌) not intuitive

### 3. Information Density
- Every issue takes 2+ lines
- No code snippets for context
- No clickable links to documentation

### 4. Missing Features
- No "group by rule" option
- No summary by detector type
- No deduplication of similar issues

---

## Proposed Color Scheme

Following [Rust's RFC 1644](https://rust-lang.github.io/rfcs/1644-default-and-expanded-rustc-errors.html):

| Element | Color | Rationale |
|---------|-------|-----------|
| **Error** | Red | Universal standard |
| **Warning** | Yellow | Universal standard |
| **Info/Note** | Blue | Non-critical information |
| **File paths** | Cyan (bold) | Easy to spot file boundaries |
| **Line numbers** | Dim/Gray | Secondary information |
| **Code snippets** | White | Primary focus |
| **Primary highlight** | Red/Yellow underline (^^^) | "What" the issue is |
| **Secondary highlight** | Blue underline (---) | "Why" it's an issue |
| **Suggestions** | Green | Positive action |

### Confidence Indicators (Revised)

| Confidence | Symbol | Color | Meaning |
|------------|--------|-------|---------|
| Confirmed | `✓` | Green | Safe to act on |
| High | `!` | Yellow | Very likely correct |
| Medium | `?` | Dim | Review recommended |
| Low | `~` | Dim italic | May be false positive |

---

## Proposed Output Modes

### 1. Default Mode (Compact)
```
/path/to/file.kt
  12:5  warning  AP017  ViewModel holds View reference 'fragment'
  45:3  warning  AP001  Public mutable state 'httpClient'

/path/to/other.kt
  8:1   info     DC001  Unused property 'oldValue'

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  3 issues (2 warnings, 1 info)
```

### 2. Verbose Mode (`--verbose`)
```
/path/to/LoginViewModel.kt

  warning[AP017]: ViewModel holds View/Context reference
    --> src/login/LoginViewModel.kt:60:5
     |
  60 |     var mainLoginFragment: Fragment? = null
     |         ^^^^^^^^^^^^^^^^^
     |         This property holds a Fragment reference
     |
   = note: ViewModels outlive Activities/Fragments, causing memory leaks
   = help: Use callbacks, LiveData, or StateFlow instead
   = docs: https://searchdeadcode.dev/rules/AP017
```

### 3. Grouped by Rule (`--group-by rule`)
```
AP017 - ViewModel holds View/Context reference (23 issues)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  core/login/BaseLoginViewModel.kt:20      property 'fragment'
  core/login/LoginViewModel.kt:60          property 'mainLoginFragment'
  core/login/NavigationViewModel.kt:54     property 'fragment'
  core/ui/WebVM.kt:36                      property 'webView'
  ... and 19 more

  → Run with --expand to see all occurrences

AP001 - Global mutable state (45 issues)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  core/adglif/AdGlifHttpClient.kt:8        object 'AdGlifHttpClient'
  core/login/LoginCore.kt:66               object 'LoginCore'
  ... and 43 more
```

### 4. Summary Only (`--summary`)
```
SearchDeadCode Analysis Summary
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Files analyzed:     6,244
Declarations:       65,708
Issues found:       558

By Category:
  Architecture      │████████████░░░░░░░░│  89 (16%)
  Kotlin Patterns   │██████░░░░░░░░░░░░░░│  45 (8%)
  Performance       │████████░░░░░░░░░░░░│  67 (12%)
  Android           │████████████████░░░░│ 234 (42%)
  Compose           │████░░░░░░░░░░░░░░░░│  28 (5%)
  Dead Code         │██████████░░░░░░░░░░│  95 (17%)

Top Issues:
  1. AP004  EventBus @Subscribe usage       203
  2. AP017  View/Context in ViewModel        50
  3. AP001  Global mutable state             45
  4. DC001  Unused code                       20
  5. AP016  Exposed mutable state             12

Run without --summary for full details.
```

### 5. JSON Mode (`--format json`)
Already implemented - no changes needed.

### 6. SARIF Mode (`--format sarif`)
Already implemented - follows standard format.

---

## Deduplication Strategy

### Problem
Same detector reports similar issues hundreds of times:
```
  AP024  Method 'test1' may have complex conditions
  AP024  Method 'test2' may have complex conditions
  AP024  Method 'test3' may have complex conditions
  ... (repeated 500+ times)
```

### Solution: Smart Aggregation

1. **Identical Messages**: Collapse into count
   ```
   AP024  Complex conditions detected (523 methods)
          Run with --expand AP024 to see all
   ```

2. **Same File, Same Rule**: Group under file
   ```
   /path/to/TestFile.kt
     AP024  Complex conditions (12 methods: test1, test2, test3...)
   ```

3. **Fingerprinting** (following [GitHub's SARIF approach](https://docs.github.com/en/code-security/code-scanning/integrating-with-code-scanning/sarif-support-for-code-scanning)):
   - Rule ID + File path + Line range = unique fingerprint
   - Track across runs for "new issues only" mode

---

## CLI Flag Changes

### New Flags
```
--group-by <mode>       Group results: file (default), rule, severity, category
--summary               Show summary statistics only
--verbose               Show code snippets and detailed explanations
--expand [RULE]         Expand collapsed/aggregated issues
--collapse              Collapse repeated issues (default: true)
--no-collapse           Show every issue individually
--top <N>               Show only top N issues per category
--new-only              Only show issues not in baseline
--baseline <file>       Compare against baseline SARIF file
```

### Updated Flags
```
--format <fmt>          terminal (default), json, sarif, compact, markdown
--min-confidence <lvl>  low, medium, high, confirmed
```

---

## Implementation Plan

### Phase 1: Color & Symbol Improvements
- [ ] Update color scheme to match proposal
- [ ] Change confidence symbols to ✓ ! ? ~
- [ ] Add underline highlighting for code snippets
- [ ] Improve visual hierarchy

### Phase 2: Grouping Options
- [ ] Add `--group-by rule` mode
- [ ] Add `--group-by category` mode
- [ ] Add `--group-by severity` mode
- [ ] Implement smart aggregation/collapse

### Phase 3: Verbose Mode
- [ ] Integrate `annotate-snippets` crate for code display
- [ ] Add contextual help messages per rule
- [ ] Add documentation links
- [ ] Show "why" explanations

### Phase 4: Summary Mode
- [ ] Add `--summary` flag
- [ ] Create ASCII bar charts for categories
- [ ] Show top issues ranking
- [ ] Calculate percentages

### Phase 5: Deduplication
- [ ] Implement fingerprinting
- [ ] Add `--baseline` comparison
- [ ] Add `--new-only` mode
- [ ] Track issues across runs

---

## Example: Before vs After

### Before (Current)
```
/Users/kevin/Desktop/work/lapresse/core/login/src/main/java/.../BaseLoginViewModel.kt
  ◉ 16:5 warning [AP016] Property 'loginMutableState' exposes mutable state publicly. Use private backing property with read-only exposure (e.g., LiveData, StateFlow).
    → property 'loginMutableState'
  ◉ 23:5 warning [AP016] Property 'errorMessageMutableState' exposes mutable state publicly. Use private backing property with read-only exposure (e.g., LiveData, StateFlow).
    → property 'errorMessageMutableState'
  ◉ 20:5 warning [AP017] Property 'fragment' in ViewModel holds View/Context reference. This causes memory leaks and violates MVVM.
    → property 'fragment'
```

### After (Proposed - Compact)
```
core/login/.../BaseLoginViewModel.kt
  16:5  ⚠ AP016  Exposed mutable state 'loginMutableState'
  23:5  ⚠ AP016  Exposed mutable state 'errorMessageMutableState'
  20:5  ⚠ AP017  View reference in ViewModel 'fragment'
```

### After (Proposed - Verbose)
```
core/login/.../BaseLoginViewModel.kt

  ⚠ warning[AP016]: Exposed mutable state
    ╭─[BaseLoginViewModel.kt:16:5]
    │
 16 │     val loginMutableState = MutableStateFlow<LoginState>(LoginState.Idle)
    │         ^^^^^^^^^^^^^^^^^
    │         ╰── This MutableStateFlow is publicly accessible
    │
    ├ note: Mutable state should be private with read-only exposure
    ├ help: Change to: private val _loginState = MutableStateFlow(...)
    │                  val loginState: StateFlow<LoginState> = _loginState
    ╰ docs: https://developer.android.com/topic/architecture/ui-layer/stateholders
```

---

## Technical Implementation

### Dependencies to Add
```toml
[dependencies]
annotate-snippets = "0.11"  # Code snippet formatting
unicode-width = "0.2"       # Proper terminal width calculation
terminal_size = "0.4"       # Detect terminal width for wrapping
```

### New Modules
```
src/report/
├── mod.rs
├── terminal.rs      # Update existing
├── compact.rs       # New: minimal output
├── verbose.rs       # New: detailed output with snippets
├── grouped.rs       # New: group-by modes
├── summary.rs       # New: statistics only
├── colors.rs        # New: centralized color scheme
├── aggregator.rs    # New: deduplication logic
├── json.rs          # Existing
└── sarif.rs         # Existing
```

---

## Success Metrics

1. **Readability**: Users can scan and understand issues in <5 seconds
2. **Actionability**: Clear next steps for each issue type
3. **Noise Reduction**: 80% fewer repeated lines for large codebases
4. **Discoverability**: Easy to find worst offenders via summary

---

## References

- [Rust Compiler Development Guide - Diagnostics](https://rustc-dev-guide.rust-lang.org/diagnostics.html)
- [RFC 1644 - Default and Expanded Rustc Errors](https://rust-lang.github.io/rfcs/1644-default-and-expanded-rustc-errors.html)
- [ESLint Formatters Reference](https://eslint.org/docs/latest/use/formatters/)
- [eslint-formatter-pretty](https://github.com/sindresorhus/eslint-formatter-pretty)
- [SARIF Specification](https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html)
- [GitHub SARIF Support](https://docs.github.com/en/code-security/code-scanning/integrating-with-code-scanning/sarif-support-for-code-scanning)
- [annotate-snippets crate](https://docs.rs/annotate-snippets)
