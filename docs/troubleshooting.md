# Troubleshooting

## macOS: Bypass Gatekeeper warning

The pre-built macOS binary is not code-signed. Three ways to run it:

**Remove the quarantine attribute (recommended)**
```bash
xattr -d com.apple.quarantine ~/Downloads/searchdeadcode-macos-*
chmod +x ~/Downloads/searchdeadcode-macos-*
```

**Right-click → Open**
- Right-click the binary in Finder
- Select "Open" from the context menu
- Click "Open" in the dialog

**System Preferences**
- Go to System Preferences → Privacy & Security
- Click "Open Anyway" next to the blocked app message

## "No Kotlin or Java files found"

- Check that your target path is correct.
- Ensure files are not excluded by `.gitignore` or `--exclude` patterns.
- Verify the project has `.kt` or `.java` files.

## False positives

If code is incorrectly reported as dead:

1. Check `entry_points` in your config — add the FQN.
2. Check `retain_patterns` — add the pattern for reflection / framework usage.
3. Check annotations — ensure framework annotations are recognized (full list in [`detectors.md`](detectors.md)).
4. Check XML — verify `AndroidManifest.xml` and layout XMLs are parsed.

```yaml
# Common false positive fixes
retain_patterns:
  - "*Adapter"           # RecyclerView adapters
  - "*ViewHolder"        # ViewHolders
  - "*Callback"          # Callback interfaces
  - "*Binding"           # Generated bindings
  - "Dagger*"            # Dagger components
```

## Extension functions named `<anonymous>`

Fixed in v0.1.0. Upgrade to the latest version.

## Generic types not matching

Generic type references like `Foo<Bar>` now correctly match declarations `Foo`. Fixed in v0.1.0.

## Glob patterns matching wrong paths

Patterns like `**/test/**` only match complete directory names, not substrings. `/test/` matches; `/testproject/` does not.

## `const val` reported as unused

Kotlin compile-time constants are inlined by the compiler. The tool now automatically skips `const val` properties to avoid false positives. If you still see it, check the version (>= 0.3.0).

## `R.string.*` references not detected

Android resource references are compile-time constants and don't create trackable references in the code graph. They are detected via XML parsing instead. Make sure `parse_layouts: true` and `parse_manifest: true` are set.

## Known limitations

1. **Reflection.** Code accessed via `Class.forName()` or runtime instantiation cannot be detected as used. Workaround: `retain_patterns`.
2. **Multi-module projects.** Each module is analyzed independently. Cross-module references work, but all modules must be in the analysis path.
3. **Annotation processors.** Generated code (Dagger, Room) should be excluded as it may reference declarations in ways not visible to static analysis. The tool now recognizes most DI annotations as entry points.
4. **ProGuard `-keep` rules.** The tool does not parse ProGuard `-keep` rules. Use `retain_patterns` for kept classes, or validate against `--proguard-usage`.
5. **R8 + R.* references.** Android resource references are compile-time constants without trackable graph edges. Detected via XML parsing.

## Filing an issue

If you hit something not covered here, please open an issue with:
- SearchDeadCode version (`searchdeadcode --version`)
- OS and architecture
- Minimal reproduction (project structure or single file if possible)
- Expected vs actual behavior
- Output with `--verbose` flag
