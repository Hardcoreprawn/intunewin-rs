//! Cache manifest structures for tracking file state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cache manifest containing all tracked files and statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheManifest {
    /// Version of the cache format
    pub version: u32,
    /// Compression level used when creating the cache
    pub compression_level: u32,
    /// Entries indexed by relative path
    pub entries: HashMap<String, CacheEntry>,
    /// Cache usage statistics
    pub stats: CacheStats,
}

/// A single cached file entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Relative path from content folder
    pub relative_path: String,
    /// File size in bytes
    pub size: u64,
    /// Modification time (Unix timestamp)
    pub mtime: u64,
    /// CRC32 of original file content
    pub crc32: u32,
    /// Size after compression
    pub compressed_size: u64,
    /// Compression method (0 = STORED, 8 = DEFLATE)
    pub compression_method: u16,
}

/// Statistics about cache usage.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of cache hits across all builds
    pub cache_hits: usize,
    /// Number of cache misses across all builds
    pub cache_misses: usize,
    /// Bytes saved by using cache
    pub bytes_saved: u64,
    /// Total number of builds using this cache
    pub total_builds: usize,
}

impl CacheManifest {
    /// Creates a new empty cache manifest with default compression level.
    pub fn new() -> Self {
        Self::with_compression_level(0)
    }

    /// Creates a new cache manifest with the specified compression level.
    pub fn with_compression_level(compression_level: u32) -> Self {
        Self {
            version: 1,
            compression_level,
            entries: HashMap::new(),
            stats: CacheStats::default(),
        }
    }
}

impl Default for CacheManifest {
    fn default() -> Self {
        Self::new()
    }
}
