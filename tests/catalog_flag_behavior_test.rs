use std::process::Command;

fn intunewin_bin() -> &'static str {
    env!("CARGO_BIN_EXE_intunewin-rs")
}

#[test]
fn test_catalog_flag_fails_explicitly() {
    let status = Command::new(intunewin_bin())
        .arg("-c")
        .arg("test_small")
        .arg("-s")
        .arg("setup.intunewin")
        .arg("-o")
        .arg("target/test_catalog_flag")
        .arg("-a")
        .arg("some-catalog")
        .status()
        .expect("Failed to run intunewin-rs");

    assert!(
        !status.success(),
        "Expected non-zero exit when --catalog is provided"
    );
}
