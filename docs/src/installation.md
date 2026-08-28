# Installation

The `derivon` binary is distributed by the MIT-licensed `derivon-cli` package and follows
Semantic Versioning. The first release is `0.1.0`, tagged
`derivon-cli-v0.1.0`.

## Cargo

```bash
cargo install derivon-cli
```

## Homebrew

```bash
brew install derivon-research/tap/derivon
```

Derivon is a source-built Homebrew formula, not a cask. The separate
`derivon-research/homebrew-tap` repository owns the formula. Rust is a build dependency,
not a runtime dependency.

## Installer

```bash
curl -fsSL https://docs.derivon.net/cli/install.sh | sh
```

The noninteractive installer accepts:

```text
DERIVON_VERSION=latest
DERIVON_INSTALL_DIR=$HOME/.local/bin
DERIVON_INSTALL_METHOD=auto
```

An explicit version is an exact SemVer such as `1.2.3`; ranges and prereleases are not
selected by `latest`. Byte-for-byte repeated installation is supported.

On macOS, `auto` delegates to Homebrew when available and otherwise to Cargo. It does not
download an unsigned macOS executable, bypass Gatekeeper, or install either package
manager. If neither tool is available, installation stops with instructions.

On Linux, `auto` downloads a static `x86_64-unknown-linux-musl` or
`aarch64-unknown-linux-musl` archive; `cargo` explicitly selects source installation. The
installer downloads into a temporary file, verifies SHA-256, runs `derivon --version`,
and atomically replaces an existing installation only after both checks succeed.

The installer never uses sudo, modifies shell startup files, or changes PATH. If the
install directory is absent from PATH, it prints instructions. Unsupported systems fail
without guessing another target.

Version 0.1 does not distribute standalone macOS or Windows archives. Linux archives
are named `derivon-VERSION-TARGET.tar.gz` and contain the binary, root license, and
README. Releases include `SHA256SUMS` and build provenance.

`derivon --version` reports the CLI version and default graph schema:

```text
derivon 0.1.0 (default graph schema: derivon.graph/v1)
```
