# Quick Reference: Commands & Checklists

## 🚀 Getting Started (5 minutes)

```powershell
cd d:\projects\rIntuneWinApp

# 1. Verify setup
.\testdata\tools\IntuneWinAppUtil.exe -v      # Should show: 1.8.7.0

# 2. Check test data
Get-ChildItem testdata\packages\small -Recurse | Measure-Object -Sum -Property Length
# Should show: ~100MB

# 3. Generate example output (if not exists)
.\testdata\tools\IntuneWinAppUtil.exe -c testdata\packages\small -s setup.exe -o testdata\output -q

# 4. View generated file
Get-Item testdata\output\setup.intunewin | Select-Object Length
```

## 📚 Documentation Quick Links

| Document | Purpose | Read Time |
|----------|---------|-----------|
| README.md | Project overview & quick start | 10 min |
| SPECIFICATION.md | Technical specification | 30 min |
| TOOL_ANALYSIS.md | MSFT tool internals | 15 min |
| BUILD_AND_TEST.md | Development workflow | 20 min |

## 🧪 Testing & Profiling

### Generate Test Packages

```powershell
# Small (100MB) - already generated
# ~100 files, ready immediately

# Medium (2.5GB) - recommended for development
.\tests\setup-test-environment.ps1 -DataSize medium
# Takes ~2-5 minutes

# Large (20GB) - for serious benchmarking
.\tests\setup-test-environment.ps1 -DataSize large
# Takes ~15-30 minutes

# XLarge (100GB) - theoretical target
.\tests\setup-test-environment.ps1 -DataSize xlarge
# Takes ~90+ minutes
```

### Benchmark MSFT Tool

```powershell
# Single package benchmark
.\testdata\benchmarks\benchmark.ps1 -PackageSize small

# Expected output:
# - Completed in: ~5 seconds
# - Throughput: ~20 MB/sec
# - Output size: ~98 MB
```

### Profile with Windows Performance Analyzer

```powershell
# Start tracing (Admin required)
xperf -on Base+DiskIO+FileIO+Memory+ProcessCounter -BufferSize 1024 -MaxBuffers 256

# Run encoder
.\target\release\intunewin-rs -c testdata\packages\small -s setup.exe -o output -q

# Save and view trace
xperf -d result.etl
xperfview result.etl
```

## 🔨 Development Workflow

### Initialize Project (One-time)

```powershell
cd d:\projects\rIntuneWinApp
cargo init --name intunewin-rs
# This creates the Rust project structure
```

### Daily Development

```powershell
# Clean rebuild
cargo clean && cargo build

# Debug build (fast compile, slow runtime)
cargo build

# Release build (slow compile, optimized runtime)
cargo build --release

# Run tests
cargo test

# Run with test data
cargo run -- -c testdata\packages\small -s setup.exe -o output -q

# Check for issues (without building)
cargo check

# Format code
cargo fmt

# Lint checks
cargo clippy -- -D warnings

# Generate docs
cargo doc --open
```

### Performance Iteration

```powershell
# 1. Build release version
cargo build --release

# 2. Measure baseline
Measure-Command { 
    .\target\release\intunewin-rs -c testdata\packages\small -s setup.exe -o output -q 
}

# 3. Record time in spreadsheet

# 4. Profile bottleneck
# See "Profile with Windows Performance Analyzer" section

# 5. Optimize hotspot

# 6. Repeat
```

## 📊 Tracking Performance

### Create benchmark log

```powershell
# Run baseline
$msft_time = (Measure-Command { 
    .\testdata\tools\IntuneWinAppUtil.exe -c testdata\packages\small -s setup.exe -o msft_out -q 
}).TotalSeconds

$rust_time = (Measure-Command { 
    .\target\release\intunewin-rs -c testdata\packages\small -s setup.exe -o rust_out -q 
}).TotalSeconds

# Calculate speedup
$speedup = $msft_time / $rust_time

Write-Host "MSFT: $msft_time seconds"
Write-Host "Rust: $rust_time seconds"
Write-Host "Speedup: ${speedup}x"
```

### Track progress

```
Phase 1 (MVP):
- Week 1: Basic CLI + file enum = 1.0x speed

Phase 2 (Parallel):
- Week 2: Rayon + chunking = 2.5x speed

Phase 3 (SIMD):
- Week 3: SHA256-NI + optimize = 5.0x speed

Phase 4 (Polish):
- Week 4: Edge cases + docs = 5-8x speed
```

## 🔐 Key Implementation Details

### Command-Line Interface

