<#
.SYNOPSIS
  Closes a running MistFences review the normal way (as its tray menu's Quit does).

.DESCRIPTION
  Asks the review over its own named pipe (mistfences-cli --instance review quit). Only the
  review instance is addressed; Fences, a regular MistFences or any other program is never
  touched.
#>
param([switch] $Release)

$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$cli = Join-Path $repo $(if ($Release) { 'target\release\mistfences-cli.exe' } else { 'target\debug\mistfences-cli.exe' })
if (-not (Test-Path $cli)) { throw "$cli not found: run scripts/mistfences/build.ps1 first" }
$ErrorActionPreference = 'Continue'
& $cli --instance review quit 2>$null
if ($LASTEXITCODE -eq 0) { Write-Host 'Review closed.' } else { Write-Host "No running review answered (exit code $LASTEXITCODE)." }
