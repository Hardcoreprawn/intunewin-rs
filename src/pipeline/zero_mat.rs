//! Zero-materialization pipeline for compression-0 packages.
//!
//! When compression is disabled (level 0), the inner ZIP structure is fully
//! deterministic — every byte's position is known before reading a single
//! source file. This lets us stream source files directly through
//! [ZIP headers → AES-CBC encryptor → outer ZIP writer] without ever
//! materializing the inner ZIP as a file or buffer.
//!
//! I/O budget: read sources once + write output once = theoretical minimum.
//! Memory budget: one source file + encryption buffer + metadata ≈ O(largest_file).
//!
//! ## Channeled variant
//!
//! `run_zero_mat_channeled()` splits the pipeline into two threads connected
//! by a bounded `crossbeam::channel`:
//!   - **Producer**: reads source files, computes CRC32, serializes ZIP structure bytes
//!   - **Consumer**: receives byte chunks, encrypts (AES-CBC), hashes (HMAC+SHA256), writes
//!
//! The bounded channel applies backpressure so memory stays bounded at
//! `channel_depth * chunk_size` regardless of package size.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use aes::cipher::KeyInit;
use hmac::Mac as HmacMac;
use indicatif::ProgressBar;
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

use crate::crypto::{
    encrypt_chunk_no_padding, encrypt_chunk_with_padding, generate_aes_key, generate_iv,
    generate_mac_key,
};
use crate::error::{IntunewinError, Result};
use crate::format::detection::{generate_detection_xml_streaming, StreamingDetectionInfo};
use crate::io::read_file_smart;
use crate::pipeline::discovery::DiscoveryResult;

const BUFFER_SIZE: usize = 64 * 1024;
const ZIP32_MAX_U32: u64 = u32::MAX as u64;
const ZIP32_MAX_ENTRY_COUNT: usize = u16::MAX as usize;
const ZIP32_MAX_NAME_LEN: usize = u16::MAX as usize;

/// Result of the zero-materialization pipeline.
pub struct ZeroMatResult {
    /// Path to the final .intunewin file
    pub output_path: PathBuf,
    /// Size of the final package
    pub final_size: u64,
    /// Size of the inner ZIP (computed, never materialized)
    pub inner_zip_size: u64,
    /// Size of the encrypted content
    pub encrypted_size: u64,
}

// ── Inner ZIP size computation ────────────────────────────────────────

/// Compute the exact inner ZIP size at compression 0 from discovery metadata.
///
/// At stored (compression 0), compressed_size == uncompressed_size for every file.
/// The ZIP structure is:
///   Per file: local header (30 + name_len) + data (file_size)
///   Central directory: per file (46 + name_len)
///   EOCD: 22 bytes
fn compute_inner_zip_size(discovery: &DiscoveryResult) -> u64 {
    let mut size: u64 = 0;
    for f in &discovery.files {
        let name_len = f.normalized_path.len() as u64;
        size += 30 + name_len + f.size; // local file header + data
        size += 46 + name_len; // central directory entry
    }
    size += 22; // EOCD
    size
}

// ── EncryptingWriter ──────────────────────────────────────────────────

/// A `Write` adapter that encrypts bytes via AES-256-CBC and accumulates
/// HMAC-SHA256 and SHA-256 digests on-the-fly.
///
/// Bytes written to this adapter are buffered to 16-byte AES block boundaries,
/// encrypted, and flushed to the inner writer in chunks. Call `finish()` to
/// apply PKCS7 padding to the final block and retrieve the crypto results.
struct EncryptingWriter<W: Write> {
    inner: W,
    cipher: aes::Aes256,
    iv: [u8; 16],
    hmac: hmac::Hmac<Sha256>,
    hasher: Sha256,
    /// Residual bytes not yet aligned to a 16-byte AES block (0..15 bytes).
    residual: Vec<u8>,
    total_encrypted: u64,
    /// Crypto material needed for Detection.xml
    key: [u8; 32],
    raw_iv: [u8; 16],
    mac_key: [u8; 32],
}

