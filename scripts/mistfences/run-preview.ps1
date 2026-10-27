<#
.SYNOPSIS
  Starts the safe MistFences preview: three test fences with sample items, next to Fences 6.

.DESCRIPTION
  The preview keeps its sample items and configuration in .preview\ inside the project
  (Desktop\ = the sandbox the fences show, config\ = its own config.json). It never hides the
  Windows desktop icons, never writes a startup entry, registers no global shortcut and
  refuses any file change outside .preview\Desktop.

  Close it from its tray icon (MistFences > Quit), with -ExitAfter, or with stop-preview.ps1.

.EXAMPLE
  ./scripts/mistfences/run-preview.ps1
  ./scripts/mistfences/run-preview.ps1 -ExitAfter 60
  ./scripts/mistfences/run-preview.ps1 -Reset      # start again from the default layout
#>
param(
    # Close automatically after this many seconds (0 = keep running).
    [int] $ExitAfter = 0,
    # Delete the preview configuration first (the sample files are kept or re-created).
    [switch] $Reset,
    [switch] $Release
)

$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$exe = Join-Path $repo $(if ($Release) { 'target\release\mistfences.exe' } else { 'target\debug\mistfences.exe' })
if (-not (Test-Path $exe)) { throw "$exe not found: run scripts/mistfences/build.ps1 first" }
if (-not (Test-Path (Join-Path (Split-Path $exe) 'WebView2Loader.dll'))) {
    throw 'WebView2Loader.dll missing next to the executable: run scripts/mistfences/build.ps1'
}

$root = Join-Path $repo '.preview'
if ($Reset -and (Test-Path (Join-Path $root 'config'))) {
    Remove-Item -LiteralPath (Join-Path $root 'config') -Recurse -Force
    Write-Host 'Preview configuration reset.'
}

$arguments = @('--mistfences-preview', '--preview-root', "`"$root`"")
if ($ExitAfter -gt 0) { $arguments += @('--exit-after', [string]($ExitAfter * 1000)) }
$process = Start-Process -FilePath $exe -ArgumentList $arguments -PassThru
Write-Host "MistFences preview started (pid $($process.Id)). Data: $root"
Write-Host 'Close it from the tray icon (Quit) or with scripts/mistfences/stop-preview.ps1.'
