/// Integration test for cache integrity
/// Verifies that cached and non-cached outputs produce identical packages
///
/// This test catches the critical issue where --cache flag produces different
/// output hashes than non-cached runs, which would indicate data corruption.
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

fn get_file_hash(path: &PathBuf) -> String {
    use sha2::{Digest, Sha256};

    let mut file = File::open(path).expect("Failed to open file");
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let n = file.read(&mut buffer).expect("Failed to read file");
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    format!("{:x}", hasher.finalize())
}

#[test]
#[ignore] // Only run with --ignored flag or when explicitly needed
fn test_cache_integrity_small_package() {
    // This test requires test data to be available
    let test_data_path = PathBuf::from("testdata/packages/small");
    if !test_data_path.exists() {
        eprintln!("⚠ Skipping cache integrity test - testdata not available");
        return;
    }

    let output_dir = PathBuf::from("target/test_cache_output_small");
    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Test with compression 0 (store only)
    test_cache_for_compression(&test_data_path, &output_dir, 0);

    // Test with compression 6 (common case)
    test_cache_for_compression(&test_data_path, &output_dir, 6);

    let _ = fs::remove_dir_all(&output_dir);
}

fn test_cache_for_compression(
    test_data_path: &PathBuf,
    output_dir: &PathBuf,
    compression_level: u32,
) {
    println!(
        "\n📋 Testing cache integrity - Compression level: {}",
        compression_level
    );

    // Clean output directory
    let _ = fs::remove_dir_all(output_dir);
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    // Run 1: Without cache
    println!("  Run 1: Building without cache...");
    let status = Command::new("./target/release/intunewin-rs")
        .arg("-c")
        .arg(test_data_path)
        .arg("-s")
        .arg("setup.exe")
        .arg("-o")
        .arg(output_dir)
        .arg("--compression")
        .arg(compression_level.to_string())
        .arg("-q")
        .status()
        .expect("Failed to run intunewin-rs");

    assert!(status.success(), "First build (no cache) failed");

    // Find output file
    let no_cache_file =
        find_intunewin_file(output_dir).expect("No .intunewin file found after first build");

    let no_cache_hash = get_file_hash(&no_cache_file);
    println!("  No-cache hash:  {}", no_cache_hash);

    // Save the file for comparison
    let no_cache_copy = output_dir.join("no_cache_output.intunewin");
    fs::copy(&no_cache_file, &no_cache_copy).expect("Failed to copy no-cache output");

    // Clean output directory
    fs::remove_dir_all(output_dir).expect("Failed to remove output directory");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    // Run 2: With cache (cold cache, first run)
    println!("  Run 2: Building with cache (cold)...");
    let status = Command::new("./target/release/intunewin-rs")
        .arg("-c")
        .arg(test_data_path)
        .arg("-s")
        .arg("setup.exe")
        .arg("-o")
        .arg(output_dir)
        .arg("--compression")
        .arg(compression_level.to_string())
        .arg("--cache")
        .arg("-q")
        .status()
        .expect("Failed to run intunewin-rs with cache");

    assert!(status.success(), "Second build (with cache) failed");

    let cached_file =
        find_intunewin_file(output_dir).expect("No .intunewin file found after second build");

    let cached_hash = get_file_hash(&cached_file);
    println!("  Cached hash:    {}", cached_hash);

    // CRITICAL: Verify hashes match
    if no_cache_hash != cached_hash {
        println!("\n❌ CACHE INTEGRITY FAILURE!");
        println!("   No-cache:  {}", no_cache_hash);
        println!("   With cache: {}", cached_hash);

        // Additional debugging: check file sizes
        let no_cache_size = fs::metadata(&no_cache_copy).map(|m| m.len()).unwrap_or(0);
        let cached_size = fs::metadata(&cached_file).map(|m| m.len()).unwrap_or(0);

        println!("\n   Size comparison:");
        println!("     No-cache:   {} bytes", no_cache_size);
        println!("     With cache: {} bytes", cached_size);

        panic!(
            "Cache integrity test failed for compression level {}: \
             cached output differs from non-cached output. \
             This indicates a critical data consistency issue.",
            compression_level
        );
    }

    println!("  ✓ Hashes match - cache integrity verified");
}

fn find_intunewin_file(dir: &PathBuf) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "intunewin")
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
}
