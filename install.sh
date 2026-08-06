#!/bin/sh
# SearchDeadCode installer.
#
#   curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sh
#
# Why this exists: every CI that is not GitHub Actions was told to run
# `cargo install searchdeadcode`, which recompiles a 5000-line Rust project on
# every build. This downloads an 11 MB binary instead, so a pipeline step costs
# seconds rather than minutes.
#
#   SDC_VERSION       pin a version ("0.17.0"); default: the latest release
#   SDC_INSTALL_DIR   where to put the binary; default: /usr/local/bin
#
# POSIX sh on purpose: it runs under dash, busybox ash and macOS sh alike, and
# a CI image is not required to ship bash.

set -eu

REPO="KevinDoremy/SearchDeadCode"
INSTALL_DIR="${SDC_INSTALL_DIR:-/usr/local/bin}"

die() { echo "install.sh: $*" >&2; exit 1; }
have() { command -v "$1" > /dev/null 2>&1; }

have curl || die "curl is required"

# The asset names published by .github/workflows/release.yml. action.yml holds
# the same table for GitHub runners: change them in both places or in neither.
# They are NOT Rust target triples — assuming they were is what made the GitHub
# action download a 404 for several releases.
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)  os_part="linux" ;;
  Darwin) os_part="macos" ;;
  MINGW*|MSYS*|CYGWIN*) die "on Windows, download searchdeadcode-windows-x86_64.exe from the releases page" ;;
  *) die "unsupported OS: $os" ;;
esac
case "$arch" in
  x86_64|amd64) arch_part="x86_64" ;;
  arm64|aarch64) arch_part="aarch64" ;;
  *) die "unsupported architecture: $arch" ;;
esac
asset="searchdeadcode-${os_part}-${arch_part}"

version="${SDC_VERSION:-}"
if [ -z "$version" ]; then
  # Neither /releases/latest nor the list order can be trusted here.
  # This repository also tags the VS Code extension (vscode-v0.1.7), and
  # /releases/latest returns whichever release is newest by date — the
  # extension, most of the time. The list is not version-ordered either: the
  # API happily returns v0.4.0 ahead of v0.17.0. So: keep only the tags that
  # look like a crate release, and take the highest.
  #
  # `sort -t. -k1,1n -k2,2n -k3,3n` rather than `sort -V`, which busybox and
  # older BSD sort do not always have.
  #
  # curl is captured FIRST, alone: in POSIX sh the status of `x="$(a | b)"`
  # is the last pipe stage's, so a `|| die` glued to the whole pipeline can
  # never fire — a rate-limited API died two lines later with the misleading
  # "no crate release found" instead of naming the network as the culprit.
  resp="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases?per_page=100")" \
    || die "could not reach GitHub to resolve the latest release (rate limit or network?). Pin one with SDC_VERSION=0.17.0"
  version="$(printf '%s\n' "$resp" \
    | tr ',' '\n' \
    | sed -n 's/.*"tag_name" *: *"v\([0-9][0-9.]*\)".*/\1/p' \
    | sort -t. -k1,1n -k2,2n -k3,3n \
    | tail -1)"
  [ -n "$version" ] || die "no crate release found. Pin one with SDC_VERSION=0.17.0"
fi

base="https://github.com/${REPO}/releases/download/v${version}"
tmp="$(mktemp -d)"
# shellcheck disable=SC2064
trap "rm -rf '$tmp'" EXIT INT TERM

echo "Downloading ${asset} (v${version})"
curl -fsSL "${base}/${asset}" -o "${tmp}/${asset}" \
  || die "${base}/${asset} is not downloadable — check that v${version} publishes this asset"

# Every release publishes a .sha256 next to each binary. Verifying it turns a
# truncated download or a wrong asset name into a clear error instead of a
# binary that fails mysteriously later.
if curl -fsSL "${base}/${asset}.sha256" -o "${tmp}/${asset}.sha256" 2> /dev/null; then
  if have shasum; then
    (cd "$tmp" && shasum -a 256 -c "${asset}.sha256" > /dev/null) \
      || die "checksum mismatch for ${asset} — the download is not what the release published"
    echo "Checksum verified"
  elif have sha256sum; then
    (cd "$tmp" && sha256sum -c "${asset}.sha256" > /dev/null) \
      || die "checksum mismatch for ${asset} — the download is not what the release published"
    echo "Checksum verified"
  else
    # Stated loudly on purpose: a CI log must tell "verified" apart from
    # "nothing was checked", or the checksum only ever protects by accident.
    echo "install.sh: no sha256 tool found, integrity NOT verified" >&2
  fi
else
  echo "install.sh: no .sha256 published for ${asset}, integrity NOT verified" >&2
fi

chmod +x "${tmp}/${asset}"

# A custom SDC_INSTALL_DIR may not exist yet — a CI step pointing at
# $WORKSPACE/bin expects it to be created, not to die on a missing directory.
[ -d "$INSTALL_DIR" ] || mkdir -p "$INSTALL_DIR" 2> /dev/null || true

target="${INSTALL_DIR}/searchdeadcode"
# The sudo arm is only entered when sudo can actually work: either stderr is a
# tty (a human ran curl | sh in a terminal — sudo may prompt there), or sudo
# succeeds without a password (`sudo -n true`). Bare `have sudo` was not
# enough: on a CI agent with a passworded sudo, the script hung at a prompt no
# one would ever answer, and the actionable SDC_INSTALL_DIR hint sat in the
# unreachable else arm.
if [ -d "$INSTALL_DIR" ] && [ -w "$INSTALL_DIR" ]; then
  mv "${tmp}/${asset}" "$target"
elif have sudo && { [ -t 2 ] || sudo -n true 2> /dev/null; }; then
  echo "install.sh: ${INSTALL_DIR} is not writable, using sudo" >&2
  sudo mkdir -p "$INSTALL_DIR" && sudo mv "${tmp}/${asset}" "$target" \
    || die "cannot write to ${INSTALL_DIR}. Set SDC_INSTALL_DIR to a directory you own, e.g. SDC_INSTALL_DIR=\$HOME/.local/bin"
else
  die "cannot write to ${INSTALL_DIR}. Set SDC_INSTALL_DIR to a directory you own, e.g. SDC_INSTALL_DIR=\$HOME/.local/bin"
fi

# Channel counters: a fire-and-forget GET on a tiny marker asset bumps a
# per-release counter GitHub already keeps. Event counts only — nothing
# identifying travels beyond the HTTP request GitHub sees for any download.
# Releases before 0.18.0 have no markers: -f turns the 404 into silence.
mark() { curl -fsSL -m 5 -o /dev/null "${base}/channel-$1" 2> /dev/null || true; }
mark install-sh
if [ -n "${CI:-}" ]; then mark install-ci; fi
# Coding agents set these; knowing the share of agent-driven installs says
# whether the docs should keep being written for them too.
if [ -n "${CLAUDECODE:-}${CURSOR_TRACE_ID:-}" ]; then mark install-agent; fi

echo "Installed: $("$target" --version)"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *) echo "install.sh: ${INSTALL_DIR} is not on PATH — add it before calling searchdeadcode" >&2 ;;
esac
