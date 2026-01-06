# Download Microsoft Intune Win App Util
# This downloads the official tool for comparison/profiling

param(
    [string]$OutputPath = '.\testdata\tools\intunewinapputil.exe'
)

Write-Host "Downloading Microsoft intunewinapputil.exe..." -ForegroundColor Cyan
Write-Host "This is the reference tool for performance comparison." -ForegroundColor Gray

# Try GitHub releases (primary source)
$urls = @(
    "https://github.com/microsoft/Intune-App-Wrapping-Tool-Windows/releases/download/1.0.0.0/intunewinapputil.exe",
    "https://github.com/microsoft/Intune-App-Wrapping-Tool-Windows/raw/master/bin/Release/intunewinapputil.exe"
)

[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12

foreach ($url in $urls) {
    try {
        Write-Host "Trying: $url" -ForegroundColor Gray
        Invoke-WebRequest -Uri $url -OutFile $OutputPath -UseBasicParsing -TimeoutSec 30 -ErrorAction Stop
        Write-Host "✓ Successfully downloaded to: $OutputPath" -ForegroundColor Green
        Get-Item $OutputPath | Select-Object FullName, @{N='SizeMB'; E={[math]::Round($_.Length/1MB, 2)}}
        exit 0
    }
    catch {
        Write-Verbose "Failed: $_"
    }
}

Write-Host "`n⚠ Download from GitHub failed." -ForegroundColor Yellow
Write-Host "`nAlternative options:" -ForegroundColor Yellow
Write-Host "1. Download manually from:" -ForegroundColor White
Write-Host "   https://github.com/microsoft/Intune-App-Wrapping-Tool-Windows/releases" -ForegroundColor Cyan
Write-Host "" -ForegroundColor White
Write-Host "2. Or from Microsoft Intune docs:" -ForegroundColor White
Write-Host "   https://learn.microsoft.com/en-us/mem/intune/developer/" -ForegroundColor Cyan
Write-Host "" -ForegroundColor White
Write-Host "3. Place the downloaded file at:" -ForegroundColor White
Write-Host "   $OutputPath" -ForegroundColor Cyan
Write-Host "" -ForegroundColor White
Write-Host "Note: If you have the tool installed on your system, copy from:" -ForegroundColor White
Write-Host "   C:\Program Files\Windows Kits\*\bin\*\intunewinapputil.exe" -ForegroundColor Cyan
