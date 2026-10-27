<#
.SYNOPSIS
  Shows an imported configuration on the real Desktop, read only (MistFences review).

.DESCRIPTION
  Starts MistFences with --mistfences-review on a fresh copy of the imported configuration
  (.review\config -> .review\session), so whatever is tried in the review never changes the
  import itself. The review:
    - shows the real Desktop items in the imported fences, next to Fences 6;
    - opens items with a double-click (the default verb only);
    - never hides the Windows icons, writes no startup entry, registers no global shortcut,
      shows no Explorer context menu and changes no file at all (rename, delete, move, copy,
      drops from and to other programs are refused).

  Close it from its tray icon (MistFences > Quit), with -ExitAfter, or with stop-review.ps1.

.EXAMPLE
  ./scripts/mistfences/run-review.ps1
  ./scripts/mistfences/run-review.ps1 -ExitAfter 120
#>
param(
    [int] $ExitAfter = 0,
    # The imported configuration to show (made by import-fences.ps1).
    [string] $ConfigDir,
    [switch] $Release
)

$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$exe = Join-Path $repo $(if ($Release) { 'target\release\mistfences.exe' } else { 'target\debug\mistfences.exe' })
if (-not (Test-Path $exe)) { throw "$exe not found: run scripts/mistfences/build.ps1 first" }
if (-not $ConfigDir) { $ConfigDir = Join-Path $repo '.review\config' }
if (-not (Test-Path (Join-Path $ConfigDir 'config.json'))) { throw "no config.json in ${ConfigDir}: run import-fences.ps1 first" }

$session = Join-Path $repo '.review\session'
if (Test-Path $session) { Remove-Item -LiteralPath $session -Recurse -Force }
New-Item -ItemType Directory -Force $session | Out-Null
Copy-Item -LiteralPath (Join-Path $ConfigDir 'config.json') -Destination $session
if (Test-Path (Join-Path $ConfigDir 'themes')) {
    Copy-Item -LiteralPath (Join-Path $ConfigDir 'themes') -Destination $session -Recurse
}

$arguments = @('--mistfences-review', '--config-dir', "`"$session`"")
if ($ExitAfter -gt 0) { $arguments += @('--exit-after', [string]($ExitAfter * 1000)) }
$process = Start-Process -FilePath $exe -ArgumentList $arguments -PassThru
Write-Host "MistFences review started (pid $($process.Id)). Configuration copy: $session"
Write-Host 'Close it from the tray icon (Quit) or with scripts/mistfences/stop-review.ps1.'
