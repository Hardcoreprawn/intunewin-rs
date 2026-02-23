# Experiment Framework (Issue #75)

This document defines the baseline framework used to evaluate high-complexity performance experiments (`#76`–`#82`) with reproducible metrics and explicit decision gates.

## Script

- Harness: `testdata/benchmarks/experiment-framework.ps1`
- Dataset manifest: `testdata/benchmarks/datasets.real.json`
- Output: timestamped folder under `testdata/benchmarks/results/`
  - `summary.json` (machine-readable)
  - `summary.md` (human summary)

## What It Measures

Per dataset and variant (control/candidate):

- Wall-clock duration per run (ms)
- p50 and p95 durations
- Peak working set (MB)
- CPU time (ms)
- Package output existence + size
- `.NET ZipArchive` readability of `Detection.xml`
- Host environment metadata (CPU model, logical/physical cores, RAM)

## Anti-Skew Protocol (Important)

To reduce misleading results on high-end local machines:

- Use `-DatasetProfile real` (default) so runs use real installers from the manifest.
- Add `-IncludeLarge` for large real installer coverage.
- Use at least `-Iterations 7` and `-WarmupRuns 1`.
- Use `-RunOrder interleaved` to alternate control/candidate ordering and reduce thermal/time drift bias.
- Keep `.NET` readability checks enabled to prevent speed wins that break compatibility.
- Use explicit cache policy for the question you are testing:
  - `-CacheControl preserve` for warm-cache/repeated-run behavior.
  - `-CacheControl clear-each-iteration` for cold-run behavior.

For publication-quality runs, execute both cache policies and compare outcomes.

## Decision Gates

Default recommendation logic (from issue `#75`):

- **Adopt**: overall p50 gain >= 15% OR overall p95 gain >= 25%
- **Conditional**: overall p50 gain >= 8%
- **Reject/Defer**: otherwise

## Basic Usage

```powershell
# Build release binary first
cargo build --release

# Compare baseline vs baseline (sanity run, real installers)
.\testdata\benchmarks\experiment-framework.ps1 -DatasetProfile real -Iterations 7 -WarmupRuns 1 -RunOrder interleaved -IncludeLarge
```

## Compare Candidate Variant

Use command templates with placeholders:

- `{CONTENT}`
- `{SETUP}`
- `{OUTPUT}`

```powershell
.\testdata\benchmarks\experiment-framework.ps1 `
  -ControlLabel baseline `
  -CandidateLabel candidate `
  -ControlCommandTemplate '.\target\release\intunewin-rs.exe -c "{CONTENT}" -s "{SETUP}" -o "{OUTPUT}" -q --compression 6' `
  -CandidateCommandTemplate '.\target\release\intunewin-rs.exe -c "{CONTENT}" -s "{SETUP}" -o "{OUTPUT}" -q --compression 6 --cache' `
  -Iterations 7 `
  -WarmupRuns 1 `
  -DatasetProfile real `
  -RunOrder interleaved `
  -CacheControl preserve
```

### Cold-run control mode

```powershell
.\testdata\benchmarks\experiment-framework.ps1 `
  -ControlLabel baseline_cold `
  -CandidateLabel candidate_cold `
  -ControlCommandTemplate '.\target\release\intunewin-rs.exe -c "{CONTENT}" -s "{SETUP}" -o "{OUTPUT}" -q --compression 6 --cache' `
  -CandidateCommandTemplate '.\target\release\intunewin-rs.exe -c "{CONTENT}" -s "{SETUP}" -o "{OUTPUT}" -q --compression 6 --cache' `
  -CacheControl clear-each-iteration `
  -Iterations 7 `
  -WarmupRuns 1
```

## Notes

- Datasets default to `real` profile from the manifest (`datasets.real.json`).
- Synthetic datasets are opt-in only: use `-DatasetProfile synthetic -AllowSynthetic`.
- Use `-Strict` to fail immediately on command failures or unreadable output.
- Intune tenant upload validation is intentionally out-of-scope for this local framework and should be tracked separately when tenant access is available.
