# CI integration

Two lines, on any platform:

```sh
curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sh
searchdeadcode . --profile ci
```

`--profile ci` is the whole pipeline setup in one flag: exit 1 on findings, no
cache file left in the workspace, and `.deadcode-baseline.json` picked up if
your project committed one. It is the equivalent of `./gradlew detekt`.

Everything below is the same two lines wrapped in each platform's syntax, plus
optional reporting for teams who want more than a red build.

## Before the first green build

A codebase that never ran this tool has existing dead code, and failing on all
of it teaches the team to ignore the job. Freeze it once, like detekt's
baseline:

```sh
searchdeadcode . --generate-baseline .deadcode-baseline.json   # commit this
```

From then on the pipeline only breaks on what a branch **adds**. `--profile ci`
finds the file by name; you never pass it again.

That one command runs outside the CI profile, so it leaves a
`.searchdeadcode-cache.json` next to your project — useful locally, 221 MB on a
9000-file repository. Add it to `.gitignore` now rather than discovering it in
a `git status`.

Keeping the baseline honest:

| | |
|---|---|
| `--baseline-prune` | drop entries whose finding no longer exists, so the file shrinks as you clean |
| `--baseline-stats` | entries per rule — where the tool cries wolf the most |
| `--baseline-rm <name>` | remove one entry, to start failing on it again |

If your team refuses one more file in the repository, `--diff-base origin/main`
reports only what became dead since that reference instead. It needs the full
history: shallow clones are the number one cause of surprises with it
(`fetch-depth: 0` on GitHub Actions, `GIT_DEPTH: 0` on GitLab).

## Exit codes

| code | meaning |
|------|---------|
| 0 | analysis ran, nothing left after filtering |
| 1 | findings remain, and the gate was asked for |
| 2 | the tool could not work: unreadable path, corrupt baseline, invalid config |
| 3 | `--ratchet` refused a count increase against the baseline |

The 1 / 2 split is what stops a pipeline from reporting "no dead code" when the
binary never started. Script against the code, never against the output text.

One deliberate exception: a run carrying `--generate-baseline` never exits 1.
Freezing the debt is an act of acceptance, and failing the step that performs
it would break the adoption command this guide opens with.

## Two things worth knowing before you wire it up

**Run it at the repository root, never per module.** Reachability needs to see
the whole project: analysed module by module, every symbol used from elsewhere
looks dead. This is the structural difference with detekt, which lints file by
file and can be split any way you like.

**The job needs no JDK, no Gradle, no build.** `**/build/**` and
`**/generated/**` are excluded by default, so a fresh checkout gives the same
answer as your machine after a full build. A small container is enough — do not
put this on your expensive Android executor, and do not make it wait for a
build.

One constraint on that container: the Linux binary links against **glibc**, so
a musl image (`alpine`) will not start it — the loader is simply absent, and
the error says "not found" about a file that visibly exists. The examples
below use `buildpack-deps:curl`, a slim Debian with curl preinstalled; any
glibc image works. On Alpine specifically, `apk add gcompat` usually suffices,
but that is Alpine's compatibility shim, not something this project tests.

**Do not cache anything.** The incremental cache halves the run (330 s → 163 s
on a 9000-file project) but weighs 221 MB. Shipping that to and from a CI cache
costs more than it saves, and it lands in your workspace. `--profile ci`
already turns it off; if you run without the profile, add `--incremental=false`
and put `.searchdeadcode-cache.json` in your `.gitignore`.

---

# Per platform

Ordered by real-world adoption.

## GitHub Actions

The published action installs, caches and runs in one step:

```yaml
# .github/workflows/dead-code.yml
name: Dead code
on: [push, pull_request]

jobs:
  dead-code:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: KevinDoremy/SearchDeadCode@v0
        with:
          args: '--profile ci'
```

| Input | Description | Default |
|---|---|---|
| `path` | Path to analyze | `.` |
| `version` | Version to install, or `latest` | `latest` |
| `format` | `terminal`, `compact`, `json`, `sarif`, `html`, `markdown`, `reviewdog`, `csv`, `gitlab`, `checkstyle` | `terminal` |
| `output` | Output file | - |
| `args` | Extra CLI arguments | - |
| `fail-on-findings` | Fail the workflow on findings | `false` |
| `min-confidence` | `low`, `medium`, `high`, `confirmed` | `medium` |

`@v0` follows every 0.x release. Pin `@v0.17.0` if you would rather upgrade by
hand.

### Findings in the Security tab

```yaml
jobs:
  dead-code:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      security-events: write
    steps:
      - uses: actions/checkout@v7
      - uses: KevinDoremy/SearchDeadCode@v0
        with:
          format: 'sarif'
          output: 'deadcode.sarif'
          args: '--profile ci'
```

