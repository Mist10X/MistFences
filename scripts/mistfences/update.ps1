<#
.SYNOPSIS
  Replaces the installed MistFences (D:\MistFences) with the release build, keeping its
  configuration, and starts it again. -Rollback puts a backup made by this script back.

.DESCRIPTION
  1. Copies the installed programs and configuration into a new folder
     D:\MistFences-Backups\update-<time> and checks the copies.
  2. Asks the running MistFences to quit (its tray Quit: it saves the configuration and shows
     the Windows icons again) and waits for it.
  3. Copies the new programs over (the configuration is left as it is; the new version
     migrates it at its first start).
  4. Starts MistFences the way the user does (through Explorer, outside any container).

  -Rollback <backup folder>: the same with the programs AND the configuration of that backup.
#>
param(
    [string] $Target = 'D:\MistFences',
    [string] $Rollback,
    [string] $BackupRoot = 'D:\MistFences-Backups'
)
$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$files = 'mistfences.exe', 'mistfences-watchdog.exe', 'mistfences-cli.exe', 'WebView2Loader.dll'
$source = if ($Rollback) { $Rollback } else { Join-Path $repo 'target\release' }
foreach ($f in $files) { if (-not (Test-Path (Join-Path $source $f))) { throw "$f missing in $source" } }

# 1. Backup.
$backup = Join-Path $BackupRoot ('update-{0:yyyyMMdd-HHmmss}' -f (Get-Date))
New-Item -ItemType Directory $backup | Out-Null
foreach ($f in $files) { Copy-Item -LiteralPath (Join-Path $Target $f) -Destination $backup }
Copy-Item -LiteralPath (Join-Path $Target 'config') -Destination $backup -Recurse
foreach ($f in $files + 'config\config.json') {
    if ((Get-FileHash (Join-Path $Target $f)).Hash -ne (Get-FileHash (Join-Path $backup $f)).Hash) { throw "backup of $f differs" }
}
Write-Host "Backup: $backup"

# 2. Quit the running copy.
$cli = Join-Path $Target 'mistfences-cli.exe'
$running = Get-Process mistfences -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Target\*" }
if ($running) {
    $ErrorActionPreference = 'Continue'
    & $cli quit --compact 2>$null | Out-Null
    $ErrorActionPreference = 'Stop'
    $deadline = (Get-Date).AddSeconds(20)
    while ((Get-Process -Id $running.Id -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
    if (Get-Process -Id $running.Id -ErrorAction SilentlyContinue) { throw 'MistFences did not quit: nothing was replaced' }
    # Its watchdog exits with it.
    Start-Sleep -Milliseconds 500
}

# 3. Replace.
foreach ($f in $files) { Copy-Item -LiteralPath (Join-Path $source $f) -Destination $Target -Force }
if ($Rollback) {
    Copy-Item -LiteralPath (Join-Path $Rollback 'config\config.json') -Destination (Join-Path $Target 'config\config.json') -Force
}
Write-Host "Installed from $source"

# 4. Start through Explorer (a program started by this script would run inside the caller's
#    container when the caller is a packaged app).
Start-Process explorer.exe -ArgumentList ('"{0}"' -f (Join-Path $Target 'Запустить MistFences.cmd'))
$deadline = (Get-Date).AddSeconds(30)
do {
    Start-Sleep -Milliseconds 500
    $ErrorActionPreference = 'Continue'
    $status = & $cli status --compact 2>$null
    $ErrorActionPreference = 'Stop'
} until (($LASTEXITCODE -eq 0 -and $status) -or (Get-Date) -gt $deadline)
Write-Host "Status: $status"
