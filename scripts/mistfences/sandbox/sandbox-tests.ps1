<#
.SYNOPSIS
  Recovery tests of the MistFences full mode, run INSIDE Windows Sandbox (see
  run-sandbox-tests.ps1 on the host). Never run this on a real desktop: it hides and shows
  the desktop icons, kills MistFences and restarts Explorer on purpose.

.DESCRIPTION
  C:\MF\in (read only): app\ with the release build, this script.
  C:\MF\out (read/write): progress.log, results.json, logs\, done.txt.
#>
$ErrorActionPreference = 'Continue'
$in = 'C:\MF\in'; $out = 'C:\MF\out'; $app = 'C:\MistFences'
$progress = Join-Path $out 'progress.log'
$results = [System.Collections.Generic.List[object]]::new()
$dataDir = Join-Path $env:LOCALAPPDATA 'MistFences'
$marker = Join-Path $dataDir 'icons-hidden.marker'

if (Test-Path -LiteralPath (Join-Path $out 'done.txt')) { exit } # a re-run of the logon command
function Log([string] $text) { Add-Content -LiteralPath $progress -Value ('{0:HH:mm:ss.fff} {1}' -f (Get-Date), $text) -Encoding UTF8 }
function Add-Result([string] $test, [bool] $ok, [string] $detail) {
    $results.Add([pscustomobject]@{ test = $test; passed = $ok; detail = $detail })
    Log ('{0} {1}: {2}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $test, $detail)
}

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Desk {
    [StructLayout(LayoutKind.Sequential)]
    struct ShellState { public uint Flags1; public uint W95a; public uint W95b; public int Sort; public int Dir; public uint Version; public uint NotUsed; public uint Flags2; }
    [DllImport("shell32.dll")] static extern void SHGetSetSettings(ref ShellState s, uint mask, bool set);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string cls, string title);
    [DllImport("user32.dll")] static extern IntPtr SendMessageTimeout(IntPtr h, uint msg, IntPtr w, IntPtr l, uint flags, uint timeout, out IntPtr result);
    public static bool Hidden() { var s = new ShellState(); SHGetSetSettings(ref s, 0x4000, false); return (s.Flags1 & (1u << 12)) != 0; }
    public static IntPtr DefView() {
        IntPtr progman = FindWindowEx(IntPtr.Zero, IntPtr.Zero, "Progman", null);
        IntPtr view = FindWindowEx(progman, IntPtr.Zero, "SHELLDLL_DefView", null);
        IntPtr w = IntPtr.Zero;
        while (view == IntPtr.Zero && (w = FindWindowEx(IntPtr.Zero, w, "WorkerW", null)) != IntPtr.Zero)
            view = FindWindowEx(w, IntPtr.Zero, "SHELLDLL_DefView", null);
        return view;
    }
    // The same command as the desktop's View > Show desktop icons.
    public static bool Toggle() { IntPtr r; IntPtr v = DefView(); return v != IntPtr.Zero && SendMessageTimeout(v, 0x111, (IntPtr)0x7402, IntPtr.Zero, 2, 2000, out r) != IntPtr.Zero; }
}
'@

function Wait-Until([scriptblock] $condition, [int] $seconds) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.Elapsed.TotalSeconds -lt $seconds) {
        if (& $condition) { return [int]$sw.Elapsed.TotalMilliseconds }
        Start-Sleep -Milliseconds 200
    }
    return -1
}
function Mf-Running { @(Get-Process mistfences -ErrorAction SilentlyContinue).Count -gt 0 }
function MfCli([string[]] $a) { (& (Join-Path $app 'mistfences-cli.exe') @a --compact 2>&1 | ForEach-Object { "$_" }) -join "`n" }
function Status { try { MfCli @('status') | ConvertFrom-Json } catch { $null } }
function Start-Mf { Start-Process -FilePath (Join-Path $app 'mistfences.exe') -ArgumentList '--portable', '--exit-after', '900000' -PassThru }
function Wait-Ready { Wait-Until { $s = Status; $s -and $s.pid } 45 }
function Quit-Mf { MfCli @('quit') | Out-Null; Wait-Until { -not (Mf-Running) } 20 }
function Kill-Mf { Get-Process mistfences -ErrorAction SilentlyContinue | Stop-Process -Force }
function Run-Entry { (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -ErrorAction SilentlyContinue).PSObject.Properties | Where-Object { $_.Name -match 'MistFences|PecoFence' } }

