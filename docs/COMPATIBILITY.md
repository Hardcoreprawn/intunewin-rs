# Compatibility Matrix

This document defines what `intunewin-rs` currently supports relative to Microsoft's `IntuneWinAppUtil` CLI and package behavior.

## CLI Flags

| Flag | Status | Notes |
| --- | --- | --- |
| `-c, --content` | ✅ Supported | Source folder is required |
| `-s, --setup` | ✅ Supported | Setup filename is required |
| `-o, --output` | ✅ Supported | Output folder is required |
| `-a, --catalog` | ⚠️ Not implemented | Accepted for CLI parity but currently fails fast with an explicit error |
| `-q, --quiet` | ✅ Supported | Minimal output |
| `--qq` | ✅ Supported | Silent mode |
| `-h, --help` | ✅ Supported | Help output |
| `-V, --version` | ✅ Supported | Version output |

## Extension Flags

| Flag | Status | Notes |
| --- | --- | --- |
| `-t, --threads` | ✅ Supported | Configures rayon thread count |
| `--compression` | ✅ Supported | `0..=9`, with smart defaults when omitted |
| `--no-mmap` | ✅ Supported | Disables mmap path |
| `--cache` / `--no-cache` | ✅ Supported | Explicit cache control |
| `--cache-stats` | ✅ Supported | Prints cache statistics |
| `--clear-cache` | ✅ Supported | Clears cache before build |
| `--keep-temp` | ✅ Supported | Keeps inner `.zip` and encrypted temp artifacts |

## Compatibility Statement

`intunewin-rs` aims to be command-compatible for the core packaging workflow (`-c/-s/-o`) and format-compatible for generated `.intunewin` output used in Intune upload flows.

Features listed as not implemented are rejected explicitly at runtime to avoid silent behavior differences.
