use std::path::PathBuf;
use thiserror::Error;

/// Custom error types for intunewin-rs
#[derive(Error, Debug)]
pub enum IntunewinError {
    #[error("Source folder not found: {0}")]
    SourceFolderNotFound(PathBuf),

    #[error("Setup file not found: {0}")]
    SetupFileNotFound(PathBuf),

    #[error("Output folder not found: {0}")]
    OutputFolderNotFound(PathBuf),

    #[error("Catalog folder not found: {0}")]
    CatalogFolderNotFound(PathBuf),

    #[error("Failed to read file: {path}")]
    FileReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file: {path}")]
    FileWriteError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("XML generation error: {0}")]
    XmlError(String),

    #[error("ZIP archive error: {0}")]
    ZipError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Thread pool error: {0}")]
    ThreadPoolError(String),

    #[error("Memory mapping error: {path}")]
    MmapError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type alias using our custom error
pub type Result<T> = std::result::Result<T, IntunewinError>;
