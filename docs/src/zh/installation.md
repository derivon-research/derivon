# 安装

`derivon` 由 MIT 许可的 `derivon-cli` package 发布并遵循 Semantic Versioning。首个版本是 `0.1.0`，tag 为 `derivon-cli-v0.1.0`。

## Cargo

```bash
cargo install derivon-cli
```

## Homebrew

```bash
brew install derivon-research/tap/derivon
```

Derivon 使用 formula，不使用 cask。独立的 `derivon-research/homebrew-tap` 仓库维护 formula 和 bottles。Rust 仅是 build dependency。

## 安装脚本

```bash
curl -fsSL https://docs.derivon.net/cli/install.sh | sh
```

脚本无交互并支持 `DERIVON_VERSION=latest`、`DERIVON_INSTALL_DIR=$HOME/.local/bin`、`DERIVON_INSTALL_METHOD=auto`。显式版本必须是精确 SemVer；latest 不选择 prerelease。

macOS auto 优先 Homebrew，否则 Cargo；不下载未签名 macOS executable、不绕过 Gatekeeper，也不安装包管理器。两者都不存在时停止并说明。

Linux auto 下载静态 `x86_64-unknown-linux-musl` 或 `aarch64-unknown-linux-musl` archive，也可显式选择 cargo。脚本下载到临时文件，校验 SHA-256，执行 `derivon --version`，全部成功后才原子替换旧安装。

脚本不使用 sudo、不修改 shell rc 或 PATH；PATH 缺失时只打印说明。v0.1 不发布独立 macOS/Windows archive。Linux archive 命名为 `derivon-VERSION-TARGET.tar.gz`，包含 binary、根 LICENSE 和 README；release 提供 `SHA256SUMS` 与 build provenance。

```text
derivon 0.1.0 (default graph schema: derivon.graph/v1)
```
