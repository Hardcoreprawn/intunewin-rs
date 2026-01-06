# Packaging Bottleneck Diagnostic
# Run this on your Azure DevOps runner to identify where time goes

param(
    [Parameter(Mandatory=$true)]
    [string]$SourceFolder,
    
    [Parameter(Mandatory=$true)]
    [string]$SetupFile,
    
    [string]$OutputFolder = ".\diagnostic_output",
    [string]$IntuneWinAppUtil = ".\testdata\tools\IntuneWinAppUtil.exe"
)

$ErrorActionPreference = "Stop"

Write-Host "╔════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  IntuneWin Packaging Bottleneck Diagnostic                 ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan

# Ensure output directory exists
if (-not (Test-Path $OutputFolder)) {
    New-Item -ItemType Directory -Path $OutputFolder -Force | Out-Null
}

$report = @{
    Timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    SourceFolder = $SourceFolder
    SetupFile = $SetupFile
    Machine = $env:COMPUTERNAME
    Results = @{}
}

# ═══════════════════════════════════════════════════════════
# 1. SYSTEM INFO
# ═══════════════════════════════════════════════════════════
Write-Host "`n📊 System Information" -ForegroundColor Yellow

$cpu = Get-CimInstance Win32_Processor
$ram = Get-CimInstance Win32_ComputerSystem
$disk = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$((Get-Location).Drive.Name):'"

$systemInfo = @{
    CPU = $cpu.Name
    Cores = $cpu.NumberOfCores
    LogicalProcessors = $cpu.NumberOfLogicalProcessors
    RAM_GB = [math]::Round($ram.TotalPhysicalMemory / 1GB, 2)
    DiskFreeSpace_GB = [math]::Round($disk.FreeSpace / 1GB, 2)
    DiskTotalSize_GB = [math]::Round($disk.Size / 1GB, 2)
}

$report.SystemInfo = $systemInfo

Write-Host "  CPU: $($systemInfo.CPU)" -ForegroundColor White
Write-Host "  Cores: $($systemInfo.Cores) (Logical: $($systemInfo.LogicalProcessors))" -ForegroundColor White
Write-Host "  RAM: $($systemInfo.RAM_GB) GB" -ForegroundColor White
Write-Host "  Disk Free: $($systemInfo.DiskFreeSpace_GB) / $($systemInfo.DiskTotalSize_GB) GB" -ForegroundColor White

# ═══════════════════════════════════════════════════════════
# 2. SOURCE FOLDER ANALYSIS
# ═══════════════════════════════════════════════════════════
Write-Host "`n📁 Source Folder Analysis" -ForegroundColor Yellow

$files = Get-ChildItem $SourceFolder -Recurse -File
$fileStats = $files | Measure-Object -Property Length -Sum -Average -Maximum

$sourceAnalysis = @{
    TotalFiles = $fileStats.Count
    TotalSize_GB = [math]::Round($fileStats.Sum / 1GB, 3)
    TotalSize_MB = [math]::Round($fileStats.Sum / 1MB, 2)
    AvgFileSize_MB = [math]::Round($fileStats.Average / 1MB, 3)
    LargestFile_MB = [math]::Round($fileStats.Maximum / 1MB, 2)
    Directories = (Get-ChildItem $SourceFolder -Recurse -Directory).Count
}

# File type distribution
$fileTypes = $files | Group-Object Extension | Sort-Object Count -Descending | Select-Object -First 10
$sourceAnalysis.TopFileTypes = $fileTypes | ForEach-Object { 
    [PSCustomObject]@{
        Extension = $_.Name
        Count = $_.Count
        TotalSize_MB = [math]::Round(($_.Group | Measure-Object -Property Length -Sum).Sum / 1MB, 2)
    }
}

$report.SourceAnalysis = $sourceAnalysis

Write-Host "  Total Files: $($sourceAnalysis.TotalFiles)" -ForegroundColor White
Write-Host "  Total Size: $($sourceAnalysis.TotalSize_GB) GB ($($sourceAnalysis.TotalSize_MB) MB)" -ForegroundColor White
Write-Host "  Avg File Size: $($sourceAnalysis.AvgFileSize_MB) MB" -ForegroundColor White
Write-Host "  Largest File: $($sourceAnalysis.LargestFile_MB) MB" -ForegroundColor White
Write-Host "  Directories: $($sourceAnalysis.Directories)" -ForegroundColor White

