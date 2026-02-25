/// Integration test for cache integrity
/// Verifies that cached and non-cached outputs produce identical packages
///
/// This test catches the critical issue where --cache flag produces different
/// output hashes than non-cached runs, which would indicate data corruption.
///
/// CRITICAL: This test must run in CI to prevent regression of the caching bug
/// that broke the application. Minimal test data is generated on-the-fly to
/// ensure the test runs everywhere without requiring large binary files.
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

fn intunewin_bin() -> &'static str {
    // Cargo exposes the built binary path to integration tests.
    // This avoids relying on a separately-built `target/release` binary.
    env!("CARGO_BIN_EXE_intunewin-rs")
}

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
fn test_cache_integrity_small_package() {
    // Use existing test data if available, otherwise generate minimal test data
    let test_data_path = PathBuf::from("testdata/packages/small");
    if !test_data_path.exists() {
        println!("📝 Generating minimal test data for cache integrity test...");
        generate_minimal_test_data(&test_data_path).expect("Failed to generate test data");
    }

    let output_dir = PathBuf::from("target/test_cache_output_small");
    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Test with compression 6 (the only case where caching applies).
    // Compression 0 uses the zero-materialization pipeline which doesn't
    // produce an intermediate ZIP and doesn't use caching.
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
    let status = Command::new(intunewin_bin())
        .arg("-c")
        .arg(test_data_path)
        .arg("-s")
        .arg("setup.exe")
        .arg("-o")
        .arg(output_dir)
        .arg("--compression")
        .arg(compression_level.to_string())
        .arg("--keep-temp")
        .arg("-q")
        .status()
        .expect("Failed to run intunewin-rs");

    assert!(status.success(), "First build (no cache) failed");

    // NOTE: Final `.intunewin` is encrypted with random keys/IV, so it is intentionally
    // non-deterministic between runs. For cache integrity we compare the *inner ZIP*.
    let no_cache_zip = find_zip_file(output_dir).expect("No .zip file found after first build");

    let no_cache_hash = get_file_hash(&no_cache_zip);
    println!("  No-cache inner ZIP hash:  {}", no_cache_hash);

    // Save the no-cache file to a temp location before we delete the directory
    let temp_dir = PathBuf::from("target/test_cache_temp");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    let no_cache_copy = temp_dir.join("no_cache_output.zip");
    fs::copy(&no_cache_zip, &no_cache_copy).expect("Failed to copy no-cache inner ZIP");

    // Clean output directory for cache test
    fs::remove_dir_all(output_dir).expect("Failed to remove output directory");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    // Run 2: With cache (cold cache, first run - populates cache)
    println!("  Run 2: Building with cache (cold - populates cache)...");
    let status = Command::new(intunewin_bin())
        .arg("-c")
        .arg(test_data_path)
        .arg("-s")
        .arg("setup.exe")
        .arg("-o")
        .arg(output_dir)
        .arg("--compression")
        .arg(compression_level.to_string())
        .arg("--cache")
        .arg("--keep-temp")
        .arg("-q")
        .status()
        .expect("Failed to run intunewin-rs with cache");

    assert!(status.success(), "Second build (cold cache) failed");

    let cold_cache_zip = find_zip_file(output_dir).expect("No .zip file found after second build");

    let cold_cache_hash = get_file_hash(&cold_cache_zip);
    println!("  Cold cache inner ZIP hash: {}", cold_cache_hash);

    // CRITICAL: Verify cold cache matches no-cache
    if no_cache_hash != cold_cache_hash {
        println!("\n❌ CACHE INTEGRITY FAILURE (cold cache)!");
        println!("   No-cache:    {}", no_cache_hash);
        println!("   Cold cache:  {}", cold_cache_hash);

        // Additional debugging: check file sizes
        let no_cache_size = fs::metadata(&no_cache_copy).map(|m| m.len()).unwrap_or(0);
        let cold_cache_size = fs::metadata(&cold_cache_zip).map(|m| m.len()).unwrap_or(0);

        println!("\n   Size comparison:");
        println!("     No-cache:    {} bytes", no_cache_size);
        println!("     Cold cache:  {} bytes", cold_cache_size);

        panic!(
            "Cache integrity test failed for compression level {} (cold cache): \
             cached output differs from non-cached output. \
             This indicates a critical data consistency issue.",
            compression_level
        );
    }

    println!("  ✓ Cold cache matches no-cache");

    // Clean output directory for warm cache test
    fs::remove_dir_all(output_dir).expect("Failed to remove output directory");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    // Run 3: With cache (warm cache - uses already populated cache)
    println!("  Run 3: Building with cache (warm - uses cache)...");
    let status = Command::new(intunewin_bin())
        .arg("-c")
        .arg(test_data_path)
        .arg("-s")
        .arg("setup.exe")
        .arg("-o")
        .arg(output_dir)
        .arg("--compression")
        .arg(compression_level.to_string())
        .arg("--cache")
        .arg("--keep-temp")
        .arg("-q")
        .status()
        .expect("Failed to run intunewin-rs with warm cache");

    assert!(status.success(), "Third build (warm cache) failed");

    let warm_cache_zip = find_zip_file(output_dir).expect("No .zip file found after third build");

    let warm_cache_hash = get_file_hash(&warm_cache_zip);
    println!("  Warm cache inner ZIP hash: {}", warm_cache_hash);

    // CRITICAL: Verify warm cache matches no-cache and cold cache
    if no_cache_hash != warm_cache_hash {
        println!("\n❌ CACHE INTEGRITY FAILURE (warm cache)!");
        println!("   No-cache:    {}", no_cache_hash);
        println!("   Cold cache:  {}", cold_cache_hash);
        println!("   Warm cache:  {}", warm_cache_hash);

        // Additional debugging: check file sizes
        let no_cache_size = fs::metadata(&no_cache_copy).map(|m| m.len()).unwrap_or(0);
        let warm_cache_size = fs::metadata(&warm_cache_zip).map(|m| m.len()).unwrap_or(0);

        println!("\n   Size comparison:");
        println!("     No-cache:    {} bytes", no_cache_size);
        println!("     Warm cache:  {} bytes", warm_cache_size);

        panic!(
            "Cache integrity test failed for compression level {} (warm cache): \
             cached output differs from non-cached output. \
             This indicates a critical data consistency issue in cache lookup path.",
            compression_level
        );
    }

    println!("  ✓ Warm cache matches no-cache and cold cache");
    println!("  ✓ Cache integrity verified for all runs");

    // Cleanup temp directory
    let _ = fs::remove_dir_all(&temp_dir);
}

