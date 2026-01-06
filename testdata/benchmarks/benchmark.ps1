# Benchmark script for IntuneWin encoding performance
# Compares Microsoft IntuneWinAppUtil vs Rust intunewin-rs

param(
    [string]$TestDataPath = '.\testdata',
    [string]$OutputPath = '.\testdata\output'
)

$ErrorActionPreference = 'Stop'

$msftTool = "$TestDataPath\tools\IntuneWinAppUtil.exe"
$rustTool = ".\target\release\intunewin-rs.exe"

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║         IntuneWin Packaging Benchmark                        ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Check tools
$hasMsft = Test-Path $msftTool
$hasRust = Test-Path $rustTool
Write-Host "Tools:"
Write-Host "  MSFT: $(if ($hasMsft) { '✓' } else { '✗' }) $msftTool"
Write-Host "  Rust: $(if ($hasRust) { '✓' } else { '✗' }) $rustTool"
Write-Host ""

if (-not $hasMsft -and -not $hasRust) {
    Write-Error "No tools found to benchmark"
    exit 1
}

# Define packages to test
$packages = @(
    @{ Name = "small"; Path = "$TestDataPath\packages\small"; Setup = "setup.exe" }
    @{ Name = "medium"; Path = "$TestDataPath\packages\medium"; Setup = "Samsung_Magician_installer_Official_9.0.0.910.exe" }
    @{ Name = "large"; Path = "$TestDataPath\packages\large\Windows Kits\10\ADK"; Setup = "adksetup.exe" }
)

$results = @()

foreach ($pkg in $packages) {
    if (-not (Test-Path $pkg.Path)) {
        Write-Host "⚠ Skipping $($pkg.Name) - not found" -ForegroundColor Yellow
        continue
    }
    
    # Get package info
    $files = Get-ChildItem $pkg.Path -Recurse -File -ErrorAction SilentlyContinue
    $sizeMB = [math]::Round(($files | Measure-Object -Property Length -Sum).Sum / 1MB, 2)
    $fileCount = $files.Count
    
    Write-Host "[$($pkg.Name.ToUpper())] $sizeMB MB, $fileCount files" -ForegroundColor White
    
    $msftSec = $null
    $rustSec = $null
    $msftOutputMB = $null
    $rustOutputMB = $null
    $rustThroughput = $null
    
    # Clean output directory
    $pkgOutput = Join-Path $OutputPath $pkg.Name
    if (Test-Path $pkgOutput) { Remove-Item $pkgOutput -Recurse -Force }
    New-Item -ItemType Directory -Path $pkgOutput -Force | Out-Null
    
    # MSFT Tool
    if ($hasMsft) {
        Write-Host "  MSFT: " -NoNewline -ForegroundColor Yellow
        $t = Measure-Command { & $msftTool -c $pkg.Path -s $pkg.Setup -o $pkgOutput -q *>$null }
        $msftSec = [math]::Round($t.TotalSeconds, 2)
        
        # Get output file size
        $msftOutput = Get-ChildItem $pkgOutput -Filter "*.intunewin" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($msftOutput) {
            $msftOutputMB = [math]::Round($msftOutput.Length / 1MB, 2)
        }
        $msftThroughput = if ($msftSec -gt 0) { [math]::Round($sizeMB / $msftSec, 1) } else { 0 }
        Write-Host "$msftSec s ($msftThroughput MB/s) -> $msftOutputMB MB" -ForegroundColor Yellow
        
        # Clean for next test
        Remove-Item "$pkgOutput\*" -Force -ErrorAction SilentlyContinue
    }
    
    # Rust Tool
    if ($hasRust) {
        Write-Host "  Rust: " -NoNewline -ForegroundColor Green
        $t = Measure-Command { & $rustTool -c $pkg.Path -s $pkg.Setup -o $pkgOutput -q *>$null }
        $rustSec = [math]::Round($t.TotalSeconds, 2)
        
        # Get output file size
        $rustOutput = Get-ChildItem $pkgOutput -Filter "*.intunewin" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($rustOutput) {
            $rustOutputMB = [math]::Round($rustOutput.Length / 1MB, 2)
        }
        $rustThroughput = if ($rustSec -gt 0) { [math]::Round($sizeMB / $rustSec, 1) } else { 0 }
        Write-Host "$rustSec s ($rustThroughput MB/s) -> $rustOutputMB MB" -ForegroundColor Green
    }
    
    # Speedup
    if ($msftSec -and $rustSec) {
        $speedup = [math]::Round($msftSec / $rustSec, 1)
        Write-Host "  Speedup: ${speedup}x" -ForegroundColor Cyan
    }
    
    Write-Host ""
    
    $results += [PSCustomObject]@{
        Package = $pkg.Name
        InputMB = $sizeMB
        Files = $fileCount
        MSFT_Sec = $msftSec
        MSFT_MBps = if ($msftSec -gt 0) { [math]::Round($sizeMB / $msftSec, 1) } else { "-" }
        MSFT_OutMB = $msftOutputMB
        Rust_Sec = $rustSec
        Rust_MBps = if ($rustSec -gt 0) { [math]::Round($sizeMB / $rustSec, 1) } else { "-" }
        Rust_OutMB = $rustOutputMB
        Speedup = if ($msftSec -and $rustSec) { [math]::Round($msftSec / $rustSec, 1) } else { "-" }
    }
}

Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
$results | Format-Table -AutoSize
Write-Host ""

if ($results.Speedup -ne "-") {
    $avg = ($results | Where-Object { $_.Speedup -ne "-" } | Measure-Object -Property Speedup -Average).Average
    Write-Host "Average Speedup: $([math]::Round($avg, 1))x" -ForegroundColor Green
}
