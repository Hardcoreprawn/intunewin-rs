# IntuneWin Package Encoder Specification

## 1. Executive Summary

This document specifies the interface, format, and requirements for a high-performance alternative to Microsoft's **Win32 Content Prep Tool** (`IntuneWinAppUtil.exe` v1.8.7+). The goal is to create a Rust-based encoder that can package Windows Line-of-Business (LOB) applications into the IntuneWin format faster than the MSFT tool, with particular focus on large packages (200GiB+).

**Reference Tool**: https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool

---

## 2. Command-Line Interface

### 2.1 Primary Usage (Compatibility Mode)

Match Microsoft's Win32 Content Prep Tool interface:

```text
intunewin-rs -c <source_folder> -s <setup_file> -o <output_folder> [OPTIONS]
```

### 2.2 Required Arguments

| Argument | Type | Description | Reference |
|----------|------|-------------|-----------|
| `-c` | Path | Source folder containing all files to package | Same as MSFT tool |
| `-s` | Filename | Setup file name (e.g., "setup.exe" or "setup.msi") | Same as MSFT tool |
| `-o` | Path | Output folder (folder, not file) | Same as MSFT tool |

### 2.3 Optional Arguments

| Argument | Type | Default | Description | Reference |
|----------|------|---------|-------------|-----------|
| `-a` | Path | (none) | Catalog folder for Win10 S mode files | Same as MSFT tool |
| `-q` | Flag | false | Quiet mode (no interactive prompts) | Same as MSFT tool |
| `-qq` | Flag | false | Silent mode (no console output) | Same as MSFT tool |
| `--threads` | Integer | Auto (CPU count) | Parallel workers for compression (NEW) | Extension |
| `--compression` | Level | 6 | Compression level 1-9 (NEW) | Extension |
| `--chunk-size` | Size | 64MiB | Chunk size for parallel processing (NEW) | Extension |
| `--temp-dir` | Path | System temp | Temporary working directory (NEW) | Extension |
| `--skip-validation` | Flag | false | Skip integrity checks (benchmarking only) | Extension |
| `-h` | Flag | N/A | Show help | Same as MSFT tool |
| `-v` | Flag | N/A | Show tool version | Same as MSFT tool |

### 2.4 Exit Codes

```text
0    = Success
1    = General error
2    = Input validation failed
3    = I/O error (read/write)
4    = Encoding/compression error
5    = Output file conflict
```

---

## 3. Input Specification

### 3.1 Input Requirements

- **Type**: Directory containing files to be packaged
- **Max Size**: Tested up to 200GiB+
- **File Types**: Any (no filtering)
- **Restrictions**:
  - Must be readable by current user
  - No symbolic links (or resolve them)
  - No open file locks (Windows-specific issue)

### 3.2 Setup File Requirements

- **Must be provided** via `-s` parameter (like MSFT tool)
- Can be: `.exe`, `.msi`, `.bat`, `.cmd`, or any executable
- Must exist in the source folder (`-c`)
- Tool uses setup file to detect install parameters for MSI files

### 3.3 Catalog Folder (Optional, Win10 S Mode)

- If `-a` parameter provided, catalog files are included
- Catalog files are security catalogs (`.cat` files)
- All `.cat` files in catalog folder are bundled

### 3.4 File Enumeration

