use anyhow::Result;
use clap::Parser;
use std::fs;

use intunewin_rs::cli::Args;
use intunewin_rs::pipeline;

fn main() -> Result<()> {
    let mut args = Args::parse();

    // Smart default: Auto-select compression level based on package size
    // This provides sensible defaults without requiring user to think about compression
    if args.compression.is_none() {
        // User didn't explicitly specify compression, apply smart default
        // Calculate total size of content folder
        let total_size = calculate_folder_size(&args.content)?;

        // Recommendation:
        // < 500 MB: Use compression 6 (files are small, cache will help on repeats)
        // >= 500 MB: Use compression 0 (STORE mode - avoid memory pressure, maximize speed)
        let selected = if total_size < 500 * 1024 * 1024 { 6 } else { 0 };
        args.compression = Some(selected);

        if !args.is_silent() {
            let size_mb = total_size as f64 / (1024.0 * 1024.0);
            let mode = if selected == 0 {
                "store-only (fastest for large packages)"
            } else {
                "compression 6 (good balance)"
            };
            println!(
                "Auto-selected compression: {} ({:.1} MB package)",
                mode, size_mb
            );
        }
    }

    // Configure rayon thread pool based on --threads flag
    if let Some(num_threads) = args.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build_global()
            .map_err(|e| anyhow::anyhow!("Failed to configure thread pool: {}", e))?;
    }
    // If threads not specified, rayon defaults to num_cpus (optimal for parallel work)

    pipeline::run(&args)?;
    Ok(())
}

/// Calculate total size of all files in a directory (recursively)
fn calculate_folder_size(path: &std::path::Path) -> Result<u64> {
    let mut total_size = 0u64;

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            // Recursively calculate size of subdirectories
            total_size += calculate_folder_size(&entry.path())?;
        } else {
            total_size += metadata.len();
        }
    }

    Ok(total_size)
}
