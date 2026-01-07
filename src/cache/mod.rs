//! Incremental caching module for faster subsequent builds.
//!
//! This module implements a caching strategy that tracks file metadata and
//! compressed data to avoid redundant work on subsequent packaging runs.
//!
//! # Cache Strategy
//!
//! The cache stores:
//! - File metadata (path, size, modification time)
//! - Pre-compressed file data (already deflated/stored)
//! - CRC32 checksums
//!
//! On subsequent runs:
//! 1. Load existing cache manifest
//! 2. Compare file metadata to detect changes
//! 3. Only compress files that are new or modified
//! 4. Reuse cached compressed data for unchanged files
//!
//! # Cache Location
//!
//! The cache is stored in the output directory as `.intunewin-cache/`

mod manifest;

pub use manifest::{CacheEntry, CacheManifest, CacheStats};

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::error::{IntunewinError, Result};
use crate::pipeline::discovery::FileEntry;

/// Cache directory name
const CACHE_DIR: &str = ".intunewin-cache";

/// Subdirectory for individual compressed file cache entries
const DATA_CACHE_DIR: &str = "files";

/// Manifest file name
const MANIFEST_FILE: &str = "manifest.json";

/// Maximum size for a single cached file (500 MB)
/// Files larger than this are not cached to avoid memory exhaustion
const MAX_CACHED_FILE_SIZE: u64 = 500 * 1024 * 1024;

/// Manages the incremental build cache
pub struct CacheManager {
    cache_dir: PathBuf,
    manifest: CacheManifest,
    data_cache: HashMap<String, CachedCompressedData>,
}

/// Cached compressed data for a single file
#[derive(Clone)]
pub struct CachedCompressedData {
    /// Relative path (key)
    pub relative_path: String,
    /// Pre-compressed data (Arc for efficient sharing without cloning)
    pub compressed_data: Arc<Vec<u8>>,
    /// CRC32 of original uncompressed data
    pub crc32: u32,
    /// Original uncompressed size
    pub uncompressed_size: u32,
    /// Compression method used (0 = STORED, 8 = DEFLATE)
    pub compression_method: u16,
}

/// Result of checking a file against the cache
pub enum CacheResult {
    /// File is unchanged, use cached data
    Hit(CachedCompressedData),
    /// File is new or changed, needs compression
    Miss,
}

impl CacheManager {
    /// Creates a new cache manager for the given output directory.
    ///
    /// If a cache exists, it will be loaded. Otherwise, a new empty cache is created.
    pub fn new(output_dir: &Path) -> Result<Self> {
        Self::with_compression_level(output_dir, 0)
    }

    /// Creates a new cache manager with a specific compression level.
    ///
    /// If a cache exists with a different compression level, it will be invalidated.
    pub fn with_compression_level(output_dir: &Path, compression_level: u32) -> Result<Self> {
        let cache_dir = output_dir.join(CACHE_DIR);

        // Try to load existing cache
        let (manifest, data_cache) = if cache_dir.exists() {
            let mut manifest = Self::load_manifest(&cache_dir)?;

            // Invalidate cache if compression level changed
            if manifest.compression_level != compression_level {
                manifest = CacheManifest::with_compression_level(compression_level);
                (manifest, HashMap::new())
            } else {
                let data_cache = Self::load_data_cache(&cache_dir)?;
                (manifest, data_cache)
            }
        } else {
            (
                CacheManifest::with_compression_level(compression_level),
                HashMap::new(),
            )
        };

        Ok(Self {
            cache_dir,
            manifest,
            data_cache,
        })
    }

    /// Returns the compression level this cache was created with.
    pub fn compression_level(&self) -> u32 {
        self.manifest.compression_level
    }

    /// Checks if a file can be served from cache.
    ///
    /// A cache hit occurs when:
    /// - The file exists in the cache manifest
    /// - The file size matches
    /// - The modification time matches exactly (not older, must be equal)
    ///
    /// Note: Compressed data is loaded lazily on-demand to minimize memory usage
    /// for large datasets.
    pub fn check(&self, entry: &FileEntry) -> CacheResult {
        let key = entry.relative_path.to_string_lossy().replace('\\', "/");

        // Check if file exists in manifest
        if let Some(cached_entry) = self.manifest.entries.get(&key) {
            // Get current file metadata
            if let Ok(metadata) = fs::metadata(&entry.absolute_path) {
                let current_size = metadata.len();
                let current_mtime = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                // Check if file is unchanged
                if cached_entry.size == current_size && cached_entry.mtime == current_mtime {
                    // Load compressed data on-demand
                    if let Ok(Some(data)) = Self::load_cached_file(&self.cache_dir, &key) {
                        return CacheResult::Hit(data);
                    }
                }
            }
        }

        CacheResult::Miss
    }

