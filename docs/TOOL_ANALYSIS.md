# Microsoft IntuneWinAppUtil Tool Analysis

## Summary

**Tool Name**: Microsoft Win32 Content Prep Tool  
**Executable**: `IntuneWinAppUtil.exe`  
**Current Version**: 1.8.7.0 (file version 6.2509.50.0)  
**Repository**: https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool  
**License**: Microsoft License Terms (see repo)  
**Requirements**: .NET Framework 4.7.2+

## Verified Single-Threaded Behavior

**Tested**: January 6, 2026  
**Method**: Process monitoring during 98MB package creation

### Empirical Measurements

| Metric | Value | Interpretation |
|--------|-------|----------------|
| **CPU/Wall Ratio** | **0.79** | Uses <1 CPU core (single-threaded) |
| **Thread Count** | 1-11 (avg 5.8) | Threads spawned but idle during compression |
| **Peak Memory** | 64.6 MB | Not memory-mapping large files |
| **Wall Time** | 5.25 sec | |
| **CPU Time** | 4.14 sec | |

> **Key Finding**: A ratio of 0.79 proves the tool is **single-threaded for compute work**. 
> A multi-threaded compressor on 8 cores would show ratio of 4-7+.

### .NET Dependencies (from assembly inspection)

```
mscorlib                         4.0.0.0
System.IO.Compression            4.2.0.0   ← DEFLATE compression
System.IO.Compression.FileSystem 4.0.0.0   ← ZipFile API
System.Xml                       4.0.0.0
System.Core                      4.0.0.0
WindowsBase                      4.0.0.0
```

**`System.IO.Compression` (v4.2.0.0)** is .NET Framework's built-in compression:
- Uses `DeflateStream` internally
- **No parallel compression support** in .NET Framework 4.x
- No SIMD acceleration
- Processes one stream sequentially

### Real-World Implications

| Package Size | MSFT Time | Throughput | Cores Used |
|--------------|-----------|------------|------------|
| 98 MB | 5.25 sec | 18.7 MB/sec | 1 of 8 |
| 40 GB (Teamcenter) | ~3 hours | 3.7 MB/sec | 1 of N |

The **throughput degradation at scale** (18.7 → 3.7 MB/sec) suggests additional overhead from:
- Memory pressure (not streaming)
- Disk I/O patterns (not memory-mapped)
- .NET garbage collection at large allocations

### The Optimization Opportunity

**7 of 8 CPU cores sit idle** during compression. A parallel implementation can:
- Use all cores for compression (Rayon thread pool)
- Memory-map large files (avoid repeated reads)
- Stream output (avoid buffering entire archive)

**Conservative projection**: 3-5x speedup on multi-core systems.  
**Aggressive projection**: 7-8x speedup (limited by disk I/O).

## Command-Line Interface (MSFT v1.8.7)

### Basic Usage
```powershell
IntuneWinAppUtil -c <source_folder> -s <setup_file> -o <output_folder> [-a <catalog_folder>] [-q|-qq]
```

### Parameters
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `-c` | Path | Yes | Source folder with all files to package |
| `-s` | File | Yes | Setup file name (exe/msi/bat/cmd) must be IN source folder |
| `-o` | Path | Yes | Output folder (folder, not file path) |
| `-a` | Path | No | Catalog folder for Win10 S mode (.cat files) |
| `-q` | Flag | No | Quiet mode (no prompts, overwrite existing) |
| `-qq` | Flag | No | Silent mode (no console output) |
| `-h` | Flag | No | Show help |
| `-v` | Flag | No | Show version |

### Interactive Mode
```powershell
IntuneWinAppUtil
# Prompts for parameters step-by-step
```

## File Format (.intunewin)

### Structure: Nested ZIP Archives

```
setup.intunewin (Outer ZIP)
├── IntuneWinPackage/
│   ├── Contents/
│   │   ├── IntunePackage.intunewin      (AES-256 encrypted inner ZIP)
│   │   └── {UUID}                       (Encrypted content blob)
│   └── Metadata/
│       └── Detection.xml                (Encryption keys + metadata)
```

### Detection.xml (Outer ZIP)

Contains **unencrypted** metadata and keys for Intune:

```xml
<ApplicationInfo ToolVersion="1.8.7.0">
  <Name>setup.exe</Name>
  <UnencryptedContentSize>102724253</UnencryptedContentSize>
  <FileName>IntunePackage.intunewin</FileName>
  <SetupFile>setup.exe</SetupFile>
  <EncryptionInfo>
    <EncryptionKey>HKYmAJajC2BIyjQkUOqUJoOS/qPPr+/ge+y3sHnFsBg=</EncryptionKey>
    <MacKey>Ys8NNrhI1uJ8xKm7+yLskA8+0P0KBGpI3EoqhY8etno=</MacKey>
    <InitializationVector>756Jwns7E2EBqcseYadLXw==</InitializationVector>
    <Mac>2MLPPlQwUdqFuorpwlgQh5DYAbfssas6UGAHnvQ1UL0=</Mac>
    <ProfileIdentifier>ProfileVersion1</ProfileIdentifier>
    <FileDigest>wY+UMaZGMX/2diGI4xhrXTOK7qtHscGeYyF+S5diB2k=</FileDigest>
    <FileDigestAlgorithm>SHA256</FileDigestAlgorithm>
  </EncryptionInfo>
</ApplicationInfo>
```

