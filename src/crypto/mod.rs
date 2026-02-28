//! Cryptographic operations for IntuneWin packages.
//!
//! This module provides AES-256-CBC encryption and HMAC-SHA256 authentication
//! as required by the Microsoft IntuneWin format.

pub mod aes;
pub mod hmac;
pub mod keygen;

pub use aes::{
    encrypt_aes256_cbc, encrypt_chunk_no_padding, encrypt_chunk_no_padding_inplace,
    encrypt_chunk_with_padding, encrypt_file_streaming, EncryptionResult,
    StreamingEncryptionResult,
};
pub use hmac::compute_hmac_sha256;
pub use keygen::{generate_aes_key, generate_iv, generate_mac_key};
