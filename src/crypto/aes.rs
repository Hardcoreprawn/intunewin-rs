//! AES-256-CBC encryption implementation.
//!
//! Provides encryption functionality compatible with Microsoft IntuneWin format,
//! using AES-256 in CBC mode with PKCS7 padding.
//!
//! Supports both in-memory and streaming encryption for handling large files
//! without excessive memory usage.

use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use hmac::Hmac;

use crate::crypto::{compute_hmac_sha256, generate_aes_key, generate_iv, generate_mac_key};
use crate::error::{IntunewinError, Result};

/// Type alias for AES-256-CBC encryptor with PKCS7 padding.
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

/// Result of encrypting data, containing all cryptographic material needed
/// for decryption and verification.
#[derive(Debug, Clone)]
pub struct EncryptionResult {
    /// The encrypted data (ciphertext)
    pub encrypted_data: Vec<u8>,
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

/// Encrypts data using AES-256-CBC with PKCS7 padding.
///
/// # Arguments
/// * `plaintext` - The data to encrypt
/// * `key` - The 256-bit AES key
/// * `iv` - The 128-bit initialization vector
///
/// # Returns
/// * `Ok(Vec<u8>)` - The encrypted ciphertext
/// * `Err(IntunewinError)` - If encryption fails
///
/// # Example
/// ```
/// use intunewin_rs::crypto::{encrypt_aes256_cbc, generate_aes_key, generate_iv};
///
/// let key = generate_aes_key();
/// let iv = generate_iv();
/// let plaintext = b"Hello, World!";
///
/// let ciphertext = encrypt_aes256_cbc(plaintext, &key, &iv).unwrap();
/// assert!(!ciphertext.is_empty());
/// // Ciphertext length is padded to block boundary (16 bytes)
/// assert_eq!(ciphertext.len() % 16, 0);
/// ```
pub fn encrypt_aes256_cbc(plaintext: &[u8], key: &[u8; 32], iv: &[u8; 16]) -> Result<Vec<u8>> {
    // Calculate the required buffer size with PKCS7 padding
    // PKCS7 always adds padding, even if input is block-aligned
    let block_size = 16;
    let padded_len = ((plaintext.len() / block_size) + 1) * block_size;

    // Create buffer with space for padding
    let mut buffer = vec![0u8; padded_len];
    buffer[..plaintext.len()].copy_from_slice(plaintext);

    // Create the encryptor
    let encryptor = Aes256CbcEnc::new(key.into(), iv.into());

    // Encrypt with PKCS7 padding
    let ciphertext = encryptor
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
        .map_err(|e| IntunewinError::EncryptionError(format!("AES encryption failed: {}", e)))?;

    Ok(ciphertext.to_vec())
}

/// Performs full encryption of data with key generation, producing all required
/// cryptographic material for the IntuneWin format.
///
/// This function:
/// 1. Generates random AES key, IV, and MAC key
/// 2. Encrypts the plaintext using AES-256-CBC
/// 3. Computes HMAC-SHA256 of the ciphertext
/// 4. Computes SHA256 digest of the ciphertext
///
/// # Arguments
/// * `plaintext` - The data to encrypt
///
/// # Returns
/// * `Ok(EncryptionResult)` - Complete encryption result with all keys and metadata
/// * `Err(IntunewinError)` - If encryption fails
pub fn encrypt_with_keygen(plaintext: &[u8]) -> Result<EncryptionResult> {
    // Generate cryptographic material
    let key = generate_aes_key();
    let iv = generate_iv();
    let mac_key = generate_mac_key();

    // Encrypt the data
    let encrypted_data = encrypt_aes256_cbc(plaintext, &key, &iv)?;

    // Compute HMAC of encrypted data
    let mac = compute_hmac_sha256(&mac_key, &encrypted_data);

    // Compute SHA256 digest of encrypted data
    let mut hasher = Sha256::new();
    hasher.update(&encrypted_data);
    let file_digest: [u8; 32] = hasher.finalize().into();

    Ok(EncryptionResult {
        encrypted_data,
        key,
        iv,
        mac_key,
        mac,
        file_digest,
    })
}

/// Result of streaming encryption (keys only, data written to file)
#[derive(Debug, Clone)]
pub struct StreamingEncryptionResult {
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
    /// Size of encrypted output in bytes
    pub encrypted_size: u64,
}

/// Encrypts a file using AES-256-CBC with streaming I/O.
///
/// This function processes the input file in chunks, avoiding loading
/// the entire file into memory. Ideal for large files (multi-GB).
///
/// # Arguments
/// * `input_path` - Path to the plaintext file to encrypt
/// * `output_path` - Path where encrypted output will be written
///
/// # Returns
/// * `Ok(StreamingEncryptionResult)` - Encryption keys and metadata
/// * `Err(IntunewinError)` - If encryption fails
pub fn encrypt_file_streaming(
    input_path: &Path,
    output_path: &Path,
) -> Result<StreamingEncryptionResult> {
    use aes::cipher::KeyInit;
    use hmac::Mac as HmacMac;
    
    // Buffer size: 64KB (must be multiple of AES block size 16)
    const BUFFER_SIZE: usize = 64 * 1024;
    
    // Generate cryptographic material
    let key = generate_aes_key();
    let iv = generate_iv();
    let mac_key = generate_mac_key();
    
    // Open input file
    let input_file = File::open(input_path).map_err(|e| IntunewinError::FileReadError {
        path: input_path.to_path_buf(),
        source: e,
    })?;
    let input_size = input_file.metadata()
        .map_err(|e| IntunewinError::FileReadError {
            path: input_path.to_path_buf(),
            source: e,
        })?.len();
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, input_file);
    
