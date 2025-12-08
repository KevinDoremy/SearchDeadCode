# SearchDeadCode

[English](../README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | **한국어**

<div align="center">

<img src="../assets/logo.svg" alt="SearchDeadCode Logo" width="120"/>

# SearchDeadCode

**Android 프로젝트에서 죽은 코드 찾기 및 제거**

[![CI](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml/badge.svg)](https://github.com/KevinDoremy/SearchDeadCode/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/searchdeadcode.svg)](https://crates.io/crates/searchdeadcode)
[![Downloads](https://img.shields.io/crates/d/searchdeadcode.svg)](https://crates.io/crates/searchdeadcode)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Rust로 작성된 초고속 CLI 도구로, Android 프로젝트(Kotlin & Java)의 죽은 코드/사용하지 않는 코드를 감지하고 안전하게 제거합니다.

Swift의 [Periphery](https://github.com/peripheryapp/periphery)에서 영감을 받았습니다.

</div>

## ✨ 기능

### 감지 기능

| 카테고리 | 감지 내용 |
|----------|-----------|
| **핵심** | 사용하지 않는 클래스, 인터페이스, 메서드, 함수, 프로퍼티, 필드, 임포트 |
| **고급** | 사용하지 않는 매개변수, enum 케이스, 타입 별칭 |
| **스마트** | 쓰기 전용 프로퍼티(쓰기만 하고 읽지 않음), 죽은 분기, 중복 public 수정자 |
| **Android 인식** | Activities, Fragments, XML 레이아웃, Manifest 항목을 진입점으로 인식 |
| **리소스** | 사용하지 않는 Android 리소스(strings, colors, dimens, styles, attrs) |

## 🚀 빠른 시작

### 설치

```bash
# Homebrew를 통해 (macOS/Linux)
brew install KevinDoremy/tap/searchdeadcode

# Cargo를 통해
cargo install searchdeadcode
```

### 기본 사용법

```bash
# Android 프로젝트 분석
searchdeadcode ./my-android-app

# 삭제될 내용 미리보기
searchdeadcode ./my-android-app --delete --dry-run

# 높은 신뢰도 결과만 표시
searchdeadcode ./my-android-app --min-confidence high
```

## 📖 전체 문서

전체 문서는 [영어 README](../README.md)를 참조하세요.

## 🤝 기여

기여를 환영합니다! 개발 설정 및 가이드라인은 [CONTRIBUTING.md](../CONTRIBUTING.md)를 참조하세요.

## 📄 라이선스

MIT
