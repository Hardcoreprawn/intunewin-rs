//! I/O utilities for efficient file operations.

pub mod mmap;

pub use mmap::{open_file_for_streaming, read_file_smart, FileBytes};