    // Create output file
    let output_file = File::create(output_path).map_err(|e| IntunewinError::FileWriteError {
        path: output_path.to_path_buf(),
        source: e,
    })?;
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, output_file);
    
    // Initialize cipher state
    let cipher = aes::Aes256::new((&key).into());
    let mut current_iv = iv;
    
    // Initialize HMAC and SHA256 for computing on-the-fly
    let mut hmac_ctx: Hmac<Sha256> = HmacMac::new_from_slice(&mac_key)
        .expect("HMAC can accept key of any size");
    let mut hasher = Sha256::new();
    
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut total_written: u64 = 0;
    let mut bytes_remaining = input_size;
    
    loop {
        let bytes_read = reader.read(&mut buffer).map_err(|e| IntunewinError::FileReadError {
            path: input_path.to_path_buf(),
            source: e,
        })?;
        
        if bytes_read == 0 {
            break;
        }
        
        bytes_remaining -= bytes_read as u64;
        let is_last_chunk = bytes_remaining == 0;
        
        // Process this chunk
        let encrypted_chunk = if is_last_chunk {
            // Last chunk: apply PKCS7 padding
            encrypt_chunk_with_padding(&buffer[..bytes_read], &cipher, &mut current_iv)
        } else {
            // Middle chunk: no padding, must be block-aligned reads
            // If not aligned, we'll handle it in the last chunk
            if bytes_read % 16 != 0 {
                // This shouldn't happen with our buffer size, but handle it
                encrypt_chunk_with_padding(&buffer[..bytes_read], &cipher, &mut current_iv)
            } else {
                encrypt_chunk_no_padding(&buffer[..bytes_read], &cipher, &mut current_iv)
            }
        };
        
        // Update HMAC and hash
        HmacMac::update(&mut hmac_ctx, &encrypted_chunk);
        hasher.update(&encrypted_chunk);
        
        // Write encrypted data
        writer.write_all(&encrypted_chunk).map_err(|e| IntunewinError::FileWriteError {
            path: output_path.to_path_buf(),
            source: e,
        })?;
        
        total_written += encrypted_chunk.len() as u64;
    }
    
    // Handle empty file case
    if input_size == 0 {
        let encrypted_chunk = encrypt_chunk_with_padding(&[], &cipher, &mut current_iv);
        HmacMac::update(&mut hmac_ctx, &encrypted_chunk);
        hasher.update(&encrypted_chunk);
        writer.write_all(&encrypted_chunk).map_err(|e| IntunewinError::FileWriteError {
            path: output_path.to_path_buf(),
            source: e,
        })?;
        total_written += encrypted_chunk.len() as u64;
    }
    
    // Flush writer
    writer.flush().map_err(|e| IntunewinError::FileWriteError {
        path: output_path.to_path_buf(),
        source: e,
    })?;
    
    // Finalize HMAC and hash
    let mac: [u8; 32] = hmac_ctx.finalize().into_bytes().into();
    let file_digest: [u8; 32] = hasher.finalize().into();
    
    Ok(StreamingEncryptionResult {
        key,
        iv,
        mac_key,
        mac,
        file_digest,
        encrypted_size: total_written,
    })
}

