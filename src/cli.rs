use clap::Parser;
use std::path::PathBuf;

fn parse_non_empty_setup(value: &str) -> std::result::Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("setup file cannot be empty or whitespace".to_string());
    }
    Ok(trimmed.to_string())
}

fn parse_positive_usize(value: &str) -> std::result::Result<usize, String> {
    let parsed = value.parse::<usize>().map_err(|_| {
        format!(
            "invalid thread count '{}': expected a positive integer",
            value
        )
    })?;
    if parsed == 0 {
        return Err("thread count must be >= 1".to_string());
    }
    Ok(parsed)
}

/// High-performance IntuneWin packager - compatible with Microsoft IntuneWinAppUtil
#[derive(Parser, Debug, Clone)]
#[command(name = "intunewin-rs")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Source folder containing the setup files
    #[arg(short = 'c', long = "content", required = true)]
    pub content: PathBuf,

    /// Setup file name (the main installer executable)
    #[arg(short = 's', long = "setup", required = true, value_parser = parse_non_empty_setup)]
    pub setup: String,

    /// Output folder for the .intunewin file
    #[arg(short = 'o', long = "output", required = true)]
    pub output: PathBuf,

    /// Catalog folder (reserved for Microsoft CLI compatibility; currently unsupported)
    #[arg(short = 'a', long = "catalog")]
    pub catalog: Option<PathBuf>,

    /// Quiet mode - minimal output
    #[arg(short = 'q', long = "quiet", default_value_t = false)]
    pub quiet: bool,

    /// Silent mode - no output
    #[arg(long = "qq", default_value_t = false)]
    pub silent: bool,

    /// Number of threads for parallel processing (default: auto-detect)
    #[arg(short = 't', long = "threads", value_parser = parse_positive_usize)]
    pub threads: Option<usize>,

    /// Disable memory-mapped file I/O
    #[arg(long = "no-mmap", default_value_t = false)]
    pub no_mmap: bool,

    /// Keep intermediate artifacts (inner .zip and encrypted .tmp) in the output folder.
    ///
    /// Useful for debugging and for cache-integrity verification, since final `.intunewin`
    /// output is intentionally non-deterministic due to random encryption keys/IV.
    #[arg(long = "keep-temp", default_value_t = false)]
    pub keep_temp: bool,
}

impl Args {
    /// Returns true if any quiet mode is enabled
    pub fn is_quiet(&self) -> bool {
        self.quiet || self.silent
    }

    /// Returns true if silent mode is enabled
    pub fn is_silent(&self) -> bool {
        self.silent
    }
}
