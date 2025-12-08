# SearchDeadCode

[English](../README.md) | **简体中文** | [日本語](README.ja.md) | [한국어](README.ko.md)

<div align="center">

<img src="../assets/logo.svg" alt="SearchDeadCode Logo" width="120"/>

# SearchDeadCode

**在 Android 项目中查找并消除死代码**

[![CI](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml/badge.svg)](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/searchdeadcode.svg)](https://crates.io/crates/searchdeadcode)
[![Downloads](https://img.shields.io/crates/d/searchdeadcode.svg)](https://crates.io/crates/searchdeadcode)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

一个用 Rust 编写的超快 CLI 工具，用于检测和安全删除 Android 项目（Kotlin 和 Java）中的死代码/未使用代码。

灵感来自 Swift 的 [Periphery](https://github.com/peripheryapp/periphery)。

</div>

## ✨ 功能特点

### 检测能力

| 类别 | 检测内容 |
|------|----------|
| **核心** | 未使用的类、接口、方法、函数、属性、字段、导入 |
| **高级** | 未使用的参数、枚举值、类型别名 |
| **智能** | 只写属性（写入但从未读取）、死分支、冗余的 public 修饰符 |
| **Android 感知** | 尊重 Activities、Fragments、XML 布局、Manifest 条目作为入口点 |
| **资源** | 未使用的 Android 资源（strings、colors、dimens、styles、attrs） |

## 🚀 快速开始

### 安装

```bash
# 通过 Homebrew（macOS/Linux）
brew install KevinDoremy/tap/searchdeadcode

# 通过 Cargo
cargo install searchdeadcode
```

### 基本用法

```bash
# 分析你的 Android 项目
searchdeadcode ./my-android-app

# 预览将被删除的内容
searchdeadcode ./my-android-app --delete --dry-run

# 只显示高置信度的发现
searchdeadcode ./my-android-app --min-confidence high
```

## 📖 完整文档

完整文档请参阅 [英文 README](../README.md)。

## 🤝 贡献

欢迎贡献！请参阅 [CONTRIBUTING.md](../CONTRIBUTING.md) 了解开发设置和指南。

## 📄 许可证

MIT