New-Item -ItemType Directory -Force $out | Out-Null
Log 'sandbox tests start'
# Explorer's desktop must be up before anything else.
$t = Wait-Until { [Desk]::DefView() -ne [IntPtr]::Zero } 120
Log "desktop view ready after $t ms"
Start-Sleep -Seconds 5
Copy-Item -LiteralPath (Join-Path $in 'app') -Destination $app -Recurse -Force
New-Item -ItemType Directory -Force (Join-Path $app 'config') | Out-Null

try {
    # T0 baseline
    $check = Start-Process (Join-Path $app 'mistfences.exe') -ArgumentList '--check-fences' -Wait -PassThru
    Add-Result 'baseline' ((-not [Desk]::Hidden()) -and $check.ExitCode -eq 0 -and -not (Run-Entry)) "icons visible=$(-not [Desk]::Hidden()), check-fences exit $($check.ExitCode), no startup entry"

    # T1 start hides the icons, writes the marker, starts the watchdog
    $p = Start-Mf
    $ready = Wait-Ready
    $hid = Wait-Until { [Desk]::Hidden() } 30
    $wd = @(Get-Process mistfences-watchdog -ErrorAction SilentlyContinue).Count
    $s = Status
    Add-Result 'full mode hides the Windows icons' ($ready -ge 0 -and $hid -ge 0 -and (Test-Path $marker) -and $wd -ge 1) "ready $ready ms, hidden after $hid ms, marker=$(Test-Path $marker), watchdogs=$wd, fences=$($s.fenceCount), items=$($s.itemCount)"

    # T2 single instance
    $second = Start-Process -FilePath (Join-Path $app 'mistfences.exe') -ArgumentList '--portable', '--exit-after', '900000' -PassThru
    $gone = Wait-Until { $second.HasExited } 15
    Add-Result 'second start exits' ($gone -ge 0 -and @(Get-Process mistfences).Count -eq 1) "second instance exited after $gone ms; running: $(@(Get-Process mistfences).Count)"

    # T3 a normal quit shows them again
    $q = Quit-Mf
    $shown = Wait-Until { -not [Desk]::Hidden() } 15
    $wdGone = Wait-Until { @(Get-Process mistfences-watchdog -ErrorAction SilentlyContinue).Count -eq 0 } 15
    Add-Result 'quit restores the icons' ($q -ge 0 -and $shown -ge 0 -and -not (Test-Path $marker)) "exited after $q ms, icons back after $shown ms, marker gone=$(-not (Test-Path $marker)), watchdog gone after $wdGone ms"

    # T4 a crash: the watchdog shows them
    $p = Start-Mf; $null = Wait-Ready; $null = Wait-Until { [Desk]::Hidden() } 30
    Kill-Mf
    $shown = Wait-Until { -not [Desk]::Hidden() } 30
    $mGone = Wait-Until { -not (Test-Path $marker) } 10
    Add-Result 'crash: watchdog restores the icons' ($shown -ge 0 -and $mGone -ge 0) "icons back $shown ms after the kill, marker gone=$($mGone -ge 0)"

    # T5 Explorer restarts while MistFences runs
    $p = Start-Mf; $null = Wait-Ready; $null = Wait-Until { [Desk]::Hidden() } 30
    $before = Status
    Get-Process explorer -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 3
    $back = Wait-Until { @(Get-Process explorer -ErrorAction SilentlyContinue).Count -gt 0 -and [Desk]::DefView() -ne [IntPtr]::Zero } 30
    if ($back -lt 0) { Start-Process explorer.exe; $back = Wait-Until { [Desk]::DefView() -ne [IntPtr]::Zero } 60 }
    Start-Sleep -Seconds 10
    $after = Status
    $stillHidden = [Desk]::Hidden()
    Add-Result 'Explorer restart' ($back -ge 0 -and $after -and $after.pid -eq $before.pid -and $after.fenceCount -eq $before.fenceCount -and $stillHidden) "explorer back after $back ms; MistFences same pid=$($after.pid -eq $before.pid), fences $($before.fenceCount)->$($after.fenceCount), icons still hidden=$stillHidden"
    $q = Quit-Mf
    $shown = Wait-Until { -not [Desk]::Hidden() } 15
    Add-Result 'quit after Explorer restart restores' ($shown -ge 0 -and -not (Test-Path $marker)) "icons back after $shown ms"

    # T6 icons the user hid stay hidden
    $null = [Desk]::Toggle(); $userHid = Wait-Until { [Desk]::Hidden() } 10
    $p = Start-Mf; $null = Wait-Ready; Start-Sleep -Seconds 3
    $markerWhileRunning = Test-Path $marker
    $null = Quit-Mf; Start-Sleep -Seconds 2
    $kept = [Desk]::Hidden()
    Add-Result 'icons hidden by the user stay hidden' ($userHid -ge 0 -and -not $markerWhileRunning -and $kept) "hidden by the user=$($userHid -ge 0), no marker while running=$(-not $markerWhileRunning), still hidden after quit=$kept"
    $null = [Desk]::Toggle(); $null = Wait-Until { -not [Desk]::Hidden() } 10

    # T7 crash loop: three crashes, then a safe start, then a clean exit ends it
    $restored = 0
    foreach ($i in 1..3) {
        $p = Start-Mf; $null = Wait-Ready; $null = Wait-Until { [Desk]::Hidden() } 30
        Kill-Mf
        if ((Wait-Until { -not [Desk]::Hidden() } 30) -ge 0) { $restored++ }
        Start-Sleep -Seconds 1
    }
    $p = Start-Mf; $ready = Wait-Ready; Start-Sleep -Seconds 8
    $safeVisible = -not [Desk]::Hidden()
    $journal = Get-Content -LiteralPath (Join-Path $dataDir 'runs.json') -Raw -ErrorAction SilentlyContinue
    $null = Quit-Mf
    Add-Result 'crash loop: safe start keeps the icons' ($restored -eq 3 -and $ready -ge 0 -and $safeVisible -and $journal -match '"safe": true') "3 crashes restored by the watchdog: $restored; 4th start ready=$($ready -ge 0), icons visible=$safeVisible"
    $p = Start-Mf; $null = Wait-Ready
    $hidAgain = Wait-Until { [Desk]::Hidden() } 30
    $null = Quit-Mf; $null = Wait-Until { -not [Desk]::Hidden() } 15
    Add-Result 'after a clean exit the next start is normal' ($hidAgain -ge 0) "hidden again after $hidAgain ms"

    # T8 the emergency restore
    $p = Start-Mf; $null = Wait-Ready; $null = Wait-Until { [Desk]::Hidden() } 30
    $r = Start-Process -FilePath (Join-Path $app 'mistfences-watchdog.exe') -ArgumentList '--restore', '--no-autostart', '--quiet' -Wait -PassThru
    $shown = Wait-Until { -not [Desk]::Hidden() } 10
    Add-Result 'emergency restore' ($r.ExitCode -eq 0 -and -not (Mf-Running) -and $shown -ge 0 -and -not (Test-Path $marker)) "exit $($r.ExitCode), MistFences stopped=$(-not (Mf-Running)), icons visible=$($shown -ge 0), marker gone=$(-not (Test-Path $marker))"

    # T9 nothing registered at logon
    Add-Result 'no startup entry written' (-not (Run-Entry)) 'HKCU Run has no MistFences entry'
} catch {
    Add-Result 'script error' $false $_.Exception.Message
} finally {
    Kill-Mf
    if ([Desk]::Hidden()) { $null = [Desk]::Toggle() }
    New-Item -ItemType Directory -Force (Join-Path $out 'logs') | Out-Null
    Get-ChildItem -LiteralPath $dataDir -File -ErrorAction SilentlyContinue | Copy-Item -Destination (Join-Path $out 'logs') -Force
    $results | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $out 'results.json') -Encoding UTF8
    Log 'sandbox tests end'
    Set-Content -LiteralPath (Join-Path $out 'done.txt') -Value 'done' -Encoding ASCII
    shutdown.exe /s /t 10
}
