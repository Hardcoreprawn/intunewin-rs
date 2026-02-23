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

    let setup_input_normalized = setup_file.replace('\\', "/");
    let setup_basename = Path::new(&setup_input_normalized)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| setup_input_normalized.clone());

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
    let mut files: Vec<FileEntry> = raw_files
        .par_iter()
        .map(|raw| {
            let metadata = std::fs::metadata(&raw.absolute_path).map_err(|e| {
                IntunewinError::FileReadError {
                    path: raw.absolute_path.clone(),
                    source: e,
                }
            })?;

            let size = metadata.len();

            // Normalize path once during discovery (forward slashes for ZIP archive)
            let normalized_path = raw.relative_path.to_string_lossy().replace('\\', "/");

            Ok(FileEntry {
                relative_path: raw.relative_path.clone(),
                absolute_path: raw.absolute_path.clone(),
                size,
                is_setup_file: false,
                normalized_path,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // Sort by size descending for better parallel load balancing
    // (process large files first so they don't become bottlenecks at the end)
    files.sort_by(|a, b| b.size.cmp(&a.size));

    // Calculate totals and find setup file index after sorting
    let total_size: u64 = files.iter().map(|f| f.size).sum();
    let file_count = files.len();

    // Select setup file deterministically:
    // 1) Exact relative-path match (if setup arg includes path)
    // 2) Basename match (if setup arg is filename only)
    // Ambiguous matches fail with a clear error.
    let exact_matches: Vec<usize> = files
        .iter()
        .enumerate()
        .filter_map(|(idx, f)| {
            if f.normalized_path == setup_input_normalized {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    let setup_file_index = if exact_matches.len() == 1 {
        exact_matches[0]
    } else if exact_matches.len() > 1 {
        return Err(IntunewinError::InvalidInput(format!(
            "Ambiguous setup file '{}': multiple exact relative-path matches found",
            setup_file
        )));
    } else {
        let name_matches: Vec<usize> = files
            .iter()
            .enumerate()
            .filter_map(|(idx, f)| {
                let file_name = f
                    .relative_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if file_name == setup_basename {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        if name_matches.is_empty() {
            return Err(IntunewinError::SetupFileNotFound(
                content_folder.join(setup_file),
            ));
        }

        if name_matches.len() > 1 {
            let matched_paths = name_matches
                .iter()
                .map(|idx| files[*idx].normalized_path.clone())
                .collect::<Vec<_>>()
                .join(", ");

            return Err(IntunewinError::InvalidInput(format!(
                "Ambiguous setup file '{}': matched multiple files [{}]. Provide a unique relative path.",
                setup_file, matched_paths
            )));
        }

        name_matches[0]
    };

    files[setup_file_index].is_setup_file = true;

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
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1073741824), "1.00 GB");
    }

    fn create_test_tree(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn discover_fails_on_ambiguous_setup_basename() {
        let root = create_test_tree("discover_ambiguous");
        let sub1 = root.join("a");
        let sub2 = root.join("b");
        fs::create_dir_all(&sub1).unwrap();
        fs::create_dir_all(&sub2).unwrap();

        let mut f1 = File::create(sub1.join("setup.exe")).unwrap();
        f1.write_all(b"one").unwrap();
        let mut f2 = File::create(sub2.join("setup.exe")).unwrap();
        f2.write_all(b"two").unwrap();

        let result = discover(&root, "setup.exe");
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("Ambiguous setup file"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discover_prefers_exact_relative_path_match() {
        let root = create_test_tree("discover_exact_path");
        let sub1 = root.join("a");
        let sub2 = root.join("b");
        fs::create_dir_all(&sub1).unwrap();
        fs::create_dir_all(&sub2).unwrap();

        let mut f1 = File::create(sub1.join("setup.exe")).unwrap();
        f1.write_all(b"one").unwrap();
        let mut f2 = File::create(sub2.join("setup.exe")).unwrap();
        f2.write_all(b"two").unwrap();

        let result = discover(&root, "b/setup.exe").unwrap();
        let selected = result.setup_file();
        assert_eq!(selected.normalized_path, "b/setup.exe");

        let _ = fs::remove_dir_all(root);
    }
}
