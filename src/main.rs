use anyhow::Result;
use clap::Parser;

use intunewin_rs::cli::Args;
use intunewin_rs::pipeline;

fn main() -> Result<()> {
    let args = Args::parse();
    
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
