# Test Data and Benchmarking

This directory contains test data and benchmarking scripts for IntuneWin performance testing.

## Directory Structure

\\\
testdata/
├── tools/                 # Microsoft tools and reference implementations
│   └── intunewinapputil.exe
├── packages/              # Test packages of various sizes
│   ├── small/            # ~100MB
│   ├── medium/           # ~2.5GB
│   ├── large/            # ~20GB
│   └── xlarge/           # ~100GB
├── output/               # Generated .intunewin files
└── benchmarks/           # Benchmark scripts and results
\\\

## Running Benchmarks

### Setup

\\\powershell

## Generate test data (small, medium, large, xlarge)

.\setup-test-environment.ps1 -DataSize medium -GenerateTestData
\\\

## Run Benchmark

\\\powershell

### Benchmark Microsoft tool

.\benchmarks\benchmark.ps1 -PackageSize medium

### Benchmark Rust implementation (when ready)

cargo run --release -- -i .\testdata\packages\medium -o .\testdata\output\rust_medium.intunewin
\\\

## Profiling

### Windows Performance Analyzer

\\\powershell

# Capture trace

xperf -on Base+DiskIO+FileIO+Memory+ProcessCounter -BufferSize 1024 -MaxBuffers 256
cargo run --release -- -i testdata\\packages\\medium -o testdata\\output\\test.intunewin
xperf -d trace.etl

# View trace

xperfview trace.etl
\\\

### Performance Insights to Track

- **Disk I/O**: Read throughput, seek patterns
- **CPU**: Thread utilization, compression efficiency
- **Memory**: Peak usage, allocation patterns
- **Overall**: Encoding time, throughput (GB/sec)

## Test Data Characteristics

| Size | Files | Approx Total | Use Case |
|------|-------|--------------|----------|
| small | 100 | 100 MB | Quick unit tests |
| medium | 500 | 2.5 GB | Daily development |
| large | 2000 | 20 GB | Performance testing |
| xlarge | 20000 | 100 GB | Stress testing |

## Expected Performance Targets

Based on SPECIFICATION.md:

- **Small (100MB)**: 1.5-2x faster than MSFT tool
- **Medium (2.5GB)**: 2-3x faster
- **Large (20GB)**: 3-5x faster
- **XLarge (100GB)**: 5-10x faster (with parallelism)

### MSFT Tool Baseline (approximate)

- 2.5GB: ~120-180 seconds
- 20GB: ~900-1200 seconds
- 100GB: Not typically used for such large packages

## Optimization Focus Areas

1. **I/O**: Memory-mapped reads, buffer pools
2. **Compression**: Parallel chunking, SIMD hashing
3. **Parallelism**: Rayon workers, thread pool tuning
4. **Memory**: Zero-copy buffers, streaming writes

## Contribution Guidelines

When adding new tests or benchmarks:

1. Document the test purpose
2. Include before/after performance metrics
3. Run on same hardware for comparison
4. Update this README with findings
