# Configuration

SearchDeadCode looks for configuration in this order:

1. Path passed via `--config <FILE>`
2. `.deadcode.yml` or `.deadcode.yaml` in project root
3. `.deadcode.toml` in project root
4. `deadcode.yml`, `deadcode.yaml`, or `deadcode.toml` in project root

## YAML schema

```yaml
# .deadcode.yml

# Directories to analyze (relative to project root)
targets:
  - "app/src/main/kotlin"
  - "app/src/main/java"
  - "feature/src/main/kotlin"
  - "core/src/main/kotlin"

# Patterns to exclude (glob syntax)
exclude:
  - "**/generated/**"      # Generated code
  - "**/build/**"          # Build outputs
  - "**/.gradle/**"        # Gradle cache
  - "**/.idea/**"          # IDE files
  - "**/test/**"           # Test files
  - "**/*Test.kt"          # Test classes
  - "**/*Spec.kt"          # Spec classes

# Patterns to retain - never report as dead (glob syntax)
# Use for code accessed via reflection, external libraries, etc.
retain_patterns:
  - "*Adapter"             # RecyclerView adapters
  - "*ViewHolder"          # ViewHolders
  - "*Callback"            # Callback interfaces
  - "*Listener"            # Event listeners
  - "*Binding"             # View bindings

# Explicit entry points (fully qualified class names)
entry_points:
  - "com.example.app.MainActivity"
  - "com.example.app.MyApplication"
  - "com.example.api.PublicApi"

# Report configuration
report:
  format: "terminal"       # terminal | json | sarif
  group_by: "file"         # file | type | severity
  show_code: true          # Show code snippets in output

# Detection configuration - enable / disable specific detectors
detection:
  unused_class: true
  unused_method: true
  unused_property: true
  unused_import: true
  unused_param: true
  unused_enum_case: true
  assign_only: true
  dead_branch: true
  redundant_public: true

# Android-specific configuration
android:
  parse_manifest: true           # Parse AndroidManifest.xml
  parse_layouts: true            # Parse layout XMLs
  auto_retain_components: true   # Auto-retain Android lifecycle components
  component_patterns:            # Additional patterns to auto-retain
    - "*Activity"
    - "*Fragment"
    - "*Service"
    - "*BroadcastReceiver"
    - "*ContentProvider"
    - "*ViewModel"
    - "*Application"
    - "*Worker"
```

## TOML schema

```toml
# .deadcode.toml

targets = [
  "app/src/main/kotlin",
  "app/src/main/java",
]

exclude = [
  "**/generated/**",
  "**/build/**",
  "**/test/**",
]

retain_patterns = [
  "*Adapter",
  "*ViewHolder",
]

entry_points = [
  "com.example.app.MainActivity",
]

[report]
format = "terminal"
group_by = "file"
show_code = true

[detection]
unused_class = true
unused_method = true
unused_property = true
unused_import = true
unused_param = true
unused_enum_case = true
assign_only = true
dead_branch = true
redundant_public = true

[android]
parse_manifest = true
parse_layouts = true
auto_retain_components = true
component_patterns = [
  "*Activity",
  "*Fragment",
  "*ViewModel",
]
```

## Tips

- Add framework-specific reflection targets (Braze, Firebase configs) to `exclude` patterns to skip false positives.
- Use `entry_points` for code referenced from build scripts, native code, or external services.
- For multi-module projects, run from the root and specify each module in `targets`.
- Enable `--incremental` (CLI flag) on large codebases to cache parsed ASTs across runs.
