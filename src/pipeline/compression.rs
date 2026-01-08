//! Parallel compression module for creating ZIP archives.
//!
//! Strategy: Compress files in parallel batches, streaming to disk to limit memory.
//! The ZIP format requires sequential writes, but compression is CPU-bound
//! and can be parallelized across all cores.
//!
//! Memory optimization: Process files in batches based on total uncompressed size,
//! writing each batch to disk before processing the next.
//!
//! Caching support: When enabled, stores compressed file data to avoid
//! recompressing unchanged files on subsequent builds.

use std::fs::File;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use flate2::write::DeflateEncoder;
use flate2::Compression;
use indicatif::ProgressBar;
use rayon::prelude::*;

use crate::cache::{CacheManager, CacheResult, CachedCompressedData};
use crate::error::{IntunewinError, Result};
use crate::io::read_file_smart;
use crate::pipeline::discovery::{DiscoveryResult, FileEntry};

/// Maximum memory to use for compression batch (500 MB)
/// After compressing this much data, we flush to disk
const BATCH_SIZE_BYTES: u64 = 500 * 1024 * 1024;

/// Pre-compressed file ready for ZIP assembly
struct CompressedEntry {
    relative_path: String,
    compressed_data: Vec<u8>,
    uncompressed_size: u32,
    crc32: u32,
    /// Compression method: 8 = DEFLATE, 0 = STORED (no compression)
    compression_method: u16,
}

impl From<CachedCompressedData> for CompressedEntry {
    fn from(cached: CachedCompressedData) -> Self {
        // Try to unwrap Arc to move Vec without cloning
        // If successful: moves ownership with zero copy
        // If it fails (other references exist): clone the Vec
        let compressed_data =
            Arc::try_unwrap(cached.compressed_data).unwrap_or_else(|arc| (*arc).clone());
        Self {
            relative_path: cached.relative_path,
            compressed_data,
            uncompressed_size: cached.uncompressed_size,
            crc32: cached.crc32,
            compression_method: cached.compression_method,
        }
    }
}

/// Compress a single file (runs on worker thread)
fn compress_file(
    entry: &FileEntry,
    level: u32,
    use_mmap: bool,
    progress_bytes: Option<&Arc<AtomicU64>>,
    progress_bar: Option<&ProgressBar>,
) -> Result<CompressedEntry> {
    // Read file
    let data = read_file_smart(&entry.absolute_path, use_mmap)?;

    // CRC32 of original data
    let crc32 = crc32fast::hash(&data);
    let uncompressed_size = data.len() as u32;

    // Normalize path separators
    let relative_path = entry.relative_path.to_string_lossy().replace('\\', "/");

    // Update progress
    if let (Some(bytes), Some(bar)) = (progress_bytes, progress_bar) {
        bytes.fetch_add(entry.size, Ordering::Relaxed);
        bar.set_position(bytes.load(Ordering::Relaxed));
    }

    // Level 0 = STORE (no compression), fastest for already-compressed files
    if level == 0 {
        return Ok(CompressedEntry {
            relative_path,
            compressed_data: data,
            uncompressed_size,
            crc32,
            compression_method: 0, // STORED
        });
    }

    // DEFLATE compress (level 1-9)
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(level));
    encoder
        .write_all(&data)
        .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;
    let compressed_data = encoder
        .finish()
        .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;

    // If compression didn't help, store uncompressed
    if compressed_data.len() >= data.len() {
        Ok(CompressedEntry {
            relative_path,
            compressed_data: data,
            uncompressed_size,
            crc32,
            compression_method: 0, // STORED
        })
    } else {
        Ok(CompressedEntry {
            relative_path,
            compressed_data,
            uncompressed_size,
            crc32,
            compression_method: 8, // DEFLATE
        })
    }
}

/// ZIP file writer that supports incremental/streaming writes
struct StreamingZipWriter {
    file: File,
    entries: Vec<ZipEntryInfo>,
}

/// Information about a written ZIP entry (for central directory)
struct ZipEntryInfo {
    relative_path: String,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    local_header_offset: u64,
    compression_method: u16,
}

impl StreamingZipWriter {
    fn new(path: &Path) -> Result<Self> {
        let file = File::create(path).map_err(|e| IntunewinError::FileWriteError {
            path: path.to_path_buf(),
            source: e,
        })?;
        Ok(Self {
            file,
            entries: Vec::new(),
        })
    }

