//! Detection.xml generation for IntuneWin packages.
//!
//! The Detection.xml file contains metadata about the encrypted package,
//! including encryption keys, MACs, and file information needed by Intune
//! to decrypt and verify the package.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::crypto::EncryptionResult;
use crate::error::Result;

/// Tool version string to match Microsoft IntuneWinAppUtil output.
const TOOL_VERSION: &str = "1.8.7.0";

/// Metadata required to generate Detection.xml.
#[derive(Debug, Clone)]
pub struct DetectionInfo {
    /// Name of the application (typically the setup file name without extension)
    pub name: String,
    /// Original unencrypted content size in bytes
    pub unencrypted_content_size: u64,
    /// Name of the setup file
    pub setup_file: String,
    /// Encryption result containing keys and ciphertext metadata
    pub encryption: EncryptionResult,
}

/// Metadata for streaming encryption (keys passed directly, no encrypted_data)
#[derive(Debug, Clone)]
pub struct StreamingDetectionInfo {
    /// Name of the application (typically the setup file name without extension)
    pub name: String,
    /// Original unencrypted content size in bytes
    pub unencrypted_content_size: u64,
    /// Name of the setup file
    pub setup_file: String,
    /// The AES-256 encryption key (32 bytes)
    pub key: [u8; 32],
    /// The initialization vector (16 bytes)
    pub iv: [u8; 16],
    /// The HMAC-SHA256 key (32 bytes)
    pub mac_key: [u8; 32],
    /// The HMAC-SHA256 of the encrypted data (32 bytes)
    pub mac: [u8; 32],
    /// SHA256 digest of the encrypted data (32 bytes)
    pub file_digest: [u8; 32],
}

/// Generates Detection.xml content matching Microsoft IntuneWinAppUtil format.
///
/// # Arguments
/// * `info` - Detection metadata including encryption details
///
/// # Returns
/// * `Ok(String)` - The XML content of Detection.xml
///
/// # Example
/// ```ignore
/// use intunewin_rs::format::detection::{generate_detection_xml, DetectionInfo};
/// use intunewin_rs::crypto::encrypt_with_keygen;
///
/// let encryption = encrypt_with_keygen(b"test data").unwrap();
/// let info = DetectionInfo {
///     name: "setup".to_string(),
///     unencrypted_content_size: 1024,
///     setup_file: "setup.exe".to_string(),
///     encryption,
/// };
///
/// let xml = generate_detection_xml(&info).unwrap();
/// assert!(xml.contains("<Name>setup</Name>"));
/// ```
pub fn generate_detection_xml(info: &DetectionInfo) -> Result<String> {
    // Encode binary values as base64
    let key_b64 = BASE64.encode(info.encryption.key);
    let mac_key_b64 = BASE64.encode(info.encryption.mac_key);
    let iv_b64 = BASE64.encode(info.encryption.iv);
    let mac_b64 = BASE64.encode(info.encryption.mac);
    let file_digest_b64 = BASE64.encode(info.encryption.file_digest);

    // Build the XML string
    // Note: Microsoft format uses specific formatting and ordering
    let xml = format!(
        r#"<ApplicationInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" ToolVersion="{}">
  <Name>{}</Name>
  <UnencryptedContentSize>{}</UnencryptedContentSize>
  <FileName>IntunePackage.intunewin</FileName>
  <SetupFile>{}</SetupFile>
  <EncryptionInfo>
    <EncryptionKey>{}</EncryptionKey>
    <MacKey>{}</MacKey>
    <InitializationVector>{}</InitializationVector>
    <Mac>{}</Mac>
    <ProfileIdentifier>ProfileVersion1</ProfileIdentifier>
    <FileDigest>{}</FileDigest>
    <FileDigestAlgorithm>SHA256</FileDigestAlgorithm>
  </EncryptionInfo>
</ApplicationInfo>"#,
        TOOL_VERSION,
        escape_xml(&info.name),
        info.unencrypted_content_size,
        escape_xml(&info.setup_file),
        key_b64,
        mac_key_b64,
        iv_b64,
        mac_b64,
        file_digest_b64
    );

    Ok(xml)
}