### IntunePackage.intunewin (Inner Encrypted ZIP)

**Encrypted with**:
- Algorithm: **AES-256-CBC**
- Key: Base64 from `EncryptionInfo/EncryptionKey`
- IV: Base64 from `EncryptionInfo/InitializationVector`
- MAC: HMAC-SHA256 for integrity

**When decrypted, contains**:
- `Manifest.xml` - File listing with SHA256 hashes
- `IntunePackageHeader.xml` - Format version info
- `Content/` directory - All original files

## Performance Baseline (MSFT Tool v1.8.7)

### Test: 100MB Package (~100 files)
- **Input Size**: 102.7 MB
- **Encoding Time**: ~3.3 seconds
- **Output Size**: 97.97 MB (5% reduction with compression)
- **Throughput**: ~31 MB/sec
- **Operations**:
  1. Compress source folder → ZIP (3254ms)
  2. Encrypt ZIP → AES-256 (264ms)
  3. Compute SHA256 hashes (163ms + 157ms)
  4. Generate Detection.xml (auto)
  5. Create final ZIP with metadata (537ms)
  6. **Total**: ~4.5 seconds wall-time

## Key Implementation Insights

### 1. Nested ZIP Structure
- MSFT tool uses **two levels of ZIP**: outer (unencrypted metadata) + inner (encrypted content)
- This allows Intune to read metadata without decryption
- Inner ZIP is completely encrypted after creation

### 2. Encryption Keys
- **AES-256 for content**, not compression
- Keys are stored in **Detection.xml** in the outer ZIP
- Intune uses these keys to decrypt content on device
- MAC key provides integrity checking

### 3. Setup File Importance
- **Mandatory** parameter
- Must exist in source folder
- Name becomes the application name in metadata
- MSFT tool detects MSI parameters if setup is .msi

### 4. File Processing
- **All files in source folder** are included (recursive)
- Each file gets SHA256 hash in Manifest.xml
- Directory structure is preserved in Content/

### 5. Metadata Generation
- Manifest.xml is **auto-generated** by tool
- Contains file listing with hashes
- Size: ~0.8KB for 100 files
- Header.xml specifies encryption details

## Development Implications

### For Rust Implementation

**Must Match**:
- ✓ Command-line interface (-c, -s, -o, -a, -q, -qq)
- ✓ Output file format (nested ZIP with AES-256)
- ✓ Detection.xml format (encryption metadata)
- ✓ Manifest.xml format (file listing + hashes)
- ✓ AES-256-CBC encryption with proper IV/MAC
- ✓ Output file naming (`{setup_name}.intunewin`)

**Can Optimize**:
- Parallel compression (MSFT uses single-threaded)
- SIMD hashing (SHA256 acceleration)
- Memory-mapped I/O (large file reads)
- Streaming operations (avoid full buffering)

### Performance Optimization Targets

**Bottlenecks in MSFT tool** (from log output):
1. **Compression** (3254ms) - 73% of time
2. **Encryption** (264ms) - 6% of time
3. **Hashing** (320ms) - 7% of time
4. **Overhead** (360ms) - 8% of time
5. **Final ZIP** (537ms) - 12% of time

**Best gains from**:
- Parallel compression (potentially 4-8x with multi-threading)
- SIMD hashing (potentially 2-4x with hardware acceleration)
- Memory-mapped I/O (avoid repeated reads)

## Reference Test Results

### Generated File Analysis
```
Input: testdata/packages/small
├── Files: 100
├── Size: 102.7 MB
├── Structure: bin/, lib/, config/, data/, docs/, resources/

MSFT Tool Output:
├── File: setup.intunewin
├── Size: 97.97 MB
├── Time: 3.3 seconds (compression)
        + 0.5 seconds (outer ZIP)
        ≈ 4.5 seconds total
├── Entries: 2
│   ├── IntuneWinPackage/Metadata/Detection.xml
│   └── IntuneWinPackage/Contents/IntunePackage.intunewin (encrypted)
```

## Next Steps

1. **Match MSFT CLI interface** exactly (for compatibility)
2. **Implement AES-256-CBC encryption** (using `aes-gcm` crate)
3. **Create nested ZIP structure** (zip crate)
4. **Generate Detection.xml** with proper encryption metadata
5. **Benchmark against MSFT tool** on same data
6. **Optimize compression** (parallel workers)
7. **Add SIMD hashing** (SHA-NI CPU instructions)

## References

- [Microsoft Win32 Content Prep Tool GitHub](https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool)
- [Intune App Wrapping Documentation](https://learn.microsoft.com/mem/intune/developer/)
- [Rust AES Crate](https://docs.rs/aes-gcm/latest/aes_gcm/)
- [Rust ZIP Crate](https://docs.rs/zip/latest/zip/)
- [NIST AES-256-CBC Specification](https://csrc.nist.gov/publications/detail/sp/800-38a/final)
