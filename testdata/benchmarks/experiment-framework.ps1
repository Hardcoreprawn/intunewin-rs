param(
    [string]$TestDataPath = '.\testdata',
    [string]$ResultsRoot = '.\testdata\benchmarks\results',
    [string]$ControlLabel = 'control',
    [string]$CandidateLabel = 'candidate',
    [string]$ControlCommandTemplate = '.\target\release\intunewin-rs.exe -c "{CONTENT}" -s "{SETUP}" -o "{OUTPUT}" -q',
    [string]$CandidateCommandTemplate = '.\target\release\intunewin-rs.exe -c "{CONTENT}" -s "{SETUP}" -o "{OUTPUT}" -q',
    [int]$WarmupRuns = 1,
    [int]$Iterations = 5,
    [switch]$IncludeLarge,
    [switch]$Strict
)

$ErrorActionPreference = 'Stop'

function Get-Percentile {
    param(
        [double[]]$Values,
        [double]$Percentile
    )

    if ($null -eq $Values -or $Values.Count -eq 0) {
        return $null
    }

    $sorted = $Values | Sort-Object
    if ($sorted.Count -eq 1) {
        return [double]$sorted[0]
    }

    $rank = ($Percentile / 100.0) * ($sorted.Count - 1)
    $low = [math]::Floor($rank)
    $high = [math]::Ceiling($rank)

    if ($low -eq $high) {
        return [double]$sorted[$low]
    }

    $weight = $rank - $low
    return [double]($sorted[$low] + (($sorted[$high] - $sorted[$low]) * $weight))
}

function Test-DetectionXmlReadable {
    param([string]$PackagePath)

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead((Resolve-Path $PackagePath))
    try {
        $entry = $zip.Entries | Where-Object { $_.FullName -eq 'IntuneWinPackage/Metadata/Detection.xml' } | Select-Object -First 1
        if ($null -eq $entry) {
            return $false
        }

        $reader = New-Object System.IO.StreamReader($entry.Open())
        try {
            $text = $reader.ReadToEnd()
            return -not [string]::IsNullOrWhiteSpace($text)
        }
        finally {
            $reader.Dispose()
        }
    }
    catch {
        return $false
    }
    finally {
        $zip.Dispose()
    }
}

function Invoke-BenchmarkCommand {
    param(
        [string]$Command,
        [string]$WorkingDirectory
    )

    Push-Location $WorkingDirectory
    try {
        $start = Get-Date
        $proc = Start-Process -FilePath 'cmd.exe' -ArgumentList '/c', $Command -PassThru -NoNewWindow
        $peakWorkingSet = 0

        while (-not $proc.HasExited) {
            if ($proc.WorkingSet64 -gt $peakWorkingSet) {
                $peakWorkingSet = $proc.WorkingSet64
            }
            Start-Sleep -Milliseconds 50
        }

        if ($proc.WorkingSet64 -gt $peakWorkingSet) {
            $peakWorkingSet = $proc.WorkingSet64
        }

        $proc.WaitForExit()
        $proc.Refresh()

        $elapsedMs = ((Get-Date) - $start).TotalMilliseconds
        return [PSCustomObject]@{
            ExitCode = [int]$proc.ExitCode
            DurationMs = [math]::Round($elapsedMs, 2)
            PeakWorkingSetMB = [math]::Round($peakWorkingSet / 1MB, 2)
            CpuTimeMs = [math]::Round($proc.TotalProcessorTime.TotalMilliseconds, 2)
        }
    }
    finally {
        Pop-Location
    }
}