# ═══════════════════════════════════════════════════════════
# 3. BASELINE MEASUREMENT (CPU, Disk, Memory during packaging)
# ═══════════════════════════════════════════════════════════
Write-Host "`n⏱️  Packaging Performance Measurement" -ForegroundColor Yellow
Write-Host "  Running IntuneWinAppUtil (this may take a while)..." -ForegroundColor Gray

# Start resource monitoring job
$monitorJob = Start-Job -ScriptBlock {
    $samples = @()
    while ($true) {
        $cpu = (Get-Counter '\Processor(_Total)\% Processor Time' -ErrorAction SilentlyContinue).CounterSamples.CookedValue
        $disk = (Get-Counter '\PhysicalDisk(_Total)\% Disk Time' -ErrorAction SilentlyContinue).CounterSamples.CookedValue
        $memAvail = (Get-Counter '\Memory\Available MBytes' -ErrorAction SilentlyContinue).CounterSamples.CookedValue
        
        $samples += [PSCustomObject]@{
            Timestamp = Get-Date
            CPU = $cpu
            Disk = $disk
            MemAvailMB = $memAvail
        }
        Start-Sleep -Milliseconds 500
    }
}

# Run packaging
$outputFile = Join-Path $OutputFolder "$($SetupFile).intunewin"
$packagingStart = Get-Date

try {
    $packagingResult = Measure-Command {
        & $IntuneWinAppUtil -c $SourceFolder -s $SetupFile -o $OutputFolder -q 2>&1
    }
    $packagingSuccess = $true
}
catch {
    $packagingSuccess = $false
    $packagingError = $_
}

$packagingEnd = Get-Date

# Stop monitoring
Stop-Job $monitorJob
$resourceSamples = Receive-Job $monitorJob
Remove-Job $monitorJob

# Analyze resource usage during packaging
$resourceAnalysis = @{
    AvgCPU = [math]::Round(($resourceSamples.CPU | Measure-Object -Average).Average, 2)
    MaxCPU = [math]::Round(($resourceSamples.CPU | Measure-Object -Maximum).Maximum, 2)
    AvgDisk = [math]::Round(($resourceSamples.Disk | Measure-Object -Average).Average, 2)
    MaxDisk = [math]::Round(($resourceSamples.Disk | Measure-Object -Maximum).Maximum, 2)
    MinMemAvailMB = [math]::Round(($resourceSamples.MemAvailMB | Measure-Object -Minimum).Minimum, 0)
    SampleCount = $resourceSamples.Count
}

$packagingStats = @{
    Success = $packagingSuccess
    Duration_Seconds = [math]::Round($packagingResult.TotalSeconds, 2)
    Duration_Minutes = [math]::Round($packagingResult.TotalMinutes, 3)
    Throughput_MBps = [math]::Round($sourceAnalysis.TotalSize_MB / $packagingResult.TotalSeconds, 2)
    ResourceUsage = $resourceAnalysis
}

if ($packagingSuccess -and (Test-Path $outputFile)) {
    $outputFileInfo = Get-Item $outputFile
    $packagingStats.OutputSize_MB = [math]::Round($outputFileInfo.Length / 1MB, 2)
    $packagingStats.CompressionRatio = [math]::Round($sourceAnalysis.TotalSize_MB / ($outputFileInfo.Length / 1MB), 2)
}

$report.PackagingStats = $packagingStats

Write-Host "  Duration: $($packagingStats.Duration_Seconds) seconds ($($packagingStats.Duration_Minutes) minutes)" -ForegroundColor White
Write-Host "  Throughput: $($packagingStats.Throughput_MBps) MB/sec" -ForegroundColor White
if ($packagingStats.OutputSize_MB) {
    Write-Host "  Output Size: $($packagingStats.OutputSize_MB) MB (Compression: $($packagingStats.CompressionRatio):1)" -ForegroundColor White
}
Write-Host "  Avg CPU: $($resourceAnalysis.AvgCPU)% (Max: $($resourceAnalysis.MaxCPU)%)" -ForegroundColor White
Write-Host "  Avg Disk: $($resourceAnalysis.AvgDisk)% (Max: $($resourceAnalysis.MaxDisk)%)" -ForegroundColor White

# ═══════════════════════════════════════════════════════════
# 4. BOTTLENECK ANALYSIS
# ═══════════════════════════════════════════════════════════
Write-Host "`n🔍 Bottleneck Analysis" -ForegroundColor Yellow

$bottleneck = "Unknown"
$recommendations = @()

