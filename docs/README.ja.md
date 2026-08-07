# SearchDeadCode

<p align="center">
  <sub><a href="../README.md">English</a> · <a href="README.zh-CN.md">简体中文</a> · 日本語 · <a href="README.ko.md">한국어</a></sub>
</p>

<p align="center">
  <img src="../assets/hero-scan.png" width="720" alt="searchdeadcode が Android プロジェクトをスキャン" />
</p>

<p align="center">
  <strong>アプリには死んだコードが眠っている。見つけて、証明して、消す。</strong><br/>
  静的スキャン。ランタイム検証。安全な削除。<br/>
  JDK 不要。Gradle ビルド不要。
</p>

## インストール

```bash
brew install KevinDoremy/tap/searchdeadcode   # macOS / Linux
cargo install searchdeadcode                  # Rust があればどこでも
```

その他の方法（Windows、CI ワンライナー、ビルド済みバイナリ）：[install.md](install.md)

## クイックスタート

```bash
searchdeadcode .                      # リポジトリ全体をスキャン
searchdeadcode . --delete --dry-run   # 削除のプレビュー、ファイルは無変更
searchdeadcode . --profile ci         # CI ゲートはフラグ 1 つ
```

詳細ドキュメントは英語です：[README](../README.md) · [DETECTORS.md](../DETECTORS.md) · [ci-integration.md](ci-integration.md) · [configuration.md](configuration.md)

[MIT](../LICENSE)
