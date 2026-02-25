param(
    [string]$ContentPath = ".\testdata\packages\small",
    [string]$SetupFile = "setup.exe",
    [string]$WorkDir = ".\target\pre-intune-validation",
    [string]$MsToolPath = ".\testdata\tools\intunewinapputil.exe",
    [int64]$AllowedUnencryptedSizeDeltaBytes = 512
)

$ErrorActionPreference = 'Stop'

function Assert-Exists {
    param(
        [string]$Path,
        [string]$Message
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw $Message
    }
}

function Invoke-DotNetZipRead {
    param([string]$PackagePath)

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead((Resolve-Path $PackagePath))

    try {
        $entry = $zip.Entries | Where-Object {
            $_.FullName -eq 'IntuneWinPackage/Metadata/Detection.xml'
        } | Select-Object -First 1

        if ($null -eq $entry) {
            throw "Detection.xml not found in $PackagePath"
        }

        $reader = New-Object System.IO.StreamReader($entry.Open())
        try {
            return $reader.ReadToEnd()
        }
        finally {
            $reader.Dispose()
        }
    }
    finally {
        $zip.Dispose()
    }
}

function Invoke-SevenZipTest {
    param([string]$PackagePath)

    $sevenZip = Get-Command 7z -ErrorAction SilentlyContinue
    if ($null -eq $sevenZip) {
        Write-Host "7z not found in PATH; skipping optional secondary reader check for $PackagePath" -ForegroundColor Yellow
        return $false
    }

    & $sevenZip.Source t $PackagePath | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "7z integrity test failed for $PackagePath"
    }

    return $true
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Pre-Intune Validation Gate" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

if (-not (Test-Path -LiteralPath $ContentPath)) {
    Write-Host "SKIP: Content path not found: $ContentPath (run setup-test-environment.ps1 to generate)" -ForegroundColor Yellow
    exit 0
}

if (-not (Test-Path -LiteralPath $MsToolPath)) {
    Write-Host "Microsoft IntuneWinAppUtil not found; attempting download..." -ForegroundColor Yellow
    & ".\tests\download-intunewinapputil.ps1" -OutputPath $MsToolPath
}

Assert-Exists -Path $MsToolPath -Message "Microsoft IntuneWinAppUtil unavailable at: $MsToolPath"

$runId = (Get-Date).ToString('yyyyMMddHHmmssfff')
$runRoot = Join-Path $WorkDir $runId
$msOutDir = Join-Path $runRoot "msft"
$rsOutDir = Join-Path $runRoot "rs"
New-Item -ItemType Directory -Path $msOutDir -Force | Out-Null
New-Item -ItemType Directory -Path $rsOutDir -Force | Out-Null

Write-Host "[1/4] Building package with Microsoft tool" -ForegroundColor Yellow
& $MsToolPath -c $ContentPath -s $SetupFile -o $msOutDir -q
if ($LASTEXITCODE -ne 0) {
    throw "IntuneWinAppUtil failed with exit code $LASTEXITCODE"
}

Write-Host "[2/4] Building package with intunewin-rs" -ForegroundColor Yellow
cargo run --release -- -c $ContentPath -s $SetupFile -o $rsOutDir -q
if ($LASTEXITCODE -ne 0) {
    throw "intunewin-rs failed with exit code $LASTEXITCODE"
}

$msPackage = Join-Path $msOutDir "setup.intunewin"
$rsPackage = Join-Path $rsOutDir "setup.intunewin"

Assert-Exists -Path $msPackage -Message "MS package not generated: $msPackage"
Assert-Exists -Path $rsPackage -Message "Rust package not generated: $rsPackage"

Write-Host "[3/4] Archive validation (.NET required, 7z optional)" -ForegroundColor Yellow
$msDetectionXml = Invoke-DotNetZipRead -PackagePath $msPackage
$rsDetectionXml = Invoke-DotNetZipRead -PackagePath $rsPackage
$ms7z = Invoke-SevenZipTest -PackagePath $msPackage
$rs7z = Invoke-SevenZipTest -PackagePath $rsPackage

Write-Host "[4/4] Detection.xml parity sanity checks" -ForegroundColor Yellow
[xml]$msXml = $msDetectionXml
[xml]$rsXml = $rsDetectionXml

$msInfo = $msXml.ApplicationInfo
$rsInfo = $rsXml.ApplicationInfo

if ($null -eq $msInfo -or $null -eq $rsInfo) {
    throw "ApplicationInfo section missing in one or both Detection.xml files"
}

$requiredFields = @(
    'FileName',
    'SetupFile',
    'EncryptionInfo',
    'UnencryptedContentSize'
)

foreach ($field in $requiredFields) {
    if ([string]::IsNullOrWhiteSpace([string]$msInfo.$field)) {
        throw "Microsoft Detection.xml missing field: $field"
    }
    if ([string]::IsNullOrWhiteSpace([string]$rsInfo.$field)) {
        throw "Rust Detection.xml missing field: $field"
    }
}

$msSize = [int64]$msInfo.UnencryptedContentSize
$rsSize = [int64]$rsInfo.UnencryptedContentSize
$delta = [math]::Abs($msSize - $rsSize)

if ($delta -gt $AllowedUnencryptedSizeDeltaBytes) {
    throw "UnencryptedContentSize drift exceeds threshold: delta=$delta bytes (allowed=$AllowedUnencryptedSizeDeltaBytes, ms=$msSize, rs=$rsSize)"
}

$result = [pscustomobject]@{
    ms_package = (Resolve-Path $msPackage).Path
    rs_package = (Resolve-Path $rsPackage).Path
    ms_unencrypted_content_size = $msSize
    rs_unencrypted_content_size = $rsSize
    absolute_delta_bytes = $delta
    allowed_delta_bytes = $AllowedUnencryptedSizeDeltaBytes
    seven_zip_checked_ms = $ms7z
    seven_zip_checked_rs = $rs7z
    status = "PASS"
}

Write-Host "Pre-Intune validation gate passed" -ForegroundColor Green
$result | ConvertTo-Json -Depth 4 | Out-Host
