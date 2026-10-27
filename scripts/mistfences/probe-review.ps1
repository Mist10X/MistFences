<#
.SYNOPSIS
  Tries forbidden operations on a running MistFences review and checks they are refused.

.DESCRIPTION
  Drives the review over its own pipe (mistfences-cli --instance review). The review shows the
  real Desktop, so no probe ever targets a Desktop item: the file probes use two folder
  portals onto a scratch folder in .review\probe, and even a failing guard could only touch
  those scratch files. Probes:

    1. the imported fences and the real Desktop items are shown         -> counts as imported
    2. Windows desktop icons                                            -> HideIcons unchanged
    3. turn on autostart                                                -> Run entries unchanged
    4. Peek shortcut (global hotkey)                                    -> refused in the log
    5. rename a file (in a scratch portal)                              -> file keeps its name
    6. move a file between two scratch portals (moves real files)       -> file stays put
    7. export the configuration to TEMP                                 -> no file written
    8. the Desktop folders                                              -> same names, sizes, times

  Settings changed by the probes are set back; the review runs on a copy of the import anyway.
#>
param([int] $ExpectFences = 12, [int] $ExpectItems = 0)
$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$cli = Join-Path $repo 'target\debug\mistfences-cli.exe'
$scratch = Join-Path $repo '.review\probe'
$log = Join-Path $env:LOCALAPPDATA 'MistFences\mistfences.review.log'
$results = [System.Collections.Generic.List[object]]::new()

function Mf { $ErrorActionPreference = 'Continue'; (& $cli --instance review @args 2>&1 | ForEach-Object { "$_" }) -join "`n" }
function Add-Result([string] $probe, [bool] $ok, [string] $detail) {
    $results.Add([pscustomobject]@{ probe = $probe; passed = $ok; detail = $detail })
    Write-Host ('{0}  {1}  {2}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $probe, $detail)
}
function Get-HideIcons { (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced').HideIcons }
function Get-RunText { $k = Get-Item 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'; ($k.GetValueNames() | Sort-Object | ForEach-Object { "$_=$($k.GetValue($_))" }) -join ';' }
function Get-DesktopState {
    $dirs = @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('CommonDesktopDirectory'))
    ($dirs | ForEach-Object { Get-ChildItem -LiteralPath $_ -Force } | Sort-Object FullName |
        ForEach-Object { '{0}|{1}|{2:o}' -f $_.FullName, $(if ($_.PSIsContainer) { 'dir' } else { $_.Length }), $_.LastWriteTimeUtc }) -join "`n"
}
function Log-Since([datetime] $since) {
    if (-not (Test-Path $log)) { return '' }
    $stamp = $since.ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ss')
    (Get-Content -LiteralPath $log -Encoding UTF8 | Where-Object { $_ -ge $stamp }) -join "`n"
}

$null = Mf status
if ($LASTEXITCODE -ne 0) { throw 'Start the review first (run-review.ps1).' }
$started = Get-Date
$desktopBefore = Get-DesktopState

# 1. What is shown
$status = Mf status | ConvertFrom-Json
$fences = Mf fence list | ConvertFrom-Json
$fenceCount = @($fences.fences).Count
$itemCount = ($fences.fences | ForEach-Object { $_.itemCount } | Measure-Object -Sum).Sum
$okShown = ($fenceCount -eq $ExpectFences) -and (($ExpectItems -eq 0) -or ($itemCount -eq $ExpectItems))
Add-Result 'imported fences shown' $okShown "$fenceCount fences, $itemCount items"

# 2. Desktop icons (the imported configuration asks for hidden icons; the review must not)
$before = Get-HideIcons
Start-Sleep -Milliseconds 300
Add-Result 'Windows icons left alone' (((Get-HideIcons) -eq $before) -and -not $status.desktopIconsHidden) "HideIcons $before -> $(Get-HideIcons), app reports hidden=$($status.desktopIconsHidden)"

# 3. Autostart
$runBefore = Get-RunText
Mf settings set autostart true | Out-Null
Start-Sleep -Milliseconds 500
Add-Result 'autostart' ((Get-RunText) -eq $runBefore) 'HKCU Run entries unchanged'
Mf settings set autostart false | Out-Null

# 4. Peek shortcut
Mf settings set peek.enabled true | Out-Null
Start-Sleep -Milliseconds 500
Add-Result 'global shortcut' ((Log-Since $started) -match 'register a global shortcut') 'refusal logged'
Mf settings set peek.enabled false | Out-Null

# 5/6. Two portals onto scratch folders
$a = Join-Path $scratch 'A'; $b = Join-Path $scratch 'B'
New-Item -ItemType Directory -Force $a, $b | Out-Null
$probeFile = Join-Path $a 'probe.txt'
Set-Content -LiteralPath $probeFile -Value 'review probe' -Encoding UTF8
$pa = Mf fence create --portal $a --title 'Проба A' | ConvertFrom-Json
$pb = Mf fence create --portal $b --title 'Проба B' | ConvertFrom-Json
Start-Sleep -Milliseconds 1000
Mf item rename $probeFile 'renamed' | Out-Null
Start-Sleep -Milliseconds 800
Add-Result 'rename' ((Test-Path $probeFile) -and -not (Test-Path (Join-Path $a 'renamed.txt'))) 'probe.txt keeps its name'
$move = Mf item move $probeFile --to 'Проба B'
Start-Sleep -Milliseconds 1500
Add-Result 'move between folders' ((Test-Path $probeFile) -and -not (Test-Path (Join-Path $b 'probe.txt'))) ($move.Trim() -replace '\s+', ' ')
foreach ($p in @($pa, $pb)) { if ($p.fence.id) { Mf fence delete $p.fence.id | Out-Null } }
Mf fence delete 'Проба A' | Out-Null
Mf fence delete 'Проба B' | Out-Null

# 7. Export outside the review folder
$export = Join-Path $env:TEMP ('mistfences-review-probe-{0}.json' -f $PID)
$reply = Mf config export $export
Add-Result 'export outside the review' (-not (Test-Path $export)) ($reply.Trim() -replace '\s+', ' ')
if (Test-Path $export) { Remove-Item -LiteralPath $export -Force }

# 8. The Desktop folders
Start-Sleep -Milliseconds 500
Add-Result 'Desktop folders unchanged' ((Get-DesktopState) -eq $desktopBefore) 'names, sizes and write times as before'

Remove-Item -LiteralPath $scratch -Recurse -Force
$refusals = ([regex]::Matches((Log-Since $started), 'mistfences::safety: refused')).Count
Write-Host "refusals logged during the probes: $refusals"
$failed = @($results | Where-Object { -not $_.passed }).Count
$out = Join-Path $repo ('.review\measurements\review-safety-{0:yyyyMMdd-HHmmss}.json' -f (Get-Date))
New-Item -ItemType Directory -Force (Split-Path $out) | Out-Null
[ordered]@{ taken = $started.ToString('o'); refusalsLogged = $refusals; results = $results } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $out -Encoding utf8
if ($failed) { Write-Host "RESULT: $failed probe(s) FAILED" -ForegroundColor Red; exit 1 }
Write-Host 'RESULT: every forbidden operation was refused' -ForegroundColor Green