    /// Write a single compressed entry
    fn write_entry(&mut self, entry: CompressedEntry) -> Result<()> {
        let offset = self
            .file
            .stream_position()
            .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;

        let name_bytes = entry.relative_path.as_bytes();
        let compressed_size = entry.compressed_data.len() as u32;

        // Local file header (30 bytes + filename)
        self.file.write_all(&0x04034b50u32.to_le_bytes())?; // Signature
        self.file.write_all(&20u16.to_le_bytes())?; // Version needed
        self.file.write_all(&0u16.to_le_bytes())?; // General purpose flags
        self.file
            .write_all(&entry.compression_method.to_le_bytes())?; // Compression method
        self.file.write_all(&0u16.to_le_bytes())?; // Mod time
        self.file.write_all(&0u16.to_le_bytes())?; // Mod date
        self.file.write_all(&entry.crc32.to_le_bytes())?; // CRC32
        self.file.write_all(&compressed_size.to_le_bytes())?;
        self.file
            .write_all(&entry.uncompressed_size.to_le_bytes())?;
        self.file
            .write_all(&(name_bytes.len() as u16).to_le_bytes())?;
        self.file.write_all(&0u16.to_le_bytes())?; // Extra field length
        self.file.write_all(name_bytes)?;
        self.file.write_all(&entry.compressed_data)?;

        // Record entry info for central directory
        self.entries.push(ZipEntryInfo {
            relative_path: entry.relative_path,
            crc32: entry.crc32,
            compressed_size,
            uncompressed_size: entry.uncompressed_size,
            local_header_offset: offset,
            compression_method: entry.compression_method,
        });

        Ok(())
    }

    /// Finalize the ZIP by writing central directory
    fn finish(mut self) -> Result<()> {
        let cd_offset = self
            .file
            .stream_position()
            .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;

        // Write central directory entries
        for entry in &self.entries {
            let name_bytes = entry.relative_path.as_bytes();

            self.file.write_all(&0x02014b50u32.to_le_bytes())?; // Signature
            self.file.write_all(&20u16.to_le_bytes())?; // Version made by
            self.file.write_all(&20u16.to_le_bytes())?; // Version needed
            self.file.write_all(&0u16.to_le_bytes())?; // Flags
            self.file
                .write_all(&entry.compression_method.to_le_bytes())?; // Compression method
            self.file.write_all(&0u16.to_le_bytes())?; // Mod time
            self.file.write_all(&0u16.to_le_bytes())?; // Mod date
            self.file.write_all(&entry.crc32.to_le_bytes())?;
            self.file.write_all(&entry.compressed_size.to_le_bytes())?;
            self.file
                .write_all(&entry.uncompressed_size.to_le_bytes())?;
            self.file
                .write_all(&(name_bytes.len() as u16).to_le_bytes())?;
            self.file.write_all(&0u16.to_le_bytes())?; // Extra field length
            self.file.write_all(&0u16.to_le_bytes())?; // Comment length
            self.file.write_all(&0u16.to_le_bytes())?; // Disk number
            self.file.write_all(&0u16.to_le_bytes())?; // Internal attrs
            self.file.write_all(&0u32.to_le_bytes())?; // External attrs
            self.file
                .write_all(&(entry.local_header_offset as u32).to_le_bytes())?;
            self.file.write_all(name_bytes)?;
        }

        let cd_size = self
            .file
            .stream_position()
            .map_err(|e| IntunewinError::CompressionError(e.to_string()))?
            - cd_offset;

        // End of central directory record
        self.file.write_all(&0x06054b50u32.to_le_bytes())?; // Signature
        self.file.write_all(&0u16.to_le_bytes())?; // Disk number
        self.file.write_all(&0u16.to_le_bytes())?; // CD start disk
        self.file
            .write_all(&(self.entries.len() as u16).to_le_bytes())?;
        self.file
            .write_all(&(self.entries.len() as u16).to_le_bytes())?;
        self.file.write_all(&(cd_size as u32).to_le_bytes())?;
        self.file.write_all(&(cd_offset as u32).to_le_bytes())?;
        self.file.write_all(&0u16.to_le_bytes())?; // Comment length

        self.file.flush()?;
        Ok(())
    }
}

/// Partition files into batches based on total size
fn partition_into_batches(files: &[FileEntry], max_batch_bytes: u64) -> Vec<Vec<&FileEntry>> {
    let mut batches = Vec::new();
    let mut current_batch = Vec::new();
    let mut current_size: u64 = 0;

    for file in files {
        // If adding this file would exceed the batch size, and we have files, start new batch
        if current_size + file.size > max_batch_bytes && !current_batch.is_empty() {
            batches.push(current_batch);
            current_batch = Vec::new();
            current_size = 0;
        }
        current_batch.push(file);
        current_size += file.size;
    }

    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    batches
}

/// Result of compression with caching statistics
pub struct CompressionResult {
    /// Path to the created ZIP file
    pub zip_path: PathBuf,
    /// Number of cache hits
    pub cache_hits: usize,
    /// Number of cache misses (files that needed compression)
    pub cache_misses: usize,
    /// Bytes saved by cache (uncompressed size of cached files)
    pub bytes_saved: u64,
}