/// Crypto results from the EncryptingWriter after finalization.
struct EncryptionFinish {
    key: [u8; 32],
    iv: [u8; 16],
    mac_key: [u8; 32],
    mac: [u8; 32],
    file_digest: [u8; 32],
    total_encrypted: u64,
}

impl<W: Write> EncryptingWriter<W> {
    fn new(inner: W) -> Self {
        let key = generate_aes_key();
        let iv = generate_iv();
        let mac_key = generate_mac_key();
        let cipher = aes::Aes256::new((&key).into());
        let hmac = <hmac::Hmac<Sha256> as HmacMac>::new_from_slice(&mac_key)
            .expect("HMAC accepts any key size");
        let hasher = Sha256::new();

        Self {
            inner,
            cipher,
            iv,
            hmac,
            hasher,
            residual: Vec::with_capacity(16),
            total_encrypted: 0,
            key,
            raw_iv: iv,
            mac_key,
        }
    }

    /// Encrypt and flush full AES-block-aligned data directly from a slice.
    /// The slice length MUST be a multiple of 16.
    fn encrypt_and_flush(&mut self, data: &[u8]) -> std::io::Result<()> {
        debug_assert!(data.len() % 16 == 0);
        let encrypted = encrypt_chunk_no_padding(data, &self.cipher, &mut self.iv);
        HmacMac::update(&mut self.hmac, &encrypted);
        self.hasher.update(&encrypted);
        self.inner.write_all(&encrypted)?;
        self.total_encrypted += encrypted.len() as u64;
        Ok(())
    }

    /// Finalize encryption: PKCS7-pad the remaining bytes and flush.
    /// Returns all crypto material needed for Detection.xml.
    fn finish(mut self) -> std::io::Result<EncryptionFinish> {
        let remaining = std::mem::take(&mut self.residual);
        let encrypted = encrypt_chunk_with_padding(&remaining, &self.cipher, &mut self.iv);
        HmacMac::update(&mut self.hmac, &encrypted);
        self.hasher.update(&encrypted);
        self.inner.write_all(&encrypted)?;
        self.total_encrypted += encrypted.len() as u64;

        let mac: [u8; 32] = self.hmac.finalize().into_bytes().into();
        let file_digest: [u8; 32] = self.hasher.finalize().into();

        Ok(EncryptionFinish {
            key: self.key,
            iv: self.raw_iv,
            mac_key: self.mac_key,
            mac,
            file_digest,
            total_encrypted: self.total_encrypted,
        })
    }
}

impl<W: Write> Write for EncryptingWriter<W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut pos = 0;

        // Step 1: If we have residual bytes, fill them to a 16-byte boundary.
        if !self.residual.is_empty() {
            let needed = 16 - self.residual.len();
            let take = data.len().min(needed);
            self.residual.extend_from_slice(&data[..take]);
            pos = take;

            if self.residual.len() == 16 {
                let block = std::mem::take(&mut self.residual);
                self.residual = Vec::with_capacity(16);
                self.encrypt_and_flush(&block)?;
            } else {
                // Still not enough for a full block — done for now.
                return Ok(data.len());
            }
        }

        // Step 2: Process full BUFFER_SIZE chunks directly from input (no copy).
        let remaining = &data[pos..];
        let aligned_end = (remaining.len() / 16) * 16;

        if aligned_end > 0 {
            // Process in BUFFER_SIZE increments to bound the encrypt allocation.
            let mut offset = 0;
            while offset < aligned_end {
                let chunk_end = (offset + BUFFER_SIZE).min(aligned_end);
                self.encrypt_and_flush(&remaining[offset..chunk_end])?;
                offset = chunk_end;
            }
        }

        // Step 3: Stash leftover bytes (< 16).
        if aligned_end < remaining.len() {
            self.residual.extend_from_slice(&remaining[aligned_end..]);
        }

        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