The action uploads the SARIF itself; results land in **Security → Code scanning**
and annotate the pull request diff. The `permissions` block is not optional:
the upload needs `security-events: write`, and on repositories where the
default token is read-only it fails silently without it — listing a permission
turns every unlisted one off, so `contents: read` must be spelled out too or
checkout stops working.

## Jenkins

```groovy
stage('Dead code') {
  steps {
    sh '''
      curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh \
        | SDC_INSTALL_DIR="$WORKSPACE/bin" sh
      "$WORKSPACE/bin/searchdeadcode" . --profile ci \
        --format checkstyle --output deadcode.xml
    '''
  }
  post {
    always {
      recordIssues tools: [checkStyle(pattern: 'deadcode.xml', name: 'Dead code')]
    }
  }
}
```

`recordIssues` comes from the Warnings Next Generation plugin, which reads
Checkstyle natively — the same format detekt publishes. You get the findings
list, the new/fixed/outstanding split and trend charts.

## GitLab CI

One include, the job comes ready:

```yaml
include:
  - remote: https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/v0.18.0/ci-templates/searchdeadcode.gitlab-ci.yml
```

Or spelled out, if you would rather own the job:

```yaml
dead-code:
  stage: test
  image: buildpack-deps:curl
  before_script:
    - curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sh
  script:
    - searchdeadcode . --profile ci --format gitlab --output gl-code-quality.json
  artifacts:
    reports:
      codequality: gl-code-quality.json
    when: always
```

The Code Quality report renders inline in the merge request widget, so a
reviewer sees the findings next to the diff.

## CircleCI

```yaml
jobs:
  dead_code:
    docker:
      - image: cimg/base:current
    steps:
      - checkout
      - run:
          name: Install SearchDeadCode
          command: curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sudo sh
      - run:
          name: Dead code
          command: searchdeadcode . --profile ci
```

A plain `cimg/base` image, not your Android executor: this job needs no
toolchain and no cache, so it is cheap and runs in parallel with the rest.

## Azure Pipelines

```yaml
- job: DeadCode
  pool:
    vmImage: ubuntu-latest
  steps:
    - checkout: self
    - script: |
        curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sudo sh
        searchdeadcode . --profile ci
      displayName: Dead code
```

## Bitbucket Pipelines

```yaml
pipelines:
  default:
    - step:
        name: Dead code
        image: buildpack-deps:curl
        script:
          - curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sh
          - searchdeadcode . --profile ci
```

## TeamCity

A command line build step:

```sh
curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh \
  | SDC_INSTALL_DIR="%teamcity.build.checkoutDir%/bin" sh
"%teamcity.build.checkoutDir%/bin/searchdeadcode" . --profile ci \
  --format checkstyle --output deadcode.xml
```

Then add an XML Report Processing build feature with the Checkstyle type on
`deadcode.xml`.

## Buildkite

```yaml
steps:
  - label: "Dead code"
    command:
      - curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sudo sh
      - searchdeadcode . --profile ci
```

## Woodpecker / Drone

```yaml
steps:
  dead-code:
    image: buildpack-deps:curl
    commands:
      - curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sh
      - searchdeadcode . --profile ci
```

---

# Inline comments on the pull request

A red build tells the author something is wrong; an inline comment tells them
where. [reviewdog](https://github.com/reviewdog/reviewdog) does that on GitHub,
GitLab, Bitbucket, CircleCI and Jenkins, and SearchDeadCode speaks its format:

```sh
searchdeadcode . --profile ci --fail-on-findings=false --format reviewdog \
  | reviewdog -f=rdjsonl -name=deadcode -reporter=github-pr-review
```

`--fail-on-findings=false` hands the gating to reviewdog, which decides from
its own `-fail-level`. Swap the reporter for `gitlab-mr-discussion` or
`bitbucket-code-report` as needed.

# Pre-commit hook

To catch it before the push rather than after:

```sh
searchdeadcode --install-hook
```

or, with [pre-commit](https://pre-commit.com), this repository is a native
hook source:

```yaml
repos:
  - repo: https://github.com/KevinDoremy/SearchDeadCode
    rev: v0.18.0
    hooks:
      - id: searchdeadcode        # diff mode, sub-second
      # - id: searchdeadcode-full # the whole-project gate, for pre-push
```

Diff mode, not the full `--profile ci` run: a commit hook fires on every
commit, and analysing only the changed files is what keeps it under a second.
It is the same command `--install-hook` writes. The binary must be on PATH
(install.sh, brew, or cargo) — the hook deliberately does not compile the
tool on contributors' machines.

# Installing another way

Homebrew, for developer machines:

```sh
brew tap KevinDoremy/tap && brew install searchdeadcode
```

`cargo install searchdeadcode` also works, but it compiles the tool from source
every time — minutes per build. Keep it for architectures with no published
binary.
