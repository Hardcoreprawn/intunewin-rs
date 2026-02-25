//! Pipeline module orchestrating the IntuneWin packaging process.

pub mod compression;
pub mod discovery;
pub mod packager;
pub mod streamed;
pub mod zero_mat;

use anyhow::Result;
use std::fs;
use std::time::Instant;

use crate::cache::CacheManager;
use crate::cli::Args;
use crate::progress::{stage_msg, ProgressTracker};

pub use compression::{compress_to_inner_zip, compress_to_inner_zip_cached, CompressionResult};
pub use discovery::{discover, format_size, DiscoveryResult, FileEntry};
pub use packager::create_intunewin;

/// Total number of pipeline stages (zero-materialization path, no cache)
const TOTAL_STAGES_ZERO_MAT: u32 = 3;

/// Total number of pipeline stages (legacy compression path, no cache)
const TOTAL_STAGES: u32 = 5;

/// Total number of pipeline stages (legacy compression path, with cache)
const TOTAL_STAGES_CACHED: u32 = 6;

/// Main pipeline entry point
///
/// Executes the full IntuneWin packaging pipeline.
///
/// Default path (compression 0 / store-only):
///   Uses the zero-materialization pipeline — source files stream directly
///   through ZIP structure → AES-CBC encryption → outer .intunewin ZIP.
///   No intermediate files, no buffers, no second pass.
///
/// Legacy path (compression > 0, hidden flag):
///   Falls back to the disk-based compress → encrypt → package pipeline
///   with optional caching.
pub fn run(args: &Args) -> Result<()> {
    let start_time = Instant::now();
    let progress = ProgressTracker::new(args.is_quiet());

    if args.catalog.is_some() {
        return Err(anyhow::anyhow!(
            "--catalog is not implemented yet in intunewin-rs. Remove -a/--catalog and retry."
        ));
    }

    validate_output_not_within_content(&args.content, &args.output)?;

    let compression = resolve_compression_level(args)?;
    let use_mmap = !args.no_mmap;

    // Stage 1: Discover files (shared by both paths)
    let discovery = discover(&args.content, &args.setup)?;

    if compression == 0 {
        // ── Zero-materialization path ─────────────────────────────────
        let stages = TOTAL_STAGES_ZERO_MAT;

        if !args.is_silent() {
            println!("IntuneWin packager v{}", env!("CARGO_PKG_VERSION"));
            println!("  Source: {}", args.content.display());
            println!("  Setup: {}", args.setup);
            println!("  Output: {}", args.output.display());
            println!();
        }

        progress.status(&format!(
            "{} Found {} files ({})",
            stage_msg(1, stages, "Discovery"),
            discovery.file_count,
            format_size(discovery.total_size)
        ));

        // Stage 2: Zero-mat (stream → encrypt → package in one pass)
        let zero_mat_bar =
            progress.create_byte_bar(discovery.total_size, &stage_msg(2, stages, "Packaging"));

        let result = zero_mat::run_zero_mat(
            &discovery,
            &discovery
                .setup_file()
                .relative_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "setup".to_string()),
            &args.output,
            use_mmap,
            Some(&zero_mat_bar),
        )
        .map_err(|e| anyhow::anyhow!("Zero-materialization pipeline failed: {}", e))?;

        zero_mat_bar.finish_with_message(format!(
            "{} Package created ({})",
            stage_msg(2, stages, "Packaging complete."),
            format_size(result.final_size),
        ));

        progress.status(&stage_msg(3, stages, "Done"));

        let elapsed = start_time.elapsed();
        if !args.is_silent() {
            let throughput = discovery.total_size as f64 / elapsed.as_secs_f64() / 1_000_000.0;
            println!();
            println!("✓ Done!");
            println!("  Output: {}", result.output_path.display());
            println!("  Size: {}", format_size(result.final_size));
            println!("  Time: {:.2}s", elapsed.as_secs_f64());
            println!("  Throughput: {:.1} MB/s", throughput);
        }
    } else {
        // ── Legacy compression path (compression > 0) ────────────────
        let use_cache = args.use_cache_with_compression(compression);
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
                if !args.cache {
                    println!("  Caching: auto-enabled (compression > 0)");
                } else {
                    println!("  Caching: enabled");
                }
            }
            println!();
        }

        // Initialize cache if enabled
        let mut cache = if use_cache {
            let mut cache_mgr = CacheManager::with_compression_level(&args.output, compression)
                .map_err(|e| anyhow::anyhow!("Cache error: {}", e))?;

            if args.clear_cache {
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

        progress.status(&format!(
            "{} Found {} files ({})",
            stage_msg(1, stages, "Discovery"),
            discovery.file_count,
            format_size(discovery.total_size)
        ));

        if let Some(ref mut c) = cache {
            c.prune(&discovery.files);
        }

        // Stage 2: Compress to inner ZIP (disk path)
        let compress_bar =
            progress.create_byte_bar(discovery.total_size, &stage_msg(2, stages, "Compressing"));

        let compression_result = compress_to_inner_zip_cached(
            &discovery,
            &args.output,
            compression,
            use_mmap,
            Some(&compress_bar),
            cache.as_mut(),
        )?;

        if let Some(ref mut c) = cache {
            c.save()
                .map_err(|e| anyhow::anyhow!("Failed to save cache after compression: {}", e))?;
        }

        let zip_path = compression_result.zip_path;
        let zip_size = std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0);
        let cache_hits = compression_result.cache_hits;
        let cache_misses = compression_result.cache_misses;
        let bytes_saved = compression_result.bytes_saved;

        let compression_ratio = if discovery.total_size > 0 {
            (1.0 - (zip_size as f64 / discovery.total_size as f64)) * 100.0
        } else {
            0.0
        };

        let cache_info = if use_cache && cache_hits > 0 {
            format!(" [cache: {} hits, {} misses]", cache_hits, cache_misses)
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

        // Stage 3: Encrypt + package
        progress.status(&format!(
            "{} Encrypting and packaging...",
            stage_msg(3, stages, "Encrypt+Package"),
        ));

        let setup_name = discovery
            .setup_file()
            .relative_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "setup".to_string());

        let streamed_result =
            streamed::encrypt_and_package_streamed(&zip_path, &setup_name, &args.output)
                .map_err(|e| anyhow::anyhow!("Streamed encrypt+package failed: {}", e))?;

        let final_path = streamed_result.output_path;
        let final_size = streamed_result.final_size;

        progress.status(&format!(
            "{} Package created ({})",
            stage_msg(4, stages, "Packaging"),
            format_size(final_size)
        ));

        // Stage 5: Cleanup
        if args.keep_temp {
            progress.status("Keeping temporary artifacts (--keep-temp)");
        } else {
            let _ = fs::remove_file(&zip_path);
        }

        progress.status(&stage_msg(5, stages, "Cleanup complete"));

        // Stage 6 (if caching): Update and save cache
        if let Some(ref mut c) = cache {
            c.update_stats(cache_hits, cache_misses, bytes_saved);
            c.save_manifest_only()
                .map_err(|e| anyhow::anyhow!("Failed to save cache manifest: {}", e))?;
            progress.status(&stage_msg(6, stages, "Cache updated"));
        }

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
    }

    Ok(())
}

