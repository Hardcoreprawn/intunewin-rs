//! Smart file reading with optional memory-mapped I/O.
//!
//! Provides efficient file reading strategies:
//! - Memory-mapped I/O for large files (> 1MB) when enabled
//! - Standard file reading for small files or when mmap is disabled

use std::fs::File;
use std::io::Read;
use std::path::Path;

use memmap2::Mmap;

use crate::error::{IntunewinError, Result};

/// Threshold for using memory-mapped I/O
/// Platform-specific:
/// - Windows: 256 KB (lower threshold benefits from different I/O patterns)
/// - Linux/macOS: 1 MB (mmap overhead not worth it below 1MB on Unix)
#[cfg(target_os = "windows")]
const MMAP_THRESHOLD: u64 = 256 * 1024;

#[cfg(not(target_os = "windows"))]
const MMAP_THRESHOLD: u64 = 1024 * 1024;

/// Reads a file using the most efficient method based on size and configuration.
///
/// # Arguments
/// * `path` - Path to the file to read
/// * `use_mmap` - Whether to use memory-mapped I/O for large files
///
/// # Returns
/// * `Ok(Vec<u8>)` - The file contents
/// * `Err(IntunewinError)` - If reading fails
///
/// # Strategy
/// - Files > 1MB with use_mmap=true: Use memory-mapped I/O
/// - Files <= 1MB or use_mmap=false: Use standard file reading
pub fn read_file_smart(path: &Path, use_mmap: bool) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|e| IntunewinError::FileReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let metadata = file.metadata().map_err(|e| IntunewinError::FileReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let file_size = metadata.len();

    if use_mmap && file_size > MMAP_THRESHOLD {
        read_with_mmap(&file, path)
    } else {
        read_standard(&file, path, file_size)
    }
}

/// Reads a file using memory-mapped I/O.
fn read_with_mmap(file: &File, path: &Path) -> Result<Vec<u8>> {
    // SAFETY: We're only reading, and the file is opened for the duration of the mapping
    let mmap = unsafe {
        Mmap::map(file).map_err(|e| IntunewinError::MmapError {
            path: path.to_path_buf(),
            source: e,
        })?
    };

    Ok(mmap.to_vec())
}

/// Reads a file using standard I/O with pre-allocated buffer.
fn read_standard(file: &File, path: &Path, size: u64) -> Result<Vec<u8>> {
    let mut reader = std::io::BufReader::new(file);
    let capacity = usize::try_from(size).map_err(|_| {
        IntunewinError::InvalidInput(format!(
            "File '{}' is too large to fit into memory on this platform",
            path.display()
        ))
    })?;
    let mut buffer = Vec::with_capacity(capacity);

    reader
        .read_to_end(&mut buffer)
        .map_err(|e| IntunewinError::FileReadError {
            path: path.to_path_buf(),
            source: e,
        })?;

    Ok(buffer)
}

/// Borrowable file contents — either an mmap or an owned buffer.
///
/// `Deref<Target=[u8]>` lets callers uniformly slice, iterate, and hash
/// the data regardless of backing storage. Unlike `read_file_smart`,
/// the `Mapped` variant keeps the mmap alive (no `.to_vec()` copy) so
/// callers can stream sub-file chunks without fully materializing the data.
pub enum FileBytes {
    Mapped(Mmap),
    Buffered(Vec<u8>),
}

impl std::ops::Deref for FileBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            FileBytes::Mapped(m) => m,
            FileBytes::Buffered(v) => v,
        }
    }
}

/// Open a file and return its contents as a borrowable byte source.
///
/// Uses mmap for large files (keeps the mapping alive, no heap copy)
/// and standard read for small files.
pub fn open_file_for_streaming(path: &Path, use_mmap: bool) -> Result<FileBytes> {
    let file = File::open(path).map_err(|e| IntunewinError::FileReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let metadata = file.metadata().map_err(|e| IntunewinError::FileReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let file_size = metadata.len();

    if use_mmap && file_size > MMAP_THRESHOLD {
        let mmap = unsafe {
            Mmap::map(&file).map_err(|e| IntunewinError::MmapError {
                path: path.to_path_buf(),
                source: e,
            })?
        };
        Ok(FileBytes::Mapped(mmap))
    } else {
        read_standard(&file, path, file_size).map(FileBytes::Buffered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_read_small_file() {
        let temp_dir = std::env::temp_dir().join(format!("mmap_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("small.txt");

        // Create a small file
        let content = b"Hello, world!";
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content).unwrap();

        // Test reading with mmap enabled (should use standard I/O due to size)
        let result = read_file_smart(&file_path, true).unwrap();
        assert_eq!(result, content);

        // Test reading with mmap disabled
        let result = read_file_smart(&file_path, false).unwrap();
        assert_eq!(result, content);

        // Cleanup
        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
