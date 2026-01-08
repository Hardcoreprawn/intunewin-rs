//! File discovery module for scanning content folders.

use rayon::prelude::*;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::{IntunewinError, Result};

/// Represents a single file entry discovered in the content folder.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Path relative to the content folder root
    pub relative_path: PathBuf,
    /// Absolute path on disk
    pub absolute_path: PathBuf,
    /// File size in bytes
    pub size: u64,
    /// Whether this is the setup file
    pub is_setup_file: bool,
    /// Normalized path (forward slashes, for use in ZIP archives)
    /// Cached during discovery to avoid repeated normalization during compression
    pub normalized_path: String,
}

/// Result of the discovery process.
#[derive(Debug)]
pub struct DiscoveryResult {
    /// All files discovered (sorted by size descending for better load balancing)
    pub files: Vec<FileEntry>,
    /// Total size of all files in bytes
    pub total_size: u64,
    /// Total number of files
    pub file_count: usize,
    /// The setup file entry (reference into files vec)
    pub setup_file_index: usize,
}

impl DiscoveryResult {
    /// Returns a reference to the setup file entry.
    pub fn setup_file(&self) -> &FileEntry {
        &self.files[self.setup_file_index]
    }
}

/// Intermediate file entry before parallel metadata collection.
struct RawFileEntry {
    relative_path: PathBuf,
    absolute_path: PathBuf,
}

/// Discovers all files in the content folder.
///
/// # Arguments
/// * `content_folder` - Path to the content folder to scan
/// * `setup_file` - Name of the setup file (must exist in content_folder)
///
/// # Returns
/// * `Ok(DiscoveryResult)` - Discovery results including all files and the setup file
/// * `Err(IntunewinError)` - If content folder doesn't exist or setup file not found
pub fn discover(content_folder: &Path, setup_file: &str) -> Result<DiscoveryResult> {
    // Validate content folder exists
    if !content_folder.exists() {
        return Err(IntunewinError::SourceFolderNotFound(
            content_folder.to_path_buf(),
        ));
    }

    let content_folder =
        content_folder
            .canonicalize()
            .map_err(|e| IntunewinError::FileReadError {
                path: content_folder.to_path_buf(),
                source: e,
            })?;

    // Phase 1: Walk directory to collect file paths (sequential due to directory iteration)
    let mut raw_files = Vec::new();

    for entry in WalkDir::new(&content_folder)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        // Skip directories, we only care about files
        if !entry.file_type().is_file() {
            continue;
        }

        let absolute_path = entry.path().to_path_buf();

        // Calculate relative path from content folder
        let relative_path = absolute_path
            .strip_prefix(&content_folder)
            .map_err(|_| {
                IntunewinError::InvalidInput(format!(
                    "Failed to compute relative path for: {}",
                    absolute_path.display()
                ))
            })?
            .to_path_buf();

        raw_files.push(RawFileEntry {
            relative_path,
            absolute_path,
        });
    }

    // Phase 2: Parallel metadata collection (stat files in parallel)
    let file_entries: Vec<(FileEntry, bool)> = raw_files
        .par_iter()
        .map(|raw| {
            let metadata = std::fs::metadata(&raw.absolute_path).map_err(|e| {
                IntunewinError::FileReadError {
                    path: raw.absolute_path.clone(),
                    source: e,
                }
            })?;

            let size = metadata.len();

            // Check if this is the setup file
            let is_setup_file = raw.relative_path.to_string_lossy() == setup_file
                || raw
                    .relative_path
                    .file_name()
                    .map(|n| n.to_string_lossy() == setup_file)
                    .unwrap_or(false);

            // Normalize path once during discovery (forward slashes for ZIP archive)
            let normalized_path = raw.relative_path.to_string_lossy().replace('\\', "/");

            Ok((
                FileEntry {
                    relative_path: raw.relative_path.clone(),
                    absolute_path: raw.absolute_path.clone(),
                    size,
                    is_setup_file,
                    normalized_path,
                },
                is_setup_file,
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    // Sort by size descending for better parallel load balancing
    // (process large files first so they don't become bottlenecks at the end)
    let mut files: Vec<FileEntry> = file_entries.into_iter().map(|(f, _)| f).collect();
    files.sort_by(|a, b| b.size.cmp(&a.size));

    // Calculate totals and find setup file index after sorting
    let total_size: u64 = files.iter().map(|f| f.size).sum();
    let file_count = files.len();

    // Find setup file index in sorted list
    let setup_file_index = files
        .iter()
        .position(|f| f.is_setup_file)
        .ok_or_else(|| IntunewinError::SetupFileNotFound(content_folder.join(setup_file)))?;

    Ok(DiscoveryResult {
        files,
        total_size,
        file_count,
        setup_file_index,
    })
}

/// Formats a byte size as a human-readable string.
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1073741824), "1.00 GB");
    }
}
