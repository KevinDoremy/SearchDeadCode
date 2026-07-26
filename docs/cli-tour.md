# CLI tour — every command, what it shows, what you get

Real outputs from a small sample project: a live `Main` + `NewCheckout` +
`SharedCart`, and a dead `legacy/` package (`OldCheckout` uses
`OldReceiptPrinter`, nobody uses `OldCheckout`).

## `searchdeadcode .` — the everyday command

**What it does**: full analysis, findings annotated with their source.

```
Found 2 dead code issues:

legacy/OldCheckout.kt
  ?     3:1   ⚠ [DC001] class 'OldCheckout' is never used
      |
    3 | class OldCheckout {
      |       ^^^^^^^^^^^ declared here
      = help: searchdeadcode --explain OldCheckout

legacy/OldReceiptPrinter.kt
  ?     3:1   ⚠ [DC001] class 'OldReceiptPrinter' is never used
      |
    3 | class OldReceiptPrinter {
      |       ^^^^^^^^^^^^^^^^^ declared here
      = help: searchdeadcode --explain OldReceiptPrinter

Next steps
  searchdeadcode --clusters          group findings into deletable units
  searchdeadcode --explain <name>    see why a symbol is considered dead
  searchdeadcode --delete --dry-run  preview the cleanup, touch nothing
```

**You get**: every dead symbol with its source line, and your next move.
Past 20 findings the report switches to one line per finding — less is more.

## `searchdeadcode . --explain OldCheckout` — trust before deleting

**What it does**: justifies a verdict instead of asking you to believe it.

```
🔎 Explain: app.legacy.OldCheckout (Class) — ./legacy/OldCheckout.kt:3
   Incoming references: 0
   Roots checked:
     - entry point (manifest, layouts, navigation, menus, annotations, inheritance, config): no
     - reachable from an entry point: no
   Verdict: DEAD — no root retains this symbol
```

**You get**: the incoming references (with referencers when alive), each root
source that was checked, and the verdict.

## `searchdeadcode . --clusters` — triage instead of a list

**What it does**: groups connected dead code into units you can delete whole.

```
🧩 1 deletable cluster(s), biggest first

Cluster 1: 4 declaration(s), ~9 lines
   - OldCheckout — ./legacy/OldCheckout.kt:3
   - OldReceiptPrinter — ./legacy/OldReceiptPrinter.kt:3
```

**You get**: a thousand-line report turned into a handful of decisions,
biggest wins first.

## `searchdeadcode . --kill-list OldCheckout` — "what falls with it?"

**What it does**: simulates deleting a symbol and shows everything only it
kept alive. Symbols shared with live code are spared.

```
💀 Kill-list for OldCheckout: 4 declarations, ~9 lines
   - OldCheckout — ./legacy/OldCheckout.kt:3
   - OldReceiptPrinter — ./legacy/OldReceiptPrinter.kt:3
```

**You get**: the true blast radius of a deletion before you make it.
(`SharedCart` is not listed: `Main` still uses it.)

## `searchdeadcode . --compare legacy=modern` — migration diff

**What it does**: during a v1/v2 migration, splits the old world into
deletable-at-flip vs still-blocked, each blocker with its referencer.

```
🔀 Migration compare: legacy → modern

Deletable at the flip (2 declarations, ~9 lines):
   - OldCheckout — ./legacy/OldCheckout.kt:3
   - OldReceiptPrinter — ./legacy/OldReceiptPrinter.kt:3

Still referenced from outside (0 blockers):
```

**You get**: the size of the prize and the exact list of what still holds
the old world in place.

## `searchdeadcode . --flag new_checkout --behavior enabled` — flag cleanup

**What it does**: assumes a feature flag's final state and shows what dies in
the losing branches (symbols used by both branches survive).

```
🚩 Flag cleanup: new_checkout = enabled (1 gate site(s))
Dead once the flag is burned in:
   - OldCheckoutScreen — CheckoutRouter.kt:8
```

**You get**: the Piranha-style burn-down list before touching the flag.

## `searchdeadcode --init` — the 30-second setup

**What it does**: scans the project and writes a commented `.deadcode.yml`:
phantom source sets pre-excluded, DI framework detected and named.

```
✅ Wrote .deadcode.yml
```

## `searchdeadcode . --delete --dry-run` — the safe preview

**What it does**: shows exactly what `--delete` would remove. Touches nothing.

## `searchdeadcode . --format json --output report.json` — for machines

**What it does**: full findings with confidence, risk and location as JSON
(SARIF via `--format sarif` for GitHub Code Scanning). stdout stays clean, so
`searchdeadcode . --format json | jq .summary` also works.

## `searchdeadcode . --summary` — the one-screen health check

**What it does**: counts, severities, categories and top rules with bar
charts — no individual findings.

---

Everything else: `searchdeadcode --help`, or the
[CLI reference](cli-reference.md).
