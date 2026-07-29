# SearchDeadCode for VS Code

Workspace-wide dead code analysis for Kotlin and Java, in the Problems panel.

This extension is a thin bridge: the analysis is done by the
[`searchdeadcode`](https://github.com/KevinDoremy/searchdeadcode) CLI, which walks the
whole workspace and resolves references across files. Findings come back as SARIF and
land on the exact lines they belong to, with quick fixes.

## Install the CLI

The extension does not ship a binary. Install it once:

```sh
brew install KevinDoremy/tap/searchdeadcode
# or
cargo install searchdeadcode
```

The extension looks for `searchdeadcode` on `PATH`, then in `/opt/homebrew/bin`,
`/usr/local/bin`, `~/.cargo/bin` and `~/.local/bin` (VS Code on macOS often starts
without Homebrew on its `PATH`). Set `searchdeadcode.path` to point somewhere else.

Version 0.10.0 or newer is required.

## Use it

Run **SearchDeadCode: Scan for dead code (workspace)** from the command palette. The
scan is never automatic: it runs when you ask for it, and the CLI caches its parse
results so later runs are quick. **SearchDeadCode: Clear dead code findings** empties
the Problems panel.

Findings for a file disappear as soon as you edit it, because their line numbers stop
being trustworthy at that point. Rescan when you are done editing.

## Quick fixes

- **Delete unused declaration** appears when the CLI reports a safe deletion range.
- **Add to searchdeadcode baseline** appends the finding to
  `.searchdeadcode-baseline.json`, the file the CLI reads to stay quiet about known
  cases.
- **Ignore here with a reason** inserts a `// deadcode:ignore(reason)` comment above
  the declaration. The reason is mandatory: the CLI refuses a bare directive, on
  purpose, so that every silenced finding says why.

## Settings

| Setting | Default | What it does |
| --- | --- | --- |
| `searchdeadcode.enabled` | `true` | Turns the scan command off entirely |
| `searchdeadcode.path` | `""` | Absolute path to the binary |
| `searchdeadcode.minConfidence` | `medium` | `low`, `medium`, `high` or `confirmed` |
| `searchdeadcode.rules` | DC001-DC005, DC008, DC010-DC013, DC019 | Rule IDs to report |
| `searchdeadcode.exclude` | `[]` | Globs passed to the CLI as `--exclude` |
| `searchdeadcode.extraArgs` | `[]` | Extra CLI arguments, for advanced use |

Style and anti-pattern rules are off by default because they are lint, not dead code.
Add their IDs to `searchdeadcode.rules` if you want them.

## Companion extension

For instant, editor-local feedback while you type (unused imports, parameters and
private declarations, with no binary and no analysis pass), see
[Kotlin Jump](https://marketplace.visualstudio.com/items?itemName=elumine.kotlin-jump).
The two are complementary: Kotlin Jump answers within a file in milliseconds, this one
answers across the whole workspace.
