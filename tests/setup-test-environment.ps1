# Setup Test Environment for IntuneWin Performance Testing
# This script downloads the Microsoft intunewinapputil tool and sets up test data

param(
    [ValidateSet('small', 'medium', 'large', 'xlarge')]
    [string]$DataSize = 'medium',
    
    [switch]$SkipIntuneWinDownload = $false,
    [switch]$GenerateTestData = $true,
    [string]$TestDataPath = '.\testdata'
)

$ErrorActionPreference = 'Stop'
$VerbosePreference = 'Continue'

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "IntuneWin Test Environment Setup" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# Create directory structure
Write-Host "`n[1/4] Creating directory structure..." -ForegroundColor Yellow
$dirs = @(
    "$TestDataPath",
    "$TestDataPath\tools",
    "$TestDataPath\packages\small",
    "$TestDataPath\packages\medium",
    "$TestDataPath\packages\large",
    "$TestDataPath\output",
    "$TestDataPath\benchmarks"
)

foreach ($dir in $dirs) {
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
        Write-Verbose "Created: $dir"
    }
}

# Download Microsoft intunewinapputil
if (-not $SkipIntuneWinDownload) {
    Write-Host "`n[2/4] Downloading Microsoft intunewinapputil..." -ForegroundColor Yellow
    
    # URL for the official tool (check Microsoft Intune docs for latest)
    $intuneWinUrl = "https://github.com/Microsoft/Intune-App-Wrapping-Tool-Windows/releases/download/v1.0/intunewinapputil.exe"
    $intuneWinPath = "$TestDataPath\tools\intunewinapputil.exe"
    
    try {
        Write-Host "Downloading from: $intuneWinUrl" -ForegroundColor Cyan
        [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $intuneWinUrl -OutFile $intuneWinPath -UseBasicParsing -TimeoutSec 300
        Write-Host "✓ Downloaded: $intuneWinPath" -ForegroundColor Green
    }
    catch {
        Write-Host "⚠ Download failed: $_" -ForegroundColor Yellow
        Write-Host "  Please download manually from: https://github.com/Microsoft/Intune-App-Wrapping-Tool-Windows/releases" -ForegroundColor Yellow
        Write-Host "  Place it in: $intuneWinPath" -ForegroundColor Yellow
    }
}
else {
    Write-Host "`n[2/4] Skipping intunewinapputil download (--SkipIntuneWinDownload)" -ForegroundColor Yellow
}

# Generate synthetic test data
if ($GenerateTestData) {
    Write-Host "`n[3/4] Generating synthetic test packages..." -ForegroundColor Yellow
    
    $configs = @{
        'small' = @{ Files = 100; FileSize = '1MB'; TotalSize = '100MB' }
        'medium' = @{ Files = 500; FileSize = '5MB'; TotalSize = '2.5GB' }
        'large' = @{ Files = 2000; FileSize = '10MB'; TotalSize = '20GB' }
        'xlarge' = @{ Files = 20000; FileSize = '5MB'; TotalSize = '100GB' }
    }
    
    $config = $configs[$DataSize]
    Write-Host "Generating $DataSize package (approx $($config.TotalSize)):" -ForegroundColor Cyan
    
    $packageDir = "$TestDataPath\packages\$DataSize"
    
    # Create subdirectories structure
    $subdirs = @(
        'bin',
        'lib',
        'config',
        'data',
        'docs',
        'resources'
    )
    
    foreach ($subdir in $subdirs) {
        $path = "$packageDir\$subdir"
        if (-not (Test-Path $path)) {
            New-Item -ItemType Directory -Path $path | Out-Null
        }
    }
    
    # Create setup.exe (required by Intune)
    $setupPath = "$packageDir\setup.exe"
    if (-not (Test-Path $setupPath)) {
        # Create a minimal PE executable stub
        $peHeader = [byte[]]@(
            0x4D, 0x5A, 0x90, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
            0xFF, 0xFF, 0x00, 0x00, 0xB8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        )
        [System.IO.File]::WriteAllBytes($setupPath, $peHeader)
        Write-Verbose "Created setup.exe stub"
    }
    
    # Create synthetic files
    Write-Host "Generating files..." -ForegroundColor Cyan
    
    [int]$numFiles = [int]($config.Files -replace '[^0-9]')
    $fileSize = switch -regex ($config.FileSize) {
        '^(\d+)MB$' { [int]$matches[1] * 1MB }
        '^(\d+)KB$' { [int]$matches[1] * 1KB }
        '^(\d+)GB$' { [int]$matches[1] * 1GB }
        default { 5MB }
    }
    
    $buffer = New-Object byte[] $fileSize
    (New-Object System.Security.Cryptography.RNGCryptoServiceProvider).GetBytes($buffer)
    
    for ($i = 1; $i -le $numFiles; $i++) {
        $subdir = $subdirs[$i % $subdirs.Count]
        $filename = "$packageDir\$subdir\file_$i.bin"
        
        # Create files with somewhat realistic variation
        $variation = [math]::Floor([int]($fileSize/10) * (Get-Random -Minimum -100 -Maximum 100) / 100)
        $actualSize = $fileSize + $variation
        if ($actualSize -lt 1KB) { $actualSize = 1KB }
        
        [System.IO.File]::WriteAllBytes($filename, $buffer[0..($actualSize-1)])
        
        if ($i % 100 -eq 0) {
            Write-Progress -Activity "Generating test files" -Status "File $i / $numFiles" -PercentComplete ($i / $numFiles * 100)
        }
    }
    
    Write-Progress -Activity "Generating test files" -Completed
    Write-Host "✓ Generated $numFiles files in: $packageDir" -ForegroundColor Green
    
    # Calculate actual size
    $actualSize = (Get-ChildItem $packageDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
    $sizeGB = [math]::Round($actualSize / 1GB, 2)
    Write-Host "  Total size: $sizeGB GB" -ForegroundColor Cyan
}

# Create benchmark script
Write-Host "`n[4/4] Creating benchmark scripts..." -ForegroundColor Yellow

$benchmarkScript = @'
# Benchmark script for IntuneWin encoding performance

param(
    [Parameter(Mandatory=$true)]
    [string]$PackageSize,
    
    [string]$TestDataPath = '.\testdata',
    [string]$OutputPath = '.\testdata\output'
)

$ErrorActionPreference = 'Stop'

Write-Host "======================================"
Write-Host "IntuneWin Performance Benchmark"
Write-Host "======================================"

$packageDir = "$TestDataPath\packages\$PackageSize"
$intuneWinPath = "$TestDataPath\tools\intunewinapputil.exe"

if (-not (Test-Path $packageDir)) {
    Write-Error "Package not found: $packageDir"
    exit 1
}

# Get package size
$packageSize = (Get-ChildItem $packageDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
$packageSizeGB = [math]::Round($packageSize / 1GB, 2)
$packageFileCount = (Get-ChildItem $packageDir -Recurse -File).Count

Write-Host "`nPackage Info:"
Write-Host "  Path: $packageDir"
Write-Host "  Size: $packageSizeGB GB"
Write-Host "  Files: $packageFileCount"

# Test with Microsoft tool (if available)
if (Test-Path $intuneWinPath) {
    Write-Host "`nBenchmarking Microsoft intunewinapputil..."
    
    $outputFile = "$OutputPath\benchmark_msft_$PackageSize.intunewin"
    $startTime = Get-Date
    
    try {
        & $intuneWinPath -c $packageDir -s $packageDir -o $outputFile
        $duration = (Get-Date) - $startTime
        $outputSize = (Get-Item $outputFile).Length
        $outputSizeGB = [math]::Round($outputSize / 1GB, 2)
        $throughput = [math]::Round($packageSizeGB / $duration.TotalSeconds, 2)
        
        Write-Host "  ✓ Completed in: $($duration.TotalSeconds) seconds"
        Write-Host "  Throughput: $throughput GB/sec"
        Write-Host "  Output size: $outputSizeGB GB"
    }
    catch {
        Write-Host "  ✗ Error: $_" -ForegroundColor Red
    }
}
else {
    Write-Host "`n⚠ Microsoft tool not found: $intuneWinPath" -ForegroundColor Yellow
}

Write-Host "`nBenchmark complete. Update with Rust implementation when ready."
'@

$benchmarkPath = "$TestDataPath\benchmarks\benchmark.ps1"
Set-Content -Path $benchmarkPath -Value $benchmarkScript
Write-Host "✓ Created: $benchmarkPath" -ForegroundColor Green

# Create README
$readme = @"
# Test Data and Benchmarking

This directory contains test data and benchmarking scripts for IntuneWin performance testing.

## Directory Structure

\`\`\`
testdata/
├── tools/                 # Microsoft tools and reference implementations
│   └── intunewinapputil.exe
├── packages/              # Test packages of various sizes
│   ├── small/            # ~100MB
│   ├── medium/           # ~2.5GB
│   ├── large/            # ~20GB
│   └── xlarge/           # ~100GB
├── output/               # Generated .intunewin files
└── benchmarks/           # Benchmark scripts and results
\`\`\`

## Running Benchmarks

### Setup
\`\`\`powershell
# Generate test data (small, medium, large, xlarge)
.\setup-test-environment.ps1 -DataSize medium -GenerateTestData
\`\`\`

### Run Benchmark
\`\`\`powershell
# Benchmark Microsoft tool
.\benchmarks\benchmark.ps1 -PackageSize medium

# Benchmark Rust implementation (when ready)
cargo run --release -- -i .\testdata\packages\medium -o .\testdata\output\rust_medium.intunewin
\`\`\`

## Profiling

### Windows Performance Analyzer
\`\`\`powershell
# Capture trace
xperf -on Base+DiskIO+FileIO+Memory+ProcessCounter -BufferSize 1024 -MaxBuffers 256
cargo run --release -- -i testdata\\packages\\medium -o testdata\\output\\test.intunewin
xperf -d trace.etl

# View trace
xperfview trace.etl
\`\`\`

### Performance Insights to Track

- **Disk I/O**: Read throughput, seek patterns
- **CPU**: Thread utilization, compression efficiency
- **Memory**: Peak usage, allocation patterns
- **Overall**: Encoding time, throughput (GB/sec)

## Test Data Characteristics

| Size | Files | Approx Total | Use Case |
|------|-------|--------------|----------|
| small | 100 | 100 MB | Quick unit tests |
| medium | 500 | 2.5 GB | Daily development |
| large | 2000 | 20 GB | Performance testing |
| xlarge | 20000 | 100 GB | Stress testing |

## Expected Performance Targets

Based on SPECIFICATION.md:

- **Small (100MB)**: 1.5-2x faster than MSFT tool
- **Medium (2.5GB)**: 2-3x faster
- **Large (20GB)**: 3-5x faster
- **XLarge (100GB)**: 5-10x faster (with parallelism)

### MSFT Tool Baseline (approximate)
- 2.5GB: ~120-180 seconds
- 20GB: ~900-1200 seconds
- 100GB: Not typically used for such large packages

## Optimization Focus Areas

1. **I/O**: Memory-mapped reads, buffer pools
2. **Compression**: Parallel chunking, SIMD hashing
3. **Parallelism**: Rayon workers, thread pool tuning
4. **Memory**: Zero-copy buffers, streaming writes

## Contribution Guidelines

When adding new tests or benchmarks:
1. Document the test purpose
2. Include before/after performance metrics
3. Run on same hardware for comparison
4. Update this README with findings
"@

Set-Content -Path "$TestDataPath\README.md" -Value $readme
Write-Host "✓ Created: $TestDataPath\README.md" -ForegroundColor Green

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "Setup Complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "`nNext steps:" -ForegroundColor Yellow
Write-Host "  1. Review testdata/README.md" -ForegroundColor White
Write-Host "  2. Run: .\testdata\benchmarks\benchmark.ps1 -PackageSize medium" -ForegroundColor White
Write-Host "  3. Profile with Windows Performance Analyzer" -ForegroundColor White
Write-Host "`n" -ForegroundColor Cyan
