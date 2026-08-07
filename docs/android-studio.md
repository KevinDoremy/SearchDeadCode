# Android Studio / IntelliJ plugin

The plugin is a thin bridge over the `searchdeadcode` CLI: the analysis, the
confidence model and the safe-delete logic all live in the binary. What the
plugin adds is the IDE loop — scan, see, act — without leaving the editor.

Works in Android Studio Ladybug (2024.2) and newer, and in IntelliJ IDEA
242+. It only depends on the base platform, so it is not tied to a Kotlin
plugin mode (K1/K2) or to Android tooling versions.

## Install

Search for **SearchDeadCode** in `Settings > Plugins > Marketplace`, or grab
the `searchdeadcode-jetbrains-<version>.zip` attached to any
[release](https://github.com/KevinDoremy/SearchDeadCode/releases) and use
`Settings > Plugins > ⚙ > Install Plugin from Disk…` (offline machines).

The plugin version **is** the analyzer version — one number on every
platform. Plugin 0.20.0 drives analyzer 0.20.0.

## The analyzer binary

Nothing is bundled. On the first scan the plugin looks for the binary in
this order:

1. the path configured in `Settings > Tools > SearchDeadCode` (per machine,
   never written into the project),
2. a binary it previously downloaded,
3. `searchdeadcode` on PATH,
4. the usual install dirs (`/opt/homebrew/bin`, `/usr/local/bin`,
   `~/.cargo/bin`, `~/.local/bin`) — IDEs launched from the Dock lose the
   shell PATH, this is the workaround.

If nothing answers, a notification offers the choices: download the release
**pinned to the plugin version** from GitHub (verified against the published
SHA-256 before it is ever executable, through the IDE's proxy settings), or
copy the Homebrew / cargo command, or point at your own build.

## What a scan gives you

`Tools > SearchDeadCode > Scan for Dead Code` (also available from the tool
window). The scan is on demand, like a linter run — not continuous analysis.

- Dead declarations are greyed out in the editor with the detector id, at
  warning severity at most (dead code is never a compile error).
- The **SearchDeadCode tool window** lists every finding grouped by file;
  double-click navigates.
- Quick fixes on each finding (Alt+Enter):
  - **Delete unused &lt;kind&gt; '&lt;name&gt;'** when the analyzer marked a
    whole-line region safe to remove — one undo step;
  - **Ignore here with a reason** — inserts
    `// deadcode:ignore(<reason>)`, and the reason is mandatory;
  - **Add to searchdeadcode baseline** — appends to
    `.deadcode-baseline.json`, the same file `--profile ci` picks up, so the
    entry takes effect in your pipeline with no extra wiring;
  - **Open searchdeadcode rule documentation**.

Findings always describe what is on disk: open documents are saved before
the scan, and the moment you edit a file its findings are dropped rather
than shown at stale line numbers. Files changed while the scan was running
are skipped on arrival for the same reason. Stale markers that delete the
wrong line are the one bug this design refuses to allow.

## Settings

`Settings > Tools > SearchDeadCode`:

| Setting | Scope | Default |
|---|---|---|
| Binary path | machine (IDE config) | empty — use the cascade above |
| Enable scanning | project (`.idea/searchdeadcode.xml`) | on |
| Minimum confidence | project | `medium` |
| Rules | project | the editor-curated DC set |
| Exclude globs | project | empty |
| Extra CLI arguments | project | empty |

Project-scope settings are committable: a team can version its scan policy.
Extra arguments are appended verbatim — `--ratchet`, `--target app`,
anything the [CLI reference](cli-reference.md) documents.

## CI is a different door

The plugin is the editor loop. For pipelines, use
[ci-integration.md](ci-integration.md) — `searchdeadcode . --profile ci`
does not involve the plugin at all.
