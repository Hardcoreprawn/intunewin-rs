//! HMAC-SHA256 computation for data authentication.
//!
//! Provides message authentication code functionality as required
//! by the Microsoft IntuneWin format.

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Type alias for HMAC-SHA256.
type HmacSha256 = Hmac<Sha256>;

/// Computes HMAC-SHA256 of the given data using the provided key.
///
/// # Arguments
/// * `key` - The HMAC key (any length, but 32 bytes recommended)
/// * `data` - The data to authenticate
///
/// # Returns
/// A 32-byte array containing the HMAC-SHA256 result.
///
/// # Example
/// ```
/// use intunewin_rs::crypto::compute_hmac_sha256;
///
/// let key = [0u8; 32];
/// let data = b"Hello, World!";
/// let mac = compute_hmac_sha256(&key, data);
/// assert_eq!(mac.len(), 32);
/// ```
pub fn compute_hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can accept key of any size");
    mac.update(data);
    let result = mac.finalize();
    result.into_bytes().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hmac_sha256_basic() {
        let key = [0u8; 32];
        let data = b"test data";

        let mac = compute_hmac_sha256(&key, data);

        assert_eq!(mac.len(), 32);
    }

    #[test]
    fn test_compute_hmac_sha256_deterministic() {
        let key = [1u8; 32];
        let data = b"test data";

        let mac1 = compute_hmac_sha256(&key, data);
        let mac2 = compute_hmac_sha256(&key, data);

        // Same key and data should produce same MAC
        assert_eq!(mac1, mac2);
    }

    #[test]
    fn test_compute_hmac_sha256_different_keys() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let data = b"test data";

        let mac1 = compute_hmac_sha256(&key1, data);
        let mac2 = compute_hmac_sha256(&key2, data);

        // Different keys should produce different MACs
        assert_ne!(mac1, mac2);
    }

    #[test]
    fn test_compute_hmac_sha256_different_data() {
        let key = [0u8; 32];
        let data1 = b"test data 1";
        let data2 = b"test data 2";

        let mac1 = compute_hmac_sha256(&key, data1);
        let mac2 = compute_hmac_sha256(&key, data2);

        // Different data should produce different MACs
        assert_ne!(mac1, mac2);
    }

    #[test]
    fn test_compute_hmac_sha256_empty_data() {
        let key = [0u8; 32];
        let data = b"";

        let mac = compute_hmac_sha256(&key, data);

        // Should still produce valid 32-byte output
        assert_eq!(mac.len(), 32);
    }

    #[test]
    fn test_compute_hmac_sha256_variable_key_length() {
        // HMAC accepts keys of any length
        // Use non-zero keys to demonstrate different results
        let short_key = vec![1u8; 16];
        let long_key = vec![1u8; 64];
        let data = b"test";

        let mac1 = compute_hmac_sha256(&short_key, data);
        let mac2 = compute_hmac_sha256(&long_key, data);

        // Different key lengths produce different results
        // (note: all-zero keys might produce same result due to HMAC padding)
        assert_ne!(mac1, mac2);
        assert_eq!(mac1.len(), 32);
        assert_eq!(mac2.len(), 32);
    }

    #[test]
    fn test_compute_hmac_sha256_known_vector() {
        // Test with a known HMAC-SHA256 test vector
        // Key: empty string, Data: empty string
        // Expected: b613679a0814d9ec772f95d778c35fc5ff1697c493715653c6c712144292c5ad
        let key = b"";
        let data = b"";

        let mac = compute_hmac_sha256(key, data);

        let expected = [
            0xb6, 0x13, 0x67, 0x9a, 0x08, 0x14, 0xd9, 0xec, 0x77, 0x2f, 0x95, 0xd7, 0x78, 0xc3,
            0x5f, 0xc5, 0xff, 0x16, 0x97, 0xc4, 0x93, 0x71, 0x56, 0x53, 0xc6, 0xc7, 0x12, 0x14,
            0x42, 0x92, 0xc5, 0xad,
        ];

        assert_eq!(mac, expected);
    }
}
