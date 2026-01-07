//! Pipeline module orchestrating the IntuneWin packaging process.

pub mod compression;
pub mod discovery;
pub mod packager;

use anyhow::Result;
use std::fs;
use std::time::Instant;

use crate::cache::CacheManager;
use crate::cli::Args;
use crate::crypto::aes::{encrypt_file_streaming, encrypt_with_keygen};
use crate::format;
use crate::format::detection::{DetectionInfo, StreamingDetectionInfo};
use crate::progress::{stage_msg, ProgressTracker};

pub use compression::{compress_to_inner_zip, compress_to_inner_zip_cached, CompressionResult};
pub use discovery::{discover, format_size, DiscoveryResult, FileEntry};
pub use packager::create_intunewin;

/// Threshold for using streaming encryption (100 MB)
/// Below this, in-memory encryption is faster
const STREAMING_THRESHOLD: u64 = 100 * 1024 * 1024;

/// Total number of pipeline stages (without caching)
const TOTAL_STAGES: u32 = 5;

/// Total number of pipeline stages (with caching)
const TOTAL_STAGES_CACHED: u32 = 6;

/// Main pipeline entry point
///
/// Executes the full IntuneWin packaging pipeline:
/// 1. Validate inputs
/// 2. Collect files from source folder
/// 3. Create ZIP archive with deflate compression (with optional caching)
/// 4. Encrypt the archive using AES-256-CBC
/// 5. Generate Detection.xml metadata
/// 6. Package everything into final .intunewin file
/// 7. Clean up temporary files
/// 8. Save cache (if enabled)
pub fn run(args: &Args) -> Result<()> {
    let start_time = Instant::now();
    let progress = ProgressTracker::new(args.is_quiet());

    let use_cache = args.use_cache();
    let stages = if use_cache {
        TOTAL_STAGES_CACHED
    } else {
        TOTAL_STAGES
    };

    if !args.is_silent() {
        println!("IntuneWin packager v{}", env!("CARGO_PKG_VERSION"));
        println!("  Source: {}", args.content.display());
        println!("  Setup: {}", args.setup);
        println!("  Output: {}", args.output.display());
        if use_cache {
            let compression = args.compression.unwrap_or(0);
            if compression > 0 && !args.cache {
                println!("  Caching: auto-enabled (compression > 0)");
            } else {
                println!("  Caching: enabled");
            }
        }
        println!();
    }

    // Initialize cache if enabled
    let mut cache = if use_cache {
        let compression = args
            .compression
            .expect("Compression should be set by main.rs auto-detection");
        let mut cache_mgr = CacheManager::with_compression_level(&args.output, compression)
            .map_err(|e| anyhow::anyhow!("Cache error: {}", e))?;

        // Handle --clear-cache flag
        if args.clear_cache {
            // Print stats before clearing if both flags are present
            if args.cache_stats {
                let stats = cache_mgr.stats();
                println!("Cache Statistics (before clearing):");
                println!("  Compression level: {}", cache_mgr.compression_level());
                println!("  Total builds: {}", stats.total_builds);
                println!("  Cache hits: {}", stats.cache_hits);
                println!("  Cache misses: {}", stats.cache_misses);
                println!("  Bytes saved: {}", format_size(stats.bytes_saved));
                if stats.cache_hits + stats.cache_misses > 0 {
                    let hit_rate = stats.cache_hits as f64
                        / (stats.cache_hits + stats.cache_misses) as f64
                        * 100.0;
                    println!("  Hit rate: {:.1}%", hit_rate);
                }
                println!();
            }

            cache_mgr
                .clear()
                .map_err(|e| anyhow::anyhow!("Failed to clear cache: {}", e))?;
            if !args.is_silent() {
                println!("Cache cleared.");
            }
        } else if args.cache_stats {
            // Handle --cache-stats flag (when not clearing)
            let stats = cache_mgr.stats();
            println!("Cache Statistics:");
            println!("  Compression level: {}", cache_mgr.compression_level());
            println!("  Total builds: {}", stats.total_builds);
            println!("  Cache hits: {}", stats.cache_hits);
            println!("  Cache misses: {}", stats.cache_misses);
            println!("  Bytes saved: {}", format_size(stats.bytes_saved));
            if stats.cache_hits + stats.cache_misses > 0 {
                let hit_rate = stats.cache_hits as f64
                    / (stats.cache_hits + stats.cache_misses) as f64
                    * 100.0;
                println!("  Hit rate: {:.1}%", hit_rate);
            }
            println!();
        }

        Some(cache_mgr)
    } else {
        None
    };

    // Stage 1: Discover files
    let discovery = discover(&args.content, &args.setup)?;

    progress.status(&format!(
        "{} Found {} files ({})",
        stage_msg(1, stages, "Discovery"),
        discovery.file_count,
        format_size(discovery.total_size)
    ));

    // Prune cache of deleted files
    if let Some(ref mut c) = cache {
        c.prune(&discovery.files);
    }

    // Stage 2: Compress to inner ZIP (with optional caching)
    let compress_bar =
        progress.create_byte_bar(discovery.total_size, &stage_msg(2, stages, "Compressing"));

    let use_mmap = !args.no_mmap;
    let compression = args
        .compression
        .expect("Compression should be set by main.rs auto-detection");

    let compression_result = compress_to_inner_zip_cached(
        &discovery,
        &args.output,
        compression,
        use_mmap,
        Some(&compress_bar),
        cache.as_mut(),
    )?;

    // Save cache immediately after successful compression to preserve incremental progress
    // even if later stages (encryption, packaging, cleanup) fail
    if let Some(ref mut c) = cache {
        c.save()
            .map_err(|e| anyhow::anyhow!("Failed to save cache after compression: {}", e))?;
    }

    let zip_path = compression_result.zip_path;
    let zip_size = std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0);

    let compression_ratio = if discovery.total_size > 0 {
        (1.0 - (zip_size as f64 / discovery.total_size as f64)) * 100.0
    } else {
        0.0
    };

    // Include cache info in completion message if caching is active
    let cache_info = if use_cache && compression_result.cache_hits > 0 {
        format!(
            " [cache: {} hits, {} misses]",
            compression_result.cache_hits, compression_result.cache_misses
        )
    } else {
        String::new()
    };

    compress_bar.finish_with_message(format!(
        "{} Compressed to {} ({:.1}% saved){}",
        stage_msg(2, stages, "Compression complete."),
        format_size(zip_size),
        compression_ratio,
        cache_info
    ));

    // Stage 3: Encrypt the inner ZIP
    let encrypt_bar = progress.create_byte_bar(zip_size, &stage_msg(3, stages, "Encrypting"));

    let encrypted_path = args.output.join("IntunePackage.intunewin.tmp");

    // Use streaming encryption for large files to avoid memory exhaustion
    let (detection_xml, encrypted_size) = if zip_size > STREAMING_THRESHOLD {
        let streaming_result = encrypt_file_streaming(&zip_path, &encrypted_path)
            .map_err(|e| anyhow::anyhow!("Streaming encryption failed: {}", e))?;

        encrypt_bar.set_position(zip_size);

        let setup_name = discovery
            .setup_file()
            .relative_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "setup".to_string());

        let detection_info = StreamingDetectionInfo {
            name: setup_name
                .rsplit_once('.')
                .map(|(name, _)| name.to_string())
                .unwrap_or_else(|| setup_name.clone()),
            unencrypted_content_size: zip_size,
            setup_file: setup_name.clone(),
            key: streaming_result.key,
            iv: streaming_result.iv,
            mac_key: streaming_result.mac_key,
            mac: streaming_result.mac,
            file_digest: streaming_result.file_digest,
        };

        let xml = format::generate_detection_xml_streaming(&detection_info)?;
        (xml, streaming_result.encrypted_size)
    } else {
        let zip_data = fs::read(&zip_path)
            .map_err(|e| anyhow::anyhow!("Failed to read ZIP file for encryption: {}", e))?;

        let encryption_result = encrypt_with_keygen(&zip_data)?;
        let encrypted_size = encryption_result.encrypted_data.len() as u64;

        encrypt_bar.set_position(zip_size);

        fs::write(&encrypted_path, &encryption_result.encrypted_data)
            .map_err(|e| anyhow::anyhow!("Failed to write encrypted data: {}", e))?;

        let setup_name = discovery
            .setup_file()
            .relative_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "setup".to_string());

        let detection_info = DetectionInfo {
            name: setup_name
                .rsplit_once('.')
                .map(|(name, _)| name.to_string())
                .unwrap_or_else(|| setup_name.clone()),
            unencrypted_content_size: zip_size,
            setup_file: setup_name.clone(),
            encryption: encryption_result,
        };

        let xml = format::generate_detection_xml(&detection_info)?;
        (xml, encrypted_size)
    };

    encrypt_bar.finish_with_message(format!(
        "{} Encrypted to {}",
        stage_msg(3, stages, "Encryption complete."),
        format_size(encrypted_size)
    ));

    // Stage 4: Create final .intunewin package
    let setup_name = discovery
        .setup_file()
        .relative_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "setup".to_string());

    let final_path = create_intunewin(&encrypted_path, &detection_xml, &setup_name, &args.output)?;

    let final_size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);

    progress.status(&format!(
        "{} Package created ({})",
        stage_msg(4, stages, "Packaging"),
        format_size(final_size)
    ));

    // Stage 5: Cleanup
    let _ = fs::remove_file(&zip_path);
    let _ = fs::remove_file(&encrypted_path);

    progress.status(&stage_msg(5, stages, "Cleanup complete"));

    // Stage 6 (if caching): Update and save cache with final stats
    if let Some(ref mut c) = cache {
        c.update_stats(
            compression_result.cache_hits,
            compression_result.cache_misses,
            compression_result.bytes_saved,
        );
        // Save manifest with updated stats. After initial compression, we re-run this to update
        // cache statistics (hit/miss counts). Using save_manifest_only() avoids redundant writes of
        // cached compressed data files which haven't changed since the initial save.
        c.save_manifest_only()
            .map_err(|e| anyhow::anyhow!("Failed to save cache manifest: {}", e))?;

        progress.status(&stage_msg(6, stages, "Cache updated"));
    }

    // Final summary
    let elapsed = start_time.elapsed();

    if !args.is_silent() {
        let throughput = discovery.total_size as f64 / elapsed.as_secs_f64() / 1_000_000.0;

        println!();
        println!("✓ Done!");
        println!("  Output: {}", final_path.display());
        println!("  Size: {}", format_size(final_size));
        println!("  Time: {:.2}s", elapsed.as_secs_f64());
        println!("  Throughput: {:.1} MB/s", throughput);
    }

    Ok(())
}
