# Test Environment Setup Complete

## ✅ What's Ready

### Directory Structure
```
rIntuneWinApp/
├── testdata/
│   ├── packages/
│   │   ├── small/          (100 files, ~100MB)  ✓ Generated
│   │   ├── medium/         (ready for generation)
│   │   ├── large/          (ready for generation)
│   │   └── xlarge/         (ready for generation)
│   ├── tools/              (for intunewinapputil.exe)
│   ├── output/             (for .intunewin output files)
│   ├── benchmarks/         (benchmark scripts & results)
│   └── README.md
├── tests/
│   ├── setup-test-environment.ps1
│   └── download-intunewinapputil.ps1
├── SPECIFICATION.md        (Format & interface spec)
└── BUILD_AND_TEST.md       (Build & profiling guide)
```

### Test Data Generated
- **Small Package**: 100 synthetic files (~100MB)
  - Realistic directory structure (bin/, lib/, config/, data/, docs/, resources/)
  - Includes setup.exe stub (required by Intune format)
  - Ready for immediate testing

### Scripts Created

| Script | Purpose |
|--------|---------|
| `setup-test-environment.ps1` | Generate test packages of various sizes |
| `download-intunewinapputil.ps1` | Download MSFT tool (manual fallback available) |
| `testdata/benchmarks/benchmark.ps1` | Compare Rust vs MSFT implementations |

### Documentation
- **SPECIFICATION.md** (16KB) - Complete technical spec
  - CLI interface
  - IntuneWin binary format
  - Optimization strategies
  - Performance targets
  
- **BUILD_AND_TEST.md** - Development workflow
  - Build instructions
  - Profiling with Windows Performance Analyzer
  - Benchmarking strategy
  - Troubleshooting

---

## 📥 Getting intunewinapputil.exe

The official Microsoft repo appears archived. Options:

### Option 1: Download from Microsoft Learn
```
https://learn.microsoft.com/en-us/mem/intune/developer/
```
Download the "Intune App Wrapping Tool for Windows" and place at:
```
.\testdata\tools\intunewinapputil.exe
```

### Option 2: Check Local Installation
If you have Windows ADK/SDK installed:
```powershell
Get-ChildItem "C:\Program Files\Windows Kits\*\bin\*\intunewinapputil.exe" -Recurse
```

### Option 3: Alternative Approach
For testing purposes, you can:
1. Start with the Rust implementation (even MVP version)
2. Compare output structure with published specs
3. Profile/benchmark as we go

---

## 🚀 Next Steps (In Order)

### 1. Generate More Test Data (5 min)
```powershell
# Medium package (~2.5GB) - recommended for development
.\tests\setup-test-environment.ps1 -DataSize medium

# Or large package (~20GB) for serious benchmarking
.\tests\setup-test-environment.ps1 -DataSize large
```

### 2. Obtain Baseline (MSFT Tool)
```powershell
# Once you place intunewinapputil.exe:
.\testdata\benchmarks\benchmark.ps1 -PackageSize small
.\testdata\benchmarks\benchmark.ps1 -PackageSize medium

# Record baseline timings for comparison
```

### 3. Initialize Rust Project
```powershell
cd d:\projects\rIntuneWinApp
cargo init --name intunewin-rs
```

### 4. Implement MVP (Week 1)
- [ ] CLI argument parsing (clap)
- [ ] File enumeration
- [ ] Manifest XML generation
- [ ] Basic streaming ZIP writer
- [ ] SHA256 hashing
- [ ] Single-threaded compression (DEFLATE)

### 5. Performance Optimization (Week 2-3)
- [ ] Parallel compression (Rayon)
- [ ] Memory-mapped I/O (memmap2)
- [ ] SIMD hashing
- [ ] Buffer pooling
- [ ] Benchmark against MSFT tool

### 6. Profiling (Ongoing)
```powershell
# Windows Performance Analyzer (built-in)
xperf -on Base+DiskIO+FileIO+Memory+ProcessCounter
cargo run --release -- -i testdata\packages\small -o test.intunewin
xperf -d trace.etl
xperfview trace.etl

# Or use Flamegraph
cargo flamegraph --release
```

