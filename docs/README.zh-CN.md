# SearchDeadCode

<p align="center">
  <sub><a href="../README.md">English</a> · 简体中文 · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a></sub>
</p>

<p align="center">
  <img src="../assets/hero-scan.png" width="720" alt="searchdeadcode 扫描 Android 项目" />
</p>

<p align="center">
  <strong>你的应用里藏着死代码。找到它，证明它，删掉它。</strong><br/>
  静态扫描。运行时验证。安全删除。<br/>
  无需 JDK。无需 Gradle 构建。
</p>

## 安装

```bash
brew install KevinDoremy/tap/searchdeadcode   # macOS / Linux
cargo install searchdeadcode                  # 任何有 Rust 的环境
```

其他方式（Windows、CI 一行安装、预编译二进制）：[install.md](install.md)

## 快速开始

```bash
searchdeadcode .                      # 扫描整个仓库
searchdeadcode . --delete --dry-run   # 预览删除，不改动任何文件
searchdeadcode . --profile ci         # CI 门禁，一个参数搞定
```

完整文档为英文：[README](../README.md) · [DETECTORS.md](../DETECTORS.md) · [ci-integration.md](ci-integration.md) · [configuration.md](configuration.md)

[MIT](../LICENSE)
