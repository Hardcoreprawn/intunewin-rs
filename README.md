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
- ⚡ **2.6x Faster** - Parallel compression and optimized encryption
- 💾 **87% Less Memory** - Streaming processing for large packages
- 🖥️ **Cross-Platform** - Windows, Linux, and macOS support
- 🔐 **Secure** - AES-256-CBC encryption with HMAC-SHA256
- 📦 **Single Binary** - No runtime dependencies

## 📊 Performance

| Package Size | Microsoft Tool | intunewin-rs | Speedup |
|:-------------|:--------------:|:------------:|:-------:|
| Small (98 MB) | 3.8s | 0.9s | **4.2x** |
| Medium (254 MB) | 9.5s | 6.9s | **1.4x** |
| Large (1.5 GB) | 57s | 27.8s | **2.1x** |

> **Average: 2.6x faster** with **87% less memory usage**

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

### Performance Tuning

> **Note:** By default, intunewin-rs uses store-only mode (no compression) because most installers are already compressed. This provides the fastest packaging speed.

```bash
# Default behavior: store-only (fastest, no compression)
intunewin-rs -c ./large-app -s setup.exe -o ./output

# Enable compression for smaller output (slower)
intunewin-rs -c ./small-app -s setup.exe -o ./output --compression 6

# Maximum compression (slowest, smallest output)
intunewin-rs -c ./small-app -s setup.exe -o ./output --compression 9

# Specify thread count
intunewin-rs -c ./app -s setup.exe -o ./output -t 8

# Disable memory-mapped I/O (for network drives)
intunewin-rs -c ./app -s setup.exe -o ./output --no-mmap
```

### Compression vs Speed

Most installers (.exe, .msi, .cab) are already compressed, so additional DEFLATE compression provides minimal size reduction. Here's real benchmark data:

| Package | Input Size | Compression 0 | Compression 6 | Size Savings |
|:--------|:----------:|:-------------:|:-------------:|:------------:|
| Small (installers) | 98 MB | **0.56s** → 97.94 MB | 0.86s → 97.94 MB | 0% |
| Medium (installer) | 254 MB | **1.58s** → 253.74 MB | 6.68s → 250.41 MB | 1.3% |
| Large (Windows ADK) | 1.5 GB | **8.13s** → 1531 MB | 26.81s → 1510 MB | 1.4% |

**Recommendation:** Use the default `--compression 0` for maximum speed. The 1-2% size reduction from compression rarely justifies the 3-4x slower packaging time.

### Incremental Caching

When using compression (`--compression 1-9`), caching is **automatically enabled** to speed up subsequent builds. The cache stores pre-compressed file data, so unchanged files don't need to be recompressed.

```bash
# First build with compression (cold cache): 26.8s
intunewin-rs -c ./large-app -s setup.exe -o ./output --compression 6

# Second build (warm cache): 11.2s - 2.4x faster!
intunewin-rs -c ./large-app -s setup.exe -o ./output --compression 6

# Check cache statistics
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6 --cache-stats

# Force disable caching
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6 --no-cache

# Clear cache before building
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6 --clear-cache
```

**Cache behavior by compression level:**

| Compression | Caching | Warm Cache Speedup | Recommendation |
|:-----------:|:-------:|:------------------:|:---------------|
| 0 (store) | Disabled | N/A | Default - fastest for pre-compressed installers |
| 1-9 | Auto-enabled | **2-3.5x faster** | Use for repeated builds or CI/CD |

The cache is stored in `.intunewin-cache/` within the output directory and is automatically invalidated when:

- Source files are modified (by size or timestamp)
- Compression level changes
- Files are added or removed

### Full Command Reference

```
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

```
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

```
intunewin-rs/
├── src/
│   ├── main.rs          # Entry point
│   ├── cli.rs           # Argument parsing (clap)
│   ├── crypto/          # AES-256, HMAC, key generation
│   ├── format/          # IntuneWin format, manifest, detection
│   ├── io/              # Memory-mapped I/O
│   └── pipeline/        # Parallel compression, packaging
├── tests/               # Integration tests
└── testdata/            # Benchmark fixtures
```

## 📖 Documentation

| Document | Description |
|----------|-------------|
| [SPECIFICATION.md](docs/SPECIFICATION.md) | Technical specification |
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
