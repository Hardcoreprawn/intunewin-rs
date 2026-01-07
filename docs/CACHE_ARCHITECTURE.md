# Cache Architecture & Design

## Overview

The intunewin-rs caching system provides incremental compilation support for repeated packaging runs. It stores pre-compressed file data to avoid re-compressing unchanged files, offering **2-3x speedup** on subsequent builds when using compression levels 1-9.

**Philosophy**: Cache is a performance optimization for repeated builds. For first-time builds or when speed is critical, use `--compression 0` (store-only, default).

---

## Architecture

### Per-File Streaming Cache (Current Design)

The cache uses **per-file storage** instead of a monolithic binary blob:

```
.intunewin-cache/
├── manifest.json           # Metadata for all cached files
└── files/                  # Individual compressed file storage (named <16-hex-digits>.cache)
    ├── <hash1>.cache       # Compressed data + header
    ├── <hash2>.cache
    └── ...
```

**Key Benefits:**

- **Memory Efficiency**: Only loads files needed for current build
- **Large Package Support**: Handles 10GB+ without memory exhaustion
- **Robust Error Handling**: Corrupted single file doesn't invalidate entire cache
- **Granular Invalidation**: Only re-compress changed files

### Manifest File (`manifest.json`)

Stores metadata about all cached files:

```json
{
  "version": 1,
  "compression_level": 6,
  "entries": [
    {
      "path": "installer/setup.exe",
      "original_hash": "abc123...",
      "original_size": 102400000,
      "compressed_size": 98765432,
      "cache_key": "abc123...".substring(0, 16),
      "mtime": 1704067200,
      "is_cached": true
    }
  ]
}
```

**Fields:**

- `version`: Manifest format version (1 = current)
- `compression_level`: Compression level when cache was created
- `entries`: Array of file metadata
  - `path`: Relative path in source folder
  - `original_hash`: SHA-256 of uncompressed file
  - `original_size`: Size in bytes (uncompressed)
  - `compressed_size`: Size in bytes (compressed)
  - `cache_key`: First 16 hex chars of original_hash (used for filename)
  - `mtime`: Modification time (used for invalidation)
  - `is_cached`: Whether file data is stored in `files/` dir (false for files >500MB)

### Cached File Format

Each `.cache` file in `files/` directory:

```text
[10-byte header]
┌─────────────────────────────────────┐
│ CRC-32 (4 bytes)                    │
│ Original size (4 bytes, big-endian) │
│ Compression method (2 bytes)        │
│ [Compressed data...]                │
└─────────────────────────────────────┘
```

**Header Fields:**

- **CRC-32** (4 bytes): Integrity check of compressed data
- **Original Size** (4 bytes, big-endian): Uncompressed file size
- **Compression Method** (2 bytes, big-endian):
  - `0x0000` = STORE (uncompressed)
  - `0x0008` = DEFLATE
  - `0x000C` = Reserved for future

This format allows verification without decompressing entire file.

---

## Cache Lifecycle

### 1. Cache Check (Manifest Lookup)

During pipeline startup, cache is checked for each source file:

```
For each file in source directory:
  1. Calculate SHA-256 hash
  2. Look up in manifest.json
  3. Verify:
     - File size matches
     - Modification time matches
     - Compression level matches
  4. If all checks pass: Mark as "cacheable"
     If any check fails: Mark as "needs recompression"
```

**Memory**: Only manifest loaded (~1-10KB per file), actual compressed data not loaded yet.

### 2. Selective Compression

During compression stage, only non-cached files are processed:

```
For each file:
  If is_cached and hash matches:
    Load from cache/files/<hash>.compressed
  Else:
    Compress using DEFLATE algorithm
    Record in manifest
    Save to cache/files/<hash>.compressed
```

**Memory**: Lazy loading - each cached file loaded only when accessed.

### 3. Cache Invalidation

Cache is automatically cleared when:

