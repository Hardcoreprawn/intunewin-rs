# IntuneWin-RS Development Plan

**Purpose**: Drop-in replacement for Microsoft's IntuneWinAppUtil with parallel performance.  
**Target**: 3-5x faster on 4-8 cores, 5-10x faster on 16+ cores.  
**Constraint**: Byte-compatible output with MSFT tool (Intune must accept our packages).

---

## Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CLI Layer (clap)                               │
│  Parse args → Validate paths → Configure pipeline → Report progress         │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Pipeline Orchestrator                             │
│  Coordinates: Discovery → Compression → Encryption → Packaging              │
└─────────────────────────────────────────────────────────────────────────────┘
          │                    │                    │                    │
          ▼                    ▼                    ▼                    ▼
┌─────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌────────────┐
│  Discovery  │    │   Compression   │    │   Encryption    │    │  Packager  │
│             │    │                 │    │                 │    │            │
│ • Walk dirs │    │ • Parallel ZIP  │    │ • AES-256-CBC   │    │ • Outer ZIP│
│ • Stat files│    │ • Chunk streams │    │ • HMAC-SHA256   │    │ • Detection│
│ • Plan work │    │ • DEFLATE       │    │ • Key gen       │    │ • Metadata │
└─────────────┘    └─────────────────┘    └─────────────────┘    └────────────┘
                            │
                            ▼
              ┌───────────────────────────┐
              │   Parallel Worker Pool    │
              │   (Rayon thread pool)     │
              │                           │
              │  ┌─────┐ ┌─────┐ ┌─────┐  │
              │  │ CPU │ │ CPU │ │ CPU │  │
              │  │  1  │ │  2  │ │ ... │  │
              │  └─────┘ └─────┘ └─────┘  │
              └───────────────────────────┘
```

---

## Module Structure

```text
src/
├── main.rs              # Entry point, CLI setup
├── cli.rs               # Argument parsing (clap)
├── pipeline/
│   ├── mod.rs           # Pipeline orchestration
│   ├── discovery.rs     # File system walking
│   ├── compression.rs   # Parallel ZIP creation
│   ├── encryption.rs    # AES-256-CBC + HMAC
│   └── packager.rs      # Final .intunewin assembly
├── format/
│   ├── mod.rs           # Format types
│   ├── detection.rs     # Detection.xml generation
│   ├── manifest.rs      # Manifest.xml generation
│   └── header.rs        # Header.xml generation
├── crypto/
│   ├── mod.rs           # Crypto primitives
│   ├── aes.rs           # AES-256-CBC encrypt/decrypt
│   ├── hmac.rs          # HMAC-SHA256
│   └── keygen.rs        # Random key generation
├── io/
│   ├── mod.rs           # I/O abstractions
│   ├── mmap.rs          # Memory-mapped file reading
│   └── streaming.rs     # Streaming write support
├── progress.rs          # Progress reporting
└── error.rs             # Error types
```

---

## Phase 1: Foundation (CLI + Basic Pipeline)

### 1.1 Project Setup

**File**: `Cargo.toml`

```toml
[package]
name = "intunewin-rs"
version = "0.1.0"
edition = "2021"
description = "High-performance IntuneWin packager"
license = "MIT"

