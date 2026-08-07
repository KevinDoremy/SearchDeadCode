#!/bin/sh
# Open (or refresh) the winget-pkgs PR for a release: three manifests under
# manifests/k/KevinDoremy/SearchDeadCode/<version>/, pushed to a branch on
# the fork, PR against microsoft/winget-pkgs.
#
#   GH_TOKEN=<pat> ./scripts/update-winget.sh 0.19.1
#
# Pure API on purpose: winget-pkgs weighs gigabytes (no clone), and komac —
# the usual tool — interviews you through a TTY, which a pipeline does not
# have. Everything here is a plain REST call.
#
# New-package PRs get human moderation on Microsoft's side; version updates
# of an accepted package are merged by the bot once validation passes.

set -eu

version="${1:?usage: update-winget.sh <version>}"
UPSTREAM="microsoft/winget-pkgs"
FORK_OWNER="KevinDoremy"
FORK="$FORK_OWNER/winget-pkgs"
ID="KevinDoremy.SearchDeadCode"
DIR="manifests/k/KevinDoremy/SearchDeadCode/${version}"
BRANCH="searchdeadcode-${version}"

die() { echo "update-winget: $*" >&2; exit 1; }

command -v gh > /dev/null || die "gh is required"
command -v python3 > /dev/null || die "python3 is required"

sha256="$(curl -fsSL "https://github.com/KevinDoremy/SearchDeadCode/releases/download/v${version}/searchdeadcode-windows-x86_64.exe.sha256" \
  | awk '{print toupper($1)}')"
[ -n "$sha256" ] || die "no published sha256 for v${version}"

# An existing PR for this version means nothing to do — re-runs are normal.
existing="$(gh pr list --repo "$UPSTREAM" --author "$FORK_OWNER" \
  --search "$ID $version" --state open --json number --jq 'length' 2> /dev/null || echo 0)"
if [ "$existing" != "0" ]; then
  echo "a winget PR for $ID $version is already open, nothing to do"
  exit 0
fi

# Fork (idempotent), then sync its master with upstream so the branch starts
# from a commit the upstream PR can be based on.
gh repo fork "$UPSTREAM" --clone=false > /dev/null 2>&1 || true
gh api -X POST "repos/${FORK}/merge-upstream" -f branch=master > /dev/null 2>&1 || true
base_sha="$(gh api "repos/${FORK}/git/ref/heads/master" --jq '.object.sha')"
[ -n "$base_sha" ] || die "cannot read the fork's master"

gh api -X POST "repos/${FORK}/git/refs" \
  -f ref="refs/heads/${BRANCH}" -f sha="$base_sha" > /dev/null 2>&1 \
  || echo "branch ${BRANCH} already exists on the fork, reusing it"

put_file() { # put_file <path> <local-file>
  content="$(python3 -c "import base64,sys;print(base64.b64encode(open(sys.argv[1],'rb').read()).decode())" "$2")"
  gh api -X PUT "repos/${FORK}/contents/$1" \
    -f message="New version: ${ID} version ${version}" \
    -f content="$content" -f branch="$BRANCH" > /dev/null
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cat > "${tmp}/version.yaml" <<YAML
# Created by SearchDeadCode's release pipeline (scripts/update-winget.sh)
PackageIdentifier: ${ID}
PackageVersion: ${version}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
YAML

cat > "${tmp}/installer.yaml" <<YAML
# Created by SearchDeadCode's release pipeline (scripts/update-winget.sh)
PackageIdentifier: ${ID}
PackageVersion: ${version}
InstallerLocale: en-US
Platform:
- Windows.Desktop
MinimumOSVersion: 10.0.0.0
InstallerType: portable
Commands:
- searchdeadcode
Installers:
- Architecture: x64
  InstallerUrl: https://github.com/KevinDoremy/SearchDeadCode/releases/download/v${version}/searchdeadcode-windows-x86_64.exe
  InstallerSha256: ${sha256}
ManifestType: installer
ManifestVersion: 1.6.0
YAML

cat > "${tmp}/locale.yaml" <<YAML
# Created by SearchDeadCode's release pipeline (scripts/update-winget.sh)
PackageIdentifier: ${ID}
PackageVersion: ${version}
PackageLocale: en-US
Publisher: Kevin Doremy
PublisherUrl: https://github.com/KevinDoremy
PublisherSupportUrl: https://github.com/KevinDoremy/SearchDeadCode/issues
PackageName: SearchDeadCode
PackageUrl: https://github.com/KevinDoremy/SearchDeadCode
License: MIT
LicenseUrl: https://github.com/KevinDoremy/SearchDeadCode/blob/main/LICENSE
ShortDescription: Detect and safely remove dead code in Android projects (Kotlin & Java)
Moniker: searchdeadcode
Tags:
- android
- dead-code
- java
- kotlin
- static-analysis
ReleaseNotesUrl: https://github.com/KevinDoremy/SearchDeadCode/blob/main/CHANGELOG.md
ManifestType: defaultLocale
ManifestVersion: 1.6.0
YAML

put_file "${DIR}/${ID}.yaml" "${tmp}/version.yaml"
put_file "${DIR}/${ID}.installer.yaml" "${tmp}/installer.yaml"
put_file "${DIR}/${ID}.locale.en-US.yaml" "${tmp}/locale.yaml"

pr_url="$(gh pr create --repo "$UPSTREAM" \
  --head "${FORK_OWNER}:${BRANCH}" --base master \
  --title "New version: ${ID} version ${version}" \
  --body "Automated submission from the [SearchDeadCode release pipeline](https://github.com/KevinDoremy/SearchDeadCode/blob/main/scripts/update-winget.sh). Portable x64 binary, SHA-256 from the release's published checksum.")"
echo "winget PR: $pr_url"
