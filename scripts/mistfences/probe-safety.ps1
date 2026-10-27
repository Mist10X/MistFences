<#
.SYNOPSIS
  Tries forbidden operations on a running MistFences preview and checks they are refused.

.DESCRIPTION
  Drives the preview over its own pipe (mistfences-cli --instance preview). Every probe
  targets the preview's own folder or the TEMP folder, never the real Desktop, so even a
  failing guard could not touch real files. Probes:

    1. hide the Windows desktop icons (settings hideRealIcons)  -> HideIcons stays 0
    2. turn on autostart                                         -> Run entries unchanged
    3. Peek shortcut (global hotkey)                             -> refused in the log
    4. rename a file in a folder portal outside the sandbox      -> file keeps its name
    5. move a sandbox item into that portal (moves real files)   -> file stays in the sandbox
    6. export the configuration to TEMP                          -> no file written
    7. rename a sandbox item (allowed) and back                  -> works

  The settings changed by the probes are set back at the end.
#>
$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$cli = Join-Path $repo 'target\debug\mistfences-cli.exe'
$root = Join-Path $repo '.preview'
$sandbox = Join-Path $root 'Desktop'
$outside = Join-Path $root 'Outside'
$log = Join-Path $env:LOCALAPPDATA 'MistFences\mistfences.preview.log'
$results = [System.Collections.Generic.List[object]]::new()

function Mf { $ErrorActionPreference = 'Continue'; (& $cli --instance preview @args 2>&1 | ForEach-Object { "$_" }) -join "`n" }
function Add-Result([string] $probe, [bool] $ok, [string] $detail) {
    $results.Add([pscustomobject]@{ probe = $probe; passed = $ok; detail = $detail })
    Write-Host ('{0}  {1}  {2}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $probe, $detail)
}
function Get-HideIcons { (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced').HideIcons }
function Get-RunText { $k = Get-Item 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'; ($k.GetValueNames() | Sort-Object | ForEach-Object { "$_=$($k.GetValue($_))" }) -join ';' }
function Log-Since([datetime] $since) {
    if (-not (Test-Path $log)) { return '' }
    $stamp = $since.ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ss')
    (Get-Content -LiteralPath $log -Encoding UTF8 | Where-Object { $_ -ge $stamp }) -join "`n"
}

& $cli --instance preview status *> $null
if ($LASTEXITCODE -ne 0) { throw 'Start the preview first (run-preview.ps1).' }
$started = Get-Date

# 1. Desktop icons
$before = Get-HideIcons
Mf settings set hideRealIcons true | Out-Null
Start-Sleep -Milliseconds 800
$status = Mf status | ConvertFrom-Json
Add-Result 'hide desktop icons' (((Get-HideIcons) -eq $before) -and -not $status.desktopIconsHidden) "HideIcons $before -> $(Get-HideIcons), app reports hidden=$($status.desktopIconsHidden)"

# 2. Autostart
$runBefore = Get-RunText
Mf settings set autostart true | Out-Null
Start-Sleep -Milliseconds 500
Add-Result 'autostart' ((Get-RunText) -eq $runBefore) 'HKCU Run entries unchanged'

# 3. Peek shortcut
Mf settings set peek.enabled true | Out-Null
Start-Sleep -Milliseconds 500
$text = Log-Since $started
Add-Result 'global shortcut' ($text -match 'register a global shortcut') 'refusal logged'

# 4/5. A portal onto a folder outside the sandbox
New-Item -ItemType Directory -Force $outside | Out-Null
$probeFile = Join-Path $outside 'probe.txt'
Set-Content -LiteralPath $probeFile -Value 'outside the sandbox' -Encoding UTF8
$created = Mf fence create --portal $outside --title 'Снаружи' | ConvertFrom-Json
Start-Sleep -Milliseconds 800
Mf item rename $probeFile 'renamed' | Out-Null
Start-Sleep -Milliseconds 800
Add-Result 'rename outside the sandbox' ((Test-Path $probeFile) -and -not (Test-Path (Join-Path $outside 'renamed.txt'))) 'probe.txt keeps its name'
$notes = Join-Path $sandbox 'Заметки.txt'
$move = Mf item move $notes --to 'Снаружи'
Start-Sleep -Milliseconds 1500
Add-Result 'move into a folder outside' ((Test-Path $notes) -and -not (Test-Path (Join-Path $outside 'Заметки.txt')) -and ($move -match '"moved":0') -and ($move -match '"reason":"refused"')) ($move.Trim() -replace '\s+', ' ')
if ($created.fence.id) { Mf fence delete $created.fence.id | Out-Null } else { Mf fence delete 'Снаружи' | Out-Null }

# 6. Export outside the preview folder
$export = Join-Path $env:TEMP ('mistfences-probe-{0}.json' -f $PID)
$reply = Mf config export $export
Add-Result 'export outside the preview' (-not (Test-Path $export)) ($reply.Trim() -replace '\s+', ' ')
if (Test-Path $export) { Remove-Item -LiteralPath $export -Force }

# 7. Allowed: rename inside the sandbox and back
Mf item rename $notes 'Заметки (проверка)' | Out-Null
Start-Sleep -Milliseconds 800
$renamed = Join-Path $sandbox 'Заметки (проверка).txt'
$ok = Test-Path $renamed
if ($ok) { Mf item rename $renamed 'Заметки' | Out-Null; Start-Sleep -Milliseconds 800 }
Add-Result 'rename inside the sandbox (allowed)' ($ok -and (Test-Path $notes)) 'renamed and back'

# Back to the preview defaults
Mf settings set hideRealIcons false | Out-Null
Mf settings set autostart false | Out-Null
Mf settings set peek.enabled false | Out-Null
Remove-Item -LiteralPath $outside -Recurse -Force

$refusals = ([regex]::Matches((Log-Since $started), 'mistfences::safety: refused')).Count
Write-Host "refusals logged during the probes: $refusals"
$failed = @($results | Where-Object { -not $_.passed }).Count
$out = Join-Path $root ('measurements\safety-{0:yyyyMMdd-HHmmss}.json' -f (Get-Date))
New-Item -ItemType Directory -Force (Split-Path $out) | Out-Null
[ordered]@{ taken = $started.ToString('o'); refusalsLogged = $refusals; results = $results } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $out -Encoding utf8
if ($failed) { Write-Host "RESULT: $failed probe(s) FAILED" -ForegroundColor Red; exit 1 }
Write-Host 'RESULT: every forbidden operation was refused' -ForegroundColor Green
