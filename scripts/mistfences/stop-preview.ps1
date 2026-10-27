<#
.SYNOPSIS
  Closes a running MistFences preview the normal way (as its tray menu's Quit does).

.DESCRIPTION
  Asks the preview over its own named pipe (mistfences-cli --instance preview quit). Only a
  preview instance is addressed; a regular MistFences or any other program is never touched.
#>
param([switch] $Release)

$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$cli = Join-Path $repo $(if ($Release) { 'target\release\mistfences-cli.exe' } else { 'target\debug\mistfences-cli.exe' })
if (-not (Test-Path $cli)) { throw "$cli not found: run scripts/mistfences/build.ps1 first" }
& $cli --instance preview quit
if ($LASTEXITCODE -eq 0) { Write-Host 'Preview closed.' } else { Write-Host "No running preview answered (exit code $LASTEXITCODE)." }
