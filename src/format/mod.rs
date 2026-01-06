//! Format modules for IntuneWin file structures.
//!
//! This module contains implementations for various XML and binary formats
//! used in IntuneWin packages.

pub mod detection;
pub mod manifest;

pub use detection::{generate_detection_xml, generate_detection_xml_streaming, DetectionInfo, StreamingDetectionInfo};
pub use manifest::{generate_manifest, ManifestFile};
