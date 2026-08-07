#!/bin/sh
# Point the scoop bucket at a release — the tap's twin for Windows.
#
#   GH_TOKEN=<pat> ./scripts/update-scoop-bucket.sh 0.19.1
#
# Regenerates bucket/searchdeadcode.json wholesale (deterministic, no jq
# surgery) and PUTs it through the contents API: the bucket repo stays
# clone-free in the pipeline.

set -eu

version="${1:?usage: update-scoop-bucket.sh <version>}"
BUCKET="KevinDoremy/scoop-bucket"
PATH_IN_REPO="bucket/searchdeadcode.json"

die() { echo "update-scoop-bucket: $*" >&2; exit 1; }
command -v gh > /dev/null || die "gh is required"

sha256="$(curl -fsSL "https://github.com/KevinDoremy/SearchDeadCode/releases/download/v${version}/searchdeadcode-windows-x86_64.exe.sha256" \
  | awk '{print $1}')"
[ -n "$sha256" ] || die "no published sha256 for v${version}"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
cat > "$tmp" <<JSON
{
    "version": "${version}",
    "description": "Detect and safely remove dead code in Android projects (Kotlin & Java)",
    "homepage": "https://github.com/KevinDoremy/SearchDeadCode",
    "license": "MIT",
    "architecture": {
        "64bit": {
            "url": "https://github.com/KevinDoremy/SearchDeadCode/releases/download/v${version}/searchdeadcode-windows-x86_64.exe#/searchdeadcode.exe",
            "hash": "${sha256}"
        }
    },
    "bin": "searchdeadcode.exe",
    "checkver": {
        "url": "https://raw.githubusercontent.com/KevinDoremy/homebrew-tap/main/Formula/searchdeadcode.rb",
        "regex": "version \\"([\\\\d.]+)\\""
    },
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "https://github.com/KevinDoremy/SearchDeadCode/releases/download/v\$version/searchdeadcode-windows-x86_64.exe#/searchdeadcode.exe",
                "hash": {
                    "url": "https://github.com/KevinDoremy/SearchDeadCode/releases/download/v\$version/searchdeadcode-windows-x86_64.exe.sha256"
                }
            }
        }
    }
}
JSON
python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$tmp" || die "generated manifest is not valid JSON"

# The contents API demands the current blob sha to update a file.
blob_sha="$(gh api "repos/${BUCKET}/contents/${PATH_IN_REPO}" --jq '.sha' 2> /dev/null || true)"
content="$(python3 -c "import base64,sys;print(base64.b64encode(open(sys.argv[1],'rb').read()).decode())" "$tmp")"
if [ -n "$blob_sha" ]; then
  gh api -X PUT "repos/${BUCKET}/contents/${PATH_IN_REPO}" \
    -f message="searchdeadcode ${version}" -f content="$content" -f sha="$blob_sha" > /dev/null
else
  gh api -X PUT "repos/${BUCKET}/contents/${PATH_IN_REPO}" \
    -f message="searchdeadcode ${version}" -f content="$content" > /dev/null
fi
echo "scoop bucket now serves ${version}"
