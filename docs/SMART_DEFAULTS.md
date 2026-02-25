# Why We Never Compress

## The Rule

**intunewin-rs always uses store-only (compression 0). There are no smart defaults, no auto-detection thresholds, no compression decisions. We never compress.**

## The Reasoning

### 1. The Content Is Already Compressed

Intune packages contain installers — `.exe`, `.msi`, `.msix`, `.cab` — produced by professional packaging tools that already apply optimal compression. Running DEFLATE over DEFLATE-compressed data achieves <2% additional size reduction:

| Package | Size | DEFLATE Savings | Build Time Cost |
| --------- | ------ | ----------------- | ----------------- |
| Medium (.exe) | 254 MB | 3.3 MB (1.3%) | 3.6× slower |
| Large (.exe tree) | 1.5 GB | 18 MB (1.2%) | 3.1× slower |

For a 254 MB package, compression saves 3.3 MB but adds 4 seconds. That's 0.8 MB/s of "savings throughput" — worse than a floppy disk.

### 2. Compression Destroys the Architecture

Store-only mode makes the inner ZIP byte-level deterministic from file metadata alone. The exact size of every byte — local headers, file data, central directory, EOCD — can be computed before reading a single source file.

This enables the **zero-materialization pipeline**: source files stream directly through ZIP structure generation → AES-CBC encryption → final output. No intermediate files. No buffers. No second pass.

With compression, this is impossible — compressed sizes are unknowable until after compression runs, so the inner ZIP must be fully materialized before encryption can begin. This forces a multi-pass pipeline:

```text
Without compression (zero-mat):
  Source → ZIP headers + data → EncryptingWriter → output
  I/O: read sources + write output = 2× data size

With compression (multi-pass):
  Source → compress → write inner ZIP → read inner ZIP → encrypt → output
  I/O: read sources + write ZIP + read ZIP + write output = 4× data size
```

Compression doesn't just waste CPU — it doubles the I/O budget.

### 3. Caching Can't Save It

Previous versions used caching to amortize compression cost on repeated builds. But caching only benefits compression > 0 runs, adds its own I/O overhead, and introduces complexity (cache invalidation, manifest management, integrity verification). When compression is 0, there's nothing to cache.

The zero-materialization pipeline is faster on *every* build — first build, hundredth build — without any cache state to manage.

## Historical Note

Earlier versions of intunewin-rs implemented "smart compression defaults" that auto-selected compression level 6 for packages under 500 MB. This was removed when benchmarking proved that even for small packages, the CPU and I/O cost of compression never justified the <2% size reduction. The `--compression` flag is retained as a hidden option for backward compatibility but is strongly discouraged.
