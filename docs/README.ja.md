# SearchDeadCode

[English](../README.md) | [简体中文](README.zh-CN.md) | **日本語** | [한국어](README.ko.md)

<div align="center">

<img src="../assets/logo.svg" alt="SearchDeadCode Logo" width="120"/>

# SearchDeadCode

**Android プロジェクトのデッドコードを検出・削除**

[![CI](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml/badge.svg)](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/searchdeadcode.svg)](https://crates.io/crates/searchdeadcode)
[![Downloads](https://img.shields.io/crates/d/searchdeadcode.svg)](https://crates.io/crates/searchdeadcode)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Rust で書かれた高速 CLI ツールで、Android プロジェクト（Kotlin & Java）のデッドコード・未使用コードを検出し、安全に削除します。

Swift の [Periphery](https://github.com/peripheryapp/periphery) にインスパイアされています。

</div>

## ✨ 機能

### 検出機能

| カテゴリ | 検出内容 |
|----------|----------|
| **コア** | 未使用のクラス、インターフェース、メソッド、関数、プロパティ、フィールド、インポート |
| **高度** | 未使用のパラメータ、enum ケース、型エイリアス |
| **スマート** | 書き込み専用プロパティ（書き込まれるが読み取られない）、デッドブランチ、冗長な public 修飾子 |
| **Android 対応** | Activities、Fragments、XML レイアウト、Manifest エントリをエントリポイントとして認識 |
| **リソース** | 未使用の Android リソース（strings、colors、dimens、styles、attrs） |

## 🚀 クイックスタート

### インストール

```bash
# Homebrew 経由（macOS/Linux）
brew install KevinDoremy/tap/searchdeadcode

# Cargo 経由
cargo install searchdeadcode
```

### 基本的な使い方

```bash
# Android プロジェクトを解析
searchdeadcode ./my-android-app

# 削除されるものをプレビュー
searchdeadcode ./my-android-app --delete --dry-run

# 高信頼度の結果のみ表示
searchdeadcode ./my-android-app --min-confidence high
```

## 📖 完全なドキュメント

完全なドキュメントは [英語の README](../README.md) をご覧ください。

## 🤝 コントリビューション

コントリビューションを歓迎します！開発のセットアップとガイドラインについては [CONTRIBUTING.md](../CONTRIBUTING.md) をご覧ください。

## 📄 ライセンス

MIT
