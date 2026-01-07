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
| README.md | Project overview, quick start, smart defaults | 10 min |
| SMART_DEFAULTS.md | How automatic compression selection works | 15 min |
| SPECIFICATION.md | Technical specification, format details | 30 min |
| CACHE_ARCHITECTURE.md | Cache design, per-file streaming, performance | 20 min |
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

## 🎯 Flag Selection Guide

### Quick Decision Tree

**"What should I use?"**

```
Is this a first-time build?
  → Use defaults: intunewin-rs -c ./app -s setup.exe -o ./output
  → Compression auto-detected based on package size

Is this a repeated build (same package)?
  → Use --compression 6 --cache (if <500MB: beneficial speedup)
  → Or stay with --compression 0 (if >500MB: fastest, cache disabled)

Is package >500MB?
  → Use --compression 0 (store-only, maximum speed)
  → Cache disabled automatically (no benefit from compression)

Is package very large (>3GB)?
  → MUST use --compression 0
  → Only option that prevents memory exhaustion

Are you running in CI/CD (building multiple times)?
  → Use --compression 6 --cache (2-3x faster on 2nd+ builds)
  → First build slower (cold cache), but subsequent builds very fast
```

### Flag Compatibility

All flags remain **100% compatible** with Microsoft's IntuneWinAppUtil:

```bash
# Microsoft compatible flags (these work exactly like MSFT tool):
-c <source_folder>        # Content folder (REQUIRED)
-s <setup_file>           # Setup file name (REQUIRED)
-o <output_folder>        # Output folder (REQUIRED)
-a <catalog_folder>       # Catalog folder (optional)
-q                        # Quiet mode
--qq                      # Silent mode
-h                        # Help
-V                        # Version

# intunewin-rs extensions (new, don't break Microsoft compatibility):
--compression <0-9>       # Compression level (default: smart detection)
-t <threads>              # Thread count (default: auto)
--cache                   # Force enable caching
--no-cache                # Force disable caching
--no-mmap                 # Disable memory-mapped I/O
--cache-stats             # Show cache statistics
--clear-cache             # Clear cache before building
```

### Recommended Configurations

#### Scenario 1: One-Time Build (Fastest)

```powershell
# No compression, no cache overhead
# Best for: Single package creation, scripts, automation
# Speed: ~7.9s for 3.5GB
intunewin-rs -c "C:\app\source" -s "setup.exe" -o "C:\app\output" --compression 0 -q
```

#### Scenario 2: Development/Iteration (Repeated Builds)

```powershell
# Compression 6 + cache: 2-3x faster on builds 2+
# Best for: Developers, testing, CI/CD pipelines
# First build: 6.5s | Subsequent: 2.2s
intunewin-rs -c "C:\app\source" -s "setup.exe" -o "C:\app\output" --compression 6 --cache -q
```

#### Scenario 3: Large Enterprise Package (>500MB)

```powershell
# Store-only mode: no memory pressure
# Best for: Enterprise installs, 200+ MB packages
# Speed: ~8-15s regardless of size, stable memory
intunewin-rs -c "C:\large\source" -s "setup.exe" -o "C:\output" --compression 0 -q
```

#### Scenario 4: Network Drive / Slow Storage

```powershell
# Disable memory-mapped I/O, use compression 6
# Best for: Network paths, slow NAS/SAN, remote shares
# Note: Slightly slower, but avoids I/O issues
intunewin-rs -c "\\server\share\app" -s "setup.exe" -o "\\server\output" --compression 6 --no-mmap -q
```

#### Scenario 5: CI/CD Pipeline (Repeated Builds, Different Machines)

```powershell
# Build without cache (each machine different), but compression
# Best for: Azure Pipelines, GitHub Actions, Jenkins
# Each build independent, compression for size in artifact
intunewin-rs -c $(Build.SourcesDirectory)/app -s "setup.exe" -o $(Build.ArtifactStagingDirectory) --compression 6 --no-cache -q
```

#### Scenario 6: Pre-compressed Installer (No Benefit from DEFLATE)

```powershell
# Store-only: 0% size reduction, waste time not compressing
# Best for: .msi files, already-compressed .exe
# Speed: 0.56s vs 0.86s for compression 6 (35% faster)
intunewin-rs -c "C:\installer" -s "setup.msi" -o "C:\output" --compression 0 -q
```

---

## ⚙️ Default Behavior Explanation

When you run without explicit `--compression` flag:

```powershell
intunewin-rs -c ./app -s setup.exe -o ./output
```

The tool **automatically selects** the best compression:

```
Input size < 500MB → Use compression 6 (good speedup with caching)
Input size ≥ 500MB → Use compression 0 (maximum speed, no memory issues)
```

**Cache is automatically managed:**
```
Compression 0 → Cache disabled (no benefit from reusing uncompressed data)
Compression 6+ → Cache enabled (2-3x speedup on subsequent builds)
```

You can override with explicit flags:
```powershell
--compression 0          # Force store-only
--compression 6          # Force compression 6
--cache                  # Force enable cache
--no-cache               # Force disable cache
```

---

## 🚀 Performance by Scenario

| Scenario | Command | First Build | 2nd Build | Notes |
|:---------|:--------|:-----------:|:---------:|:------|
| Small package (98 MB) | `default` | 0.86s | 0.86s | No cache benefit (comp 6 adds overhead) |
| Small package, CI/CD | `--compression 6 --cache` | 0.86s | **0.28s** | 3.0x faster on repeat builds |
| Large package (3.5 GB) | `default` | **7.9s** | 7.9s | Auto-disables cache, no compression |
| Large package, decompress | `--compression 0` | **7.9s** | 7.9s | Fastest, stable memory, no cache noise |
| Very large (10+ GB) | `--compression 0` | ~20s | ~20s | Only stable option |

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