- **Compression level changes**: Entire manifest cleared (all files must be recompressed)
- **File modified**: Removed from manifest (hash doesn't match)
- **File deleted**: Removed from manifest
- **File added**: Added to manifest if recompressed

**Manual invalidation:**

```bash
# Clear all cached data before building
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6 --clear-cache

# Disable caching for this build
intunewin-rs -c ./app -s setup.exe -o ./output --compression 6 --no-cache
```

### 4. Cache Saving

After successful compression stage, cache is saved:

```
For each newly compressed file:
  1. Write to cache/files/<hash>.cache
  2. Update manifest.json with metadata
  3. Call fsync() to ensure durability
```

This ensures that even if packaging fails later (encryption error, output I/O error), the cache is preserved for next run.

---

## Smart Cache Defaults

### Auto-Enable/Disable Logic

The cache is automatically managed based on compression level:

```rust
pub fn use_cache(&self) -> bool {
    if self.no_cache {
        // Explicit --no-cache always wins
        false
    } else if self.cache {
        // Explicit --cache
        true
    } else {
        // Auto-enable when compression > 0 (beneficial)
        // Auto-disable when compression = 0 (adds overhead)
        self.compression > 0
    }
}
```

**Rationale:**

| Compression | Caching Benefit | Auto Setting |
|:-----------:|:---------------:|:------------|
| 0 (STORE) | 0% | Disabled (no recompression saved) |
| 1-6 | 2-3x | Enabled (worth the overhead) |
| 7-9 | 2-3x | Enabled (worth the overhead) |

### Why Disabled for Compression 0?

With STORE mode (no compression), every file is simply copied. Cache adds overhead:

- Manifest I/O overhead: ~5-10ms
- Cache validation overhead: ~20-50ms
- Total overhead: ~50-100ms

This overhead outweighs any benefit since files aren't being recompressed anyway.

---

## Size Limits & Performance

### Per-File Size Limit

Files larger than `MAX_CACHED_FILE_SIZE` (500MB) are **not cached**:

```rust
const MAX_CACHED_FILE_SIZE: u64 = 500 * 1024 * 1024; // 500 MB
```

**Rationale:**

- Prevents single huge files from dominating cache storage
- Avoids loading multi-GB compressed data into memory
- These files are infrequently modified anyway

**Manifest Behavior**: Even if not cached, file metadata is recorded:

```json
{
  "path": "large-install.iso",
  "original_hash": "def456...",
  "is_cached": false,  // ← File data not stored
  "reason": "exceeds_size_limit"
}
```

### Cache Storage Expectations

For typical corporate LOB applications:

| Package Size | Estimated Cache Size | Compression |
|:-------------|:--------------------:|:------------|
| 100 MB | 5-10 MB | 6 |
| 500 MB | 20-50 MB | 6 |
| 1 GB | 40-100 MB | 6 |
| 3.5 GB | 100-300 MB | 6 |

**Note:** Size reduction depends on input files. Pre-compressed installers (.exe, .msi, .cab) compress minimally (1-2%).

---

## Performance Characteristics

### Cold Cache (First Run)

```
Time = File Enumeration + Hashing + Compression + Encryption + Packaging
     = ~500ms + ~1s + 6.5s + 0.5s + 0.2s = ~8.7s (for 254MB package with compression 6)
```

### Warm Cache (Subsequent Run, No Changes)

```
Time = File Enumeration + Hashing + Cache Lookup + Cache Load + Encryption + Packaging
     = ~500ms + ~1s + ~200ms + ~300ms + 0.5s + 0.2s = ~2.7s (3.2x faster!)
```

**Speedup Sources:**

- Compression eliminated: 6.5s → 0s (85% of improvement)
- Cache load faster than compression: 300ms vs 6.5s (21x faster)

### Large Package Behavior (3.5 GB)

With `--compression 0` (STORE, default):

- No recompression needed → cache provides no benefit
- Cache validation adds 50-100ms overhead
- Result: Disabled by default

With `--compression 6` on 3.5 GB (not recommended):

- Memory pressure during compression phase
- Streaming cache loads per-file → manageable
- Still completes, but slower than baseline

**Recommendation**: Use `--compression 0` for packages >500MB.

---

## Error Handling

### Corrupted Cache File

If a cached file is corrupted:

```
During cache load:
  Try to load cache/files/<hash>.compressed
  If read fails or CRC check fails:
    Log warning: "Cache file corrupted, skipping"
    Recompress from source file
    Overwrite corrupted cache file
  Continue pipeline
```

**Result**: Graceful degradation. User doesn't know about corruption; build succeeds.

### Corrupted Manifest

If manifest.json is corrupted:

```
During manifest load:
  Try to parse manifest.json
  If parse fails:
    Log warning: "Cache manifest corrupted"
    Clear entire cache directory
    Proceed with cold cache (no hits)
  Continue pipeline
```

**Result**: Recoverable. Next build will rebuild manifest.

### Stale Cache

If cache becomes stale (e.g., user modified compression level):

```
During cache validation:
  Read manifest.compression_level
  If compression_level != current_compression:
    Clear entire manifest
    Mark all files as "needs recompression"
  Continue pipeline
```

**Result**: Automatic invalidation. No manual intervention needed.

---

## Implementation Details

### Key Code Components

**Cache manifest file**: `src/cache/manifest.rs`

- Serialization/deserialization of manifest.json
- Validation logic
- Per-file entry tracking

**Cache manager**: `src/cache/mod.rs`

- `load_data_cache()`: Manifest loading
- `load_cached_file(path)`: Lazy load individual compressed file
- `save_data_cache()`: Save manifest + per-file data
- `check()`: Validate cache hits
- `record()`: Add new entry to manifest
- `clear()`: Invalidate cache

**Pipeline integration**: `src/pipeline/compression.rs`

- `partition()`: Separate cached vs non-cached files
- `compress_to_inner_zip_cached()`: Process compression with cache awareness
- Cache saving after compression phase

### Dependencies

```toml
sha2 = "0.10"       # SHA-256 hashing
serde = "1.0"       # JSON serialization
flate2 = "1.0"      # Compression (DEFLATE, STORE)
```

---

## Testing

### Test Coverage

```bash
# Run cache-specific tests
cargo test cache --lib

# Benchmark cache behavior
.\testdata\benchmarks\benchmark-cache.ps1
```

### Test Scenarios

1. **Empty cache**: First run should create manifest + cache files
2. **Cache hit**: Unchanged files loaded from cache
3. **Cache invalidation**: Modified file recompressed
4. **Size limit**: Files >500MB not cached but still packaged
5. **Compression level change**: Cache cleared when level changes
6. **Corrupted cache**: Graceful fallback to recompression
7. **Large packages**: 3.5GB+ processes without memory exhaustion

---

## Future Improvements

### Potential Optimizations

1. **Adaptive cache size limits**: Based on available disk space
2. **Compression level detection**: Recommend best level for input
3. **Multi-level caching**: In-memory + disk cache hybrid
4. **Cache statistics**: Detailed hit rate and speedup reporting
5. **Incremental manifest updates**: Faster cache validation for large manifests
6. **Distributed caching**: Share cache across build machines (CI/CD)

### Backward Compatibility

- Cache format uses version field for future migrations
- Current version: 1
- Version mismatch triggers full cache clear and rebuild
- No data loss; just slower first build after format change

---

## Summary

The per-file streaming cache architecture enables:

✅ **Performance**: 2-3x speedup for repeated builds with compression  
✅ **Memory Efficiency**: Handles 10GB+ packages without exhaustion  
✅ **Reliability**: Graceful error handling for corrupted files  
✅ **Smart Defaults**: Auto-enabled only when beneficial (compression > 0)  
✅ **Transparency**: Cache behavior automatic, no manual tuning needed  

For most use cases, you don't need to think about caching—it works transparently. For large packages (>500MB), use the default `--compression 0` (STORE) for maximum speed.