/// Resolve the effective compression level for this run.
///
/// If user explicitly sets `--compression`, that value is used.
/// Otherwise, defaults to compression 0 (store-only) for maximum speed.
/// Real-world installers (.exe, .msi) are already compressed —
/// DEFLATE adds <2% size reduction but costs significant time.
fn resolve_compression_level(args: &Args) -> Result<u32> {
    if let Some(level) = args.compression {
        return Ok(level);
    }

    Ok(0)
}

fn validate_output_not_within_content(
    content: &std::path::Path,
    output: &std::path::Path,
) -> Result<()> {
    let content_abs = content.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "Failed to resolve content path '{}': {}",
            content.display(),
            e
        )
    })?;

    let output_abs = resolve_path_for_compare(output).map_err(|e| {
        anyhow::anyhow!(
            "Failed to resolve output path '{}': {}",
            output.display(),
            e
        )
    })?;

    if output_abs.starts_with(&content_abs) {
        return Err(anyhow::anyhow!(
            "Output directory '{}' must not be inside content directory '{}'. Choose an output path outside content to avoid recursive/self-inclusion hazards.",
            output.display(),
            content.display()
        ));
    }

    Ok(())
}

fn resolve_path_for_compare(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    if path.exists() {
        return path.canonicalize();
    }

    let cwd = std::env::current_dir()?;
    Ok(cwd.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn create_temp_dir(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn resolve_compression_uses_explicit_value() {
        let args = Args {
            content: std::path::PathBuf::from("."),
            setup: "setup.exe".to_string(),
            output: std::path::PathBuf::from("."),
            catalog: None,
            quiet: true,
            silent: true,
            threads: None,
            compression: Some(9),
            no_mmap: false,
            cache: false,
            no_cache: false,
            clear_cache: false,
            cache_stats: false,
            keep_temp: false,
        };

        assert_eq!(resolve_compression_level(&args).unwrap(), 9);
    }

    #[test]
    fn resolve_compression_auto_selects_store_only() {
        let args = Args {
            content: std::path::PathBuf::from("."),
            setup: "setup.exe".to_string(),
            output: std::path::PathBuf::from("."),
            catalog: None,
            quiet: true,
            silent: true,
            threads: None,
            compression: None,
            no_mmap: false,
            cache: false,
            no_cache: false,
            clear_cache: false,
            cache_stats: false,
            keep_temp: false,
        };

        assert_eq!(resolve_compression_level(&args).unwrap(), 0);
    }

    #[test]
    fn validate_output_not_within_content_rejects_nested_output() {
        let content = create_temp_dir("pipeline_content_nested");
        let output = content.join("out");
        fs::create_dir_all(&output).unwrap();

        let result = validate_output_not_within_content(&content, &output);
        assert!(result.is_err());

        let _ = fs::remove_dir_all(content);
    }

    #[test]
    fn validate_output_not_within_content_allows_separate_output() {
        let content = create_temp_dir("pipeline_content_separate");
        let output = create_temp_dir("pipeline_output_separate");

        let result = validate_output_not_within_content(&content, &output);
        assert!(result.is_ok());

        let _ = fs::remove_dir_all(content);
        let _ = fs::remove_dir_all(output);
    }
}
