//! Cryptographic key generation using secure random bytes.
//!
//! Provides functions for generating AES keys, initialization vectors,
//! and HMAC keys with cryptographically secure random data.

use rand::RngCore;

/// Generates a cryptographically secure 256-bit AES key.
///
/// # Returns
/// A 32-byte array containing the random key material.
///
/// # Example
/// ```
/// use intunewin_rs::crypto::generate_aes_key;
/// let key = generate_aes_key();
/// assert_eq!(key.len(), 32);
/// ```
pub fn generate_aes_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::rng().fill_bytes(&mut key);
    key
}

/// Generates a cryptographically secure 128-bit initialization vector for AES-CBC.
///
/// # Returns
/// A 16-byte array containing the random IV.
///
/// # Example
/// ```
/// use intunewin_rs::crypto::generate_iv;
/// let iv = generate_iv();
/// assert_eq!(iv.len(), 16);
/// ```
pub fn generate_iv() -> [u8; 16] {
    let mut iv = [0u8; 16];
    rand::rng().fill_bytes(&mut iv);
    iv
}

/// Generates a cryptographically secure 256-bit HMAC key.
///
/// # Returns
/// A 32-byte array containing the random key material.
///
/// # Example
/// ```
/// use intunewin_rs::crypto::generate_mac_key;
/// let mac_key = generate_mac_key();
/// assert_eq!(mac_key.len(), 32);
/// ```
pub fn generate_mac_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::rng().fill_bytes(&mut key);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_aes_key_length() {
        let key = generate_aes_key();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_generate_aes_key_randomness() {
        let key1 = generate_aes_key();
        let key2 = generate_aes_key();
        // Two random keys should not be equal (with overwhelming probability)
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_generate_iv_length() {
        let iv = generate_iv();
        assert_eq!(iv.len(), 16);
    }

    #[test]
    fn test_generate_iv_randomness() {
        let iv1 = generate_iv();
        let iv2 = generate_iv();
        assert_ne!(iv1, iv2);
    }

    #[test]
    fn test_generate_mac_key_length() {
        let key = generate_mac_key();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_generate_mac_key_randomness() {
        let key1 = generate_mac_key();
        let key2 = generate_mac_key();
        assert_ne!(key1, key2);
    }
}