// ── ZIP format helpers ────────────────────────────────────────────────

fn checked_u32(value: u64, field: &str) -> Result<u32> {
    u32::try_from(value).map_err(|_| {
        IntunewinError::CompressionError(format!(
            "ZIP32 limit exceeded for {}: {} > {}",
            field, value, ZIP32_MAX_U32
        ))
    })
}

fn checked_u16_from_usize(value: usize, field: &str) -> Result<u16> {
    u16::try_from(value).map_err(|_| {
        IntunewinError::CompressionError(format!(
            "ZIP32 limit exceeded for {}: {} > {}",
            field,
            value,
            u16::MAX
        ))
    })
}

/// Metadata collected per file during ZIP entry writing, for the central directory.
struct CdEntry {
    normalized_path: String,
    crc32: u32,
    size: u32,
    local_header_offset: u32,
}

fn write_local_header(
    w: &mut impl Write,
    name: &[u8],
    crc32: u32,
    size: u32,
) -> std::io::Result<()> {
    let name_len = name.len() as u16;
    w.write_all(&0x04034b50u32.to_le_bytes())?; // signature
    w.write_all(&20u16.to_le_bytes())?; // version needed
    w.write_all(&0u16.to_le_bytes())?; // flags
    w.write_all(&0u16.to_le_bytes())?; // compression method (stored)
    w.write_all(&0u16.to_le_bytes())?; // mod time
    w.write_all(&0u16.to_le_bytes())?; // mod date
    w.write_all(&crc32.to_le_bytes())?;
    w.write_all(&size.to_le_bytes())?; // compressed size
    w.write_all(&size.to_le_bytes())?; // uncompressed size
    w.write_all(&name_len.to_le_bytes())?;
    w.write_all(&0u16.to_le_bytes())?; // extra field length
    w.write_all(name)?;
    Ok(())
}

fn write_central_dir_entry(w: &mut impl Write, entry: &CdEntry) -> std::io::Result<()> {
    let name_bytes = entry.normalized_path.as_bytes();
    let name_len = name_bytes.len() as u16;
    w.write_all(&0x02014b50u32.to_le_bytes())?; // signature
    w.write_all(&20u16.to_le_bytes())?; // version made by
    w.write_all(&20u16.to_le_bytes())?; // version needed
    w.write_all(&0u16.to_le_bytes())?; // flags
    w.write_all(&0u16.to_le_bytes())?; // compression method (stored)
    w.write_all(&0u16.to_le_bytes())?; // mod time
    w.write_all(&0u16.to_le_bytes())?; // mod date
    w.write_all(&entry.crc32.to_le_bytes())?;
    w.write_all(&entry.size.to_le_bytes())?; // compressed size
    w.write_all(&entry.size.to_le_bytes())?; // uncompressed size
    w.write_all(&name_len.to_le_bytes())?;
    w.write_all(&0u16.to_le_bytes())?; // extra field length
    w.write_all(&0u16.to_le_bytes())?; // comment length
    w.write_all(&0u16.to_le_bytes())?; // disk number
    w.write_all(&0u16.to_le_bytes())?; // internal attrs
    w.write_all(&0u32.to_le_bytes())?; // external attrs
    w.write_all(&entry.local_header_offset.to_le_bytes())?;
    w.write_all(name_bytes)?;
    Ok(())
}

fn write_eocd(
    w: &mut impl Write,
    entry_count: u16,
    cd_size: u32,
    cd_offset: u32,
) -> std::io::Result<()> {
    w.write_all(&0x06054b50u32.to_le_bytes())?; // signature
    w.write_all(&0u16.to_le_bytes())?; // disk number
    w.write_all(&0u16.to_le_bytes())?; // CD start disk
    w.write_all(&entry_count.to_le_bytes())?;
    w.write_all(&entry_count.to_le_bytes())?; // total entries
    w.write_all(&cd_size.to_le_bytes())?;
    w.write_all(&cd_offset.to_le_bytes())?;
    w.write_all(&0u16.to_le_bytes())?; // comment length
    Ok(())
}

