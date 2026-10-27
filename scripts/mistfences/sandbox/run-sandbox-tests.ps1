<#
.SYNOPSIS
  Runs the MistFences recovery tests in Windows Sandbox and prints the results.

.DESCRIPTION
  Copies the release build and sandbox-tests.ps1 to .sandbox\in (mapped read only), starts a
  sandbox with no network and no clipboard, and waits for .sandbox\out\done.txt. Inside, the
  tests hide and show the desktop icons, kill MistFences and restart Explorer; the sandbox
  shuts itself down when they are done and everything in it is thrown away. The real desktop,
  Fences and this PC's settings are not touched.

.EXAMPLE
  ./scripts/mistfences/sandbox/run-sandbox-tests.ps1
#>
param(
    [string] $AppDir = 'D:\MistFences',
    [int] $TimeoutMinutes = 20,
    # The script to run inside: sandbox-tests.ps1, or sandbox-diag.ps1 for diagnostics.
    [string] $Script = 'sandbox-tests.ps1'
)
$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..\..')
$root = Join-Path $repo '.sandbox'
$in = Join-Path $root 'in'; $out = Join-Path $root 'out'
if (-not (Test-Path "$env:windir\System32\WindowsSandbox.exe")) { throw 'Windows Sandbox is not installed' }
# A sandbox from the previous run may still be shutting itself down.
$wait = [Diagnostics.Stopwatch]::StartNew()
while ((Get-Process WindowsSandboxClient, WindowsSandboxRemoteSession -ErrorAction SilentlyContinue) -and $wait.Elapsed.TotalSeconds -lt 90) { Start-Sleep -Seconds 2 }
if (Get-Process WindowsSandboxClient, WindowsSandboxRemoteSession -ErrorAction SilentlyContinue) { throw 'a Windows Sandbox is already running: close it first' }

foreach ($d in $in, $out) { if (Test-Path $d) { Remove-Item -LiteralPath $d -Recurse -Force } }
New-Item -ItemType Directory -Force (Join-Path $in 'app'), $out | Out-Null
foreach ($f in 'mistfences.exe', 'mistfences-watchdog.exe', 'mistfences-cli.exe', 'WebView2Loader.dll') {
    Copy-Item -LiteralPath (Join-Path $AppDir $f) -Destination (Join-Path $in 'app')
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot $Script) -Destination (Join-Path $in 'sandbox-tests.ps1')

$wsb = Join-Path $root 'MistFences-tests.wsb'
@"
<Configuration>
  <Networking>Disable</Networking>
  <ClipboardRedirection>Disable</ClipboardRedirection>
  <PrinterRedirection>Disable</PrinterRedirection>
  <AudioInput>Disable</AudioInput>
  <VideoInput>Disable</VideoInput>
  <MappedFolders>
    <MappedFolder><HostFolder>$in</HostFolder><SandboxFolder>C:\MF\in</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
    <MappedFolder><HostFolder>$out</HostFolder><SandboxFolder>C:\MF\out</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
  </MappedFolders>
  <LogonCommand><Command>powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\MF\in\sandbox-tests.ps1</Command></LogonCommand>
</Configuration>
"@ | Set-Content -LiteralPath $wsb -Encoding UTF8

# The sandbox window opens in front and takes the focus. It goes to the back of the z-order
# instead, and the window that had the focus gets it back. It is not minimized: a minimized
# sandbox stops drawing its desktop, and the tests inside capture it and move its mouse.
Add-Type -Namespace MfSandbox -Name Win -MemberDefinition @'
[DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr h);
[DllImport("user32.dll")] public static extern bool SetWindowPos(System.IntPtr h, System.IntPtr after, int x, int y, int cx, int cy, uint flags);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);
public delegate bool EnumProc(System.IntPtr h, System.IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, System.IntPtr l);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
'@
$userWindow = [MfSandbox.Win]::GetForegroundWindow()
$pushedBack = @{}
function Send-SandboxBack {
    $pids = @(Get-Process WindowsSandbox* -ErrorAction SilentlyContinue | ForEach-Object Id)
    if (-not $pids) { return }
    $fg = [MfSandbox.Win]::GetForegroundWindow()
    $fgPid = [uint32]0; [void][MfSandbox.Win]::GetWindowThreadProcessId($fg, [ref]$fgPid)
    $tookFocus = $pids -contains [int]$fgPid
    [MfSandbox.Win]::EnumWindows({
        param($h, $l)
        $p = [uint32]0; [void][MfSandbox.Win]::GetWindowThreadProcessId($h, [ref]$p)
        if ($pids -contains [int]$p -and [MfSandbox.Win]::IsWindowVisible($h)) {
            # HWND_BOTTOM; SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE
            if ($tookFocus -or -not $pushedBack.ContainsKey($h)) {
                [void][MfSandbox.Win]::SetWindowPos($h, [IntPtr]1, 0, 0, 0, 0, 0x13)
                $pushedBack[$h] = $true
            }
        }
        $true
    }, [IntPtr]::Zero) | Out-Null
    if ($tookFocus -and $userWindow -ne [IntPtr]::Zero) { [void][MfSandbox.Win]::SetForegroundWindow($userWindow) }
}

Write-Host "Starting Windows Sandbox ($wsb) in the background"
Start-Process -FilePath "$env:windir\System32\WindowsSandbox.exe" -ArgumentList "`"$wsb`""
$deadline = (Get-Date).AddMinutes($TimeoutMinutes)
while (-not (Test-Path (Join-Path $out 'done.txt')) -and (Get-Date) -lt $deadline) {
    Send-SandboxBack
    Start-Sleep -Milliseconds 500
}
if (-not (Test-Path (Join-Path $out 'done.txt'))) {
    Write-Host "No result after $TimeoutMinutes minutes. Progress so far:" -ForegroundColor Red
    if (Test-Path (Join-Path $out 'progress.log')) { Get-Content (Join-Path $out 'progress.log') -Encoding UTF8 }
    exit 1
}
$results = Get-Content (Join-Path $out 'results.json') -Raw -Encoding UTF8 | ConvertFrom-Json
foreach ($r in $results) { Write-Host ('{0}  {1}  {2}' -f $(if ($r.passed) { 'PASS' } else { 'FAIL' }), $r.test, $r.detail) }
$failed = @($results | Where-Object { -not $_.passed }).Count
if ($failed) { Write-Host "RESULT: $failed test(s) failed" -ForegroundColor Red; exit 1 }
Write-Host "RESULT: all $(@($results).Count) tests passed" -ForegroundColor Green
