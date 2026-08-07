# SearchDeadCode

<p align="center">
  <sub><a href="../README.md">English</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.ja.md">日本語</a> · 한국어</sub>
</p>

<p align="center">
  <img src="../assets/hero-scan.png" width="720" alt="searchdeadcode가 Android 프로젝트를 스캔" />
</p>

<p align="center">
  <strong>앱 안에는 죽은 코드가 숨어 있다. 찾고, 증명하고, 지운다.</strong><br/>
  정적 스캔. 런타임 검증. 안전한 삭제.<br/>
  JDK 불필요. Gradle 빌드 불필요.
</p>

## 설치

```bash
brew install KevinDoremy/tap/searchdeadcode   # macOS / Linux
cargo install searchdeadcode                  # Rust가 있는 어디서나
```

다른 방법(Windows, CI 원라이너, 사전 빌드 바이너리): [install.md](install.md)

## 빠른 시작

```bash
searchdeadcode .                      # 저장소 전체 스캔
searchdeadcode . --delete --dry-run   # 삭제 미리보기, 파일은 그대로
searchdeadcode . --profile ci         # 플래그 하나로 CI 게이트
```

전체 문서는 영어로 제공됩니다: [README](../README.md) · [DETECTORS.md](../DETECTORS.md) · [ci-integration.md](ci-integration.md) · [configuration.md](configuration.md)

[MIT](../LICENSE)