// ── Main pipeline ─────────────────────────────────────────────────────

fn requires_zip64(size_bytes: u64) -> bool {
    size_bytes > ZIP32_MAX_U32
}

fn derive_output_filename(setup_name: &str) -> String {
    let stem = Path::new(setup_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| setup_name.to_string());
    format!("{}.intunewin", stem)
}

/// Pre-flight validation for the zero-materialization path.
fn validate_zero_mat(discovery: &DiscoveryResult) -> Result<()> {
    if discovery.files.len() > ZIP32_MAX_ENTRY_COUNT {
        return Err(IntunewinError::CompressionError(format!(
            "Too many files for ZIP32: {} (max {})",
            discovery.files.len(),
            ZIP32_MAX_ENTRY_COUNT
        )));
    }
    for f in &discovery.files {
        if f.size > ZIP32_MAX_U32 {
            return Err(IntunewinError::CompressionError(format!(
                "File '{}' is {} bytes, exceeds ZIP32 limit of {} bytes",
                f.normalized_path, f.size, ZIP32_MAX_U32
            )));
        }
        if f.normalized_path.len() > ZIP32_MAX_NAME_LEN {
            return Err(IntunewinError::CompressionError(format!(
                "File name too long for ZIP32: {} bytes (max {})",
                f.normalized_path.len(),
                ZIP32_MAX_NAME_LEN
            )));
        }
    }
    Ok(())
}

