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
use std::time::SystemTime;

use crate::error::{IntunewinError, Result};
use crate::pipeline::discovery::FileEntry;

/// Cache directory name
const CACHE_DIR: &str = ".intunewin-cache";

/// Compressed data cache file
const DATA_CACHE_FILE: &str = "compressed_data.bin";

/// Manifest file name
const MANIFEST_FILE: &str = "manifest.json";

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
    /// Pre-compressed data
    pub compressed_data: Vec<u8>,
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
    /// - The modification time matches (or is older)
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
                    // Try to get compressed data from cache
                    if let Some(data) = self.data_cache.get(&key) {
                        return CacheResult::Hit(data.clone());
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

        // Store compressed data
        self.data_cache.insert(
            key.clone(),
            CachedCompressedData {
                relative_path: key,
                compressed_data,
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
        self.manifest = CacheManifest::new();
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

    fn load_data_cache(cache_dir: &Path) -> Result<HashMap<String, CachedCompressedData>> {
        let data_path = cache_dir.join(DATA_CACHE_FILE);
        if !data_path.exists() {
            return Ok(HashMap::new());
        }

        let mut file = File::open(&data_path).map_err(|e| IntunewinError::FileReadError {
            path: data_path.clone(),
            source: e,
        })?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| IntunewinError::FileReadError {
                path: data_path.clone(),
                source: e,
            })?;

        Self::deserialize_data_cache(&data)
    }

    fn save_data_cache(&self) -> Result<()> {
        let data_path = self.cache_dir.join(DATA_CACHE_FILE);
        let data = self.serialize_data_cache();

        let mut file = File::create(&data_path).map_err(|e| IntunewinError::FileWriteError {
            path: data_path.clone(),
            source: e,
        })?;

        file.write_all(&data)
            .map_err(|e| IntunewinError::FileWriteError {
                path: data_path,
                source: e,
            })?;

        Ok(())
    }

    /// Serialize data cache to binary format.
    ///
    /// Format per entry:
    /// - u32: path length
    /// - bytes: path bytes
    /// - u32: crc32
    /// - u32: uncompressed_size
    /// - u16: compression_method
    /// - u32: compressed_data length
    /// - bytes: compressed_data
    fn serialize_data_cache(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Write entry count
        data.extend_from_slice(&(self.data_cache.len() as u32).to_le_bytes());

        for entry in self.data_cache.values() {
            let path_bytes = entry.relative_path.as_bytes();

            data.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
            data.extend_from_slice(path_bytes);
            data.extend_from_slice(&entry.crc32.to_le_bytes());
            data.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
            data.extend_from_slice(&entry.compression_method.to_le_bytes());
            data.extend_from_slice(&(entry.compressed_data.len() as u32).to_le_bytes());
            data.extend_from_slice(&entry.compressed_data);
        }

        data
    }

    fn deserialize_data_cache(data: &[u8]) -> Result<HashMap<String, CachedCompressedData>> {
        let mut cache = HashMap::new();
        let mut offset = 0;

        if data.len() < 4 {
            return Ok(cache);
        }

        let entry_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        for _ in 0..entry_count {
            if offset + 4 > data.len() {
                break;
            }

            let path_len =
                u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;

            if offset + path_len > data.len() {
                break;
            }

            let path = String::from_utf8_lossy(&data[offset..offset + path_len]).to_string();
            offset += path_len;

            if offset + 14 > data.len() {
                break;
            }

            let crc32 = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            offset += 4;

            let uncompressed_size =
                u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            offset += 4;

            let compression_method =
                u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
            offset += 2;

            let compressed_len =
                u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;

            if offset + compressed_len > data.len() {
                break;
            }

            let compressed_data = data[offset..offset + compressed_len].to_vec();
            offset += compressed_len;

            cache.insert(
                path.clone(),
                CachedCompressedData {
                    relative_path: path,
                    compressed_data,
                    crc32,
                    uncompressed_size,
                    compression_method,
                },
            );
        }

        Ok(cache)
    }
}
