//! Pipeline module orchestrating the IntuneWin packaging process.

pub mod compression;
pub mod discovery;
pub mod packager;

use anyhow::Result;
use std::fs;
use std::time::Instant;

use crate::cli::Args;
use crate::crypto::aes::{encrypt_file_streaming, encrypt_with_keygen};
use crate::format;
use crate::format::detection::{DetectionInfo, StreamingDetectionInfo};
use crate::progress::{ProgressTracker, stage_msg};

pub use compression::compress_to_inner_zip;
pub use discovery::{discover, format_size, DiscoveryResult, FileEntry};
pub use packager::create_intunewin;

/// Threshold for using streaming encryption (100 MB)
/// Below this, in-memory encryption is faster
const STREAMING_THRESHOLD: u64 = 100 * 1024 * 1024;

/// Total number of pipeline stages
const TOTAL_STAGES: u32 = 5;

/// Main pipeline entry point
///
/// Executes the full IntuneWin packaging pipeline:
/// 1. Validate inputs
/// 2. Collect files from source folder
/// 3. Create ZIP archive with deflate compression
/// 4. Encrypt the archive using AES-256-CBC
/// 5. Generate Detection.xml metadata
/// 6. Package everything into final .intunewin file
/// 7. Clean up temporary files
pub fn run(args: &Args) -> Result<()> {
    let start_time = Instant::now();
    let progress = ProgressTracker::new(args.is_quiet());

    if !args.is_silent() {
        println!("IntuneWin packager v{}", env!("CARGO_PKG_VERSION"));
        println!("  Source: {}", args.content.display());
        println!("  Setup: {}", args.setup);
        println!("  Output: {}", args.output.display());
        println!();
    }

    // Stage 1: Discover files
    let discovery = discover(&args.content, &args.setup)?;
    
    progress.status(&format!(
        "{} Found {} files ({})",
        stage_msg(1, TOTAL_STAGES, "Discovery"),
        discovery.file_count,
        format_size(discovery.total_size)
    ));

    // Stage 2: Compress to inner ZIP
    let compress_bar = progress.create_byte_bar(
        discovery.total_size,
        &stage_msg(2, TOTAL_STAGES, "Compressing"),
    );
    
    let use_mmap = !args.no_mmap;
    let zip_path = compress_to_inner_zip(
        &discovery,
        &args.output,
        args.compression,
        use_mmap,
        Some(&compress_bar),
    )?;

    let zip_size = std::fs::metadata(&zip_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let compression_ratio = if discovery.total_size > 0 {
        (1.0 - (zip_size as f64 / discovery.total_size as f64)) * 100.0
    } else {
        0.0
    };

    compress_bar.finish_with_message(format!(
        "{} Compressed to {} ({:.1}% saved)",
        stage_msg(2, TOTAL_STAGES, "Compression complete."),
        format_size(zip_size),
        compression_ratio
    ));

    // Stage 3: Encrypt the inner ZIP
    let encrypt_bar = progress.create_byte_bar(
        zip_size,
        &stage_msg(3, TOTAL_STAGES, "Encrypting"),
    );
    
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
        let zip_data = fs::read(&zip_path).map_err(|e| {
            anyhow::anyhow!("Failed to read ZIP file for encryption: {}", e)
        })?;

        let encryption_result = encrypt_with_keygen(&zip_data)?;
        let encrypted_size = encryption_result.encrypted_data.len() as u64;

        encrypt_bar.set_position(zip_size);

        fs::write(&encrypted_path, &encryption_result.encrypted_data).map_err(|e| {
            anyhow::anyhow!("Failed to write encrypted data: {}", e)
        })?;
        
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
        stage_msg(3, TOTAL_STAGES, "Encryption complete."),
        format_size(encrypted_size)
    ));

    // Stage 4: Create final .intunewin package
    let setup_name = discovery
        .setup_file()
        .relative_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "setup".to_string());

    let final_path = create_intunewin(
        &encrypted_path,
        &detection_xml,
        &setup_name,
        &args.output,
    )?;

    let final_size = std::fs::metadata(&final_path)
        .map(|m| m.len())
        .unwrap_or(0);

    progress.status(&format!(
        "{} Package created ({})",
        stage_msg(4, TOTAL_STAGES, "Packaging"),
        format_size(final_size)
    ));

    // Stage 5: Cleanup
    let _ = fs::remove_file(&zip_path);
    let _ = fs::remove_file(&encrypted_path);
    
    progress.status(&stage_msg(5, TOTAL_STAGES, "Cleanup complete"));

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
