# Installing SearchDeadCode

Every install channel ships the same binary at the same version. Pick the
one that fits the machine.

## One line (CI, containers, anywhere)

```bash
curl -fsSL https://raw.githubusercontent.com/KevinDoremy/SearchDeadCode/main/install.sh | sh
```

Downloads the binary for your platform, checks its published SHA-256,
installs it in `/usr/local/bin`. `SDC_INSTALL_DIR` and `SDC_VERSION`
override where and which.

Note for Alpine and other musl-based images: the Linux binary links glibc.
Use a glibc image (`buildpack-deps:curl` works) or `cargo install`.

## Homebrew (macOS / Linux)

```bash
brew tap KevinDoremy/tap
brew install searchdeadcode
```

## Windows (Scoop / winget)

```powershell
scoop bucket add kevindoremy https://github.com/KevinDoremy/scoop-bucket
scoop install searchdeadcode
# or
winget install KevinDoremy.SearchDeadCode
```

## Android Studio / IntelliJ IDEA

Search for **SearchDeadCode** in `Settings > Plugins > Marketplace`. The
plugin finds the CLI on PATH or downloads it, pinned to the plugin version
and SHA-256 checked. Details in [android-studio.md](android-studio.md).

## VS Code

Install **SearchDeadCode** from the
[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=elumine.searchdeadcode)
or [Open VSX](https://open-vsx.org/extension/elumine/searchdeadcode).
Platform builds bundle the binary.

## Cargo

```bash
cargo install searchdeadcode
```

Compiles from source, MSRV 1.80. Fine on a workstation, several minutes on
every CI build. Prefer the one-liner there.

## Pre-built binaries

Download from
[GitHub Releases](https://github.com/KevinDoremy/SearchDeadCode/releases):
Linux x86_64/aarch64, macOS Intel/Apple Silicon, Windows x86_64. Each asset
has a `.sha256` file, a cosign signature and SLSA provenance.

macOS may quarantine a browser-downloaded binary. Run
`xattr -d com.apple.quarantine ~/Downloads/searchdeadcode-macos-*` then
`chmod +x` it. More in [troubleshooting.md](troubleshooting.md).

## From source

```bash
git clone https://github.com/KevinDoremy/SearchDeadCode
cd SearchDeadCode
cargo install --path .
```
