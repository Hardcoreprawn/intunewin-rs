# 🚀 intunewin-rs

[![CI](https://github.com/hardcoreprawn/intunewin-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/hardcoreprawn/intunewin-rs/actions/workflows/ci.yml)
[![Release](https://github.com/hardcoreprawn/intunewin-rs/actions/workflows/release.yml/badge.svg)](https://github.com/hardcoreprawn/intunewin-rs/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[Installation](#-installation) •
[Usage](#-usage) •
[Performance](#-performance-philosophy) •
[Documentation](#-documentation) •
[Contributing](#-contributing)

</div>

---

## ✨ Features

A high-performance Rust implementation of Microsoft's Win32 Content Prep Tool

- 🔄 **Core CLI Compatibility** - Compatible with the Microsoft `-c/-s/-o` workflow
- ⚡ **2.6x Faster** - Zero-materialization pipeline with single-pass I/O
- 💾 **Minimal Memory** - Peak memory ≈ largest single source file
- 🖥️ **Cross-Platform** - Windows, Linux, and macOS support
- 🔐 **Secure** - AES-256-CBC encryption with HMAC-SHA256
- 📦 **Single Binary** - No runtime dependencies
- 🎯 **Zero Overhead** - No wasted CPU on already-compressed content

## 📊 Performance Philosophy

**Primary Goal:** Maximum speed and I/O efficiency. We never compress.

> **Why no compression?** Real-world Intune packages (.exe, .msi, .cab) are already compressed by their authors. DEFLATE achieves <2% additional size reduction on these files — but costs 3-10× in build time and forces a multi-pass I/O pipeline. Compression is not just unhelpful in our model — it's actively harmful to performance.
>
> Store-only mode means the inner ZIP is byte-for-byte deterministic from file metadata alone. This enables the zero-materialization pipeline: source files stream directly through ZIP structure generation → AES encryption → final output. No intermediate files, no buffering, no wasted I/O.

## 📦 Installation

### Download Pre-built Binary (Recommended)

Download the latest release for your platform from the [Releases page](https://github.com/hardcoreprawn/intunewin-rs/releases).

#### Windows (PowerShell)

```powershell
# Download and extract
$version = "0.1.0"
Invoke-WebRequest -Uri "https://github.com/hardcoreprawn/intunewin-rs/releases/download/v$version/intunewin-rs-x86_64-pc-windows-msvc.zip" -OutFile "intunewin-rs.zip"
Expand-Archive -Path "intunewin-rs.zip" -DestinationPath "."

# Add to PATH (optional)
$env:PATH += ";$PWD"

# Verify installation
.\intunewin-rs.exe --version
```

#### Linux / macOS

```bash
# Download (Linux x86_64)
curl -LO https://github.com/hardcoreprawn/intunewin-rs/releases/latest/download/intunewin-rs-x86_64-unknown-linux-gnu.tar.gz
tar -xzf intunewin-rs-x86_64-unknown-linux-gnu.tar.gz

# Download (macOS Apple Silicon)
curl -LO https://github.com/hardcoreprawn/intunewin-rs/releases/latest/download/intunewin-rs-aarch64-apple-darwin.tar.gz
tar -xzf intunewin-rs-aarch64-apple-darwin.tar.gz

# Move to PATH
sudo mv intunewin-rs /usr/local/bin/

# Verify installation
intunewin-rs --version
```

### Build from Source

```bash
git clone https://github.com/hardcoreprawn/intunewin-rs.git
cd intunewin-rs
cargo build --release

# Binary is at target/release/intunewin-rs
```

## 🎮 Usage

### Basic Usage (Compatible with Microsoft Tool)

```bash
# Create an .intunewin package
intunewin-rs -c <source_folder> -s <setup_file> -o <output_folder>

# Example
intunewin-rs -c ./my-app -s setup.exe -o ./output
```

### Common Examples

```bash
# Package a simple installer
intunewin-rs -c ./installer -s MyApp-Setup.exe -o ./packages

# Package with quiet mode (minimal output)
intunewin-rs -c ./installer -s setup.msi -o ./output -q

# Silent mode (no output)
intunewin-rs -c ./installer -s setup.exe -o ./output --qq

# Catalog support note
# -a/--catalog is reserved for compatibility and currently returns an explicit "not implemented" error
```

### Options

```bash
# Default usage — store-only, zero-materialization pipeline
intunewin-rs -c ./app -s setup.exe -o ./output

# Fine-tuning options
intunewin-rs -c ./app -s setup.exe -o ./output -t 8              # Custom thread count
intunewin-rs -c ./app -s setup.exe -o ./output --no-mmap        # Disable memory-mapping
```

### Why We Never Compress

Intune packages contain installers — `.exe`, `.msi`, `.msix`, `.cab` — that are already compressed by their authors. Running DEFLATE over pre-compressed data is pure waste:

| Package | Size | Store-only (comp 0) | DEFLATE (comp 6) | Size Saved | Time Wasted |
| ------- | ---- | ------------------- | ----------------- | ---------- | ----------- |
| Medium | 254 MB | 1.51s | 5.51s | 1.3% | **3.6×** |
| Large | 1.5 GB | 7.91s | 24.29s | 1.2% | **3.1×** |

Compression also forces a multi-pass pipeline (compress → write ZIP → read ZIP → encrypt → write output), while store-only enables zero-materialization: source files stream directly through ZIP headers → AES encryption → final output in a single pass.

**There is no scenario where compression is worth it in this model. We always store.**

> **Large package safety**: Automatically switches to ZIP64 streaming mode for very large inputs to keep memory bounded and support >4 GiB/65k-entry cases.

### Full Command Reference

```text
intunewin-rs 0.3.0
High-performance IntuneWin packager for Intune app deployment

USAGE:
    intunewin-rs [OPTIONS] -c <CONTENT> -s <SETUP> -o <OUTPUT>

OPTIONS:
    -c, --content <CONTENT>        Source folder containing the setup files
    -s, --setup <SETUP>            Setup file name (the main installer executable)
    -o, --output <OUTPUT>          Output folder for the .intunewin file
    -a, --catalog <CATALOG>        Catalog folder (reserved for compatibility; currently unsupported)
    -q, --quiet                    Quiet mode - minimal output
        --qq                       Silent mode - no output
    -t, --threads <THREADS>        Number of threads (default: auto-detect)
        --no-mmap                  Disable memory-mapped file I/O
    -h, --help                     Print help
    -V, --version                  Print version
```

## 🔧 How It Works

```text
Source Folder                    Output (.intunewin)
┌─────────────┐                  ┌─────────────────────────────────────┐
│ setup.exe   │                  │ setup.intunewin (outer ZIP)         │
│ data/       │   ──────────►    │ ├── IntuneWinPackage/               │
│ config.ini  │    Package       │ │   ├── Metadata/Detection.xml      │
│ readme.txt  │                  │ │   └── Contents/                   │
└─────────────┘                  │ │       └── IntunePackage.intunewin │
                                 │ └────────────────────────────────── │
                                 └─────────────────────────────────────┘
                                           ▲
                                           │
                                   AES-256-CBC Encrypted
                                   HMAC-SHA256 Authenticated
```

**Process:**

1. 📁 Scan source folder and enumerate all files
2. 📝 Generate manifest with SHA-256 hashes
3. � Stream through AES-256-CBC encryption
4. 📦 Package into outer ZIP with metadata

## 🏗️ Architecture

### Processing Pipeline

```text
Source Files
    ↓
[Discovery] ──→ Enumerate & hash files
    ↓
[Zero-Mat] ──→ Stream ZIP headers + file data → AES-CBC → outer ZIP
    ↓
.intunewin File (AES-encrypted, HMAC-authenticated)
```

The zero-materialization pipeline never creates an intermediate inner ZIP file or buffer. Source files are read once, their bytes flow through ZIP structure generation and AES encryption directly into the final `.intunewin` output. Total I/O = read sources once + write output once.

### Design Principles

1. **Zero Materialization**: No intermediate files or buffers — source bytes flow directly to encrypted output
2. **Single-Pass I/O**: Read sources once, write output once — theoretical minimum I/O
3. **Memory Efficiency**: Peak memory ≈ largest single source file, not total package size
4. **Compatibility-First**: Core packaging workflow is Microsoft-compatible; unsupported paths fail explicitly

### Directory Structure

```text
intunewin-rs/
├── src/
│   ├── main.rs          # Entry point & orchestration
│   ├── cli.rs           # Argument parsing (clap)
│   ├── error.rs         # Error types & handling
│   ├── crypto/          # AES-256-CBC, HMAC-SHA256, key generation
│   ├── format/          # IntuneWin format parsing, manifest, detection.xml
│   ├── io/              # Memory-mapped file reading, streaming writes
│   └── pipeline/        # Discovery, zero-materialization, packaging stages
├── tests/               # Integration tests
└── testdata/            # Benchmark fixtures (small, medium, large)
```

## 📖 Documentation

| Document | Description |
| ---------- | ------------- |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | High-level design philosophy and decisions |
| [SPECIFICATION.md](docs/SPECIFICATION.md) | Technical specification |
| [COMPATIBILITY.md](docs/COMPATIBILITY.md) | Current compatibility contract and support matrix |
| [BUILD_AND_TEST.md](docs/BUILD_AND_TEST.md) | Development guide |
| [TOOL_ANALYSIS.md](docs/TOOL_ANALYSIS.md) | Microsoft tool analysis |
| [AUDIT_REPORT.md](docs/AUDIT_REPORT.md) | Security & performance audit |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guidelines |
| [CHANGELOG.md](CHANGELOG.md) | Release history |

## 🔐 Security

- **Encryption**: AES-256-CBC with random IV
- **Authentication**: HMAC-SHA256 for integrity verification
- **Hashing**: SHA-256 for file integrity
- **Key Generation**: Cryptographically secure random

All cryptographic operations use well-audited Rust crates from the [RustCrypto](https://github.com/RustCrypto) project.

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run benchmarks (requires test data)
.\testdata\benchmarks\benchmark.ps1 -PackageSize small
```

### Generate Test Data

```powershell
# Small package (100MB, 100 files)
.\tests\setup-test-environment.ps1 -DataSize small

# Medium package (2.5GB, 500 files)
.\tests\setup-test-environment.ps1 -DataSize medium

# Large package (20GB, 2000 files)
.\tests\setup-test-environment.ps1 -DataSize large
```

## 🤝 Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.

```bash
# Clone and setup
git clone https://github.com/hardcoreprawn/intunewin-rs.git
cd intunewin-rs

# Enable pre-commit hooks (recommended)
git config core.hooksPath .githooks

# Run checks
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# Submit a PR!
```

## 📋 Roadmap

- [x] Core implementation
- [x] Zero-materialization pipeline (single-pass I/O)
- [x] Memory optimization (peak mem ≈ largest file, not package size)
- [x] Streaming encryption
- [x] Cross-platform builds
- [x] CI/CD pipeline
- [ ] Progress bars (indicatif)
- [ ] Configuration file support
- [ ] Ctrl+C signal handling
- [ ] Async I/O (tokio)

## 📄 License

This project is licensed under the [MIT License](LICENSE).

## 🙏 Acknowledgments

- [Microsoft Win32 Content Prep Tool](https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool) - Original implementation
- [RustCrypto](https://github.com/RustCrypto) - Cryptographic primitives
- [Rayon](https://github.com/rayon-rs/rayon) - Data parallelism

---

**⭐ Star this repo if you find it useful!**

Made with ❤️ in Rust

</div>
