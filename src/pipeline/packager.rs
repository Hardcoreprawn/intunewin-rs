//! Final .intunewin package assembly.
//!
//! Creates the outer ZIP file structure that constitutes the final .intunewin package.
//! The structure must exactly match the Microsoft IntuneWinAppUtil format:
//!
//! ```text
//! {setup_name}.intunewin (ZIP file)
//! └── IntuneWinPackage/
//!     ├── Contents/
//!     │   └── IntunePackage.intunewin   (encrypted inner ZIP blob)
//!     └── Metadata/
//!         └── Detection.xml             (unencrypted, contains keys)
//! ```

use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

use crate::error::{IntunewinError, Result};

/// Creates the final .intunewin package (outer ZIP).
///
/// # Arguments
/// * `encrypted_content` - Path to the encrypted inner ZIP blob
/// * `detection_xml` - The Detection.xml content as a string
/// * `setup_name` - The setup file name (e.g., "setup.exe") used to derive the output filename
/// * `output_folder` - Directory where the .intunewin file will be created
///
/// # Returns
/// * `Ok(PathBuf)` - Path to the created .intunewin file
/// * `Err(IntunewinError)` - If packaging fails
///
/// # Example
/// ```ignore
/// use std::path::Path;
/// use intunewin_rs::pipeline::packager::create_intunewin;
///
/// let encrypted_path = Path::new("temp/IntunePackage.intunewin");
/// let detection_xml = "<ApplicationInfo>...</ApplicationInfo>";
/// let output = create_intunewin(encrypted_path, detection_xml, "setup.exe", Path::new("output")).unwrap();
/// ```
pub fn create_intunewin(
    encrypted_content: &Path,
    detection_xml: &str,
    setup_name: &str,
    output_folder: &Path,
) -> Result<PathBuf> {
    // Derive output filename from setup_name
    // setup.exe -> setup.intunewin
    let output_filename = derive_output_filename(setup_name);
    let output_path = output_folder.join(&output_filename);

    // Ensure output directory exists
    if !output_folder.exists() {
        std::fs::create_dir_all(output_folder).map_err(|e| IntunewinError::FileWriteError {
            path: output_folder.to_path_buf(),
            source: e,
        })?;
    }

    // Create the outer ZIP file
    let file = File::create(&output_path).map_err(|e| IntunewinError::FileWriteError {
        path: output_path.clone(),
        source: e,
    })?;

    let mut zip = ZipWriter::new(file);

    // Add Detection.xml - stored uncompressed as per Microsoft format
    // Path: IntuneWinPackage/Metadata/Detection.xml
    let detection_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored);

    zip.start_file("IntuneWinPackage/Metadata/Detection.xml", detection_options)
        .map_err(|e| IntunewinError::ZipError(e.to_string()))?;

    zip.write_all(detection_xml.as_bytes())
        .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;

    // Add encrypted content blob
    // Path: IntuneWinPackage/Contents/IntunePackage.intunewin
    // Use stored (no compression) since the content is already encrypted
    let content_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored);

    zip.start_file("IntuneWinPackage/Contents/IntunePackage.intunewin", content_options)
        .map_err(|e| IntunewinError::ZipError(e.to_string()))?;

    // Stream the encrypted file into the ZIP
    let encrypted_file = File::open(encrypted_content).map_err(|e| IntunewinError::FileReadError {
        path: encrypted_content.to_path_buf(),
        source: e,
    })?;

    let mut reader = BufReader::new(encrypted_file);
    let mut buffer = vec![0u8; 64 * 1024]; // 64KB buffer

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|e| IntunewinError::FileReadError {
                path: encrypted_content.to_path_buf(),
                source: e,
            })?;

        if bytes_read == 0 {
            break;
        }

        zip.write_all(&buffer[..bytes_read])
            .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;
    }

    // Finalize the ZIP file
    zip.finish()
        .map_err(|e| IntunewinError::ZipError(e.to_string()))?;

    Ok(output_path)
}

/// Derives the output filename from the setup file name.
///
/// Examples:
/// - "setup.exe" -> "setup.intunewin"
/// - "install.msi" -> "install.intunewin"
/// - "app" -> "app.intunewin"
fn derive_output_filename(setup_name: &str) -> String {
    let stem = Path::new(setup_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| setup_name.to_string());

    format!("{}.intunewin", stem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_derive_output_filename() {
        assert_eq!(derive_output_filename("setup.exe"), "setup.intunewin");
        assert_eq!(derive_output_filename("install.msi"), "install.intunewin");
        assert_eq!(derive_output_filename("app"), "app.intunewin");
        assert_eq!(derive_output_filename("my.app.exe"), "my.app.intunewin");
    }

    #[test]
    fn test_create_intunewin_structure() {
        let temp_dir = std::env::temp_dir().join(format!("intunewin_test_{}", std::process::id()));
        let temp_path = temp_dir.as_path();
        let _ = std::fs::create_dir_all(temp_path);

        // Create a mock encrypted content file
        let encrypted_path = temp_path.join("IntunePackage.intunewin");
        let mut encrypted_file = File::create(&encrypted_path).unwrap();
        encrypted_file.write_all(b"mock encrypted content").unwrap();

        // Create the .intunewin package
        let detection_xml = r#"<ApplicationInfo>test</ApplicationInfo>"#;
        let output_dir = temp_path.join("output");

        let result = create_intunewin(
            &encrypted_path,
            detection_xml,
            "setup.exe",
            &output_dir,
        );

        assert!(result.is_ok());
        let output_path = result.unwrap();

        // Verify the file was created
        assert!(output_path.exists());
        assert_eq!(output_path.file_name().unwrap(), "setup.intunewin");

        // Verify the ZIP structure
        let file = File::open(&output_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        // Check that both expected files exist
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        assert!(names.contains(&"IntuneWinPackage/Metadata/Detection.xml".to_string()));
        assert!(names.contains(&"IntuneWinPackage/Contents/IntunePackage.intunewin".to_string()));

        // Verify Detection.xml content
        {
            let mut detection = archive.by_name("IntuneWinPackage/Metadata/Detection.xml").unwrap();
            let mut content = String::new();
            detection.read_to_string(&mut content).unwrap();
            assert_eq!(content, detection_xml);
        }

        // Verify encrypted content
        {
            let mut encrypted = archive.by_name("IntuneWinPackage/Contents/IntunePackage.intunewin").unwrap();
            let mut enc_content = Vec::new();
            encrypted.read_to_end(&mut enc_content).unwrap();
            assert_eq!(enc_content, b"mock encrypted content");
        }
    }
}