/// Creates the inner ZIP file with parallel compression, streaming writes, and optional caching.
///
/// Memory optimization: Files are processed in batches. Each batch is compressed
/// in parallel, written to disk, then freed from memory before the next batch.
///
/// # Arguments
/// * `discovery` - The discovery result containing files to compress
/// * `output_path` - Directory where the ZIP will be created
/// * `compression_level` - 0 = STORE only (fastest), 1-9 = DEFLATE level
/// * `use_mmap` - Whether to use memory-mapped I/O for large files
/// * `progress_bar` - Optional progress bar to update during compression
/// * `cache` - Optional cache manager for incremental builds
pub fn compress_to_inner_zip_cached(
    discovery: &DiscoveryResult,
    output_path: &Path,
    compression_level: u32,
    use_mmap: bool,
    progress_bar: Option<&ProgressBar>,
    mut cache: Option<&mut CacheManager>,
) -> Result<CompressionResult> {
    if !output_path.exists() {
        std::fs::create_dir_all(output_path).map_err(|e| IntunewinError::FileWriteError {
            path: output_path.to_path_buf(),
            source: e,
        })?;
    }

    let setup_name = discovery
        .setup_file()
        .relative_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "IntunePackage".to_string());

    let zip_path = output_path.join(format!("{}.zip", setup_name));

    // Create streaming ZIP writer
    let mut zip_writer = StreamingZipWriter::new(&zip_path)?;

    // Track bytes processed across all threads
    let progress_bytes = Arc::new(AtomicU64::new(0));

    // Cache statistics
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;
    let mut bytes_saved = 0u64;

    // Partition files into memory-bounded batches
    let batches = partition_into_batches(&discovery.files, BATCH_SIZE_BYTES);

    for batch in batches {
        // Process files in order: collect uncached indices while storing cached results
        let mut results: Vec<Option<CompressedEntry>> = (0..batch.len()).map(|_| None).collect();
        let mut uncached_with_indices: Vec<(usize, &FileEntry)> = Vec::new();

        // First pass: check cache for each file in order
        for (idx, file) in batch.iter().enumerate() {
            if let Some(ref c) = cache {
                match c.check(file) {
                    CacheResult::Hit(data) => {
                        cache_hits += 1;
                        bytes_saved += data.uncompressed_size as u64;

                        // Update progress
                        if let Some(bar) = progress_bar {
                            progress_bytes
                                .fetch_add(data.uncompressed_size as u64, Ordering::Relaxed);
                            bar.set_position(progress_bytes.load(Ordering::Relaxed));
                        }

                        results[idx] = Some(data.into());
                    }
                    CacheResult::Miss => {
                        uncached_with_indices.push((idx, file));
                    }
                }
            } else {
                uncached_with_indices.push((idx, file));
            }
        }

        // Second pass: compress all uncached files in parallel
        if !uncached_with_indices.is_empty() {
            cache_misses += uncached_with_indices.len();

            let pb = progress_bar;
            let bytes_ref = &progress_bytes;

            let compressed: Vec<CompressedEntry> = uncached_with_indices
                .par_iter()
                .map(|(_, f)| compress_file(f, compression_level, use_mmap, Some(bytes_ref), pb))
                .collect::<Result<Vec<_>>>()?;

            // Record in cache and store at original positions
            if let Some(c) = cache.as_mut() {
                for (entry, (_, file)) in compressed.iter().zip(uncached_with_indices.iter()) {
                    c.record(
                        file,
                        entry.compressed_data.clone(),
                        entry.crc32,
                        entry.uncompressed_size,
                        entry.compression_method,
                    );
                }
            }

            // Store compressed entries at their original positions
            for (entry, (idx, _)) in compressed.into_iter().zip(uncached_with_indices.iter()) {
                results[*idx] = Some(entry);
            }
        }

        // Check for None values before flattening to provide specific error messages
        for (idx, entry) in results.iter().enumerate() {
            if entry.is_none() {
                return Err(IntunewinError::CompressionError(
                    format!("File at batch index {} was not processed", idx)
                ));
            }
        }

        // Write all entries in ORIGINAL BATCH ORDER
        let mut entry_count = 0;
        for entry in results.into_iter().flatten() {
            entry_count += 1;
            zip_writer.write_entry(entry)?;
        }

        // Verify all files were written
        if entry_count != batch.len() {
            return Err(IntunewinError::CompressionError(format!(
                "File count mismatch: wrote {} entries but batch has {}",
                entry_count,
                batch.len()
            )));
        }
    }

    // Finalize ZIP file
    zip_writer.finish()?;

    Ok(CompressionResult {
        zip_path,
        cache_hits,
        cache_misses,
        bytes_saved,
    })
}

/// Creates the inner ZIP file with parallel compression and streaming writes.
///
/// Memory optimization: Files are processed in batches. Each batch is compressed
/// in parallel, written to disk, then freed from memory before the next batch.
///
/// # Arguments
/// * `discovery` - The discovery result containing files to compress
/// * `output_path` - Directory where the ZIP will be created
/// * `compression_level` - 0 = STORE only (fastest), 1-9 = DEFLATE level
/// * `use_mmap` - Whether to use memory-mapped I/O for large files
/// * `progress_bar` - Optional progress bar to update during compression
pub fn compress_to_inner_zip(
    discovery: &DiscoveryResult,
    output_path: &Path,
    compression_level: u32,
    use_mmap: bool,
    progress_bar: Option<&ProgressBar>,
) -> Result<PathBuf> {
    let result = compress_to_inner_zip_cached(
        discovery,
        output_path,
        compression_level,
        use_mmap,
        progress_bar,
        None,
    )?;
    Ok(result.zip_path)
}
