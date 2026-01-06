//! Progress reporting for IntuneWin packaging operations.
//!
//! Provides progress bars using indicatif for long-running operations:
//! - Compression (bytes processed)
//! - Encryption (bytes processed)

use indicatif::{ProgressBar, ProgressStyle};

/// Progress tracker that manages progress indicators.
///
/// Supports quiet/silent modes where no output is shown.
pub struct ProgressTracker {
    /// Whether progress should be hidden (quiet mode)
    hidden: bool,
}

impl ProgressTracker {
    /// Create a new progress tracker.
    ///
    /// If `hidden` is true, all progress bars are no-ops.
    pub fn new(hidden: bool) -> Self {
        Self { hidden }
    }

    /// Create a progress bar for byte-based operations.
    pub fn create_byte_bar(&self, total_bytes: u64, message: &str) -> ProgressBar {
        if self.hidden {
            return ProgressBar::hidden();
        }

        let pb = ProgressBar::new(total_bytes);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({percent}%) {bytes_per_sec}",
                )
                .unwrap()
                .progress_chars("█▓▒░  "),
        );
        pb.set_message(message.to_string());
        pb
    }

    /// Print a status line (respects quiet mode)
    pub fn status(&self, msg: &str) {
        if !self.hidden {
            println!("{}", msg);
        }
    }

    /// Check if progress is hidden
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }
}

/// Helper to format stage messages consistently
pub fn stage_msg(stage: u32, total: u32, name: &str) -> String {
    format!("[{}/{}] {}", stage, total, name)
}
