# Contributing to intunewin-rs

Thank you for your interest in contributing to intunewin-rs! This document provides guidelines and information for contributors.

## 📋 Table of Contents

- [Code of Conduct](#-code-of-conduct)
- [Getting Started](#-getting-started)
- [Development Setup](#-development-setup)
- [Making Changes](#-making-changes)
- [Testing](#-testing)
- [Submitting Changes](#-submitting-changes)
- [Release Process](#-release-process)

## 📜 Code of Conduct {#-code-of-conduct}

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and constructive in all interactions.

## 🚀 Getting Started {#getting-started}

### Prerequisites

- Rust 1.70 or higher
- Git
- Windows (for full testing with Microsoft tool comparison)

### Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork locally:

   ```bash
   git clone https://github.com/YOUR_USERNAME/intunewin-rs.git
   cd intunewin-rs
   ```

3. Add the upstream remote:

   ```bash
   git remote add upstream https://github.com/ORIGINAL_OWNER/intunewin-rs.git
   ```

## 🔧 Development Setup

### Install Dependencies

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install development tools
rustup component add clippy rustfmt

# Optional: Install cargo-watch for auto-rebuild
cargo install cargo-watch

# Optional: Install cargo-audit for security checks
cargo install cargo-audit
```

### Build the Project

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Watch mode (auto-rebuild on changes)
cargo watch -x build
```

### Generate Test Data

```powershell
# Windows only - generate test packages
.\tests\setup-test-environment.ps1 -DataSize small
```

## 📝 Making Changes

### Branch Naming

Use descriptive branch names:

- `feature/parallel-compression` - New features
- `fix/memory-leak` - Bug fixes
- `docs/readme-update` - Documentation
- `refactor/crypto-module` - Code refactoring
- `perf/streaming-encryption` - Performance improvements

### Code Style

We follow standard Rust conventions:

```bash
# Format code
cargo fmt

# Check for lints
cargo clippy --all-targets --all-features -- -D warnings

# Check for security vulnerabilities
cargo audit
```

### Commit Messages

Follow conventional commits format:

```text
type(scope): description

[optional body]

[optional footer]
```

Types:

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

Examples:

```text
feat(crypto): add streaming encryption for large files
fix(cli): handle paths with spaces correctly
docs(readme): add installation instructions
perf(compression): implement parallel DEFLATE
```

## 🧪 Testing

### Run Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests in release mode
cargo test --release

# Run with coverage (requires cargo-llvm-cov)
cargo llvm-cov --all-features
```

### Test Categories

1. **Unit Tests**: Test individual functions and modules
2. **Integration Tests**: Test end-to-end functionality
3. **Benchmark Tests**: Compare against Microsoft tool
4. **Compatibility Tests**: Verify output format matches Microsoft

### Adding Tests

- Add unit tests in the same file as the code
- Add integration tests in the `tests/` directory
- Ensure tests are deterministic and don't depend on external state

## 📤 Submitting Changes

### Pull Request Process

1. **Update your branch**:

   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Run all checks**:

   ```bash
   cargo fmt --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-features
   cargo doc --no-deps
   ```

3. **Push your branch**:

   ```bash
   git push origin your-branch-name
   ```

4. **Create a Pull Request**:
   - Use a clear, descriptive title
   - Reference any related issues
   - Describe what changes were made and why
   - Include any breaking changes

### PR Checklist

- [ ] Code follows Rust conventions
- [ ] All tests pass
- [ ] Documentation is updated
- [ ] CHANGELOG.md is updated (for significant changes)
- [ ] No new clippy warnings
- [ ] Commit messages follow conventions

## 🚢 Release Process

Releases are automated via GitHub Actions. To create a release:

1. Update `CHANGELOG.md` with the new version
2. Update version in `Cargo.toml`
3. Commit the changes:

   ```bash
   git commit -am "chore: prepare release vX.Y.Z"
   ```

4. Create and push a tag:

   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

The CI/CD pipeline will automatically:

- Build binaries for all platforms
- Create a GitHub release
- Upload release assets
- Generate SHA256 checksums
- Publish to crates.io (if configured)

## 📁 Project Structure

```text
intunewin-rs/
├── src/
│   ├── main.rs          # Entry point
│   ├── lib.rs           # Library root
│   ├── cli.rs           # Command-line interface
│   ├── error.rs         # Error types
│   ├── progress.rs      # Progress tracking
│   ├── crypto/          # Encryption module
│   ├── format/          # IntuneWin format handling
│   ├── io/              # I/O utilities
│   └── pipeline/        # Processing pipeline
├── tests/               # Integration tests
├── testdata/            # Test fixtures
├── docs/                # Documentation
└── .github/workflows/   # CI/CD configuration
```

## 🆘 Getting Help

- Open an issue for bugs or feature requests
- Start a discussion for questions
- Check existing issues and discussions first

## 📄 License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to intunewin-rs! 🎉
