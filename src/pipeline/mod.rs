//! Pipeline module orchestrating the IntuneWin packaging process.

pub mod compression;
pub mod discovery;
pub mod packager;
pub mod zero_mat;

use anyhow::Result;
use std::time::Instant;

use crate::cli::Args;
use crate::progress::{stage_msg, ProgressTracker};

pub use discovery::{discover, format_size, DiscoveryResult, FileEntry};
pub use packager::create_intunewin;

/// Total number of pipeline stages
const TOTAL_STAGES: u32 = 3;

/// Main pipeline entry point
///
/// Executes the full IntuneWin packaging pipeline:
///   Source files stream directly through ZIP structure → AES-CBC encryption
///   → outer .intunewin ZIP. No intermediate files, no buffers, no second pass.
pub fn run(args: &Args) -> Result<()> {
    let start_time = Instant::now();
    let progress = ProgressTracker::new(args.is_quiet());

    if args.catalog.is_some() {
        return Err(anyhow::anyhow!(
            "--catalog is not implemented yet in intunewin-rs. Remove -a/--catalog and retry."
        ));
    }

    validate_output_not_within_content(&args.content, &args.output)?;

    let use_mmap = !args.no_mmap;

    // Stage 1: Discover files
    let discovery = discover(&args.content, &args.setup)?;

    if !args.is_silent() {
        println!("IntuneWin packager v{}", env!("CARGO_PKG_VERSION"));
        println!("  Source: {}", args.content.display());
        println!("  Setup: {}", args.setup);
        println!("  Output: {}", args.output.display());
        println!();
    }

    progress.status(&format!(
        "{} Found {} files ({})",
        stage_msg(1, TOTAL_STAGES, "Discovery"),
        discovery.file_count,
        format_size(discovery.total_size)
    ));

    // Stage 2: Zero-mat (stream → encrypt → package in one pass)
    let zero_mat_bar = progress.create_byte_bar(
        discovery.total_size,
        &stage_msg(2, TOTAL_STAGES, "Packaging"),
    );

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
        stage_msg(2, TOTAL_STAGES, "Packaging complete."),
        format_size(result.final_size),
    ));

    progress.status(&stage_msg(3, TOTAL_STAGES, "Done"));

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

    Ok(())
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
