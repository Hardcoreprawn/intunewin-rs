use anyhow::Result;
use clap::Parser;
use std::fs;

use intunewin_rs::cli::Args;
use intunewin_rs::pipeline;

fn main() -> Result<()> {
    let args = Args::parse();

    // Apply smart defaults to args before running pipeline
    let args = apply_smart_defaults(args)?;

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

/// Apply smart defaults to parsed arguments
fn apply_smart_defaults(mut args: Args) -> Result<Args> {
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

    Ok(args)
}

/// Calculate total size of all files in a directory (recursively)
///
/// # Behavior
/// - Symlinks are NOT followed (skipped without error)
/// - Permission errors on individual entries are logged but don't stop the scan
/// - Uses stack-safe iterative approach to handle large directory trees
fn calculate_folder_size(path: &std::path::Path) -> Result<u64> {
    let mut total_size = 0u64;
    let mut queue = vec![path.to_path_buf()];

    while let Some(current_dir) = queue.pop() {
        // Use read_dir with error handling to skip permission errors
        match fs::read_dir(&current_dir) {
            Ok(entries) => {
                for entry_result in entries {
                    match entry_result {
                        Ok(entry) => {
                            // Get metadata without following symlinks
                            match entry.metadata() {
                                Ok(metadata) => {
                                    if metadata.is_dir() && !metadata.is_symlink() {
                                        // Queue subdirectory for processing (iterative, not recursive)
                                        queue.push(entry.path());
                                    } else if metadata.is_file() {
                                        total_size += metadata.len();
                                    }
                                    // Skip symlinks: they are neither followed nor counted
                                }
                                Err(_) => {
                                    // Permission denied or other errors on individual file
                                    // Log and continue scanning other files
                                    if let Ok(file_name) = entry.file_name().into_string() {
                                        eprintln!(
                                            "Warning: Could not read metadata for '{}' in '{}', skipping",
                                            file_name,
                                            current_dir.display()
                                        );
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            // Error reading entry, continue with next entry
                            continue;
                        }
                    }
                }
            }
            Err(e) => {
                // Permission denied on directory - log warning but continue
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    eprintln!(
                        "Warning: Permission denied reading directory '{}', skipping",
                        current_dir.display()
                    );
                } else {
                    // Other errors (not found, etc.) are still returned as errors
                    return Err(e.into());
                }
            }
        }
    }

    Ok(total_size)
}
