# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Future features and improvements will be listed here

## [0.1.0] - 2026-01-06

### Added

- 🚀 Initial release of intunewin-rs
- Full compatibility with Microsoft IntuneWinAppUtil output format
- CLI interface matching Microsoft tool (`-c`, `-s`, `-o`, `-a`, `-q`, `--qq`)
- Extended options: `--threads`, `--compression`, `--no-mmap`
- Parallel compression using Rayon for multi-core CPUs
- Memory-mapped file I/O for efficient large file handling
- Streaming AES-256-CBC encryption for memory efficiency
- Configurable compression levels (0-9)
- Cross-platform support (Windows, Linux, macOS)
- Comprehensive unit test suite (33 tests)

### Performance

- **2.6x average speedup** over Microsoft IntuneWinAppUtil
- **4.2x faster** for small packages (~100MB)
- **2.1x faster** for large packages (~1.5GB)
- **87% memory reduction** compared to naive implementation
- Optimized for packages up to 40GB+

### Security

- AES-256-CBC encryption with secure key generation
- HMAC-SHA256 message authentication
- SHA-256 file hashing for integrity verification

### Documentation

- Comprehensive README with usage examples
- Technical specification (SPECIFICATION.md)
- Build and test guide (BUILD_AND_TEST.md)
- Tool analysis documentation (TOOL_ANALYSIS.md)
- Security audit report (AUDIT_REPORT.md)

### Infrastructure

- GitHub Actions CI/CD pipeline
- Cross-platform release builds
- Automated testing on Windows, Linux, macOS
- Code coverage reporting
- Security vulnerability scanning

[Unreleased]: https://github.com/hardcoreprawn/intunewin-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/hardcoreprawn/intunewin-rs/releases/tag/v0.1.0
