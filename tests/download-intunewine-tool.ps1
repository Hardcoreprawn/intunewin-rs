# Download Microsoft Win32 Content Prep Tool (IntuneWinAppUtil)
# This is the official tool at: https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool

param(
    [string]$OutputPath = '.\testdata\tools\IntuneWinAppUtil.exe',
    [string]$Version = 'v1.8.7'
)

$ErrorActionPreference = 'Stop'
Write-Host "Downloading Microsoft Win32 Content Prep Tool..." -ForegroundColor Cyan
Write-Host "Repository: https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool" -ForegroundColor Gray
Write-Host "Version: $Version" -ForegroundColor Gray

# Ensure output directory exists
$outputDir = Split-Path -Parent $OutputPath
if (-not (Test-Path $outputDir)) {
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
}

[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12

# Download from GitHub releases
$releaseUrl = "https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool/releases/download/$Version/IntuneWinAppUtil.exe"

try {
    Write-Host "Downloading from: $releaseUrl" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $releaseUrl -OutFile $OutputPath -UseBasicParsing -TimeoutSec 60 -ErrorAction Stop
    Write-Host "✓ Downloaded successfully!" -ForegroundColor Green
    
    # Verify file
    if (Test-Path $OutputPath) {
        $fileInfo = Get-Item $OutputPath
        Write-Host "  Path: $($fileInfo.FullName)" -ForegroundColor White
        Write-Host "  Size: $([math]::Round($fileInfo.Length/1MB, 2)) MB" -ForegroundColor White
        Write-Host "  Version: " -NoNewline
        & $OutputPath -v
        exit 0
    }
}
catch {
    Write-Host "✗ Download failed: $_" -ForegroundColor Red
}

# Fallback: Try alternate download location
Write-Host "`nAttempting fallback download..." -ForegroundColor Yellow
$fallbackUrl = "https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool/raw/$Version/IntuneWinAppUtil.exe"

try {
    Invoke-WebRequest -Uri $fallbackUrl -OutFile $OutputPath -UseBasicParsing -TimeoutSec 60 -ErrorAction Stop
    Write-Host "✓ Downloaded from fallback location" -ForegroundColor Green
    exit 0
}
catch {
    Write-Host "✗ Fallback also failed" -ForegroundColor Red
}

Write-Host "`n⚠ Could not download automatically." -ForegroundColor Yellow
Write-Host "`nManual download options:" -ForegroundColor Yellow
Write-Host "1. Visit: https://github.com/microsoft/Microsoft-Win32-Content-Prep-Tool/releases" -ForegroundColor Cyan
Write-Host "2. Download IntuneWinAppUtil.exe (v1.8.7 or latest)" -ForegroundColor Cyan
Write-Host "3. Place at: $OutputPath" -ForegroundColor Cyan
Write-Host "`nRequirements:" -ForegroundColor Yellow
Write-Host "  - .NET Framework 4.7.2 or higher" -ForegroundColor White
Write-Host "  - Windows 7 or later" -ForegroundColor White
