<#
.SYNOPSIS
  Imports the Fences 6 layout into a MistFences configuration folder (read-only for Fences).

.DESCRIPTION
  Runs mistfences.exe --import-fences without windows. It reads a COPY of the snapshot (kept in
  <ConfigDir>\import), the monitors and the Desktop listing; it never moves, renames or deletes
  a desktop item and never touches Fences, its settings or Explorer.

  -Preview writes only the report (import\preview-latest.md). Without it the configuration is
  written, read back and checked (import\result-latest.md), then check-import.ps1 compares it
  with the snapshot independently. An existing config.json is kept unless -Replace moves it
  aside (config.before-import-<time>.json).

.EXAMPLE
  ./scripts/mistfences/import-fences.ps1 -Preview
  ./scripts/mistfences/import-fences.ps1
#>
param(
    # The snapshot; default: the newest DailySnapshot*.xml of the latest backup in D:\MistFences-Backups.
    [string] $Snapshot,
    [string] $ConfigDir,
    [switch] $Preview,
    [switch] $Replace,
    [switch] $Release
)

$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$exe = Join-Path $repo $(if ($Release) { 'target\release\mistfences.exe' } else { 'target\debug\mistfences.exe' })
if (-not (Test-Path $exe)) { throw "$exe not found: run scripts/mistfences/build.ps1 first" }
if (-not $ConfigDir) { $ConfigDir = Join-Path $repo '.review\config' }
if (-not $Snapshot) {
    $backup = Get-ChildItem 'D:\MistFences-Backups' -Directory | Sort-Object Name -Descending | Select-Object -First 1
    if (-not $backup) { throw 'no backup in D:\MistFences-Backups: run backup.ps1 first, or pass -Snapshot' }
    $Snapshot = Get-ChildItem (Join-Path $backup.FullName 'fences\Backups') -Filter 'DailySnapshot*.xml' |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
}
Write-Host "Snapshot: $Snapshot"

$arguments = @('--import-fences', "`"$Snapshot`"", '--config-dir', "`"$ConfigDir`"")
if ($Preview) { $arguments += '--dry-run' }
if ($Replace) { $arguments += '--replace' }
$process = Start-Process -FilePath $exe -ArgumentList $arguments -Wait -PassThru
$report = Join-Path $ConfigDir $(if ($Preview) { 'import\preview-latest.md' } else { 'import\result-latest.md' })
switch ($process.ExitCode) {
    0 { Write-Host "Done. Report: $report" -ForegroundColor Green }
    2 { Write-Host "Problems found, nothing imported. Report: $(Join-Path $ConfigDir 'import\preview-latest.md')" -ForegroundColor Yellow; exit 2 }
    3 { Write-Host "$ConfigDir already has a config.json: pass -Replace to move it aside" -ForegroundColor Yellow; exit 3 }
    4 { Write-Host "Written, but the check found differences. Report: $report" -ForegroundColor Red; exit 4 }
    default { Write-Host "Import failed (exit $($process.ExitCode)); see $(Join-Path $ConfigDir 'import\last-error.txt')" -ForegroundColor Red; exit 1 }
}
if (-not $Preview) {
    & (Join-Path $PSScriptRoot 'check-import.ps1') -Snapshot $Snapshot -ConfigDir $ConfigDir
    exit $LASTEXITCODE
}