if ($resourceAnalysis.AvgCPU -gt 80) {
    $bottleneck = "CPU-bound"
    $recommendations += "✅ Rust CLI with parallel compression will help significantly"
    $recommendations += "✅ Consider runner with more CPU cores"
    $recommendations += "✅ Expected improvement: 3-5x with multi-threading"
}
elseif ($resourceAnalysis.AvgDisk -gt 80) {
    $bottleneck = "I/O-bound"
    $recommendations += "⚠️ Disk is the bottleneck, not CPU"
    $recommendations += "✅ Upgrade to NVMe SSD"
    $recommendations += "✅ Consider RAM disk for temp files"
    $recommendations += "⚠️ Rust CLI will help less (maybe 1.5-2x)"
}
elseif ($resourceAnalysis.AvgCPU -lt 50 -and $resourceAnalysis.AvgDisk -lt 50) {
    $bottleneck = "Neither (possibly single-threaded)"
    $recommendations += "✅ Tool may be single-threaded"
    $recommendations += "✅ Rust CLI with parallelism will help"
    $recommendations += "✅ Can also run multiple packaging jobs in parallel"
}
else {
    $bottleneck = "Mixed"
    $recommendations += "✅ Both CPU and I/O are being used"
    $recommendations += "✅ Rust CLI will help on CPU side"
    $recommendations += "✅ Consider hardware upgrades for further gains"
}

$report.BottleneckAnalysis = @{
    Bottleneck = $bottleneck
    Recommendations = $recommendations
}

Write-Host "  Primary Bottleneck: $bottleneck" -ForegroundColor Cyan
Write-Host "`n  Recommendations:" -ForegroundColor Yellow
foreach ($rec in $recommendations) {
    Write-Host "    $rec" -ForegroundColor White
}

# ═══════════════════════════════════════════════════════════
# 5. PROJECTIONS
# ═══════════════════════════════════════════════════════════
Write-Host "`n📈 Performance Projections" -ForegroundColor Yellow

$projections = @{
    CurrentTime_Sec = $packagingStats.Duration_Seconds
    WithRustCLI_Sec = [math]::Round($packagingStats.Duration_Seconds / 3, 2)  # Conservative 3x
    WithCaching_Sec = 5  # Near instant for unchanged content
    PackagesPerHour_Current = [math]::Round(3600 / $packagingStats.Duration_Seconds, 1)
    PackagesPerHour_Optimized = [math]::Round(3600 / ($packagingStats.Duration_Seconds / 3), 1)
}

$report.Projections = $projections

Write-Host "  Current: $($projections.CurrentTime_Sec) sec/package ($($projections.PackagesPerHour_Current) packages/hour)" -ForegroundColor White
Write-Host "  With Rust CLI (3x): $($projections.WithRustCLI_Sec) sec/package ($($projections.PackagesPerHour_Optimized) packages/hour)" -ForegroundColor Green
Write-Host "  With Caching (unchanged): ~5 sec (hash check only)" -ForegroundColor Green

# ═══════════════════════════════════════════════════════════
# 6. SAVE REPORT
# ═══════════════════════════════════════════════════════════
$reportPath = Join-Path $OutputFolder "diagnostic_report_$(Get-Date -Format 'yyyyMMdd_HHmmss').json"
$report | ConvertTo-Json -Depth 5 | Set-Content $reportPath

Write-Host "`n📄 Report saved to: $reportPath" -ForegroundColor Green

# Summary
Write-Host "`n╔════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  SUMMARY                                                   ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Source Size:     $($sourceAnalysis.TotalSize_MB) MB ($($sourceAnalysis.TotalFiles) files)" -ForegroundColor White
Write-Host "  Packaging Time:  $($packagingStats.Duration_Seconds) seconds" -ForegroundColor White
Write-Host "  Throughput:      $($packagingStats.Throughput_MBps) MB/sec" -ForegroundColor White
Write-Host "  Bottleneck:      $bottleneck" -ForegroundColor Yellow
Write-Host ""
Write-Host "  🎯 Rust CLI Expected Improvement: " -NoNewline -ForegroundColor White
if ($bottleneck -eq "CPU-bound") {
    Write-Host "3-5x faster" -ForegroundColor Green
} elseif ($bottleneck -eq "I/O-bound") {
    Write-Host "1.5-2x faster" -ForegroundColor Yellow
} else {
    Write-Host "2-4x faster" -ForegroundColor Green
}
Write-Host ""