/// Generates Detection.xml content for streaming encryption (no in-memory encrypted data).
pub fn generate_detection_xml_streaming(info: &StreamingDetectionInfo) -> Result<String> {
    // Encode binary values as base64
    let key_b64 = BASE64.encode(info.key);
    let mac_key_b64 = BASE64.encode(info.mac_key);
    let iv_b64 = BASE64.encode(info.iv);
    let mac_b64 = BASE64.encode(info.mac);
    let file_digest_b64 = BASE64.encode(info.file_digest);

    let xml = format!(
        r#"<ApplicationInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" ToolVersion="{}">
  <Name>{}</Name>
  <UnencryptedContentSize>{}</UnencryptedContentSize>
  <FileName>IntunePackage.intunewin</FileName>
  <SetupFile>{}</SetupFile>
  <EncryptionInfo>
    <EncryptionKey>{}</EncryptionKey>
    <MacKey>{}</MacKey>
    <InitializationVector>{}</InitializationVector>
    <Mac>{}</Mac>
    <ProfileIdentifier>ProfileVersion1</ProfileIdentifier>
    <FileDigest>{}</FileDigest>
    <FileDigestAlgorithm>SHA256</FileDigestAlgorithm>
  </EncryptionInfo>
</ApplicationInfo>"#,
        TOOL_VERSION,
        escape_xml(&info.name),
        info.unencrypted_content_size,
        escape_xml(&info.setup_file),
        key_b64,
        mac_key_b64,
        iv_b64,
        mac_b64,
        file_digest_b64
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::aes::encrypt_with_keygen;

    fn create_test_encryption_result() -> EncryptionResult {
        // Create a deterministic encryption result for testing
        EncryptionResult {
            encrypted_data: vec![1, 2, 3, 4],
            key: [0u8; 32],
            iv: [0u8; 16],
            mac_key: [0u8; 32],
            mac: [0u8; 32],
            file_digest: [0u8; 32],
        }
    }

    #[test]
    fn test_generate_detection_xml_structure() {
        let info = DetectionInfo {
            name: "TestApp".to_string(),
            unencrypted_content_size: 1024,
            setup_file: "setup.exe".to_string(),
            encryption: create_test_encryption_result(),
        };

        let xml = generate_detection_xml(&info).unwrap();

        // Verify XML structure
        assert!(xml.contains("<ApplicationInfo"));
        assert!(xml.contains("xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\""));
        assert!(xml.contains("xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\""));
        assert!(xml.contains("ToolVersion=\"1.8.7.0\""));
        assert!(xml.contains("<Name>TestApp</Name>"));
        assert!(xml.contains("<UnencryptedContentSize>1024</UnencryptedContentSize>"));
        assert!(xml.contains("<FileName>IntunePackage.intunewin</FileName>"));
        assert!(xml.contains("<SetupFile>setup.exe</SetupFile>"));
        assert!(xml.contains("<EncryptionInfo>"));
        assert!(xml.contains("<EncryptionKey>"));
        assert!(xml.contains("<MacKey>"));
        assert!(xml.contains("<InitializationVector>"));
        assert!(xml.contains("<Mac>"));
        assert!(xml.contains("<ProfileIdentifier>ProfileVersion1</ProfileIdentifier>"));
        assert!(xml.contains("<FileDigest>"));
        assert!(xml.contains("<FileDigestAlgorithm>SHA256</FileDigestAlgorithm>"));
        assert!(xml.contains("</EncryptionInfo>"));
        assert!(xml.contains("</ApplicationInfo>"));
    }

    #[test]
    fn test_generate_detection_xml_base64_encoding() {
        let info = DetectionInfo {
            name: "TestApp".to_string(),
            unencrypted_content_size: 1024,
            setup_file: "setup.exe".to_string(),
            encryption: create_test_encryption_result(),
        };

        let xml = generate_detection_xml(&info).unwrap();

        // All-zeros key should encode to specific base64
        // 32 zero bytes = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        let zero_32_b64 = BASE64.encode([0u8; 32]);
        let zero_16_b64 = BASE64.encode([0u8; 16]);

        assert!(xml.contains(&format!("<EncryptionKey>{}</EncryptionKey>", zero_32_b64)));
        assert!(xml.contains(&format!("<MacKey>{}</MacKey>", zero_32_b64)));
        assert!(xml.contains(&format!("<InitializationVector>{}</InitializationVector>", zero_16_b64)));
        assert!(xml.contains(&format!("<Mac>{}</Mac>", zero_32_b64)));
        assert!(xml.contains(&format!("<FileDigest>{}</FileDigest>", zero_32_b64)));
    }

    #[test]
    fn test_generate_detection_xml_escapes_special_chars() {
        let info = DetectionInfo {
            name: "Test<App>&\"Name'".to_string(),
            unencrypted_content_size: 1024,
            setup_file: "setup<>&.exe".to_string(),
            encryption: create_test_encryption_result(),
        };

        let xml = generate_detection_xml(&info).unwrap();

        // Special characters should be escaped
        assert!(xml.contains("<Name>Test&lt;App&gt;&amp;&quot;Name&apos;</Name>"));
        assert!(xml.contains("<SetupFile>setup&lt;&gt;&amp;.exe</SetupFile>"));
    }

    #[test]
    fn test_generate_detection_xml_with_real_encryption() {
        let encryption = encrypt_with_keygen(b"test data for encryption").unwrap();
        let info = DetectionInfo {
            name: "RealApp".to_string(),
            unencrypted_content_size: 25,
            setup_file: "install.msi".to_string(),
            encryption,
        };

        let xml = generate_detection_xml(&info).unwrap();

        // Verify structure is valid
        assert!(xml.contains("<Name>RealApp</Name>"));
        assert!(xml.contains("<UnencryptedContentSize>25</UnencryptedContentSize>"));
        assert!(xml.contains("<SetupFile>install.msi</SetupFile>"));

        // Verify base64 values are present (they should be valid base64 strings)
        // The actual values will vary due to random key generation
        assert!(xml.contains("<EncryptionKey>"));
        assert!(xml.contains("</EncryptionKey>"));
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("normal text"), "normal text");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(escape_xml("it's"), "it&apos;s");
        assert_eq!(
            escape_xml("<a & 'b' \"c\">"),
            "&lt;a &amp; &apos;b&apos; &quot;c&quot;&gt;"
        );
    }
}
