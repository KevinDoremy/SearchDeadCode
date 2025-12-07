# Contributing to SearchDeadCode

Thank you for your interest in contributing to SearchDeadCode! This document provides guidelines and instructions for contributing.

## Getting Started

### Prerequisites

- Rust 1.70 or later
- Git

### Setup

```bash
# Clone the repository
git clone https://github.com/KevinDoremy/SearchDeadCode.git
cd SearchDeadCode

# Build the project
cargo build

# Run tests
cargo test

# Run with your changes
cargo run -- /path/to/android/project
```

## How to Contribute

### Reporting Bugs

1. Check if the issue already exists in [GitHub Issues](https://github.com/KevinDoremy/SearchDeadCode/issues)
2. If not, create a new issue using the bug report template
3. Include:
   - Steps to reproduce
   - Expected vs actual behavior
   - Rust version (`rustc --version`)
   - OS and architecture

### Suggesting Features

1. Open a [GitHub Issue](https://github.com/KevinDoremy/SearchDeadCode/issues/new)
2. Describe the feature and its use case
3. Explain why it would benefit the project

### Submitting Code

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests: `cargo test`
5. Run lints: `cargo clippy`
6. Format code: `cargo fmt`
7. Commit with a clear message
8. Push and create a Pull Request

## Code Guidelines

### Style

- Follow Rust conventions and idioms
- Run `cargo fmt` before committing
- Run `cargo clippy` and fix any warnings
- Write descriptive commit messages

### Testing

- Add tests for new functionality
- Ensure all existing tests pass
- Test with real Android projects when possible

### Documentation

- Document public APIs with doc comments
- Update README if adding user-facing features
- Include examples where helpful

## Project Structure

```
src/
├── main.rs              # CLI entry point
├── analysis/            # Dead code detection engine
│   └── detectors/       # Individual detection algorithms
├── graph/               # Code dependency graph
├── parser/              # tree-sitter parsing
├── discovery/           # File discovery
├── report/              # Output formatting
└── refactor/            # Safe delete functionality
```

## Adding a New Detector

1. Create a new file in `src/analysis/detectors/`
2. Implement the `Detector` trait
3. Add the module to `src/analysis/detectors/mod.rs`
4. Add CLI flag in `src/main.rs` if needed
5. Add tests in the same file or `tests/`
6. Update README with the new detection type

## Questions?

- Open an issue for questions
- Check existing issues and discussions

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
