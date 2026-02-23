# Experiment Framework (Issue #75)

This document defines the baseline framework used to evaluate high-complexity performance experiments (`#76`–`#82`) with reproducible metrics and explicit decision gates.

## Script

- Harness: `testdata/benchmarks/experiment-framework.ps1`
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

## Decision Gates

Default recommendation logic (from issue `#75`):

- **Adopt**: overall p50 gain >= 15% OR overall p95 gain >= 25%
- **Conditional**: overall p50 gain >= 8%
- **Reject/Defer**: otherwise

## Basic Usage

```powershell
# Build release binary first
cargo build --release

# Compare baseline vs baseline (sanity run)
.\testdata\benchmarks\experiment-framework.ps1
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
  -Iterations 5 `
  -WarmupRuns 1
```

## Notes

- Datasets default to `small` + `medium`; use `-IncludeLarge` if available.
- Use `-Strict` to fail immediately on command failures or unreadable output.
- Intune tenant upload validation is intentionally out-of-scope for this local framework and should be tracked separately when tenant access is available.
