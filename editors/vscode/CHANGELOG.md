# Changelog

All notable changes to the SearchDeadCode extension. The extension versions
independently from the [`searchdeadcode` crate](https://github.com/KevinDoremy/SearchDeadCode):
its tags are `vscode-v*`, the crate's are `v*`.

## 0.20.0

Bundles the searchdeadcode 0.20.0 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.19.1

Bundles the searchdeadcode 0.19.1 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.19.0

Bundles the searchdeadcode 0.19.0 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.1.8

Bundles the searchdeadcode 0.18.0 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.1.7

Bundles the searchdeadcode 0.17.0 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.1.6

Bundles the searchdeadcode 0.16.1 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.1.5

Bundles the searchdeadcode 0.16.0 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.1.4

Bundles the searchdeadcode 0.15.1 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.1.3

Bundles the searchdeadcode 0.15.0 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.1.2

Bundles the searchdeadcode 0.14.1 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.1.1

Bundles the searchdeadcode 0.14.0 analyzer. See the crate
[CHANGELOG](https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md)
for what changed in the analysis.

## 0.1.0

First release.

### Added

- **Scan for dead code (workspace)**: runs the analyzer over every Kotlin and Java
  file and reports what nothing reaches, in the Problems panel.
- **Clear dead code findings**: empties the panel without rescanning.
- **Bundled analyzer**: platform builds ship the CLI inside the extension, so a
  fresh install needs nothing else. `searchdeadcode.path` still wins when set.
- **Quick fixes** on each finding: add it to `.searchdeadcode-baseline.json`, or
  insert an inline ignore, which the CLI requires a reason for.
- **Settings**: `enabled`, `path`, `minConfidence`, `rules`, `exclude`, `extraArgs`.

### Notes

- Requires `searchdeadcode` 0.10.0 or newer when using an external binary.
- Desktop only. The analyzer runs as a local process, which vscode.dev has no
  way to provide.