**Must support** (for MSFT compatibility):
```powershell
intunewin-rs -c <source_folder> -s <setup_file> -o <output_folder>
intunewin-rs -c src -s setup.exe -o out -a catalog -q -qq
intunewin-rs -h    # Show help
intunewin-rs -v    # Show version
```

**Extended features** (Rust-only):
```powershell
intunewin-rs -c src -s setup.exe -o out --threads 16 --compression 4 --chunk-size 256MiB
```

### File Format

**Output file structure** (must match MSFT):
```
setup.intunewin (ZIP)
├── IntuneWinPackage/Metadata/Detection.xml       [Encryption keys]
└── IntuneWinPackage/Contents/IntunePackage.intunewin [Encrypted ZIP]
```

**Encryption**:
- Algorithm: AES-256-CBC
- MAC: HMAC-SHA256
- Keys: Random, stored in Detection.xml

### Critical Crates

```toml
clap = "4.4"         # CLI parsing
zip = "0.6"          # ZIP creation
sha2 = "0.10"        # SHA-256 (SIMD support)
aes-gcm = "0.10"     # AES-256 encryption
rayon = "1.8"        # Parallelism
memmap2 = "0.9"      # Memory mapping
```

## 🐛 Common Issues & Fixes

### Build Error: "MSVC not found"
```powershell
# Install Visual Studio Build Tools
# Or check if C++ tools installed:
Get-Command cl.exe -ErrorAction SilentlyContinue
```

### Test Data Generation Slow
```powershell
# Use smaller size or SSD
.\tests\setup-test-environment.ps1 -DataSize small
```

### Profiling Permission Denied
```powershell
# Run as Administrator
# Or check Windows Performance Toolkit installed
```

### Output File Doesn't Match MSFT Format
```powershell
# Compare structure:
Add-Type -AssemblyName System.IO.Compression
$zip = [System.IO.Compression.ZipFile]::OpenRead('output.intunewin')
$zip.Entries | ForEach-Object { $_.FullName }
```

## 📈 Performance Milestones

```
Phase 1 MVP:
  ✓ Functional (1.0x MSFT)
  ✓ Small package: ~5s

Phase 2 Parallel:
  ✓ Multi-threaded (2.5x)
  ✓ Small package: ~2s
  ✓ Medium package (2.5GB): ~50s

Phase 3 Optimized:
  ✓ SIMD + tuning (5-8x)
  ✓ Small package: ~0.6s
  ✓ Medium package: ~20s
  ✓ Large package (20GB): ~150s

Phase 4 Production:
  ✓ Full featured
  ✓ Comprehensive tests
  ✓ Performance locked
```

## 🎯 Implementation Checklist

### Phase 1: MVP (Week 1)
- [ ] Initialize Cargo project
- [ ] Implement CLI parsing (-c, -s, -o, -a, -q, -qq)
- [ ] File enumeration (recursive directory walk)
- [ ] File hashing (SHA256)
- [ ] Create inner ZIP with files
- [ ] Implement AES-256 encryption
- [ ] Generate Detection.xml
- [ ] Create outer ZIP structure
- [ ] Test against MSFT tool output
- [ ] Benchmark: 1.0x MSFT speed target

### Phase 2: Parallelization (Week 2)
- [ ] Add Rayon for parallel compression
- [ ] Implement chunked processing (64MB)
- [ ] Add memory-mapped I/O
- [ ] Tune thread count
- [ ] Benchmark: 2.5x MSFT speed target
- [ ] Profile bottlenecks

### Phase 3: SIMD Optimization (Week 3)
- [ ] Implement SIMD SHA-256 (SHA-NI)
- [ ] Optimize compression settings
- [ ] Fine-tune buffer sizes
- [ ] Reduce memory allocations
- [ ] Benchmark: 5-8x MSFT speed target
- [ ] Profile and optimize

### Phase 4: Production (Week 4)
- [ ] Error handling & recovery
- [ ] Progress reporting
- [ ] Comprehensive testing
- [ ] Documentation & examples
- [ ] Release build optimization
- [ ] Final benchmarking

## 🔗 Useful Links

- Cargo Book: https://doc.rust-lang.org/cargo/
- Rust Performance: https://nnethercote.github.io/perf-book/
- Windows Performance Toolkit: https://learn.microsoft.com/en-us/windows-hardware/test/wpt/
- MSFT Win32 Prep Tool: https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool

## 📝 Notes

- Test data is reusable across runs (don't delete)
- Generated .intunewin files are encrypted (normal)
- Progress targets are cumulative (1→2→3→4)
- All commands assume `d:\projects\rIntuneWinApp` is CWD
- Admin required for profiling with xperf
