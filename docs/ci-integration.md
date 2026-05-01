# CI integration

## GitHub Actions

The simplest setup uses the published action:

```yaml
# .github/workflows/dead-code.yml
name: Dead Code Detection

on: [push, pull_request]

jobs:
  dead-code:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Detect Dead Code
        uses: KevinDoremy/SearchDeadCode@v0
        with:
          path: '.'
          min-confidence: 'medium'
```

### Action inputs

| Input | Description | Default |
|---|---|---|
| `path` | Path to analyze | `.` |
| `version` | SearchDeadCode version | `latest` |
| `format` | `terminal`, `json`, `sarif` | `terminal` |
| `output` | Output file path | - |
| `args` | Additional CLI arguments | - |
| `fail-on-findings` | Fail if dead code found | `false` |
| `min-confidence` | `low`, `medium`, `high`, `confirmed` | `medium` |

### Fail CI on dead code

```yaml
- uses: KevinDoremy/SearchDeadCode@v0
  with:
    fail-on-findings: 'true'
    min-confidence: 'high'
```

### SARIF output for GitHub Security tab

```yaml
- name: Detect Dead Code
  uses: KevinDoremy/SearchDeadCode@v0
  with:
    format: 'sarif'
    output: 'dead-code.sarif'

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v2
  with:
    sarif_file: dead-code.sarif
```

### Deep analysis with all detectors

```yaml
- uses: KevinDoremy/SearchDeadCode@v0
  with:
    args: '--deep --unused-params --write-only --sealed-variants'
```

### Manual install (fallback)

If you cannot use the action:

```yaml
- name: Install SearchDeadCode
  run: cargo install searchdeadcode

- name: Run analysis
  run: searchdeadcode . --format sarif --output deadcode.sarif
```

## GitLab CI

```yaml
deadcode:
  stage: analyze
  image: rust:latest
  script:
    - cargo install searchdeadcode
    - searchdeadcode . --format json --output deadcode.json
  artifacts:
    paths:
      - deadcode.json
    when: always
```

## Bitbucket Pipelines

```yaml
pipelines:
  default:
    - step:
        name: Dead code analysis
        image: rust:latest
        script:
          - cargo install searchdeadcode
          - searchdeadcode . --format json --output deadcode.json
        artifacts:
          - deadcode.json
```

## Pre-commit hook

Block commits introducing dead code:

```bash
#!/bin/bash
# .git/hooks/pre-commit (or scripts/pre-commit-hook.sh)

set -e

# Only check changed files
CHANGED=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(kt|java)$' || true)

if [ -z "$CHANGED" ]; then
  exit 0
fi

# Run SearchDeadCode on the project, fail if new issues found
searchdeadcode . \
  --baseline .deadcode-baseline.json \
  --min-confidence high \
  --quiet

if [ $? -ne 0 ]; then
  echo "❌ New dead code detected. Run 'searchdeadcode .' for details."
  exit 1
fi
```

Make it executable:
```bash
chmod +x .git/hooks/pre-commit
```

## Baseline workflow (gradual adoption)

For codebases with existing dead code, generate a baseline once and only flag new issues:

```bash
# 1. Generate baseline (one-time, commit it)
searchdeadcode . --generate-baseline .deadcode-baseline.json

# 2. CI runs with baseline
searchdeadcode . --baseline .deadcode-baseline.json --fail-on-findings
```

Commit `.deadcode-baseline.json` to track existing issues. New issues introduced by PRs will fail CI; existing ones are ignored until cleaned up.

## Reviewing SARIF in GitHub

Once SARIF is uploaded via `codeql-action/upload-sarif`, results appear in:
- **Security tab** → Code scanning alerts
- **Pull request** → annotated diff with inline warnings

Each finding links back to the source file and line, with confidence level and detector code (`DC001`–`DC007`).
