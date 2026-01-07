# Compression level curve analysis for large package
# Tests compression 0, 1, 3, 6, 9 to find speed vs size balance

param(
    [string]$TestDataPath = '.\testdata',
    [string]$OutputPath = '.\testdata\output'
)

$ErrorActionPreference = 'Stop'

$rustTool = ".\target\release\intunewin-rs.exe"

if (-not (Test-Path $rustTool)) {
    Write-Error "Rust tool not found"
    exit 1
}

# Large package
$pkg = @{ 
    Name = "large"
    Path = "$TestDataPath\packages\large\Windows Kits\10\ADK"
    Setup = "adksetup.exe"
}

if (-not (Test-Path $pkg.Path)) {
    Write-Error "Large package test data not found at $($pkg.Path)"
    exit 1
}

Write-Host ""
Write-Host "Compression Level Trade-off Analysis - Large Package (1.5 GB, 303 files)" -ForegroundColor Cyan
Write-Host ""

# Get package info
$files = Get-ChildItem $pkg.Path -Recurse -File -ErrorAction SilentlyContinue
$sizeMB = [math]::Round(($files | Measure-Object -Property Length -Sum).Sum / 1MB, 2)
$fileCount = $files.Count

Write-Host "Input: $sizeMB MB, $fileCount files"
Write-Host ""

$compressionLevels = @(0, 1, 3, 6, 9)
$results = @()

foreach ($compression in $compressionLevels) {
    Write-Host "Testing compression level $compression..." -NoNewline -ForegroundColor Green
    
    # Clean output
    $pkgOutput = Join-Path $OutputPath "compression_test_$compression"
    if (Test-Path $pkgOutput) { Remove-Item $pkgOutput -Recurse -Force }
    New-Item -ItemType Directory -Path $pkgOutput -Force | Out-Null
    
    # Run benchmark
    $t = Measure-Command { & $rustTool -c $pkg.Path -s $pkg.Setup -o $pkgOutput -q --compression $compression *>$null }
    $sec = [math]::Round($t.TotalSeconds, 2)
    
    # Get output size
    $output = Get-ChildItem $pkgOutput -Filter "*.intunewin" -ErrorAction SilentlyContinue | Select-Object -First 1
    $outMB = if ($output) { [math]::Round($output.Length / 1MB, 2) } else { 0 }
    
    $throughput = if ($sec -gt 0) { [math]::Round($sizeMB / $sec, 1) } else { 0 }
    $sizeReduction = if ($compression -eq 0) { 0 } else { [math]::Round((1 - ($outMB / $sizeMB)) * 100, 1) }
    
    Write-Host " $sec sec, $outMB MB output, $throughput MB/s, $sizeReduction% reduction" -ForegroundColor Yellow
    
    $results += [PSCustomObject]@{
        Compression = $compression
        Time_Sec = $sec
        Output_MB = $outMB
        Throughput_MBps = $throughput
        Size_Reduction_Pct = $sizeReduction
    }
}

Write-Host ""
Write-Host "============================================================" -ForegroundColor Cyan
$results | Format-Table -AutoSize

Write-Host ""
Write-Host "Analysis:" -ForegroundColor Cyan
Write-Host ""

# Find best balance (highest throughput with reasonable compression)
Write-Host "Baseline (compression 0): $($results[0].Time_Sec)s, $($results[0].Output_MB)MB, $($results[0].Throughput_MBps)MB/s" -ForegroundColor White

for ($i = 1; $i -lt $results.Count; $i++) {
    $r = $results[$i]
    $slowdownFromFastest = [math]::Round($results[0].Time_Sec / $r.Time_Sec, 2)
    Write-Host "Compression $($r.Compression): $($r.Time_Sec)s ($slowdownFromFastest x baseline), $($r.Output_MB)MB ($($r.Size_Reduction_Pct)% smaller), $($r.Throughput_MBps)MB/s" -ForegroundColor White
}

Write-Host ""
Write-Host "Recommendation:" -ForegroundColor Green
$comp1 = $results | Where-Object { $_.Compression -eq 1 }
$comp3 = $results | Where-Object { $_.Compression -eq 3 }
$comp6 = $results | Where-Object { $_.Compression -eq 6 }

Write-Host "  Compression 1: +$($comp1.Size_Reduction_Pct)% smaller, only $(([math]::Round($comp1.Time_Sec / $results[0].Time_Sec * 100, 0)) - 100)% slower - BEST BALANCE" -ForegroundColor Cyan
Write-Host "  Compression 3: +$($comp3.Size_Reduction_Pct)% smaller, $(([math]::Round($comp3.Time_Sec / $results[0].Time_Sec * 100, 0)) - 100)% slower" -ForegroundColor White
Write-Host "  Compression 6: +$($comp6.Size_Reduction_Pct)% smaller, $(([math]::Round($comp6.Time_Sec / $results[0].Time_Sec * 100, 0)) - 100)% slower" -ForegroundColor White