/// Run the zero-materialization pipeline.
///
/// Streams source files directly through ZIP structure generation →
/// AES-CBC encryption → outer .intunewin ZIP writer. The inner ZIP
/// never exists as a file or buffer.
pub fn run_zero_mat(
    discovery: &DiscoveryResult,
    setup_name: &str,
    output_folder: &Path,
    use_mmap: bool,
    progress_bar: Option<&ProgressBar>,
) -> Result<ZeroMatResult> {
    validate_zero_mat(discovery)?;

    let inner_zip_size = compute_inner_zip_size(discovery);
    let encrypted_size = ((inner_zip_size / 16) + 1) * 16;

    let output_filename = derive_output_filename(setup_name);
    let output_path = output_folder.join(&output_filename);

    if !output_folder.exists() {
        std::fs::create_dir_all(output_folder).map_err(|e| IntunewinError::FileWriteError {
            path: output_folder.to_path_buf(),
            source: e,
        })?;
    }

    // Open outer ZIP and start the encrypted content entry.
    let file = File::create(&output_path).map_err(|e| IntunewinError::FileWriteError {
        path: output_path.clone(),
        source: e,
    })?;
    let buffered_file = BufWriter::with_capacity(BUFFER_SIZE, file);
    let mut outer_zip = ZipWriter::new(buffered_file);

    let content_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(requires_zip64(encrypted_size));

    outer_zip
        .start_file(
            "IntuneWinPackage/Contents/IntunePackage.intunewin",
            content_options,
        )
        .map_err(|e| IntunewinError::ZipError(e.to_string()))?;

    // Wrap the outer ZIP in the encrypting writer.
    // Everything written to `enc` is encrypted and streamed into the open ZIP entry.
    let mut enc = EncryptingWriter::new(&mut outer_zip);

    // ── Stream inner ZIP entries through the encryptor ────────────────

    let mut cd_entries: Vec<CdEntry> = Vec::with_capacity(discovery.files.len());
    let mut local_offset: u64 = 0;

    for file_entry in &discovery.files {
        let name_bytes = file_entry.normalized_path.as_bytes();
        let name_len = name_bytes.len();
        let file_size = checked_u32(file_entry.size, "file size")?;
        let header_offset = checked_u32(local_offset, "local header offset")?;

        // Read the source file.
        let data = read_file_smart(&file_entry.absolute_path, use_mmap)?;

        // CRC32 from the raw file data.
        let crc = crc32fast::hash(&data);

        // Write local file header → encryptor.
        write_local_header(&mut enc, name_bytes, crc, file_size)
            .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;

        // Write file data → encryptor.
        enc.write_all(&data)
            .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;

        // Accumulate for central directory (data is dropped here).
        cd_entries.push(CdEntry {
            normalized_path: file_entry.normalized_path.clone(),
            crc32: crc,
            size: file_size,
            local_header_offset: header_offset,
        });

        local_offset += 30 + name_len as u64 + file_entry.size;

        // Update progress bar.
        if let Some(bar) = progress_bar {
            bar.inc(file_entry.size);
        }
    }

    // ── Central directory ─────────────────────────────────────────────

    let cd_offset = checked_u32(local_offset, "central directory offset")?;

    let mut cd_size: u64 = 0;
    for entry in &cd_entries {
        write_central_dir_entry(&mut enc, entry)
            .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;
        cd_size += 46 + entry.normalized_path.len() as u64;
    }
    let cd_size_u32 = checked_u32(cd_size, "central directory size")?;
    let entry_count = checked_u16_from_usize(cd_entries.len(), "entry count")?;

    // ── EOCD ──────────────────────────────────────────────────────────

    write_eocd(&mut enc, entry_count, cd_size_u32, cd_offset)
        .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;

    // ── Finalize encryption ───────────────────────────────────────────

    let crypto = enc
        .finish()
        .map_err(|e| IntunewinError::EncryptionError(e.to_string()))?;

    // ── Detection.xml ─────────────────────────────────────────────────

    let detection_info = StreamingDetectionInfo {
        name: setup_name.to_string(),
        unencrypted_content_size: inner_zip_size,
        setup_file: setup_name.to_string(),
        key: crypto.key,
        iv: crypto.iv,
        mac_key: crypto.mac_key,
        mac: crypto.mac,
        file_digest: crypto.file_digest,
    };

    let detection_xml = generate_detection_xml_streaming(&detection_info)?;

    let detection_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(false);

    outer_zip
        .start_file("IntuneWinPackage/Metadata/Detection.xml", detection_options)
        .map_err(|e| IntunewinError::ZipError(e.to_string()))?;

    outer_zip
        .write_all(detection_xml.as_bytes())
        .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;

    outer_zip
        .finish()
        .map_err(|e| IntunewinError::ZipError(e.to_string()))?;

    let final_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(ZeroMatResult {
        output_path,
        final_size,
        inner_zip_size,
        encrypted_size: crypto.total_encrypted,
    })
}

// ── Channeled (two-thread) variant ───────────────────────────────────

/// Default bounded channel depth. Each slot holds one source file's
/// worth of data (header + body), so memory ceiling ≈ depth × largest_file.
const CHANNEL_DEPTH: usize = 4;

/// Message from producer to consumer.
enum Chunk {
    /// A file's local header bytes + raw file data, plus progress increment.
    FileEntry {
        header: Vec<u8>,
        data: Vec<u8>,
        progress_bytes: u64,
    },
    /// Trailer: central directory + EOCD bytes (sent once at the end).
    Trailer(Vec<u8>),
}

/// Serialize a local file header into a standalone Vec.
fn serialize_local_header(name: &[u8], crc32: u32, size: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(30 + name.len());
    // Infallible — writing to Vec<u8> cannot fail.
    let _ = write_local_header(&mut buf, name, crc32, size);
    buf
}

/// Serialize central directory + EOCD into a standalone Vec.
fn serialize_trailer(
    cd_entries: &[CdEntry],
    cd_offset: u32,
) -> std::result::Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let mut cd_size: u64 = 0;
    for entry in cd_entries {
        write_central_dir_entry(&mut buf, entry).map_err(|e| e.to_string())?;
        cd_size += 46 + entry.normalized_path.len() as u64;
    }
    let cd_size_u32 = u32::try_from(cd_size)
        .map_err(|_| format!("central directory size overflow: {cd_size}"))?;
    let entry_count = u16::try_from(cd_entries.len())
        .map_err(|_| format!("entry count overflow: {}", cd_entries.len()))?;
    write_eocd(&mut buf, entry_count, cd_size_u32, cd_offset).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Run the zero-materialization pipeline with a bounded channel between