function Invoke-VariantOnDataset {
    param(
        [string]$Variant,
        [string]$CommandTemplate,
        [hashtable]$Dataset,
        [string]$RunRoot,
        [int]$Warmups,
        [int]$Runs,
        [switch]$StrictMode
    )

    $variantRoot = Join-Path $RunRoot "$($Dataset.Name)_$Variant"
    if (Test-Path $variantRoot) {
        Remove-Item $variantRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $variantRoot -Force | Out-Null

    $setupStem = [System.IO.Path]::GetFileNameWithoutExtension($Dataset.Setup)
    $packagePath = Join-Path $variantRoot ("$setupStem.intunewin")

    $cmd = $CommandTemplate.Replace('{CONTENT}', $Dataset.Path).Replace('{SETUP}', $Dataset.Setup).Replace('{OUTPUT}', $variantRoot)

    for ($i = 1; $i -le $Warmups; $i++) {
        Write-Host "  [$Variant/$($Dataset.Name)] Warmup $i/$Warmups" -ForegroundColor DarkGray
        $warm = Invoke-BenchmarkCommand -Command $cmd -WorkingDirectory (Get-Location)
        if ($warm.ExitCode -ne 0) {
            throw "Warmup failed for $Variant/$($Dataset.Name) with exit code $($warm.ExitCode)"
        }
    }

    $samples = @()
    for ($i = 1; $i -le $Runs; $i++) {
        Write-Host "  [$Variant/$($Dataset.Name)] Iteration $i/$Runs" -ForegroundColor Gray
        $result = Invoke-BenchmarkCommand -Command $cmd -WorkingDirectory (Get-Location)

        $packageExists = Test-Path $packagePath
        $packageSizeMB = if ($packageExists) { [math]::Round((Get-Item $packagePath).Length / 1MB, 3) } else { $null }
        $dotnetReadable = if ($packageExists) { Test-DetectionXmlReadable -PackagePath $packagePath } else { $false }

        if ($StrictMode -and ($result.ExitCode -ne 0 -or -not $dotnetReadable)) {
            throw "Strict mode failure for $Variant/$($Dataset.Name): exit=$($result.ExitCode), dotnetReadable=$dotnetReadable"
        }

        $samples += [PSCustomObject]@{
            iteration = $i
            exit_code = $result.ExitCode
            duration_ms = $result.DurationMs
            peak_working_set_mb = $result.PeakWorkingSetMB
            cpu_time_ms = $result.CpuTimeMs
            package_exists = $packageExists
            package_size_mb = $packageSizeMB
            dotnet_detection_xml_readable = $dotnetReadable
        }
    }

    $durations = @($samples | ForEach-Object { [double]$_.duration_ms })
    $rss = @($samples | ForEach-Object { [double]$_.peak_working_set_mb })

    return [PSCustomObject]@{
        variant = $Variant
        dataset = $Dataset.Name
        setup = $Dataset.Setup
        content_path = $Dataset.Path
        samples = $samples
        summary = [PSCustomObject]@{
            p50_duration_ms = [math]::Round((Get-Percentile -Values $durations -Percentile 50), 2)
            p95_duration_ms = [math]::Round((Get-Percentile -Values $durations -Percentile 95), 2)
            avg_duration_ms = [math]::Round((($durations | Measure-Object -Average).Average), 2)
            peak_rss_mb = [math]::Round((($rss | Measure-Object -Maximum).Maximum), 2)
            any_failure = ($samples | Where-Object { $_.exit_code -ne 0 }).Count -gt 0
            all_dotnet_readable = ($samples | Where-Object { -not $_.dotnet_detection_xml_readable }).Count -eq 0
        }
    }
}

Write-Host ''
Write-Host '==============================================================' -ForegroundColor Cyan
Write-Host 'IntuneWin Experiment Framework (Issue #75 baseline)' -ForegroundColor Cyan
Write-Host '==============================================================' -ForegroundColor Cyan
Write-Host ''

$datasets = @(
    @{ Name = 'small'; Path = "$TestDataPath\packages\small"; Setup = 'setup.exe' },
    @{ Name = 'medium'; Path = "$TestDataPath\packages\medium"; Setup = 'Samsung_Magician_installer_Official_9.0.0.910.exe' }
)

if ($IncludeLarge) {
    $datasets += @{ Name = 'large'; Path = "$TestDataPath\packages\large\Windows Kits\10\ADK"; Setup = 'adksetup.exe' }
}

$available = @()
foreach ($d in $datasets) {
    if (Test-Path $d.Path) {
        $available += $d
    }
    else {
        Write-Host "Skipping dataset '$($d.Name)' (missing path: $($d.Path))" -ForegroundColor Yellow
    }
}

if ($available.Count -eq 0) {
    throw 'No datasets available to run benchmark framework'
}

$timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$runRoot = Join-Path $ResultsRoot $timestamp
New-Item -ItemType Directory -Path $runRoot -Force | Out-Null

Write-Host "Results root: $runRoot" -ForegroundColor Cyan

$allResults = @()
foreach ($dataset in $available) {
    Write-Host "Running dataset: $($dataset.Name)" -ForegroundColor White

    $allResults += Invoke-VariantOnDataset -Variant $ControlLabel -CommandTemplate $ControlCommandTemplate -Dataset $dataset -RunRoot $runRoot -Warmups $WarmupRuns -Runs $Iterations -StrictMode:$Strict
    $allResults += Invoke-VariantOnDataset -Variant $CandidateLabel -CommandTemplate $CandidateCommandTemplate -Dataset $dataset -RunRoot $runRoot -Warmups $WarmupRuns -Runs $Iterations -StrictMode:$Strict
}

$comparisons = @()
foreach ($dataset in ($allResults | Select-Object -ExpandProperty dataset -Unique)) {
    $control = $allResults | Where-Object { $_.dataset -eq $dataset -and $_.variant -eq $ControlLabel } | Select-Object -First 1
    $candidate = $allResults | Where-Object { $_.dataset -eq $dataset -and $_.variant -eq $CandidateLabel } | Select-Object -First 1

    if ($null -eq $control -or $null -eq $candidate) {
        continue
    }

    $controlP50 = [double]$control.summary.p50_duration_ms
    $candidateP50 = [double]$candidate.summary.p50_duration_ms
    $controlP95 = [double]$control.summary.p95_duration_ms
    $candidateP95 = [double]$candidate.summary.p95_duration_ms

    $p50GainPct = if ($controlP50 -gt 0) { [math]::Round((($controlP50 - $candidateP50) / $controlP50) * 100, 2) } else { 0 }
    $p95GainPct = if ($controlP95 -gt 0) { [math]::Round((($controlP95 - $candidateP95) / $controlP95) * 100, 2) } else { 0 }

    $comparisons += [PSCustomObject]@{
        dataset = $dataset
        control_p50_ms = $controlP50
        candidate_p50_ms = $candidateP50
        p50_gain_pct = $p50GainPct
        control_p95_ms = $controlP95
        candidate_p95_ms = $candidateP95
        p95_gain_pct = $p95GainPct
        candidate_peak_rss_mb = $candidate.summary.peak_rss_mb
        candidate_all_dotnet_readable = $candidate.summary.all_dotnet_readable
    }
}

$overallP50Gain = if ($comparisons.Count -gt 0) { [math]::Round((($comparisons | Measure-Object -Property p50_gain_pct -Average).Average), 2) } else { 0 }
$overallP95Gain = if ($comparisons.Count -gt 0) { [math]::Round((($comparisons | Measure-Object -Property p95_gain_pct -Average).Average), 2) } else { 0 }
$allReadable = ($comparisons | Where-Object { -not $_.candidate_all_dotnet_readable }).Count -eq 0

$recommendation = if (($overallP50Gain -ge 15) -or ($overallP95Gain -ge 25)) {
    'Adopt'
}
elseif ($overallP50Gain -ge 8) {
    'Conditional'
}
else {
    'Reject/Defer'
}

$report = [PSCustomObject]@{
    generated_at_utc = (Get-Date).ToUniversalTime().ToString('o')
    control_label = $ControlLabel
    candidate_label = $CandidateLabel
    iterations = $Iterations
    warmups = $WarmupRuns
    strict_mode = [bool]$Strict
    datasets = $available
    results = $allResults
    comparisons = $comparisons
    decision = [PSCustomObject]@{
        overall_p50_gain_pct = $overallP50Gain
        overall_p95_gain_pct = $overallP95Gain
        all_candidate_dotnet_readable = $allReadable
        recommendation = $recommendation
        gates = [PSCustomObject]@{
            adopt_p50_threshold_pct = 15
            adopt_p95_threshold_pct = 25
            conditional_lower_pct = 8
        }
    }
}

$jsonPath = Join-Path $runRoot 'summary.json'
$mdPath = Join-Path $runRoot 'summary.md'

$report | ConvertTo-Json -Depth 8 | Set-Content -Path $jsonPath -Encoding UTF8

$md = @()
$md += '# Experiment Summary'
$md += ''
$md += "- Generated: $($report.generated_at_utc)"
$md += "- Control: $ControlLabel"
$md += "- Candidate: $CandidateLabel"
$md += "- Iterations: $Iterations"
$md += "- Warmups: $WarmupRuns"
$md += ''
$md += '## Decision'
$md += ''
$md += "- Overall p50 gain: $overallP50Gain%"
$md += "- Overall p95 gain: $overallP95Gain%"
$md += "- Candidate .NET readability: $allReadable"
$md += "- Recommendation: **$recommendation**"
$md += ''
$md += '## Dataset Comparisons'
$md += ''
$md += '| Dataset | Control p50 (ms) | Candidate p50 (ms) | p50 gain % | Control p95 (ms) | Candidate p95 (ms) | p95 gain % | Candidate Peak RSS (MB) | .NET Readable |'
$md += '|---|---:|---:|---:|---:|---:|---:|---:|---|'
foreach ($row in $comparisons) {
    $md += "| $($row.dataset) | $($row.control_p50_ms) | $($row.candidate_p50_ms) | $($row.p50_gain_pct) | $($row.control_p95_ms) | $($row.candidate_p95_ms) | $($row.p95_gain_pct) | $($row.candidate_peak_rss_mb) | $($row.candidate_all_dotnet_readable) |"
}

$md -join "`r`n" | Set-Content -Path $mdPath -Encoding UTF8

Write-Host ''
Write-Host 'Framework run complete' -ForegroundColor Green
Write-Host "Summary JSON: $jsonPath" -ForegroundColor Green
Write-Host "Summary MD:   $mdPath" -ForegroundColor Green
Write-Host ''
Write-Host "Recommendation: $recommendation (p50=$overallP50Gain%, p95=$overallP95Gain%)" -ForegroundColor Cyan
