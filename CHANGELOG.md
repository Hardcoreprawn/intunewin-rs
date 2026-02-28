# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- markdownlint-configure-file {"MD024": {"siblings_only": true}} -->

## [Unreleased]

### Added

- Future features and improvements will be listed here

## [0.4.0] - 2026-02-28

### Added

- Channeled two-thread producer/consumer pipeline via `crossbeam-channel` for
  overlapped I/O and encryption
- Sub-file chunking (1 MB) enables fine-grained parallelism even for single
  large files
- In-place AES-CBC encryption (`encrypt_chunk_no_padding_inplace`) eliminates
  per-chunk allocation
- `FileBytes` enum and `open_file_for_streaming` for zero-copy mmap streaming

### Changed

- Zero-materialization pipeline is now channeled by default (no env-var toggle)
- Large-file benchmark: +9.7% p50 / +16.8% p95 throughput improvement

### Removed

- Legacy single-thread `run_zero_mat` code path
- `INTUNEWIN_CHANNELED` environment variable toggle

## [0.3.0] - 2026-02-23

### Added

- ZIP64 streaming fallback for very large package inputs to keep memory bounded
  and support >4 GiB / >65k-entry scenarios
- Explicit catalog-flag behavior test coverage (`--catalog` now documented
  and validated as reserved/unsupported)
- Compatibility contract documentation (`docs/COMPATIBILITY.md`) for supported
  and unsupported behavior

### Changed

- Centralized preflight and smart-default resolution in pipeline
  orchestration
- README and build/test docs aligned with current behavior and
  compatibility contract
- Setup-file discovery now deterministic with explicit ambiguity errors

### Fixed

- ZIP32 boundary guardrails for offsets, sizes, entry counts, and
  name-length safety
- Setup-path sanitization to reject unsafe inputs (absolute paths and
  parent traversal)
- Output path safety checks to prevent writing package output within
  content roots
- Checked memory allocation conversions in mmap path (`u64` to `usize`)
  for safer large-file handling
- CI confidence improved by making cache-integrity tests first-class and
  stabilizing mmap threshold variance expectations

## [0.2.0] - 2026-02-23

### Added

- Performance optimizations: platform-specific mmap thresholds (#53)
- Buffered ZIP writes for improved I/O efficiency (#58)
- Cached normalized paths during file discovery (#57)

### Fixed

- Detection.xml `Name` field now includes file extension for full parity with IntuneWinAppUtil (#65)
- Platform-aware mmap threshold validation: 256KB (Windows) vs 1MB (Linux/macOS)
- Test reliability on different platforms

### Dependencies

- Updated dependencies:
  - `zip`: 7.0 → 8.0.0 (Rust 2024 migration)
  - `rand`: 0.9 → 0.10.0 (API improvements)
  - `quick-xml`: 0.38 → 0.39.1 (namespace scope fixes)

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

[Unreleased]: https://github.com/hardcoreprawn/intunewin-rs/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/hardcoreprawn/intunewin-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/hardcoreprawn/intunewin-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/hardcoreprawn/intunewin-rs/releases/tag/v0.1.0
