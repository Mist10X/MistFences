<#
.SYNOPSIS
  Measures a running MistFences preview: memory, idle CPU, CPU per roll/unroll, startup time.

.DESCRIPTION
  Read-only for Windows: it only reads process counters and drives the preview over its own
  pipe (fence roll / unroll). Results go to .preview\measurements\<time>.json and the console.

  -StartupRuns N   also starts N short-lived previews (--exit-after) one after another and
                   times process start -> IPC ready. Close a running preview first.

  Numbers are for this PC and this moment (other programs, Wallpaper Engine and the GPU
  driver all influence them); compare runs made under the same conditions.

.EXAMPLE
  ./scripts/mistfences/measure-preview.ps1 -IdleSeconds 60 -Cycles 20
  ./scripts/mistfences/measure-preview.ps1 -StartupRuns 5
#>
param(
    [int] $IdleSeconds = 60,
    [int] $Cycles = 20,
    [int] $StartupRuns = 0,
    # Title of the fence to roll and unroll; the first virtual fence when empty.
    [string] $Fence = '',
    # Measure target\release instead of target\debug.
    [switch] $Release
)

$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$bin = if ($Release) { 'target\release' } else { 'target\debug' }
$exe = Join-Path $repo "$bin\mistfences.exe"
$cli = Join-Path $repo "$bin\mistfences-cli.exe"
$root = Join-Path $repo '.preview'
$outDir = Join-Path $root 'measurements'
New-Item -ItemType Directory -Force $outDir | Out-Null
$result = [ordered]@{ taken = (Get-Date).ToString('o'); build = $(if ($Release) { 'release (opt-level 3, LTO)' } else { 'debug (opt-level 1)' }); machine = $env:COMPUTERNAME }

function Wait-Ready([int] $timeoutMs) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt $timeoutMs) {
        & $cli --instance preview status *> $null
        if ($LASTEXITCODE -eq 0) { return $true }
        Start-Sleep -Milliseconds 20
    }
    return $false
}

