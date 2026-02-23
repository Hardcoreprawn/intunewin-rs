param(
    [string]$TestDataPath = '.\testdata',
    [string]$ResultsRoot = '.\testdata\benchmarks\results',
    [string]$DatasetManifestPath = '.\testdata\benchmarks\datasets.real.json',
    [string]$ControlLabel = 'control',
    [string]$CandidateLabel = 'candidate',
    [string]$ControlCommandTemplate = '.\target\release\intunewin-rs.exe -c "{CONTENT}" -s "{SETUP}" -o "{OUTPUT}" -q',
    [string]$CandidateCommandTemplate = '.\target\release\intunewin-rs.exe -c "{CONTENT}" -s "{SETUP}" -o "{OUTPUT}" -q',
    [int]$WarmupRuns = 1,
    [int]$Iterations = 7,
    [string]$DatasetProfile = 'real',
    [ValidateSet('interleaved', 'sequential')]
    [string]$RunOrder = 'interleaved',
    [ValidateSet('preserve', 'clear-each-iteration')]
    [string]$CacheControl = 'preserve',
    [int]$CooldownMs = 150,
    [switch]$ShuffleDatasets,
    [switch]$IncludeLarge,
    [switch]$AllowSynthetic,
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

function Get-EnvironmentSnapshot {
    $cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
    $os = Get-CimInstance Win32_OperatingSystem
    $totalRamGiB = [math]::Round(([double]$os.TotalVisibleMemorySize * 1KB) / 1GB, 2)

    return [PSCustomObject]@{
        host_name = $env:COMPUTERNAME
        os = $os.Caption
        os_version = $os.Version
        cpu_model = $cpu.Name
        logical_cores = [int]$cpu.NumberOfLogicalProcessors
        physical_cores = [int]$cpu.NumberOfCores
        total_ram_gib = $totalRamGiB
        powershell = $PSVersionTable.PSVersion.ToString()
    }
}

function Get-DatasetsFromManifest {
    param(
        [string]$ManifestPath,
        [string]$Profile,
        [switch]$IncludeLargeDatasets,
        [switch]$SyntheticAllowed
    )

    if (-not (Test-Path $ManifestPath)) {
        throw "Dataset manifest not found: $ManifestPath"
    }

    $manifest = Get-Content $ManifestPath -Raw | ConvertFrom-Json
    if ($null -eq $manifest.datasets) {
        throw "Invalid dataset manifest: missing datasets array"
    }

    $selected = @()
    foreach ($entry in $manifest.datasets) {
        $profiles = @($entry.profiles)
        if ($profiles -notcontains $Profile) {
            continue
        }

        if (-not $IncludeLargeDatasets -and [string]$entry.size_profile -eq 'large') {
            continue
        }

        if (-not $SyntheticAllowed -and [string]$entry.source -ne 'real') {
            continue
        }

        $selected += @{
            Name = [string]$entry.name
            Path = [string]$entry.path
            Setup = [string]$entry.setup
            Source = [string]$entry.source
            SizeProfile = [string]$entry.size_profile
            Notes = [string]$entry.notes
        }
    }

    return $selected
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

function Invoke-VariantSample {
    param(
        [string]$Variant,
        [string]$CommandTemplate,
        [hashtable]$Dataset,
        [string]$RunRoot,
        [string]$Phase,
        [int]$Iteration,
        [ValidateSet('preserve', 'clear-each-iteration')]
        [string]$CachePolicy,
        [switch]$StrictMode
    )

    $variantRoot = Join-Path $RunRoot "$($Dataset.Name)_$Variant"
    if (-not (Test-Path $variantRoot)) {
        New-Item -ItemType Directory -Path $variantRoot -Force | Out-Null
    }

    if ($CachePolicy -eq 'clear-each-iteration') {
        $cacheDir = Join-Path $variantRoot '.intunewin-cache'
        if (Test-Path $cacheDir) {
            Remove-Item $cacheDir -Recurse -Force
        }
    }

    $setupStem = [System.IO.Path]::GetFileNameWithoutExtension($Dataset.Setup)
    $packagePath = Join-Path $variantRoot ("$setupStem.intunewin")
    $cmd = $CommandTemplate.Replace('{CONTENT}', $Dataset.Path).Replace('{SETUP}', $Dataset.Setup).Replace('{OUTPUT}', $variantRoot)

    $result = Invoke-BenchmarkCommand -Command $cmd -WorkingDirectory (Get-Location)
    $packageExists = Test-Path $packagePath
    $packageSizeMB = if ($packageExists) { [math]::Round((Get-Item $packagePath).Length / 1MB, 3) } else { $null }
    $dotnetReadable = if ($packageExists) { Test-DetectionXmlReadable -PackagePath $packagePath } else { $false }

    if ($StrictMode -and ($result.ExitCode -ne 0 -or -not $dotnetReadable)) {
        throw "Strict mode failure for $Variant/$($Dataset.Name): exit=$($result.ExitCode), dotnetReadable=$dotnetReadable"
    }

    return [PSCustomObject]@{
        phase = $Phase
        iteration = $Iteration
        exit_code = $result.ExitCode
        duration_ms = $result.DurationMs
        peak_working_set_mb = $result.PeakWorkingSetMB
        cpu_time_ms = $result.CpuTimeMs
        package_exists = $packageExists
        package_size_mb = $packageSizeMB
        dotnet_detection_xml_readable = $dotnetReadable
    }
}

function New-VariantResult {
    param(
        [string]$Variant,
        [hashtable]$Dataset,
        [object[]]$Samples
    )

    $durations = @($Samples | ForEach-Object { [double]$_.duration_ms })
    $rss = @($Samples | ForEach-Object { [double]$_.peak_working_set_mb })

    return [PSCustomObject]@{
        variant = $Variant
        dataset = $Dataset.Name
        setup = $Dataset.Setup
        content_path = $Dataset.Path
        samples = $Samples
        summary = [PSCustomObject]@{
            p50_duration_ms = [math]::Round((Get-Percentile -Values $durations -Percentile 50), 2)
            p95_duration_ms = [math]::Round((Get-Percentile -Values $durations -Percentile 95), 2)
            avg_duration_ms = [math]::Round((($durations | Measure-Object -Average).Average), 2)
            peak_rss_mb = [math]::Round((($rss | Measure-Object -Maximum).Maximum), 2)
            any_failure = ($Samples | Where-Object { $_.exit_code -ne 0 }).Count -gt 0
            all_dotnet_readable = ($Samples | Where-Object { -not $_.dotnet_detection_xml_readable }).Count -eq 0
        }
    }
}

Write-Host ''
Write-Host '==============================================================' -ForegroundColor Cyan
Write-Host 'IntuneWin Experiment Framework (Issue #75 baseline)' -ForegroundColor Cyan
Write-Host '==============================================================' -ForegroundColor Cyan
Write-Host ''

$datasets = Get-DatasetsFromManifest -ManifestPath $DatasetManifestPath -Profile $DatasetProfile -IncludeLargeDatasets:$IncludeLarge -SyntheticAllowed:$AllowSynthetic

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

if ($ShuffleDatasets) {
    $available = @($available | Sort-Object { Get-Random })
}

$timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$runRoot = Join-Path $ResultsRoot $timestamp
New-Item -ItemType Directory -Path $runRoot -Force | Out-Null

Write-Host "Results root: $runRoot" -ForegroundColor Cyan

$environment = Get-EnvironmentSnapshot
Write-Host "Host: $($environment.host_name), CPU: $($environment.cpu_model), Cores: $($environment.physical_cores)c/$($environment.logical_cores)t, RAM: $($environment.total_ram_gib) GiB" -ForegroundColor Cyan
Write-Host "Dataset profile: $DatasetProfile | Run order: $RunOrder | Cache control: $CacheControl | Synthetic allowed: $($AllowSynthetic.IsPresent)" -ForegroundColor Cyan

$allResults = @()
foreach ($dataset in $available) {
    Write-Host "Running dataset: $($dataset.Name)" -ForegroundColor White

    $controlSamples = @()
    $candidateSamples = @()

    for ($w = 1; $w -le $WarmupRuns; $w++) {
        Write-Host "  [$ControlLabel/$($dataset.Name)] Warmup $w/$WarmupRuns" -ForegroundColor DarkGray
        $controlWarm = Invoke-VariantSample -Variant $ControlLabel -CommandTemplate $ControlCommandTemplate -Dataset $dataset -RunRoot $runRoot -Phase 'warmup' -Iteration $w -CachePolicy $CacheControl -StrictMode:$Strict
        if ($controlWarm.exit_code -ne 0) {
            throw "Warmup failed for $ControlLabel/$($dataset.Name) with exit code $($controlWarm.exit_code)"
        }

        Write-Host "  [$CandidateLabel/$($dataset.Name)] Warmup $w/$WarmupRuns" -ForegroundColor DarkGray
        $candidateWarm = Invoke-VariantSample -Variant $CandidateLabel -CommandTemplate $CandidateCommandTemplate -Dataset $dataset -RunRoot $runRoot -Phase 'warmup' -Iteration $w -CachePolicy $CacheControl -StrictMode:$Strict
        if ($candidateWarm.exit_code -ne 0) {
            throw "Warmup failed for $CandidateLabel/$($dataset.Name) with exit code $($candidateWarm.exit_code)"
        }
    }

    for ($i = 1; $i -le $Iterations; $i++) {
        $order = @($ControlLabel, $CandidateLabel)
        if ($RunOrder -eq 'interleaved' -and ($i % 2 -eq 0)) {
            $order = @($CandidateLabel, $ControlLabel)
        }

        foreach ($variant in $order) {
            if ($variant -eq $ControlLabel) {
                Write-Host "  [$ControlLabel/$($dataset.Name)] Iteration $i/$Iterations" -ForegroundColor Gray
                $sample = Invoke-VariantSample -Variant $ControlLabel -CommandTemplate $ControlCommandTemplate -Dataset $dataset -RunRoot $runRoot -Phase 'sample' -Iteration $i -CachePolicy $CacheControl -StrictMode:$Strict
                $controlSamples += $sample
            }
            else {
                Write-Host "  [$CandidateLabel/$($dataset.Name)] Iteration $i/$Iterations" -ForegroundColor Gray
                $sample = Invoke-VariantSample -Variant $CandidateLabel -CommandTemplate $CandidateCommandTemplate -Dataset $dataset -RunRoot $runRoot -Phase 'sample' -Iteration $i -CachePolicy $CacheControl -StrictMode:$Strict
                $candidateSamples += $sample
            }

            if ($CooldownMs -gt 0) {
                Start-Sleep -Milliseconds $CooldownMs
            }
        }
    }

    $allResults += New-VariantResult -Variant $ControlLabel -Dataset $dataset -Samples $controlSamples
    $allResults += New-VariantResult -Variant $CandidateLabel -Dataset $dataset -Samples $candidateSamples
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
    environment = $environment
    dataset_manifest_path = $DatasetManifestPath
    control_label = $ControlLabel
    candidate_label = $CandidateLabel
    iterations = $Iterations
    warmups = $WarmupRuns
    dataset_profile = $DatasetProfile
    run_order = $RunOrder
    cache_control = $CacheControl
    cooldown_ms = $CooldownMs
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