/// Encrypt a chunk with CBC mode, no padding (for middle chunks)
fn encrypt_chunk_no_padding(
    plaintext: &[u8],
    cipher: &aes::Aes256,
    current_iv: &mut [u8; 16],
) -> Vec<u8> {
    use aes::cipher::BlockEncrypt;
    
    let mut output = Vec::with_capacity(plaintext.len());
    
    for chunk in plaintext.chunks(16) {
        let mut block = [0u8; 16];
        block.copy_from_slice(chunk);
        
        // XOR with IV (CBC mode)
        for (b, iv_byte) in block.iter_mut().zip(current_iv.iter()) {
            *b ^= iv_byte;
        }
        
        // Encrypt block
        cipher.encrypt_block((&mut block).into());
        
        // Update IV for next block
        current_iv.copy_from_slice(&block);
        
        output.extend_from_slice(&block);
    }
    
    output
}

/// Encrypt a chunk with PKCS7 padding (for last chunk)
fn encrypt_chunk_with_padding(
    plaintext: &[u8],
    cipher: &aes::Aes256,
    current_iv: &mut [u8; 16],
) -> Vec<u8> {
    use aes::cipher::BlockEncrypt;
    
    // Calculate padded size
    let block_size = 16;
    let padding_len = block_size - (plaintext.len() % block_size);
    let padded_len = plaintext.len() + padding_len;
    
    let mut padded = vec![0u8; padded_len];
    padded[..plaintext.len()].copy_from_slice(plaintext);
    
    // Apply PKCS7 padding
    for byte in padded[plaintext.len()..].iter_mut() {
        *byte = padding_len as u8;
    }
    
    // Encrypt all blocks
    let mut output = Vec::with_capacity(padded_len);
    
    for chunk in padded.chunks(16) {
        let mut block = [0u8; 16];
        block.copy_from_slice(chunk);
        
        // XOR with IV (CBC mode)
        for (b, iv_byte) in block.iter_mut().zip(current_iv.iter()) {
            *b ^= iv_byte;
        }
        
        // Encrypt block
        cipher.encrypt_block((&mut block).into());
        
        // Update IV for next block
        current_iv.copy_from_slice(&block);
        
        output.extend_from_slice(&block);
    }
    
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_aes256_cbc_basic() {
        let key = [0u8; 32];
        let iv = [0u8; 16];
        let plaintext = b"Hello, World!";

        let ciphertext = encrypt_aes256_cbc(plaintext, &key, &iv).unwrap();

        // Ciphertext should be block-aligned
        assert_eq!(ciphertext.len() % 16, 0);
        // For 13-byte input, should be 16 bytes (one block with padding)
        assert_eq!(ciphertext.len(), 16);
    }

    #[test]
    fn test_encrypt_aes256_cbc_block_aligned_input() {
        let key = [0u8; 32];
        let iv = [0u8; 16];
        // Exactly 16 bytes
        let plaintext = b"0123456789ABCDEF";

        let ciphertext = encrypt_aes256_cbc(plaintext, &key, &iv).unwrap();

        // PKCS7 always adds padding, so 16-byte input becomes 32 bytes
        assert_eq!(ciphertext.len(), 32);
    }

    #[test]
    fn test_encrypt_aes256_cbc_deterministic() {
        let key = [1u8; 32];
        let iv = [2u8; 16];
        let plaintext = b"Test data";

        let ciphertext1 = encrypt_aes256_cbc(plaintext, &key, &iv).unwrap();
        let ciphertext2 = encrypt_aes256_cbc(plaintext, &key, &iv).unwrap();

        // Same key, IV, and plaintext should produce same ciphertext
        assert_eq!(ciphertext1, ciphertext2);
    }

    #[test]
    fn test_encrypt_aes256_cbc_different_keys() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let iv = [0u8; 16];
        let plaintext = b"Test data";

        let ciphertext1 = encrypt_aes256_cbc(plaintext, &key1, &iv).unwrap();
        let ciphertext2 = encrypt_aes256_cbc(plaintext, &key2, &iv).unwrap();

        // Different keys should produce different ciphertext
        assert_ne!(ciphertext1, ciphertext2);
    }

    #[test]
    fn test_encrypt_aes256_cbc_different_ivs() {
        let key = [0u8; 32];
        let iv1 = [1u8; 16];
        let iv2 = [2u8; 16];
        let plaintext = b"Test data";

        let ciphertext1 = encrypt_aes256_cbc(plaintext, &key, &iv1).unwrap();
        let ciphertext2 = encrypt_aes256_cbc(plaintext, &key, &iv2).unwrap();

        // Different IVs should produce different ciphertext
        assert_ne!(ciphertext1, ciphertext2);
    }

    #[test]
    fn test_encrypt_aes256_cbc_empty_input() {
        let key = [0u8; 32];
        let iv = [0u8; 16];
        let plaintext = b"";

        let ciphertext = encrypt_aes256_cbc(plaintext, &key, &iv).unwrap();

        // Empty input with PKCS7 padding produces one block
        assert_eq!(ciphertext.len(), 16);
    }

    #[test]
    fn test_encrypt_aes256_cbc_large_input() {
        let key = [0u8; 32];
        let iv = [0u8; 16];
        let plaintext = vec![0u8; 10000];

        let ciphertext = encrypt_aes256_cbc(&plaintext, &key, &iv).unwrap();

        // Should be padded to next block boundary
        assert_eq!(ciphertext.len(), 10016);
    }

    #[test]
    fn test_encrypt_with_keygen() {
        let plaintext = b"Test encryption data";

        let result = encrypt_with_keygen(plaintext).unwrap();

        // Verify all fields are populated
        assert!(!result.encrypted_data.is_empty());
        assert_eq!(result.key.len(), 32);
        assert_eq!(result.iv.len(), 16);
        assert_eq!(result.mac_key.len(), 32);
        assert_eq!(result.mac.len(), 32);
        assert_eq!(result.file_digest.len(), 32);

        // Verify ciphertext is block-aligned
        assert_eq!(result.encrypted_data.len() % 16, 0);
    }

    #[test]
    fn test_encrypt_with_keygen_randomness() {
        let plaintext = b"Test encryption data";

        let result1 = encrypt_with_keygen(plaintext).unwrap();
        let result2 = encrypt_with_keygen(plaintext).unwrap();

        // Different keys should be generated
        assert_ne!(result1.key, result2.key);
        assert_ne!(result1.iv, result2.iv);
        assert_ne!(result1.mac_key, result2.mac_key);

        // Different ciphertext due to different keys
        assert_ne!(result1.encrypted_data, result2.encrypted_data);
    }
}