/// a producer thread (file reads + ZIP structure) and a consumer thread
/// (AES-CBC encryption + disk writes).
///
/// Same correctness guarantees as `run_zero_mat()`, but overlaps disk
/// reads with encryption/hashing work.
pub fn run_zero_mat_channeled(
    discovery: &DiscoveryResult,
    setup_name: &str,
    output_folder: &Path,
    use_mmap: bool,
    progress_bar: Option<&ProgressBar>,
) -> Result<ZeroMatResult> {
    validate_zero_mat(discovery)?;

    let inner_zip_size = compute_inner_zip_size(discovery);
    let encrypted_size = ((inner_zip_size / 16) + 1) * 16;

    let output_filename = derive_output_filename(setup_name);
    let output_path = output_folder.join(&output_filename);

    if !output_folder.exists() {
        std::fs::create_dir_all(output_folder).map_err(|e| IntunewinError::FileWriteError {
            path: output_folder.to_path_buf(),
            source: e,
        })?;
    }

    // Clone file metadata for the producer thread (just paths + sizes, not data).
    let files: Vec<_> = discovery
        .files
        .iter()
        .map(|f| (f.absolute_path.clone(), f.normalized_path.clone(), f.size))
        .collect();
    let use_mmap_clone = use_mmap;

    let (tx, rx) = crossbeam_channel::bounded::<Chunk>(CHANNEL_DEPTH);

    // ── Producer thread ───────────────────────────────────────────────
    let producer = std::thread::spawn(move || -> std::result::Result<(), String> {
        let mut cd_entries: Vec<CdEntry> = Vec::with_capacity(files.len());
        let mut local_offset: u64 = 0;

        for (abs_path, normalized_path, file_size) in &files {
            let size_u32 = u32::try_from(*file_size)
                .map_err(|_| format!("file size overflow: {file_size}"))?;
            let header_offset = u32::try_from(local_offset)
                .map_err(|_| format!("local header offset overflow: {local_offset}"))?;

            let data = read_file_smart(abs_path, use_mmap_clone).map_err(|e| e.to_string())?;
            let crc = crc32fast::hash(&data);

            let header = serialize_local_header(normalized_path.as_bytes(), crc, size_u32);

            tx.send(Chunk::FileEntry {
                header,
                data,
                progress_bytes: *file_size,
            })
            .map_err(|_| "consumer thread dropped".to_string())?;

            cd_entries.push(CdEntry {
                normalized_path: normalized_path.clone(),
                crc32: crc,
                size: size_u32,
                local_header_offset: header_offset,
            });

            local_offset += 30 + normalized_path.len() as u64 + file_size;
        }

        // Send the trailer (central directory + EOCD).
        let cd_offset = u32::try_from(local_offset)
            .map_err(|_| format!("cd offset overflow: {local_offset}"))?;
        let trailer = serialize_trailer(&cd_entries, cd_offset)?;
        tx.send(Chunk::Trailer(trailer))
            .map_err(|_| "consumer thread dropped".to_string())?;

        Ok(())
    });

    // ── Consumer (this thread): encrypt + write ───────────────────────

    let file = File::create(&output_path).map_err(|e| IntunewinError::FileWriteError {
        path: output_path.clone(),
        source: e,
    })?;
    let buffered_file = BufWriter::with_capacity(BUFFER_SIZE, file);
    let mut outer_zip = ZipWriter::new(buffered_file);

    let content_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(requires_zip64(encrypted_size));

    outer_zip
        .start_file(
            "IntuneWinPackage/Contents/IntunePackage.intunewin",
            content_options,
        )
        .map_err(|e| IntunewinError::ZipError(e.to_string()))?;

    let mut enc = EncryptingWriter::new(&mut outer_zip);

    // Receive chunks from producer and write through encryptor.
    for chunk in &rx {
        match chunk {
            Chunk::FileEntry {
                header,
                data,
                progress_bytes,
            } => {
                enc.write_all(&header)
                    .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;
                enc.write_all(&data)
                    .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;
                if let Some(bar) = progress_bar {
                    bar.inc(progress_bytes);
                }
            }
            Chunk::Trailer(trailer_bytes) => {
                enc.write_all(&trailer_bytes)
                    .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;
            }
        }
    }

    // Wait for producer to finish and propagate errors.
    producer
        .join()
        .map_err(|_| IntunewinError::CompressionError("producer thread panicked".into()))?
        .map_err(IntunewinError::CompressionError)?;

    // Finalize encryption.
    let crypto = enc
        .finish()
        .map_err(|e| IntunewinError::EncryptionError(e.to_string()))?;

    // Detection.xml
    let detection_info = StreamingDetectionInfo {
        name: setup_name.to_string(),
        unencrypted_content_size: inner_zip_size,
        setup_file: setup_name.to_string(),
        key: crypto.key,
        iv: crypto.iv,
        mac_key: crypto.mac_key,
        mac: crypto.mac,
        file_digest: crypto.file_digest,
    };

    let detection_xml = generate_detection_xml_streaming(&detection_info)?;

    let detection_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(false);

    outer_zip
        .start_file("IntuneWinPackage/Metadata/Detection.xml", detection_options)
        .map_err(|e| IntunewinError::ZipError(e.to_string()))?;

    outer_zip
        .write_all(detection_xml.as_bytes())
        .map_err(|e| IntunewinError::CompressionError(e.to_string()))?;

    outer_zip
        .finish()
        .map_err(|e| IntunewinError::ZipError(e.to_string()))?;

    let final_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(ZeroMatResult {
        output_path,
        final_size,
        inner_zip_size,
        encrypted_size: crypto.total_encrypted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inner_zip_size_empty() {
        let discovery = DiscoveryResult {
            files: vec![],
            total_size: 0,
            file_count: 0,
            setup_file_index: 0,
        };
        // Just EOCD
        assert_eq!(compute_inner_zip_size(&discovery), 22);
    }

    #[test]
    fn test_inner_zip_size_single_file() {
        use crate::pipeline::discovery::FileEntry;
        use std::path::PathBuf;

        let discovery = DiscoveryResult {
            files: vec![FileEntry {
                relative_path: PathBuf::from("test.exe"),
                absolute_path: PathBuf::from("/tmp/test.exe"),
                size: 1000,
                is_setup_file: true,
                normalized_path: "test.exe".to_string(),
            }],
            total_size: 1000,
            file_count: 1,
            setup_file_index: 0,
        };

        // local header: 30 + 8 = 38
        // data: 1000
        // central dir: 46 + 8 = 54
        // EOCD: 22
        // Total: 38 + 1000 + 54 + 22 = 1114
        assert_eq!(compute_inner_zip_size(&discovery), 1114);
    }

    #[test]
    fn test_encrypting_writer_alignment() {
        // Verify that the EncryptingWriter produces output that is always
        // a multiple of 16 bytes (AES block size) after finish().
        let mut output = Vec::new();
        let mut enc = EncryptingWriter::new(&mut output);

        // Write various non-aligned sizes
        enc.write_all(&[0u8; 7]).unwrap();
        enc.write_all(&[1u8; 3]).unwrap();
        enc.write_all(&[2u8; 20]).unwrap();
        enc.write_all(&[3u8; 1]).unwrap();

        let result = enc.finish().unwrap();

        assert_eq!(result.total_encrypted as usize, output.len());
        assert_eq!(output.len() % 16, 0);
        // 31 bytes of input → 2 full blocks (32 bytes, with 1 byte padding on last block)
        assert_eq!(output.len(), 32);
    }
}
