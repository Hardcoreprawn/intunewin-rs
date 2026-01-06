//! Manifest.xml generation for IntuneWin packages.
//!
//! The Manifest.xml file contains metadata about all files in the package,
//! including their SHA256 hashes, sizes, and relative paths.

use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::error::{IntunewinError, Result};
use crate::pipeline::discovery::DiscoveryResult;

/// Represents a file entry in the manifest.
#[derive(Debug, Clone)]
pub struct ManifestFile {
    /// Relative path of the file
    pub path: String,
    /// SHA256 hash of the file contents (hex encoded)
    pub hash: String,
    /// File size in bytes
    pub size: u64,
}

/// Calculates SHA256 hash of a file.
fn calculate_sha256(path: &Path) -> Result<String> {
    let file = File::open(path).map_err(|e| IntunewinError::FileReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024]; // 64KB buffer

    loop {
        let bytes_read = reader.read(&mut buffer).map_err(|e| IntunewinError::FileReadError {
            path: path.to_path_buf(),
            source: e,
        })?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(hex::encode(hash))
}

/// Generates manifest entries for all discovered files using parallel hashing.
pub fn generate_manifest_entries(discovery: &DiscoveryResult) -> Result<Vec<ManifestFile>> {
    // Parallel SHA256 computation for all files
    discovery
        .files
        .par_iter()
        .map(|file_entry| {
            let hash = calculate_sha256(&file_entry.absolute_path)?;
            
            // Use forward slashes for manifest paths
            let path = file_entry
                .relative_path
                .to_string_lossy()
                .replace('\\', "/");

            Ok(ManifestFile {
                path,
                hash,
                size: file_entry.size,
            })
        })
        .collect()
}

/// Generates the Manifest.xml content as a string.
///
/// # Arguments
/// * `discovery` - The discovery result containing files to include
///
/// # Returns
/// * `Ok(String)` - The XML content of the manifest
/// * `Err(IntunewinError)` - If manifest generation fails
pub fn generate_manifest(discovery: &DiscoveryResult) -> Result<String> {
    let entries = generate_manifest_entries(discovery)?;

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xml.push_str("<ApplicationManifest xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n");
    xml.push_str("  <Files>\n");

    for entry in &entries {
        xml.push_str(&format!(
            "    <File Name=\"{}\" Size=\"{}\" Hash=\"{}\" />\n",
            escape_xml(&entry.path),
            entry.size,
            entry.hash.to_uppercase()
        ));
    }

    xml.push_str("  </Files>\n");
    xml.push_str("</ApplicationManifest>");

    Ok(xml)
}

/// Escapes special XML characters in a string.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// Simple hex encoding (to avoid adding another dependency)
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("test"), "test");
        assert_eq!(escape_xml("a&b"), "a&amp;b");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_hex_encode() {
        assert_eq!(hex::encode([0x00, 0xff, 0xab]), "00ffab");
    }
}
