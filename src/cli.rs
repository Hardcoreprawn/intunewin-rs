use clap::Parser;
use std::path::PathBuf;

/// High-performance IntuneWin packager - compatible with Microsoft IntuneWinAppUtil
#[derive(Parser, Debug, Clone)]
#[command(name = "intunewin-rs")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Source folder containing the setup files
    #[arg(short = 'c', long = "content", required = true)]
    pub content: PathBuf,

    /// Setup file name (the main installer executable)
    #[arg(short = 's', long = "setup", required = true)]
    pub setup: String,

    /// Output folder for the .intunewin file
    #[arg(short = 'o', long = "output", required = true)]
    pub output: PathBuf,

    /// Catalog folder (optional)
    #[arg(short = 'a', long = "catalog")]
    pub catalog: Option<PathBuf>,

    /// Quiet mode - minimal output
    #[arg(short = 'q', long = "quiet", default_value_t = false)]
    pub quiet: bool,

    /// Silent mode - no output
    #[arg(long = "qq", default_value_t = false)]
    pub silent: bool,

    /// Number of threads for parallel processing (default: auto-detect)
    #[arg(short = 't', long = "threads")]
    pub threads: Option<usize>,

    /// Compression level: 1-9 = DEFLATE, or 0 = store only
    /// If not specified, defaults are auto-detected based on package size:
    /// - <500MB packages: compression 6 (good for caching)
    /// - >=500MB packages: compression 0 (maximum speed, minimal memory)
    #[arg(long = "compression", value_parser = clap::value_parser!(u32).range(0..=9))]
    pub compression: Option<u32>,

    /// Disable memory-mapped file I/O
    #[arg(long = "no-mmap", default_value_t = false)]
    pub no_mmap: bool,

    /// Force enable incremental caching (auto-enabled when compression > 0)
    /// Cache stores compressed file data to avoid recompressing unchanged files
    #[arg(long = "cache", default_value_t = false)]
    pub cache: bool,

    /// Disable incremental caching (overrides auto-enable)
    #[arg(long = "no-cache", default_value_t = false)]
    pub no_cache: bool,

    /// Clear the cache before building
    #[arg(long = "clear-cache", default_value_t = false)]
    pub clear_cache: bool,

    /// Show cache statistics
    #[arg(long = "cache-stats", default_value_t = false)]
    pub cache_stats: bool,
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

    /// Returns true if caching should be used.
    ///
    /// Caching is automatically enabled when compression > 0 (where it provides
    /// 2-3x speedup on subsequent builds). It's disabled for compression = 0
    /// (STORE mode) where it adds overhead without benefit.
    ///
    /// Use --cache to force enable, --no-cache to force disable.
    pub fn use_cache(&self) -> bool {
        let compression = self.compression.unwrap_or(0); // If not specified, will be auto-detected to 0 or 6
        if self.no_cache {
            // Explicit disable always wins
            false
        } else if self.cache {
            // Explicit enable
            true
        } else {
            // Auto-enable when compression > 0 (beneficial)
            // Auto-disable when compression = 0 (adds overhead)
            compression > 0
        }
    }
}
