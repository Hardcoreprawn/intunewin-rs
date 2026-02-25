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
//! - Zero-materialization pipeline (single-pass I/O)
//! - Memory-mapped file I/O for large files
//! - AES-256-CBC encryption with HMAC-SHA256
//! - Progress indication

pub mod cache;
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