function Get-Proc { Get-Process mistfences -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$repo*" } | Select-Object -First 1 }

if ($StartupRuns -gt 0) {
    if (Get-Proc) { throw 'Close the running preview first (stop-preview.ps1) to measure startup.' }
    $times = @()
    for ($i = 0; $i -lt $StartupRuns; $i++) {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $p = Start-Process -FilePath $exe -ArgumentList @('--mistfences-preview', '--preview-root', "`"$root`"", '--exit-after', '6000') -PassThru
        if (Wait-Ready 15000) { $times += $sw.ElapsedMilliseconds }
        $p.WaitForExit(20000) | Out-Null
        Start-Sleep -Milliseconds 500
    }
    $sorted = $times | Sort-Object
    $result.startup = [ordered]@{
        runs = $times
        medianMsToIpcReady = $sorted[[int][Math]::Floor($sorted.Count / 2)]
        note = 'process start -> named pipe answers status (fences created, sample items synced); includes the 20 ms polling step'
    }
}

$proc = Get-Proc
if ($proc) {
    $cores = [Environment]::ProcessorCount
    $proc.Refresh()
    $result.memory = [ordered]@{
        privateMB = [math]::Round($proc.PrivateMemorySize64 / 1MB, 1)
        workingSetMB = [math]::Round($proc.WorkingSet64 / 1MB, 1)
        peakWorkingSetMB = [math]::Round($proc.PeakWorkingSet64 / 1MB, 1)
        handles = $proc.HandleCount
        threads = $proc.Threads.Count
    }

    # Idle: no interaction for $IdleSeconds.
    $cpu0 = $proc.TotalProcessorTime; $t0 = Get-Date
    Start-Sleep -Seconds $IdleSeconds
    $proc.Refresh(); $cpu1 = $proc.TotalProcessorTime; $t1 = Get-Date
    $idleCpuMs = ($cpu1 - $cpu0).TotalMilliseconds
    $result.idle = [ordered]@{
        seconds = $IdleSeconds
        cpuMs = [math]::Round($idleCpuMs, 1)
        cpuPercentOfOneCore = [math]::Round($idleCpuMs / ($t1 - $t0).TotalMilliseconds * 100, 3)
        cpuPercentOfAllCores = [math]::Round($idleCpuMs / ($t1 - $t0).TotalMilliseconds * 100 / $cores, 4)
    }

    # Roll / unroll through the pipe: each cycle = unroll, 600 ms, roll, 600 ms.
    $list = (& $cli --instance preview fence list) | ConvertFrom-Json
    $target = $list | Where-Object { $_.kind -eq 'virtual' -and ($Fence -eq '' -or $_.title -eq $Fence) } | Select-Object -First 1
    if (-not $target) { throw "fence '$Fence' not found in the preview" }
    $fenceId = $target.id
    $check = (& $cli --instance preview fence unroll $fenceId) | ConvertFrom-Json
    if (-not $check.fence) { throw "roll test failed: $check" }
    & $cli --instance preview fence roll $fenceId | Out-Null
    Start-Sleep -Milliseconds 800
    $dwm = Get-Process dwm | Select-Object -First 1
    $proc.Refresh(); $c0 = $proc.TotalProcessorTime; $d0 = $dwm.TotalProcessorTime; $w0 = Get-Date
    for ($i = 0; $i -lt $Cycles; $i++) {
        & $cli --instance preview fence unroll $fenceId | Out-Null
        Start-Sleep -Milliseconds 600
        & $cli --instance preview fence roll $fenceId | Out-Null
        Start-Sleep -Milliseconds 600
    }
    $proc.Refresh(); $dwm.Refresh(); $c1 = $proc.TotalProcessorTime; $d1 = $dwm.TotalProcessorTime; $w1 = Get-Date

    # Control: the same number of pipe requests (status) at the same pace, no animation.
    $proc.Refresh(); $k0 = $proc.TotalProcessorTime; $dk0 = $dwm.TotalProcessorTime
    for ($i = 0; $i -lt $Cycles; $i++) {
        & $cli --instance preview status | Out-Null
        Start-Sleep -Milliseconds 600
        & $cli --instance preview status | Out-Null
        Start-Sleep -Milliseconds 600
    }
    $proc.Refresh(); $dwm.Refresh(); $k1 = $proc.TotalProcessorTime; $dk1 = $dwm.TotalProcessorTime

    $rollMs = ($c1 - $c0).TotalMilliseconds
    $controlMs = ($k1 - $k0).TotalMilliseconds
    $result.rollCycles = [ordered]@{
        cycles = $Cycles
        fence = $target.title
        appCpuMsTotal = [math]::Round($rollMs, 1)
        appCpuMsControlStatusRequests = [math]::Round($controlMs, 1)
        appCpuMsPerTransition = [math]::Round(($rollMs - $controlMs) / ($Cycles * 2), 2)
        dwmCpuMsDuringRolls = [math]::Round(($d1 - $d0).TotalMilliseconds, 1)
        dwmCpuMsDuringControl = [math]::Round(($dk1 - $dk0).TotalMilliseconds, 1)
        note = 'per transition = (roll run - control run) / transitions; the control run sends the same number of pipe requests (status) at the same pace; dwm.exe CPU covers the whole desktop'
    }
    $proc.Refresh()
    $result.memoryAfter = [ordered]@{
        privateMB = [math]::Round($proc.PrivateMemorySize64 / 1MB, 1)
        workingSetMB = [math]::Round($proc.WorkingSet64 / 1MB, 1)
    }
} else {
    Write-Host 'No running preview: start it with run-preview.ps1 to measure memory and CPU.'
}

$file = Join-Path $outDir ('{0:yyyyMMdd-HHmmss}.json' -f (Get-Date))
$result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $file -Encoding utf8
$result | ConvertTo-Json -Depth 5
Write-Host "saved $file"
