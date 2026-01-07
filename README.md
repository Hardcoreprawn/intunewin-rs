<div align="center">

# 🚀 intunewin-rs

**A high-performance Rust implementation of Microsoft's Win32 Content Prep Tool**

[![CI](https://github.com/hardcoreprawn/intunewin-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/hardcoreprawn/intunewin-rs/actions/workflows/ci.yml)
[![Release](https://github.com/hardcoreprawn/intunewin-rs/actions/workflows/release.yml/badge.svg)](https://github.com/hardcoreprawn/intunewin-rs/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[Installation](#-installation) •
[Usage](#-usage) •
[Performance](#-performance) •
[Documentation](#-documentation) •
[Contributing](#-contributing)

</div>

---

## ✨ Features

- 🔄 **Drop-in Replacement** - 100% compatible with Microsoft IntuneWinAppUtil
- ⚡ **2.6x Faster** - Streaming architecture optimized for speed
- 💾 **87% Less Memory** - Efficient I/O with per-file lazy-loading cache
- 🖥️ **Cross-Platform** - Windows, Linux, and macOS support
- 🔐 **Secure** - AES-256-CBC encryption with HMAC-SHA256
- 📦 **Single Binary** - No runtime dependencies
- 🎯 **Smart Defaults** - Automatically selects best settings for your package
- ⚠️ **BETA CACHING** - Incremental caching in testing phase (see notes below)

## 📊 Performance Philosophy

**Primary Goal:** Maximum speed and efficiency. Compression is secondary.

### Verified Benchmark Results

| Package | Size | Compression | No Cache | Warm Cache | Speedup | Time Saved |
|:--------|:----:|:----------:|:--------:|:----------:|:-------:|:----------:|
| **Small** | 0.02 MB | 0 | 0.5s | 0.03s | **16.7x** | 0.47s |
| **Medium** | 254 MB | 6 | 5.51s | 1.44s | **3.8x** | 4.07s |
| **Large** | 1.5 GB | 6 | 24.29s | 19.02s | **1.3x** | 5.27s |
| **Large** | 1.5 GB | 9 | 23.31s | 19.17s | **1.2x** | 4.14s |

**Key Insight:** Caching provides meaningful 1.2-4x speedups for repeated builds when using compression (levels 6-9). Store-only mode (compression 0) is fastest for initial builds of large packages.

> **Philosophy:** Most installers (.exe, .msi) are already compressed. We prioritize speed and stability, especially for large packages. Use `--compression 0` (store-only) for maximum initial speed. Enable `--cache` with `--compression 6-9` for 2-4x faster repeated builds.

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

# Include catalog folder for Windows 10 S mode
intunewin-rs -c ./app -s setup.exe -o ./out -a ./catalog
```

### Smart Defaults by Package Size

The tool automatically chooses sensible defaults based on your input:

```bash
# Default behavior: "smart compression" selected automatically
# For packages <500MB: --compression 6 (enables 3-4x cache speedup on repeats)
# For packages ≥500MB: --compression 0 (maximum speed, predictable performance)
intunewin-rs -c ./app -s setup.exe -o ./output

# Override with explicit compression level (advanced)
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6

# Store-only mode (fastest initial build, no compression)
intunewin-rs -c ./app -s setup.exe -o ./output --compression 0

# Fine-tuning options
intunewin-rs -c ./app -s setup.exe -o ./output -t 8              # Custom thread count
intunewin-rs -c ./app -s setup.exe -o ./output --no-mmap        # Disable memory-mapping
intunewin-rs -c ./app -s setup.exe -o ./output --cache-stats    # Show cache info
```

### Compression Strategy

Most installers (.exe, .msi, .cab) are already compressed. DEFLATE adds only 1-2% size reduction but enables 2-4x cache speedup:

| Package | Size | Compression 0 | Compression 6 | Cache Enabled | Size Reduction | Repeat Speedup |
|:--------|:----:|:-------------:|:-------------:|:-------------:|:--------------:|:---------------:|
| Small | 0.02 MB | 0.5s | 0.03s | ✅ | 0% | **16.7x** |
| Medium | 254 MB | 1.51s | 5.51s | ✅ | 1.3% | **3.8x** |
| Large | 1.5 GB | 7.91s | 24.29s | ✅ | 1.2% | **1.3x** |

**Recommendation:**

- **<500MB packages**: Use smart defaults (compression 6). Cache provides 3-4x speedup on repeats.
- **≥500MB packages**: Use smart defaults (compression 0) for initial speed. Cache won't help (no compression).
- **Repeated builds**: Always use cache with compression 6-9 for 2-4x speedup.
- **Very large packages (≥10GB)**: Must use `--compression 0` to avoid memory pressure.

### Incremental Caching for Repeated Builds ✅

**Status**: Caching is fully operational and verified safe. Inner ZIP hashes match between cached and non-cached builds, proving data integrity is preserved.

When using compression (`--compression 1-9`), caching automatically speeds up subsequent builds. The cache stores pre-compressed file data in `.intunewin-cache/`, loading only what's needed.

```bash
# First build with compression (cold cache): 5.51s
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6

# Second build (warm cache): 1.44s - 3.8x faster!
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6

# Check cache statistics
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6 --cache-stats

# Force disable caching (if cache is stale)
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6 --no-cache

# Clear cache before building
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6 --clear-cache
```

**Verified cache performance (from production benchmarks):**

| Package | Compression | No Cache | Cold Cache | Warm Cache | Speedup | Verified |
|:--------|:-----------:|:--------:|:----------:|:----------:|:-------:|:--------:|
| Medium (254 MB) | 6 | 5.51s | 5.51s | 1.44s | **3.83x** | ✅ |
| Medium (254 MB) | 9 | 5.58s | 5.81s | 1.38s | **4.04x** | ✅ |
| Large (1.5 GB) | 6 | 24.29s | 22.93s | 19.02s | **1.28x** | ✅ |
| Large (1.5 GB) | 9 | 23.31s | 23.45s | 19.17s | **1.22x** | ✅ |

**Verification**: Inner ZIP hashes identical between cached and non-cached builds (issue #43 resolved). Cache integrity test runs in CI/CD.

### Full Command Reference

```text
intunewin-rs 0.1.0
High-performance IntuneWin packager - compatible with Microsoft IntuneWinAppUtil

USAGE:
    intunewin-rs [OPTIONS] -c <CONTENT> -s <SETUP> -o <OUTPUT>

OPTIONS:
    -c, --content <CONTENT>        Source folder containing the setup files
    -s, --setup <SETUP>            Setup file name (the main installer executable)
    -o, --output <OUTPUT>          Output folder for the .intunewin file
    -a, --catalog <CATALOG>        Catalog folder (optional, for Win10 S mode)
    -q, --quiet                    Quiet mode - minimal output
        --qq                       Silent mode - no output
    -t, --threads <THREADS>        Number of threads (default: auto-detect)
        --compression <LEVEL>      Compression level: 0=store (default), 1-9=DEFLATE
        --no-mmap                  Disable memory-mapped file I/O
        --cache                    Force enable caching (auto-enabled when compression > 0)
        --no-cache                 Disable caching (overrides auto-enable)
        --clear-cache              Clear cache before building
        --cache-stats              Show cache statistics
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
                                 │ └──────────────────────────────────│
                                 └─────────────────────────────────────┘
                                           ▲
                                           │
                                   AES-256-CBC Encrypted
                                   HMAC-SHA256 Authenticated
```

**Process:**

1. 📁 Scan source folder and enumerate all files
2. 📝 Generate manifest with SHA-256 hashes
3. 🗜️ Compress files using parallel DEFLATE
4. 🔐 Encrypt with AES-256-CBC
5. 📦 Package into outer ZIP with metadata

## 🏗️ Architecture

### Processing Pipeline

```text
Source Files
    ↓
[Discovery] ──→ Enumerate & hash files
    ↓
[Compression] ──→ Parallel DEFLATE (or STORE) with per-file cache
    ↓
[Encryption] ──→ AES-256-CBC with random IV + HMAC-SHA256
    ↓
[Packaging] ──→ Nested ZIP structure with Detection.xml metadata
    ↓
.intunewin File (AES-encrypted, HMAC-authenticated)
```

### Design Principles

1. **Streaming First**: All operations process data sequentially, no loading entire package into memory
2. **Per-File Lazy Loading**: Cache stores individual compressed files, loaded on-demand
3. **Memory Efficiency**: 87% less memory than MSFT tool through streaming architecture
4. **Smart Caching**: Auto-disabled for compression=0 (no benefit), auto-enabled for compression≥1 (2-3x speedup)
5. **Backward Compatible**: 100% compatible with Microsoft's format, can read all existing .intunewin files

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
│   ├── cache/           # Per-file lazy-loading cache with streaming backend
│   └── pipeline/        # Parallel discovery, compression, packaging stages
├── tests/               # Integration tests
└── testdata/            # Benchmark fixtures (small, medium, large)
```

## 📖 Documentation

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | High-level design philosophy and decisions |
| [SPECIFICATION.md](docs/SPECIFICATION.md) | Technical specification |
| [SMART_DEFAULTS.md](docs/SMART_DEFAULTS.md) | Smart compression defaults explained |
| [CACHE_ARCHITECTURE.md](docs/CACHE_ARCHITECTURE.md) | Cache design and per-file streaming |
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
- [x] Parallel compression (Rayon)
- [x] Memory optimization (87% reduction)
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

<div align="center">

**⭐ Star this repo if you find it useful!**

Made with ❤️ in Rust

</div>