---

## 📊 Benchmarking Setup

### Small Package (100MB) - Recommended for Development
- **Generation**: <5 seconds
- **Encoding (MSFT)**: ~5-10 seconds
- **Perfect for quick iterations**

### Medium Package (2.5GB) - Good for Daily Testing
- **Generation**: ~2-5 minutes
- **Encoding (MSFT)**: ~120-180 seconds
- **Realistic performance testing**

### Large Package (20GB) - Full Performance Testing
- **Generation**: ~15-30 minutes
- **Encoding (MSFT)**: ~900-1200 seconds
- **Stress testing & optimization focus**

### XLarge Package (100GB) - Theoretical Target
- **Generation**: ~90+ minutes
- **Your target: 3-5x faster than MSFT**

---

## 📋 Performance Targets

### Phase 1: MVP (Basic Functionality)
- Small: 1x MSFT speed (parity)
- Memory: <500MB
- Output: Valid .intunewin files

### Phase 2: Parallel + SIMD (Next 2 weeks)
- Small: 1.5-2x faster
- Medium: 1.5-2x faster
- Memory: <1.5GB

### Phase 3: Optimized (Weeks 3-4)
- Small: 2x faster
- Medium: 3-4x faster
- Large: 5-8x faster
- Memory: <2GB peak

---

## 🔧 Tools & Technologies Stack

### Core Rust Crates
```toml
clap = "4.4"           # CLI parsing
zip = "0.6"            # ZIP creation
sha2 = "0.10"          # SHA256 (SIMD support)
rayon = "1.8"          # Parallelism
memmap2 = "0.9"        # Memory mapping
zstd = "0.13"          # Compression
anyhow = "1.0"         # Error handling
log = "0.4"            # Logging
indicatif = "0.17"     # Progress bars
```

### Profiling Tools
- **Windows Performance Analyzer** (built-in)
- **Cargo flamegraph** (visualization)
- **Task Manager** (real-time monitoring)

### Testing
- Unit tests (cargo test)
- Integration tests (end-to-end)
- Benchmarks (criterion)

---

## 📝 Important Notes

### About the MSFT Tool
- **Not fully documented** - reverse-engineered format from specs
- **May have undocumented features** - compare outputs carefully
- **Version-specific** - different versions may have quirks

### Testing Strategy
1. **Small packages first** - verify correctness
2. **Medium packages** - measure performance
3. **Large packages** - stress test & optimize
4. **Output validation** - ensure Intune compatibility

### Performance Expectations
- **Disk I/O dominates** (40-50% of time)
- **Compression second** (30-40%)
- **Parallelism helps most** on 10GB+ packages
- **Diminishing returns** after 8-16 threads

---

## ⚡ Quick Commands Reference

```powershell
# Setup
.\tests\setup-test-environment.ps1 -DataSize small

# Generate medium package
.\tests\setup-test-environment.ps1 -DataSize medium

# Benchmark MSFT tool
.\testdata\benchmarks\benchmark.ps1 -PackageSize small

# Future: Build Rust version
cargo build --release
.\target\release\intunewin-rs -i .\testdata\packages\small -o out.intunewin

# Profile
xperf -on Base+DiskIO+FileIO+Memory+ProcessCounter
# ... run command ...
xperf -d trace.etl
xperfview trace.etl
```

---

## 📞 Troubleshooting

### Test data generation slow?
- Use SSD (not USB drive)
- Use smaller size (-DataSize small)
- Close other applications

### intunewinapputil.exe not found?
- Check Microsoft Learn docs (link above)
- Or skip until you have Rust MVP ready
- Compare output format instead

### Want to start coding now?
- All infrastructure ready
- Run: `cargo init --name intunewin-rs`
- Follow BUILD_AND_TEST.md phases

---

## Summary

✅ **Infrastructure**: Complete
✅ **Test data**: Ready (100MB small package generated)
✅ **Documentation**: Comprehensive
✅ **Benchmarking**: Scripts ready
⏳ **Next**: Generate baseline with MSFT tool, then start Rust implementation

Ready to initialize the Rust project and start development!
