//! # intunewin-rs
//!
//! High-performance IntuneWin packager compatible with Microsoft IntuneWinAppUtil.
//!
//! This library provides functionality to create `.intunewin` packages for
//! Microsoft Intune application deployment.
//!
//! ## Features
//!
//! - Compatible with Microsoft IntuneWinAppUtil output format
//! - Parallel compression using rayon
//! - Memory-mapped file I/O for large files
//! - Configurable compression levels
//! - Progress indication

pub mod cli;
pub mod crypto;
pub mod error;
pub mod format;
pub mod io;
pub mod pipeline;
pub mod progress;

pub use cli::Args;
pub use error::{IntunewinError, Result};
pub use progress::ProgressTracker;