    /// Records a compressed file in the cache.
    pub fn record(
        &mut self,
        entry: &FileEntry,
        compressed_data: Vec<u8>,
        crc32: u32,
        uncompressed_size: u32,
        compression_method: u16,
    ) {
        // Skip caching very large files to avoid memory exhaustion
        // The manifest is still updated for metadata tracking
        if compressed_data.len() as u64 > MAX_CACHED_FILE_SIZE {
            let key = entry.relative_path.to_string_lossy().replace('\\', "/");

            // Get current metadata
            let (size, mtime) = fs::metadata(&entry.absolute_path)
                .map(|m| {
                    let size = m.len();
                    let mtime = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    (size, mtime)
                })
                .unwrap_or((entry.size, 0));

            // Update manifest only (no data cache)
            self.manifest.entries.insert(
                key.clone(),
                CacheEntry {
                    relative_path: key,
                    size,
                    mtime,
                    crc32,
                    compressed_size: compressed_data.len() as u64,
                    compression_method,
                },
            );
            return;
        }

        let key = entry.relative_path.to_string_lossy().replace('\\', "/");

        // Get current metadata
        let (size, mtime) = fs::metadata(&entry.absolute_path)
            .map(|m| {
                let size = m.len();
                let mtime = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                (size, mtime)
            })
            .unwrap_or((entry.size, 0));

        // Update manifest
        self.manifest.entries.insert(
            key.clone(),
            CacheEntry {
                relative_path: key.clone(),
                size,
                mtime,
                crc32,
                compressed_size: compressed_data.len() as u64,
                compression_method,
            },
        );

        // Store compressed data with Arc to avoid cloning on cache hits
        self.data_cache.insert(
            key.clone(),
            CachedCompressedData {
                relative_path: key,
                compressed_data: Arc::new(compressed_data),
                crc32,
                uncompressed_size,
                compression_method,
            },
        );
    }

    /// Saves the cache to disk.
    pub fn save(&self) -> Result<()> {
        // Create cache directory if needed
        if !self.cache_dir.exists() {
            fs::create_dir_all(&self.cache_dir).map_err(|e| IntunewinError::FileWriteError {
                path: self.cache_dir.clone(),
                source: e,
            })?;
        }

        // Save manifest
        self.save_manifest()?;

        // Save compressed data cache
        self.save_data_cache()?;

        Ok(())
    }

    /// Returns cache statistics.
    pub fn stats(&self) -> CacheStats {
        self.manifest.stats.clone()
    }

    /// Updates cache statistics after a build.
    pub fn update_stats(&mut self, hits: usize, misses: usize, bytes_saved: u64) {
        self.manifest.stats.cache_hits += hits;
        self.manifest.stats.cache_misses += misses;
        self.manifest.stats.bytes_saved += bytes_saved;
        self.manifest.stats.total_builds += 1;
    }