- Recursively traverse input directory
- Calculate total size of all files
- Build manifest with file paths relative to input root
- Support long paths (>260 chars on Windows via `\\?\` prefix)
- Process ALL files in source folder

---

## 4. IntuneWin Binary Format Specification (MSFT v1.8.7)

### 4.1 High-Level Structure

**Important**: The `.intunewin` file is a **nested ZIP archive**, NOT a flat ZIP:

```
[Outer ZIP: setup.intunewin]
├── IntuneWinPackage/
│   ├── Contents/
│   │   ├── IntunePackage.intunewin    [AES-256 encrypted ZIP]
│   │   └── [UUID]                     [Encrypted content blob]
│   └── Metadata/
│       └── Detection.xml              [Setup info + encryption keys]
```

### 4.2 Outer ZIP Files

#### 4.2.1 Detection.xml

**Location**: `IntuneWinPackage/Metadata/Detection.xml`

Contains application metadata and encryption keys for Intune:

```xml
<?xml version="1.0" encoding="utf-8"?>
<ApplicationInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema" 
                 xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" 
                 ToolVersion="1.8.7.0">
  <Name>{setup_file_name}</Name>
  <UnencryptedContentSize>{size_bytes}</UnencryptedContentSize>
  <FileName>IntunePackage.intunewin</FileName>
  <SetupFile>{setup_file_name}</SetupFile>
  <EncryptionInfo>
    <EncryptionKey>{base64_AES256_key}</EncryptionKey>
    <MacKey>{base64_HMAC_key}</MacKey>
    <InitializationVector>{base64_IV}</InitializationVector>
    <Mac>{base64_HMAC_SHA256}</Mac>
    <ProfileIdentifier>ProfileVersion1</ProfileIdentifier>
    <FileDigest>{base64_SHA256}</FileDigest>
    <FileDigestAlgorithm>SHA256</FileDigestAlgorithm>
  </EncryptionInfo>
</ApplicationInfo>
```

**Encryption Details**:
- **Algorithm**: AES-256-CBC
- **Key Size**: 256 bits (32 bytes, base64 encoded)
- **IV**: 128 bits (16 bytes, base64 encoded)  
- **MAC**: HMAC-SHA256 for integrity
- **Digest**: SHA256 hash of encrypted content

#### 4.2.2 IntunePackage.intunewin (Encrypted Inner ZIP)

**Location**: `IntuneWinPackage/Contents/IntunePackage.intunewin`

This is the **encrypted** inner ZIP file. When decrypted using the keys from Detection.xml, it contains:

```
[Decrypted Inner ZIP Structure]
├── [Manifest.xml]           - App metadata (auto-generated if not provided)
├── [IntunePackageHeader.xml]- Package format version info
└── [Content/]
    ├── [setup.exe or setup.msi]
    ├── [file1.bin]
    ├── [file2.bin]
    └── [all subdirectories with original files]
```

**Before Encryption**: Standard ZIP with DEFLATE compression
**After Encryption**: Binary blob stored as single entry

#### 4.2.3 Optional Files

**Catalog Files** (if `-a` parameter used):
- Win10 S mode security catalogs (`.cat` files)
- Bundled with encrypted content

### 4.3 Inner ZIP: Manifest.xml Format

**Location** (after decryption): Root of inner ZIP

```xml
<?xml version="1.0" encoding="utf-8"?>
<AppPackageManifest xmlns="http://schemas.microsoft.com/intune/applicationManifest/v1">
  <ApplicationName>{setup_file_name}</ApplicationName>
  <Version>1.0.0.0</Version>
  <Publisher></Publisher>
  <IntuneWindowsPackageId>{UUID}</IntuneWindowsPackageId>
  <PublishedDate>{ISO8601_timestamp}</PublishedDate>
  <Description></Description>
  <SetupFile>{setup_file_name}</SetupFile>
  <IncludedFiles>
    <File Name="{relative_path}" Size="{bytes}">
      <Hash Algorithm="SHA256">{hex_digest}</Hash>
    </File>
    <!-- ... repeated for each file ... -->
  </IncludedFiles>
</AppPackageManifest>
```

### 4.4 Inner ZIP: IntunePackageHeader.xml Format

**Location** (after decryption): Root of inner ZIP

```xml
<?xml version="1.0" encoding="utf-8"?>
<IntunePackageHeader xmlns="http://schemas.microsoft.com/intune/packageHeader/v1">
  <PackageFormat>IntuneWindowsPackage</PackageFormat>
  <MinimumRequiredCompanyPortalVersion>5.0.0</MinimumRequiredCompanyPortalVersion>
  <PackageType>LOB</PackageType>
  <EncryptionRequired>true</EncryptionRequired>
  <ContentEncryption>
    <Algorithm>AES256</Algorithm>
    <KeySize>256</KeySize>
  </ContentEncryption>
</IntunePackageHeader>
```
    <File Name="{relative_path}" Size="{bytes}">
      <Hash Algorithm="SHA256">{hex_digest}</Hash>
    </File>
    <!-- repeated for each file -->
  </IncludedFiles>
</AppPackageManifest>
```

#### 4.2.3 IntunePackageHeader.xml

Located at: `IntunePackageHeader.xml` (root of ZIP)

```xml
<?xml version="1.0" encoding="utf-8"?>
<IntunePackageHeader xmlns="http://schemas.microsoft.com/intune/packageHeader/v1">
  <PackageFormat>IntuneWindowsPackage</PackageFormat>
  <MinimumRequiredCompanyPortalVersion>5.0.0</MinimumRequiredCompanyPortalVersion>
  <PackageType>LOB</PackageType>
  <EncryptionRequired>false</EncryptionRequired>
  <ContentEncryption>
    <Algorithm>None</Algorithm>
  </ContentEncryption>
</IntunePackageHeader>
```

#### 4.2.4 Files.meta

Located at: `Files.meta` (root of ZIP)

Binary or text format listing all files with:

- Relative path
- Size
- CRC32 or SHA256
- Modification timestamp

#### 4.2.5 Content Directory

All application files are stored under `/Content/` in the ZIP, maintaining the original directory structure:

```text
Content/
├── setup.exe
├── app/
│   ├── bin/
│   │   ├── library.dll
│   │   └── config.ini
│   └── readme.txt
└── licenses/
    └── LICENSE.txt
```

---

## 5. Algorithm & Processing Strategy

### 5.1 High-Level Flow

```text
1. Validate inputs
2. Enumerate files and calculate hashes
3. Create ZIP archive in streaming mode
4. Parallel compress chunks
5. Stream compressed data to output
6. Write metadata files
7. Finalize ZIP (central directory)
8. Verify output integrity
```

### 5.2 Optimization Strategies

#### 5.2.1 Parallel Chunked Compression

For large files (>100MiB):

- Split into independent chunks
- Compress each chunk in parallel threads
- Write to temporary buffer
- Assemble in correct order

This works because DEFLATE (ZIP's default) can compress independently.

#### 5.2.2 Memory-Mapped I/O

For reading large source files:

- Use OS-level memory mapping (`mmap` on Unix, `MapViewOfFile` on Windows)
- Reduces syscall overhead
- Lets OS handle paging efficiently

#### 5.2.3 Streaming ZIP Writing

- Don't build entire ZIP in memory
- Write entries sequentially to output file
- Calculate CRCs on-the-fly
- Write ZIP central directory at end

#### 5.2.4 Hash Calculation

- Calculate SHA256/CRC32 during file read (not separate pass)
- Use SIMD-accelerated hashing if available (SHA-NI on modern CPUs)
- Parallel hashing for metadata generation

### 5.3 Buffering Strategy

```text
Worker Thread 1          Worker Thread 2          Worker Thread N
     |                        |                         |
  Read chunk 1             Read chunk 2           Read chunk N
     |                        |                         |
  Compress (parallel)  Compress (parallel)      Compress (parallel)
     |                        |                         |
  Buffer pool 1           Buffer pool 2          Buffer pool N
     |__________________________________________________|
                          |
                    ZIP output stream
                    (sequential write)
```

---

## 6. Performance Targets

### 6.1 Baseline Metrics

Comparison target: Microsoft's `intunewinapputil.exe`

| Metric | Target | Notes |
|--------|--------|-------|
| 200GiB package | 3-5x faster | With parallelism + optimization |
| 50GiB package | 2-3x faster | Parallelism helps less on smaller data |
| Small files (<1GiB) | 1.5-2x faster | Lower overhead, good parallelization |
| Memory usage | <2GiB peak | Rust + buffer pools vs .NET GC |

### 6.2 Bottleneck Analysis

When optimizing, measure in this order of impact:

1. **Disk I/O** (40-50% of time typically)
   - Solution: Parallel reads, buffer pools, large read chunks

2. **Compression** (30-40% of time)
   - Solution: Parallel compression, SIMD, tune compression level

3. **Hashing/Verification** (10-15% of time)
   - Solution: SIMD hashing, calculate during read pass

4. **ZIP overhead** (5-10%)
   - Solution: Streaming write, minimal copying

---

## 7. Output Specification

### 7.1 Output File

- **Format**: ZIP archive with `.intunewin` extension
- **Size**: Similar to input (with compression overhead of 5-15%)
- **Compatibility**: Must be readable by Microsoft Intune service and Company Portal app

### 7.2 Verification

After creation, verify:

- ✓ File is valid ZIP
- ✓ Manifest.xml is well-formed XML
- ✓ All files in manifest exist in Content/
- ✓ All files in Content/ are in manifest
- ✓ Hashes match (optional, can skip with `--skip-validation`)

---

## 8. Metadata Requirements

### 8.1 Manifest Fields (Mandatory)

```text
ApplicationName       - Name of the app
Version              - Version string
Publisher            - Publisher/Company name
SetupFile            - Main executable filename
PublishedDate        - ISO 8601 timestamp
IncludedFiles        - List of all files with SHA256 hashes
```

### 8.2 Manifest Fields (Optional)

```text
Description          - Human-readable description
IntuneWindowsPackageId - UUID for tracking (generate if not provided)
MinimumOS           - Minimum Windows version
MinimumRAM          - Minimum RAM requirement
Architecture        - x86, x64, ARM64
```

### 8.3 UUID Generation

If `IntuneWindowsPackageId` not provided, generate deterministically:

```text
SHA256(ApplicationName + PublishedDate) -> first 16 bytes as UUID
```

Or use true UUID v4 random generation.

---

## 9. Error Handling

### 9.1 Graceful Failure Scenarios

| Scenario | Behavior |
|----------|----------|
| Input folder doesn't exist | Error + exit(2) |
| Output path not writable | Error + exit(3) |
| Insufficient disk space | Error + exit(3), cleanup temp files |
| File disappears during encoding | Log warning, skip file, note in manifest |
| Corrupted ZIP write | Detect, error + exit(4) |
| Hash mismatch on verification | Warn, optionally fail |

### 9.2 Logging

- Always log start/end timestamps
- Log warnings for skipped files
- Log performance metrics (throughput, parallelism efficiency)
- Optional: per-file progress (verbose mode)

---

## 10. Configuration & Management

### 10.1 Configuration File (Optional)

Support optional TOML config file:

```toml
[default]
compression_level = 6
chunk_size = "64MiB"
threads = 8
temp_dir = "/tmp"

[profiles]
fast = { compression_level = 1, threads = 16 }
maximum = { compression_level = 9, threads = 4 }
small_files = { compression_level = 9, chunk_size = "4MiB" }
```

Usage: `intunewin-rs --profile maximum --input ... --output ...`

### 10.2 Performance Tuning

Expose these knobs for advanced users:

- `--chunk-size`: Larger = better for huge files, worse for many small files
- `--threads`: More = faster on multi-core, but overhead on contention
- `--compression`: Lower = faster (try 1-4 for 200GiB packages)
- `--skip-validation`: Skip final integrity check (dangerous but fast)

---

## 11. Project Structure (Rust)

```text
intunewin-rs/
├── Cargo.toml
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── cli/
│   │   └── args.rs            # Argument parsing
│   ├── encoder/
│   │   ├── mod.rs
│   │   ├── manifest.rs        # Manifest XML generation
│   │   ├── header.rs          # IntunePackageHeader XML
│   │   ├── zip_writer.rs      # Streaming ZIP writer
│   │   └── compressor.rs      # Parallel compression
│   ├── hasher/
│   │   └── parallel_sha256.rs # SIMD SHA256 hashing
│   ├── io/
│   │   ├── file_enumerator.rs # Recursive file listing
│   │   ├── buffer_pool.rs     # Memory-pooled buffers
│   │   └── mmap.rs            # Memory-mapped file reading
│   └── utils/
│       ├── progress.rs        # Progress reporting
│       └── verification.rs    # Output validation
├── tests/
│   ├── integration_tests.rs   # End-to-end tests
│   └── fixtures/              # Test data
└── README.md
```

---

## 12. Dependencies (Recommended Crates)

### Core

- `clap`: CLI argument parsing (with derive macros)
- `zip`: ZIP archive creation
- `sha2`: SHA256 hashing (SIMD support)
- `rayon`: Data parallelism for chunks
- `crossbeam`: Thread-safe queue for work distribution

### Performance

- `memmap2`: Memory-mapped file I/O
- `zstd` or `flate2`: Compression algorithms (flate2 for DEFLATE compatibility)
- `bytemuck`: Zero-copy data access

### Utilities

- `anyhow` or `eyre`: Error handling
- `log` + `env_logger`: Logging
- `indicatif`: Progress bars
- `chrono`: Timestamp handling
- `uuid`: UUID generation
- `xml-rs`: XML parsing/generation

---

## 13. Testing Strategy

### 13.1 Unit Tests

- Manifest XML generation (valid XML, all required fields)
- Header XML generation
- File hashing (known test vectors)
- Path normalization (Windows-specific edge cases)

### 13.2 Integration Tests

- End-to-end: create intunewin, verify it's valid ZIP
- Comparison: encode same input with MSFT tool, compare structure
- Large files: test with 50GiB+ packages
- Edge cases: symlinks, long paths, special characters, permissions

### 13.3 Benchmarks

```rust
// Benchmark different chunk sizes, thread counts, compression levels
// Target: Compare perf vs intunewinapputil.exe
```

---

## 14. Compatibility Notes

### 14.1 MSFT Tool Compatibility

- Output must be readable by Company Portal app (Windows 10+)
- Must work with Intune Management Service
- Manifest XML schema must match (or be superset of) MSFT's

### 14.2 Windows-Specific

- Handle UNC paths (`\\?\C:\...`)
- Respect file locking (skip files that can't be read)
- Handle alternate data streams (or document exclusion)
- Case-insensitive path handling

---

## 15. Future Enhancements

1. **Incremental updates**: Only re-package changed files
2. **Cloud streaming**: Stream directly to Azure blob storage
3. **Signature verification**: Sign the .intunewin with certificates
4. **Custom compression**: Support zstd, Brotli as alternatives to DEFLATE
5. **Parallel ZIP writes**: Use rayon to write multiple ZIP entries in parallel
6. **Caching**: Store file hashes locally to skip unchanged files
7. **Network optimization**: Compress on-the-fly for network transfers

---

## 16. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-01 | Initial specification |
| TBD | Future | Real-time compression, incremental updates |

---

## Appendix A: Sample Manifest.xml

```xml
<?xml version="1.0" encoding="utf-8"?>
<AppPackageManifest xmlns="http://schemas.microsoft.com/intune/applicationManifest/v1">
  <ApplicationName>My Corporate App</ApplicationName>
  <Version>2.1.0.0</Version>
  <Publisher>Acme Corp</Publisher>
  <IntuneWindowsPackageId>550e8400-e29b-41d4-a716-446655440000</IntuneWindowsPackageId>
  <PublishedDate>2026-01-06T10:30:00Z</PublishedDate>
  <Description>Enterprise application for internal use</Description>
  <SetupFile>setup.exe</SetupFile>
  <IncludedFiles>
    <File Name="setup.exe" Size="5242880">
      <Hash Algorithm="SHA256">e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855</Hash>
    </File>
    <File Name="app/config.ini" Size="1024">
      <Hash Algorithm="SHA256">e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855</Hash>
    </File>
  </IncludedFiles>
</AppPackageManifest>
```

---

## Appendix B: Command Examples

```bash
# Basic usage
intunewin-rs -i C:\MyApp -o MyApp.intunewin

# With optimization for 200GiB package
intunewin-rs \
  -i D:\LargePackage \
  -o LargePackage.intunewin \
  --compression 4 \
  --threads 16 \
  --chunk-size 256MiB \
  --skip-validation

# With metadata
intunewin-rs \
  -i C:\MyApp \
  -o MyApp.intunewin \
  --publisher "Acme Corp" \
  --version "2.1.0.0" \
  --description "Line-of-business app"

# Profile-based (from config file)
intunewin-rs --profile maximum -i input -o output
```

---

## References

- [Microsoft Intune App Wrapping Tool](https://github.com/Microsoft/Intune-App-Wrapping-Tool-Windows)
- [ZIP File Format Specification](https://pkware.com/appnote)
- [Intune App SDK Documentation](https://learn.microsoft.com/en-us/intune/developer)
- Rust crates: zip, sha2, rayon, clap