fn find_zip_file(dir: &PathBuf) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "zip")
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
}

/// Generate minimal but realistic test data for cache integrity testing.
/// Creates a small package directory with a setup.exe and a few supporting files.
/// This ensures the cache integrity test runs everywhere without requiring large binary files.
fn generate_minimal_test_data(test_data_path: &PathBuf) -> std::io::Result<()> {
    fs::create_dir_all(test_data_path)?;

    // Create setup.exe (minimal PE executable stub)
    // A minimal PE header is ~512 bytes but we'll make it realistic size (~10KB)
    let setup_exe_path = test_data_path.join("setup.exe");
    let mut setup_file = File::create(&setup_exe_path)?;

    // Write minimal PE header (MZ header)
    // This is just enough to be recognized as PE format
    setup_file.write_all(b"MZ")?; // DOS header signature
    setup_file.write_all(&[0; 58])?; // Minimal DOS header padding up to e_lfanew
    setup_file.write_all(&[0x40, 0, 0, 0])?; // PE offset at 0x3C (60 decimal)

    // PE signature and minimal headers
    setup_file.write_all(b"PE\0\0")?; // PE signature
    setup_file.write_all(&[0; 1000])?; // Minimal PE headers and sections

    // Pad to ~10KB to make it realistic
    setup_file.write_all(&[0xAA; 9000])?;

    // Create some supporting files with realistic content
    let files = vec![
        (
            "readme.txt",
            "This is a test package for cache integrity testing.\n",
        ),
        ("config.ini", "[Settings]\nVersion=1.0\nCacheTest=true\n"),
        (
            "data.bin",
            "BinaryDataSectionForCacheIntegrityTestingPurposes",
        ),
    ];

    for (filename, content) in files {
        let file_path = test_data_path.join(filename);
        let mut file = File::create(file_path)?;
        // Write content multiple times to make files larger and more realistic
        for _ in 0..50 {
            file.write_all(content.as_bytes())?;
        }
    }

    println!(
        "✓ Generated minimal test data at {}",
        test_data_path.display()
    );
    Ok(())
}