    /// Clears the cache.
    pub fn clear(&mut self) -> Result<()> {
        let compression_level = self.manifest.compression_level;
        self.manifest = CacheManifest::with_compression_level(compression_level);
        self.data_cache.clear();

        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).map_err(|e| IntunewinError::FileWriteError {
                path: self.cache_dir.clone(),
                source: e,
            })?;
        }

        Ok(())
    }

    /// Prunes entries that no longer exist in the source.
    pub fn prune(&mut self, current_files: &[FileEntry]) {
        let current_keys: std::collections::HashSet<String> = current_files
            .iter()
            .map(|f| f.relative_path.to_string_lossy().replace('\\', "/"))
            .collect();

        // Remove entries not in current files
        self.manifest
            .entries
            .retain(|k, _| current_keys.contains(k));
        self.data_cache.retain(|k, _| current_keys.contains(k));
    }

    fn load_manifest(cache_dir: &Path) -> Result<CacheManifest> {
        let manifest_path = cache_dir.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            return Ok(CacheManifest::new());
        }

        let content =
            fs::read_to_string(&manifest_path).map_err(|e| IntunewinError::FileReadError {
                path: manifest_path.clone(),
                source: e,
            })?;

        serde_json::from_str(&content).map_err(|e| {
            IntunewinError::InvalidInput(format!("Failed to parse cache manifest: {}", e))
        })
    }

    fn save_manifest(&self) -> Result<()> {
        let manifest_path = self.cache_dir.join(MANIFEST_FILE);
        let content = serde_json::to_string_pretty(&self.manifest).map_err(|e| {
            IntunewinError::InvalidInput(format!("Failed to serialize cache manifest: {}", e))
        })?;

        fs::write(&manifest_path, content).map_err(|e| IntunewinError::FileWriteError {
            path: manifest_path,
            source: e,
        })?;

        Ok(())
    }

    fn load_data_cache(_cache_dir: &Path) -> Result<HashMap<String, CachedCompressedData>> {
        // With the new per-file cache design, we don't pre-load all data.
        // Instead, individual files are loaded on-demand via load_cached_file().
        // This returns an empty map - data is loaded lazily.
        Ok(HashMap::new())
    }

    /// Load a specific file's cached data on-demand
    fn load_cached_file(
        cache_dir: &Path,
        relative_path: &str,
    ) -> Result<Option<CachedCompressedData>> {
        let files_dir = cache_dir.join(DATA_CACHE_DIR);
        if !files_dir.exists() {
            return Ok(None);
        }

        // Create a safe filename from the relative path
        let safe_name = Self::path_to_cache_filename(relative_path);
        let file_path = files_dir.join(&safe_name);

        if !file_path.exists() {
            return Ok(None);
        }

        // Read the small header to get metadata
        let mut file = File::open(&file_path).map_err(|e| IntunewinError::FileReadError {
            path: file_path.clone(),
            source: e,
        })?;

        // Read metadata: crc32 (4) + uncompressed_size (4) + compression_method (2)
        let mut header = [0u8; 10];
        file.read_exact(&mut header)
            .map_err(|e| IntunewinError::FileReadError {
                path: file_path.clone(),
                source: e,
            })?;

        let crc32 = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let uncompressed_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        let compression_method = u16::from_le_bytes([header[8], header[9]]);

        // Read the compressed data
        let mut compressed_data = Vec::new();
        file.read_to_end(&mut compressed_data)
            .map_err(|e| IntunewinError::FileReadError {
                path: file_path,
                source: e,
            })?;

        Ok(Some(CachedCompressedData {
            relative_path: relative_path.to_string(),
            compressed_data: Arc::new(compressed_data),
            crc32,
            uncompressed_size,
            compression_method,
        }))
    }

    /// Convert a relative path to a safe cache filename (hash-based)
    fn path_to_cache_filename(relative_path: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        relative_path.hash(&mut hasher);
        let hash = hasher.finish();
        format!("{:016x}.cache", hash)
    }

    fn save_data_cache(&self) -> Result<()> {
        let files_dir = self.cache_dir.join(DATA_CACHE_DIR);
        if !files_dir.exists() {
            fs::create_dir_all(&files_dir).map_err(|e| IntunewinError::FileWriteError {
                path: files_dir.clone(),
                source: e,
            })?;
        }

        // Save each cached file separately
        for entry in self.data_cache.values() {
            let safe_name = Self::path_to_cache_filename(&entry.relative_path);
            let file_path = files_dir.join(&safe_name);

            let mut file =
                File::create(&file_path).map_err(|e| IntunewinError::FileWriteError {
                    path: file_path.clone(),
                    source: e,
                })?;

            // Write metadata header: crc32 (4) + uncompressed_size (4) + compression_method (2)
            file.write_all(&entry.crc32.to_le_bytes()).map_err(|e| {
                IntunewinError::FileWriteError {
                    path: file_path.clone(),
                    source: e,
                }
            })?;
            file.write_all(&entry.uncompressed_size.to_le_bytes())
                .map_err(|e| IntunewinError::FileWriteError {
                    path: file_path.clone(),
                    source: e,
                })?;
            file.write_all(&entry.compression_method.to_le_bytes())
                .map_err(|e| IntunewinError::FileWriteError {
                    path: file_path.clone(),
                    source: e,
                })?;

            // Write compressed data
            file.write_all(&entry.compressed_data)
                .map_err(|e| IntunewinError::FileWriteError {
                    path: file_path,
                    source: e,
                })?;
        }

        Ok(())
    }
}
