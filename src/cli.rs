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

    /// Compression level: 0 = store only (default), 1-9 = DEFLATE
    /// Most installers are already compressed, so 0 is fastest
    #[arg(long = "compression", default_value_t = 0, value_parser = clap::value_parser!(u32).range(0..=9))]
    pub compression: u32,

    /// Disable memory-mapped file I/O
    #[arg(long = "no-mmap", default_value_t = false)]
    pub no_mmap: bool,
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