[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Compression
zip = { version = "0.6", default-features = false, features = ["deflate"] }
flate2 = "1.0"

# Cryptography
aes = "0.8"
cbc = "0.1"
hmac = "0.12"
sha2 = "0.10"
rand = "0.8"

# Parallelism  
rayon = "1.8"

# I/O
memmap2 = "0.9"
walkdir = "2"

# XML
quick-xml = { version = "0.31", features = ["serialize"] }

# Progress
indicatif = "0.17"

# Error handling
thiserror = "1"
anyhow = "1"

# Base64 encoding
base64 = "0.21"

# UUID generation
uuid = { version = "1", features = ["v4"] }

[profile.release]
lto = true
codegen-units = 1
opt-level = 3
strip = true
```

### 1.2 CLI Interface

**File**: `src/cli.rs`

Must match MSFT tool exactly:

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "intunewin-rs")]
#[command(about = "High-performance IntuneWin packager")]
#[command(version)]
pub struct Args {
    /// Source folder containing content to package
    #[arg(short = 'c', long = "content")]
    pub content_folder: PathBuf,

    /// Setup file (exe, msi, bat, cmd, ps1) - must be in content folder
    #[arg(short = 's', long = "setup")]
    pub setup_file: String,

    /// Output folder for .intunewin file
    #[arg(short = 'o', long = "output")]
    pub output_folder: PathBuf,

    /// Catalog folder for Win10 S mode (optional)
    #[arg(short = 'a', long = "catalog")]
    pub catalog_folder: Option<PathBuf>,

    /// Quiet mode - no prompts, overwrite existing
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Silent mode - no console output
    #[arg(long = "qq")]
    pub silent: bool,

    // === Performance tuning (our extensions) ===
    
    /// Number of compression threads (default: CPU count)
    #[arg(long = "threads", short = 't')]
    pub threads: Option<usize>,

    /// Compression level 1-9 (default: 6)
    #[arg(long = "compression", default_value = "6")]
    pub compression_level: u32,

    /// Disable memory-mapped I/O
    #[arg(long = "no-mmap")]
    pub no_mmap: bool,
}
```

### 1.3 Entry Point

**File**: `src/main.rs`

```rust
mod cli;
mod pipeline;
mod format;
mod crypto;
mod io;
mod progress;
mod error;

use anyhow::Result;
use clap::Parser;
use cli::Args;

fn main() -> Result<()> {
    let args = Args::parse();
    
    // Configure thread pool
    if let Some(threads) = args.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()?;
    }
    
    // Run pipeline
    pipeline::run(&args)?;
    
    Ok(())
}
```

---

## Phase 2: Core Pipeline

### 2.1 Discovery Module

**File**: `src/pipeline/discovery.rs`

**Purpose**: Walk source directory, collect file metadata, plan work distribution.

**Key Types**:

```rust
pub struct FileEntry {
    pub relative_path: PathBuf,  // Path within archive
    pub absolute_path: PathBuf,  // Path on disk
    pub size: u64,               // File size in bytes
    pub is_setup_file: bool,     // Is this the setup file?
}

pub struct DiscoveryResult {
    pub files: Vec<FileEntry>,
    pub total_size: u64,
    pub file_count: usize,
    pub setup_file: FileEntry,
}
```

**Algorithm**:

1. `walkdir::WalkDir` to enumerate all files recursively
2. `rayon::par_iter` to stat files in parallel
3. Sort files by size descending (large files first for better parallelism)
4. Validate setup file exists

### 2.2 Compression Module (CRITICAL PATH)

**File**: `src/pipeline/compression.rs`

**Purpose**: Create inner ZIP with parallel compression.

**Strategy**: Parallel file compression, sequential archive assembly.

```rust
/// Compress all files into inner ZIP
pub fn compress_to_zip(
    files: &[FileEntry],
    output: &Path,
    compression_level: u32,
    use_mmap: bool,
    progress: &ProgressTracker,
) -> Result<PathBuf>
```

**Parallel Compression Strategy**:

```
Phase 1: Parallel Compression (CPU-bound, parallelized)
┌─────────────────────────────────────────────────────────────┐
│  File 1 ──→ [DEFLATE on Thread 1] ──→ Compressed Buffer 1   │
│  File 2 ──→ [DEFLATE on Thread 2] ──→ Compressed Buffer 2   │
│  File 3 ──→ [DEFLATE on Thread 3] ──→ Compressed Buffer 3   │
│  ...                                                        │
│  File N ──→ [DEFLATE on Thread N] ──→ Compressed Buffer N   │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
Phase 2: Sequential Assembly (I/O-bound, single-threaded)
┌─────────────────────────────────────────────────────────────┐
│  ZIP Header + Buffer 1 + Buffer 2 + ... + Central Directory │
└─────────────────────────────────────────────────────────────┘
```

**Implementation Approach**:

```rust
use rayon::prelude::*;
use flate2::Compression;
use flate2::write::DeflateEncoder;

/// Pre-compressed file data ready for ZIP assembly
struct CompressedFile {
    relative_path: PathBuf,
    compressed_data: Vec<u8>,
    uncompressed_size: u64,
    crc32: u32,
}

/// Compress files in parallel
fn compress_files_parallel(
    files: &[FileEntry],
    compression_level: u32,
    use_mmap: bool,
) -> Result<Vec<CompressedFile>> {
    files.par_iter()
        .map(|file| compress_single_file(file, compression_level, use_mmap))
        .collect()
}

/// Compress a single file (runs on worker thread)
fn compress_single_file(
    file: &FileEntry,
    compression_level: u32,
    use_mmap: bool,
) -> Result<CompressedFile> {
    // Read file (memory-mapped for large files)
    let data = if use_mmap && file.size > 1_000_000 {
        read_mmap(&file.absolute_path)?
    } else {
        std::fs::read(&file.absolute_path)?
    };
    
    // Calculate CRC32 (needed for ZIP)
    let crc32 = crc32fast::hash(&data);
    
    // Compress with DEFLATE
    let mut encoder = DeflateEncoder::new(
        Vec::new(),
        Compression::new(compression_level),
    );
    encoder.write_all(&data)?;
    let compressed = encoder.finish()?;
    
    Ok(CompressedFile {
        relative_path: file.relative_path.clone(),
        compressed_data: compressed,
        uncompressed_size: file.size,
        crc32,
    })
}
```

**Memory Management for Large Files**:

For 40GB packages, we can't hold everything in memory. Strategy:

```rust
/// For very large packages, use chunked processing
const CHUNK_THRESHOLD: u64 = 2 * 1024 * 1024 * 1024; // 2GB

fn compress_large_package(files: &[FileEntry]) -> Result<PathBuf> {
    // Sort files by size
    let (large_files, small_files): (Vec<_>, Vec<_>) = 
        files.iter().partition(|f| f.size > 100_000_000); // 100MB
    
    // Process large files sequentially with streaming
    // Process small files in parallel batches
    // Write directly to temp file to avoid memory pressure
}
```

### 2.3 Encryption Module

**File**: `src/crypto/aes.rs`

**Purpose**: AES-256-CBC encryption matching MSFT format.

```rust
use aes::Aes256;
use cbc::{Cipher, Encryptor};
use rand::RngCore;

pub struct EncryptionResult {
    pub encrypted_data: Vec<u8>,
    pub key: [u8; 32],        // AES-256 key
    pub iv: [u8; 16],         // Initialization vector
    pub mac_key: [u8; 32],    // HMAC key
    pub mac: [u8; 32],        // HMAC-SHA256 of encrypted data
}

/// Encrypt data with AES-256-CBC
pub fn encrypt_aes256_cbc(plaintext: &[u8]) -> Result<EncryptionResult> {
    // Generate random keys
    let mut key = [0u8; 32];
    let mut iv = [0u8; 16];
    let mut mac_key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    rand::thread_rng().fill_bytes(&mut iv);
    rand::thread_rng().fill_bytes(&mut mac_key);
    
    // Encrypt with PKCS7 padding
    let cipher = Encryptor::<Aes256>::new(&key.into(), &iv.into());
    let encrypted = cipher.encrypt_padded_vec::<Pkcs7>(plaintext);
    
    // Calculate HMAC
    let mac = calculate_hmac(&mac_key, &encrypted);
    
    Ok(EncryptionResult {
        encrypted_data: encrypted,
        key,
        iv,
        mac_key,
        mac,
    })
}
```

**Streaming Encryption for Large Files**:

```rust
/// Stream-encrypt large file without loading into memory
pub fn encrypt_file_streaming(
    input: &Path,
    output: &Path,
) -> Result<EncryptionKeys> {
    // Process in 64KB blocks
    const BLOCK_SIZE: usize = 64 * 1024;
    
    // AES-CBC encrypts block-by-block
    // Each block's output becomes next block's IV
}
```

### 2.4 Packager Module

**File**: `src/pipeline/packager.rs`

**Purpose**: Assemble final .intunewin (outer ZIP).

**Output Structure**:

```
{setup_name}.intunewin
└── IntuneWinPackage/
    ├── Contents/
    │   └── IntunePackage.intunewin  (encrypted inner ZIP)
    └── Metadata/
        └── Detection.xml            (encryption keys + metadata)
```

```rust
pub fn create_intunewin(
    encrypted_content: &Path,    // Encrypted inner ZIP
    encryption: &EncryptionResult,
    setup_name: &str,
    unencrypted_size: u64,
    output_folder: &Path,
) -> Result<PathBuf> {
    let output_path = output_folder.join(format!("{}.intunewin", setup_name));
    
    let file = File::create(&output_path)?;
    let mut zip = ZipWriter::new(file);
    
    // Add Detection.xml (uncompressed for Intune to read)
    let detection_xml = generate_detection_xml(
        setup_name,
        unencrypted_size,
        encryption,
    )?;
    zip.start_file("IntuneWinPackage/Metadata/Detection.xml", options)?;
    zip.write_all(detection_xml.as_bytes())?;
    
    // Add encrypted content
    zip.start_file("IntuneWinPackage/Contents/IntunePackage.intunewin", options)?;
    std::io::copy(&mut File::open(encrypted_content)?, &mut zip)?;
    
    zip.finish()?;
    Ok(output_path)
}
```

---

## Phase 3: XML Format Generation

### 3.1 Detection.xml

**File**: `src/format/detection.rs`

Must match MSFT format exactly:

```xml
<ApplicationInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema" 
                 xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" 
                 ToolVersion="1.8.7.0">
  <Name>setup.exe</Name>
  <UnencryptedContentSize>102724253</UnencryptedContentSize>
  <FileName>IntunePackage.intunewin</FileName>
  <SetupFile>setup.exe</SetupFile>
  <EncryptionInfo>
    <EncryptionKey>{base64_key}</EncryptionKey>
    <MacKey>{base64_mac_key}</MacKey>
    <InitializationVector>{base64_iv}</InitializationVector>
    <Mac>{base64_mac}</Mac>
    <ProfileIdentifier>ProfileVersion1</ProfileIdentifier>
    <FileDigest>{base64_sha256_of_encrypted}</FileDigest>
    <FileDigestAlgorithm>SHA256</FileDigestAlgorithm>
  </EncryptionInfo>
</ApplicationInfo>
```

```rust
use quick_xml::se::to_string;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename = "ApplicationInfo")]
pub struct ApplicationInfo {
    #[serde(rename = "@ToolVersion")]
    pub tool_version: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "UnencryptedContentSize")]
    pub unencrypted_content_size: u64,
    #[serde(rename = "FileName")]
    pub file_name: String,
    #[serde(rename = "SetupFile")]
    pub setup_file: String,
    #[serde(rename = "EncryptionInfo")]
    pub encryption_info: EncryptionInfo,
}

pub fn generate_detection_xml(
    setup_name: &str,
    unencrypted_size: u64,
    encryption: &EncryptionResult,
) -> Result<String> {
    let info = ApplicationInfo {
        tool_version: "1.8.7.0".to_string(),  // Match MSFT version
        name: setup_name.to_string(),
        // ... fill other fields
    };
    
    let xml = to_string(&info)?;
    Ok(add_xml_declaration(xml))
}
```

### 3.2 Manifest.xml (Inner ZIP)

**File**: `src/format/manifest.rs`

File listing with SHA256 hashes:

```rust
pub fn generate_manifest_xml(files: &[FileEntry]) -> Result<String> {
    // Generate parallel hashes
    let hashes: Vec<_> = files.par_iter()
        .map(|f| (f.relative_path.clone(), sha256_file(&f.absolute_path)))
        .collect();
    
    // Build XML
}
```

---

## Phase 4: Performance Optimizations

### 4.1 Memory-Mapped I/O

**File**: `src/io/mmap.rs`

For large files, avoid copying data:

```rust
use memmap2::Mmap;

pub fn read_mmap(path: &Path) -> Result<Mmap> {
    let file = File::open(path)?;
    unsafe { Mmap::map(&file) }
}
```

### 4.2 Progress Reporting

**File**: `src/progress.rs`

Show progress for long operations:

```rust
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};

pub struct ProgressTracker {
    multi: MultiProgress,
    overall: ProgressBar,
    current_file: ProgressBar,
}

impl ProgressTracker {
    pub fn new(total_bytes: u64, quiet: bool, silent: bool) -> Self {
        if silent {
            return Self::null();
        }
        // Create progress bars
    }
    
    pub fn file_started(&self, name: &str, size: u64) { }
    pub fn bytes_processed(&self, bytes: u64) { }
    pub fn file_completed(&self) { }
}
```

### 4.3 Thread Pool Tuning

```rust
/// Configure optimal thread count based on workload
pub fn configure_thread_pool(file_count: usize, total_size: u64) -> usize {
    let cpus = num_cpus::get();
    
    // For many small files: use all CPUs
    // For few large files: limit to avoid memory pressure
    let size_factor = if total_size > 10_000_000_000 { // 10GB
        cpus.min(8)  // Cap at 8 for huge packages
    } else {
        cpus
    };
    
    // Don't create more threads than files
    size_factor.min(file_count).max(1)
}
```

---

## Phase 5: Testing & Validation

### 5.1 Output Compatibility Tests

```rust
#[cfg(test)]
mod tests {
    /// Verify our output can be read by MSFT tool patterns
    #[test]
    fn test_output_structure() {
        // Create package
        // Verify ZIP structure matches expected
        // Verify Detection.xml schema
        // Verify encryption can be decrypted
    }
    
    /// Compare our output with MSFT tool output
    #[test]
    fn test_msft_compatibility() {
        // Package same content with both tools
        // Compare Detection.xml format
        // Verify Intune would accept both
    }
}
```

### 5.2 Performance Benchmarks

```rust
#[bench]
fn bench_100mb_package(b: &mut Bencher) {
    b.iter(|| {
        package("testdata/packages/small", "setup.exe", "testdata/output")
    });
}

#[bench]
fn bench_parallel_vs_sequential(b: &mut Bencher) {
    // Compare with threads=1 vs threads=N
}
```

---

## Implementation Order for Agents

### Sprint 1: Skeleton (Day 1)

1. [ ] `Cargo.toml` with all dependencies
2. [ ] `src/main.rs` entry point
3. [ ] `src/cli.rs` argument parsing
4. [ ] `src/error.rs` error types
5. [ ] Basic build verification

### Sprint 2: Discovery + Single-Threaded Compression (Days 2-3)

1. [ ] `src/pipeline/discovery.rs` - file walking
2. [ ] `src/pipeline/compression.rs` - single-threaded ZIP creation
3. [ ] `src/format/manifest.rs` - file listing XML
4. [ ] End-to-end test: create valid ZIP

### Sprint 3: Encryption (Days 4-5)

1. [ ] `src/crypto/keygen.rs` - random key generation
2. [ ] `src/crypto/aes.rs` - AES-256-CBC encryption
3. [ ] `src/crypto/hmac.rs` - HMAC-SHA256
4. [ ] `src/format/detection.rs` - Detection.xml with keys
5. [ ] End-to-end test: create encrypted inner ZIP

### Sprint 4: Final Assembly (Day 6)

1. [ ] `src/pipeline/packager.rs` - outer ZIP creation
2. [ ] Full pipeline integration
3. [ ] Output compatibility test with MSFT format

### Sprint 5: Parallelization (Days 7-8)

1. [ ] Parallel file compression with Rayon
2. [ ] Memory-mapped I/O for large files
3. [ ] Progress reporting
4. [ ] Thread pool tuning

### Sprint 6: Polish (Days 9-10)

1. [ ] Error messages and user feedback
2. [ ] Edge cases (empty folders, special characters)
3. [ ] Benchmark suite
4. [ ] Documentation

---

## Success Criteria

### Functional

- [ ] Output accepted by Intune (upload + install works)
- [ ] CLI compatible with MSFT tool (drop-in replacement)
- [ ] Handles packages up to 100GB
- [ ] Handles special characters in filenames
- [ ] Handles empty directories

### Performance

- [ ] 3x faster than MSFT tool on 4-core machine
- [ ] 5x faster than MSFT tool on 8-core machine  
- [ ] 8x faster than MSFT tool on 16-core machine
- [ ] Memory usage < 2GB for 40GB packages (streaming)

### Quality

- [ ] No unsafe code except mmap (audited)
- [ ] All errors have actionable messages
- [ ] Progress bar for operations > 10 seconds
- [ ] Clean shutdown on Ctrl+C

---

## Future Extensions (Not in Scope Now)

These are explicitly deferred:

1. **Direct Intune Upload**: Stream package directly to Graph API
2. **Caching**: Skip unchanged files on re-package
3. **Differential Updates**: Only upload changed content
4. **Decompression**: Extract/inspect existing .intunewin files
5. **GUI**: Drag-and-drop interface
6. **Watch Mode**: Auto-repackage on file changes

---

## Reference: MSFT Tool Behavior to Match

```powershell
# These must all work identically:
intunewin-rs -c .\source -s setup.exe -o .\output
intunewin-rs -c .\source -s setup.exe -o .\output -q
intunewin-rs -c .\source -s setup.exe -o .\output -qq
intunewin-rs -c "C:\path with spaces\source" -s "My Setup.exe" -o .\output

# Output file naming:
# Input: -s setup.exe → Output: setup.intunewin
# Input: -s "My App.msi" → Output: My App.intunewin
```
