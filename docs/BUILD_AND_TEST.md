# Building and Testing the Rust IntuneWin Encoder

## Prerequisites

- **Rust 1.70+**: [Install from rustup.rs](https://rustup.rs/)
- **Windows 10/11** with development tools
- **PowerShell 5.1+** (for benchmark scripts)
- **~150GB disk space** (if generating xlarge test data)

## Quick Start

### 1. Setup Test Environment

```powershell
cd d:\projects\rIntuneWinApp

# Generate medium test package (~2.5GB)
.\tests\setup-test-environment.ps1 -DataSize medium

# Or download existing if available
.\tests\setup-test-environment.ps1 -SkipIntuneWinDownload
```

### 2. Build the Existing Project

```powershell
# Project is already initialized; just build it
cargo build
```

### 3. Build

```powershell
# Debug build (fast compilation, slow execution)
cargo build

# Release build (slow compilation, optimized execution for benchmarking)
cargo build --release
```

### 4. Run Tests

```powershell
# Unit tests
cargo test

# Integration tests
cargo test --test '*' -- --nocapture

# Benchmark against reference tool
.\tests\setup-test-environment.ps1 -DataSize small
.\testdata\benchmarks\benchmark.ps1 -PackageSize small
```

## Development Workflow

### Iterative Development

```bash
# Watch for changes and rebuild
cargo watch -x build

# Run tests on file change
cargo watch -x test

# Run benchmarks
cargo bench
```

### Profiling with Windows Performance Analyzer

```powershell
# Start tracing
xperf -on Base+DiskIO+FileIO+Memory+ProcessCounter -BufferSize 1024 -MaxBuffers 256

# Run your encoder
.\target\release\intunewin-rs -c .\testdata\packages\small -s setup.exe -o .\testdata\output -q

# Stop tracing and generate ETL
xperf -d .\testdata\benchmarks\profiling_result.etl

# Analyze (opens in GUI)
xperfview .\testdata\benchmarks\profiling_result.etl
```

### Flamegraph Profiling

```powershell
# Install flamegraph (Windows version)
cargo install flamegraph

# Run with sampling
cargo flamegraph --release -- -c .\testdata\packages\small -s setup.exe -o .\testdata\output -q

# View result
.\flamegraph.svg  # Opens in browser
```

## Legacy Design Notes (Historical)

The following structure and `Cargo.toml` template are from earlier planning phases and are retained for historical context. For current repository structure, refer to `README.md` and the actual workspace tree.

## Project Structure During Development

```text
rIntuneWinApp/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library root
│   ├── cli/                 # CLI argument parsing
│   │   └── args.rs
│   ├── encoder/             # Core encoding logic
│   │   ├── mod.rs
│   │   ├── manifest.rs
│   │   ├── header.rs
│   │   ├── zip_writer.rs
│   │   └── compressor.rs
│   ├── hasher/              # Parallel hashing
│   │   └── parallel_sha256.rs
│   ├── io/                  # I/O optimizations
│   │   ├── file_enumerator.rs
│   │   ├── buffer_pool.rs
│   │   └── mmap.rs
│   └── utils/
│       ├── progress.rs
│       └── verification.rs
├── tests/
│   ├── integration_tests.rs
│   ├── setup-test-environment.ps1
│   └── fixtures/
├── benches/
│   └── encoder_bench.rs
├── Cargo.toml
├── SPECIFICATION.md
└── BUILD_AND_TEST.md        # This file
```

## Cargo.toml Template

```toml
[package]
name = "intunewin-rs"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4.4", features = ["derive"] }
zip = "0.6"
sha2 = "0.10"
rayon = "1.8"
crossbeam = "0.8"
memmap2 = "0.9"
zstd = "0.13"
anyhow = "1.0"
log = "0.4"
env_logger = "0.11"
indicatif = "0.17"
chrono = "0.4"
uuid = { version = "1.6", features = ["v4", "serde"] }
serde = { version = "1.0", features = ["derive"] }
serde_xml_rs = "0.6"

[dev-dependencies]
tempfile = "3.8"
criterion = "0.5"

[[bench]]
name = "encoder_bench"
harness = false

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = false  # Keep symbols for profiling

[profile.bench]
inherits = "release"
```

## Benchmarking Strategy

### Experiment Framework (Issue #75)

Use the shared harness to evaluate candidate experiment branches with the same metrics and decision gates.

```powershell
# Build release binary first
cargo build --release

# Run framework baseline/candidate comparison
.\testdata\benchmarks\experiment-framework.ps1
```

For advanced usage and command-template overrides, see `docs/EXPERIMENT_FRAMEWORK.md`.

### Phase 1: Baseline

```powershell
# Profile MSFT tool on small/medium/large packages
.\testdata\benchmarks\benchmark.ps1 -PackageSize small
.\testdata\benchmarks\benchmark.ps1 -PackageSize medium
# Save baseline metrics
```

### Phase 2: Early Development

```powershell
# Build simple single-threaded encoder
cargo build --release

# Compare against MSFT (expect 0.5-1.5x)
Measure-Command { 
    .\target\release\intunewin-rs -c .\testdata\packages\small -s setup.exe -o .\testdata\output -q
}
```

### Phase 3: Optimization

```powershell
# After adding parallelism, SIMD, memory mapping
# Target: 2-3x improvement vs MSFT

# Run with profiling
xperfview .\testdata\benchmarks\profiling_result.etl

# Identify bottleneck (I/O? Compression? Hashing?)
# Focus optimization on top 3 bottlenecks
```

### Phase 4: Validation

```powershell
# Verify output matches MSFT tool
.\testdata\benchmarks\validate-output.ps1 `
    -MsftOutput .\testdata\output\benchmark_msft_medium.intunewin `
    -RustOutput .\testdata\output\rust_medium.intunewin
```

## Performance Targets

| Milestone | Small (100MB) | Medium (2.5GB) | Large (20GB) |
| --------- | ------------- | -------------- | ------------ |
| Phase 1 (MVP) | 1x MSFT | 0.8x MSFT | N/A |
| Phase 2 (Parallel) | 1.5x MSFT | 1.5x MSFT | 2x MSFT |
| Phase 3 (Optimized) | 2x MSFT | 3x MSFT | 5x MSFT |
| Phase 4 (Final) | 2.5x MSFT | 4x MSFT | 8x MSFT |

## Troubleshooting

### Build Fails on Windows

- Ensure Visual Studio Build Tools or Visual C++ are installed
- `rustc --version` should report version info
- `cargo --version` should work

### Test Data Generation is Slow

- Use `-DataSize small` for faster initial testing
- Run with SSD for better performance
- Skip with `-SkipIntuneWinDownload` flag

### Memory Issues During Testing

- Reduce `--chunk-size` in benchmarks
- Use smaller test packages
- Monitor with Task Manager

### Performance Differences Between Runs

- Close other applications
- Use same test package consistently
- Run multiple iterations and average
- Note CPU temperature and throttling

## Useful Commands

```powershell
# Check Rust version/components
rustc --version
cargo --version
rustup show

# Clean build artifacts
cargo clean

# Format code
cargo fmt

# Lint (clippy)
cargo clippy -- -D warnings

# Generate documentation
cargo doc --open

# Check without building
cargo check

# See binary size
cargo bloat --release
```

## Next Steps

1. **Validate local toolchain and repository checkout**
2. **Implement CLI argument parsing** (using clap)
3. **Build file enumerator** (recursive directory traversal)
4. **Implement manifest generation** (XML output)
5. **Create streaming ZIP writer** (zip crate + custom buffering)
6. **Add SHA256 hashing** (sha2 crate)
7. **Implement parallel compression** (rayon + zstd)
8. **Profile and optimize** (Windows Performance Analyzer)
9. **Benchmark vs MSFT tool** (compare timings)
10. **Package and release**

## References

- [Rust Book](https://doc.rust-lang.org/book/)
- [Cargo Documentation](https://doc.rust-lang.org/cargo/)
- [Windows Performance Toolkit](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/)
- [Flamegraph for Rust](https://www.brendangregg.com/flamegraphs.html)
