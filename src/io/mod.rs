//! I/O utilities for efficient file operations.

pub mod mmap;

pub use mmap::read_file_smart;
