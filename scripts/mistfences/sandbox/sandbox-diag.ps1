# Diagnostics inside Windows Sandbox: can mistfences-cli reach a running MistFences?
$ErrorActionPreference = 'Continue'
$in = 'C:\MF\in'; $out = 'C:\MF\out'; $app = 'C:\MistFences'
$log = Join-Path $out 'progress.log'
if (Test-Path -LiteralPath (Join-Path $out 'done.txt')) { exit }
function Log([string] $t) { Add-Content -LiteralPath $log -Value ('{0:HH:mm:ss.fff} {1}' -f (Get-Date), $t) -Encoding UTF8 }
New-Item -ItemType Directory -Force $out | Out-Null
Start-Sleep -Seconds 8
Copy-Item -LiteralPath (Join-Path $in 'app') -Destination $app -Recurse -Force
New-Item -ItemType Directory -Force (Join-Path $app 'config') | Out-Null
$cli = Join-Path $app 'mistfences-cli.exe'
Log ("whoami: " + (whoami) + " / elevated: " + ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator))
$o = & $cli paths --compact 2>&1 | ForEach-Object { "$_" }; Log "cli paths exit $LASTEXITCODE : $($o -join ' | ')"
$p = Start-Process -FilePath (Join-Path $app 'mistfences.exe') -ArgumentList '--portable', '--exit-after', '120000' -PassThru
Start-Sleep -Seconds 15
Log "app running: $(-not $p.HasExited)"
foreach ($i in 1..3) {
    $o = & $cli status --compact 2>&1 | ForEach-Object { "$_" }; Log "cli status (direct) exit $LASTEXITCODE : $($o -join ' | ')"
    $o = cmd /c "`"$cli`" status --compact 2>&1"; Log "cli status (cmd) exit $LASTEXITCODE : $($o -join ' | ')"
    Start-Sleep -Seconds 2
}
$r = Start-Process -FilePath $cli -ArgumentList 'status', '--compact' -Wait -PassThru -RedirectStandardOutput (Join-Path $out 'so.txt') -RedirectStandardError (Join-Path $out 'se.txt')
Log "cli status (Start-Process) exit $($r.ExitCode): $(Get-Content (Join-Path $out 'so.txt') -Raw) $(Get-Content (Join-Path $out 'se.txt') -Raw)"
$o = & $cli quit --compact 2>&1 | ForEach-Object { "$_" }; Log "cli quit exit $LASTEXITCODE : $($o -join ' | ')"
Start-Sleep -Seconds 5
Log "app running after quit: $(-not $p.HasExited)"
Get-Process mistfences -ErrorAction SilentlyContinue | Stop-Process -Force
New-Item -ItemType Directory -Force (Join-Path $out 'logs') | Out-Null
Get-ChildItem (Join-Path $env:LOCALAPPDATA 'MistFences') -File -ErrorAction SilentlyContinue | Copy-Item -Destination (Join-Path $out 'logs') -Force
Set-Content -LiteralPath (Join-Path $out 'results.json') -Value '[]' -Encoding UTF8
Set-Content -LiteralPath (Join-Path $out 'done.txt') -Value 'done' -Encoding ASCII
shutdown.exe /s /t 5
