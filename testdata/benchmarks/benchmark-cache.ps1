# Benchmark script for IntuneWin caching performance
# Tests incremental caching benefits across different package sizes and compression levels

param(
    [string]$TestDataPath = '.\testdata',
    [string]$OutputPath = '.\testdata\output',
    [switch]$SkipLarge
)

$ErrorActionPreference = 'Stop'

$rustTool = ".\target\release\intunewin-rs.exe"

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║         IntuneWin Caching Benchmark                          ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Check tool
if (-not (Test-Path $rustTool)) {
    Write-Error "Rust tool not found. Run 'cargo build --release' first."
    exit 1
}

Write-Host "Tool: $rustTool" -ForegroundColor Green
Write-Host ""

# Define packages to test
$packages = @(
    @{ Name = "small"; Path = "$TestDataPath\packages\small"; Setup = "setup.exe" }
    @{ Name = "medium"; Path = "$TestDataPath\packages\medium"; Setup = "Samsung_Magician_installer_Official_9.0.0.910.exe" }
)

if (-not $SkipLarge) {
    $packages += @{ Name = "large"; Path = "$TestDataPath\packages\large\Windows Kits\10\ADK"; Setup = "adksetup.exe" }
}

# Compression levels to test
$compressionLevels = @(0, 6, 9)

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
    
    Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
    Write-Host "[$($pkg.Name.ToUpper())] $sizeMB MB, $fileCount files" -ForegroundColor White
    Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
    
    foreach ($level in $compressionLevels) {
        Write-Host ""
        Write-Host "  Compression Level: $level" -ForegroundColor Magenta
        
        # Clean output directory
        $pkgOutput = Join-Path $OutputPath "cache_bench_$($pkg.Name)"
        if (Test-Path $pkgOutput) { Remove-Item $pkgOutput -Recurse -Force }
        New-Item -ItemType Directory -Path $pkgOutput -Force | Out-Null
        
        # Run 1: No cache (baseline)
        Write-Host "    No Cache:     " -NoNewline -ForegroundColor Gray
        $t1 = Measure-Command { & $rustTool -c $pkg.Path -s $pkg.Setup -o $pkgOutput --compression $level -q *>$null }
        $noCacheSec = [math]::Round($t1.TotalSeconds, 2)
        $noCacheThroughput = if ($noCacheSec -gt 0) { [math]::Round($sizeMB / $noCacheSec, 1) } else { 0 }
        Write-Host "$noCacheSec s ($noCacheThroughput MB/s)" -ForegroundColor Gray
        
        # Clean for cache test
        Remove-Item "$pkgOutput\*" -Force -Recurse -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Path $pkgOutput -Force | Out-Null
        
        # Run 2: With cache (cold)
        Write-Host "    Cache (cold): " -NoNewline -ForegroundColor Yellow
        $t2 = Measure-Command { & $rustTool -c $pkg.Path -s $pkg.Setup -o $pkgOutput --compression $level --cache -q *>$null }
        $coldCacheSec = [math]::Round($t2.TotalSeconds, 2)
        $coldCacheThroughput = if ($coldCacheSec -gt 0) { [math]::Round($sizeMB / $coldCacheSec, 1) } else { 0 }
        Write-Host "$coldCacheSec s ($coldCacheThroughput MB/s)" -ForegroundColor Yellow
        
        # Run 3: With cache (warm)
        Write-Host "    Cache (warm): " -NoNewline -ForegroundColor Green
        $t3 = Measure-Command { & $rustTool -c $pkg.Path -s $pkg.Setup -o $pkgOutput --compression $level --cache -q *>$null }
        $warmCacheSec = [math]::Round($t3.TotalSeconds, 2)
        $warmCacheThroughput = if ($warmCacheSec -gt 0) { [math]::Round($sizeMB / $warmCacheSec, 1) } else { 0 }
        Write-Host "$warmCacheSec s ($warmCacheThroughput MB/s)" -ForegroundColor Green
        
        # Run 4: With cache (warm, 2nd run to confirm consistency)
        Write-Host "    Cache (warm2):" -NoNewline -ForegroundColor Green
        $t4 = Measure-Command { & $rustTool -c $pkg.Path -s $pkg.Setup -o $pkgOutput --compression $level --cache -q *>$null }
        $warm2CacheSec = [math]::Round($t4.TotalSeconds, 2)
        $warm2CacheThroughput = if ($warm2CacheSec -gt 0) { [math]::Round($sizeMB / $warm2CacheSec, 1) } else { 0 }
        Write-Host " $warm2CacheSec s ($warm2CacheThroughput MB/s)" -ForegroundColor Green
        
        # Verification: Compare cached and non-cached outputs
        Write-Host "    Verifying... " -NoNewline -ForegroundColor DarkGray
        $noCacheOutput = Join-Path $pkgOutput "no-cache.intunewin"
        $cachedOutput = Join-Path $pkgOutput "cached.intunewin"
        
        # Get the actual output file (intunewin format)
        $outputFiles = Get-ChildItem $pkgOutput -Filter "*.intunewin" -ErrorAction SilentlyContinue
        if ($outputFiles) {
            $actualOutput = $outputFiles[0].FullName
            $noCacheHash = (Get-FileHash $actualOutput -Algorithm SHA256).Hash
            
            # Re-run non-cached to get fresh output for comparison
            Remove-Item "$pkgOutput\*" -Force -Recurse -ErrorAction SilentlyContinue
            New-Item -ItemType Directory -Path $pkgOutput -Force | Out-Null
            & $rustTool -c $pkg.Path -s $pkg.Setup -o $pkgOutput --compression $level --no-cache -q *>$null
            
            $outputFiles = Get-ChildItem $pkgOutput -Filter "*.intunewin" -ErrorAction SilentlyContinue
            if ($outputFiles) {
                $noCacheActualOutput = $outputFiles[0].FullName
                $noCacheActualHash = (Get-FileHash $noCacheActualOutput -Algorithm SHA256).Hash
                
                # Run cached version for comparison
                Remove-Item "$pkgOutput\*" -Force -Recurse -ErrorAction SilentlyContinue
                New-Item -ItemType Directory -Path $pkgOutput -Force | Out-Null
                & $rustTool -c $pkg.Path -s $pkg.Setup -o $pkgOutput --compression $level --cache -q *>$null
                
                $outputFiles = Get-ChildItem $pkgOutput -Filter "*.intunewin" -ErrorAction SilentlyContinue
                if ($outputFiles) {
                    $cachedActualOutput = $outputFiles[0].FullName
                    $cachedHash = (Get-FileHash $cachedActualOutput -Algorithm SHA256).Hash
                    
                    if ($noCacheActualHash -eq $cachedHash) {
                        Write-Host "✓ Files match" -ForegroundColor Green
                    } else {
                        Write-Host "✗ Files differ!" -ForegroundColor Red
                        Write-Host "      No-cache: $noCacheActualHash" -ForegroundColor Red
                        Write-Host "      Cached:   $cachedHash" -ForegroundColor Red
                    }
                }
            }
        }
        
        # Calculate speedups
        $cacheOverhead = if ($noCacheSec -gt 0) { [math]::Round((($coldCacheSec - $noCacheSec) / $noCacheSec) * 100, 1) } else { 0 }
        $warmSpeedup = if ($warmCacheSec -gt 0) { [math]::Round($noCacheSec / $warmCacheSec, 2) } else { 0 }
        $timeSaved = [math]::Round($noCacheSec - $warmCacheSec, 2)
        
        Write-Host "    ────────────────────────────────────────────────" -ForegroundColor DarkGray
        Write-Host "    Cold cache overhead: " -NoNewline
        if ($cacheOverhead -gt 0) {
            Write-Host "+$cacheOverhead%" -ForegroundColor Yellow
        } else {
            Write-Host "$cacheOverhead%" -ForegroundColor Green
        }
        Write-Host "    Warm cache speedup:  " -NoNewline
        if ($warmSpeedup -ge 1.1) {
            Write-Host "${warmSpeedup}x faster ($timeSaved s saved)" -ForegroundColor Green
        } elseif ($warmSpeedup -ge 0.95) {
            Write-Host "${warmSpeedup}x (similar)" -ForegroundColor Yellow
        } else {
            Write-Host "${warmSpeedup}x slower" -ForegroundColor Red
        }
        
        # Get cache size
        $cacheDir = Join-Path $pkgOutput ".intunewin-cache"
        $cacheSize = 0
        if (Test-Path $cacheDir) {
            $cacheSize = [math]::Round((Get-ChildItem $cacheDir -Recurse -File | Measure-Object -Property Length -Sum).Sum / 1MB, 2)
        }
        Write-Host "    Cache size:          $cacheSize MB" -ForegroundColor DarkGray
        
        $results += [PSCustomObject]@{
            Package = $pkg.Name
            InputMB = $sizeMB
            Files = $fileCount
            Compression = $level
            NoCache_Sec = $noCacheSec
            ColdCache_Sec = $coldCacheSec
            WarmCache_Sec = $warmCacheSec
            Speedup = $warmSpeedup
            TimeSaved_Sec = $timeSaved
            CacheMB = $cacheSize
        }
        
        # Clean up
        if (Test-Path $pkgOutput) { Remove-Item $pkgOutput -Recurse -Force }
    }
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "SUMMARY" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

$results | Format-Table -AutoSize -Property Package, InputMB, Files, Compression, NoCache_Sec, WarmCache_Sec, Speedup, TimeSaved_Sec, CacheMB

Write-Host ""
Write-Host "Key Insights:" -ForegroundColor Cyan

# Group by compression level
$byCompression = $results | Group-Object Compression
foreach ($group in $byCompression) {
    $avgSpeedup = [math]::Round(($group.Group | Measure-Object -Property Speedup -Average).Average, 2)
    $avgTimeSaved = [math]::Round(($group.Group | Measure-Object -Property TimeSaved_Sec -Average).Average, 2)
    Write-Host "  Compression $($group.Name): Average speedup ${avgSpeedup}x, Average time saved ${avgTimeSaved}s" -ForegroundColor White
}

Write-Host ""
Write-Host "Recommendations:" -ForegroundColor Cyan
Write-Host "  • Use --cache for repeated builds of the same package" -ForegroundColor White
Write-Host "  • Higher compression levels benefit more from caching" -ForegroundColor White
Write-Host "  • Cache is automatically invalidated when compression level changes" -ForegroundColor White
Write-Host ""
