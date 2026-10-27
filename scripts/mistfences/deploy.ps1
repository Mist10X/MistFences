<#
.SYNOPSIS
  Puts the MistFences full mode together in one folder (default D:\MistFences). Starts nothing.

.DESCRIPTION
  Copies the release build (mistfences.exe, mistfences-watchdog.exe, mistfences-cli.exe,
  WebView2Loader.dll) and the imported configuration into the folder, and writes two
  double-clickable helpers next to them:

    Запустить MistFences.cmd         starts the full mode (portable: configuration in .\config)
    Восстановить рабочий стол.cmd    the emergency way back: quits MistFences, shows the
                                     Windows icons, turns MistFences' autostart off

  It registers nothing, starts nothing and changes no Windows setting. An existing
  config\config.json is never overwritten: the imported one goes next to it as
  config.imported.json unless -ReplaceConfig moves the old one aside first.

.EXAMPLE
  ./scripts/mistfences/deploy.ps1
#>
param(
    [string] $Target = 'D:\MistFences',
    [string] $ConfigDir,
    [switch] $ReplaceConfig,
    # Use the existing release build instead of building it.
    [switch] $NoBuild
)

$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')
if (-not $ConfigDir) { $ConfigDir = Join-Path $repo '.review\config' }
foreach ($desktop in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('CommonDesktopDirectory'))) {
    if ($Target.TrimEnd('\').StartsWith($desktop.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)) {
        throw "the MistFences folder must not be on the Desktop ($desktop)"
    }
}
if (-not $NoBuild) { & (Join-Path $PSScriptRoot 'build.ps1') -Release }
$release = Join-Path $repo 'target\release'
$files = 'mistfences.exe', 'mistfences-watchdog.exe', 'mistfences-cli.exe', 'WebView2Loader.dll'
foreach ($f in $files) { if (-not (Test-Path (Join-Path $release $f))) { throw "$f missing in $release" } }

$running = Get-Process mistfences -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($Target, [StringComparison]::OrdinalIgnoreCase) }
if ($running) { throw "MistFences is running from ${Target}: quit it first" }

New-Item -ItemType Directory -Force $Target | Out-Null
foreach ($f in $files) { Copy-Item -LiteralPath (Join-Path $release $f) -Destination $Target -Force }

$config = Join-Path $Target 'config'
New-Item -ItemType Directory -Force $config | Out-Null
$imported = Join-Path $ConfigDir 'config.json'
if (-not (Test-Path $imported)) { throw "no imported configuration in ${ConfigDir}: run import-fences.ps1 first" }
$current = Join-Path $config 'config.json'
if (Test-Path $current) {
    if ($ReplaceConfig) {
        $aside = Join-Path $config ('config.before-deploy-{0:yyyyMMdd-HHmmss}.json' -f (Get-Date))
        Move-Item -LiteralPath $current -Destination $aside
        Copy-Item -LiteralPath $imported -Destination $current
        Write-Host "Configuration replaced; the old one is $aside"
    } else {
        Copy-Item -LiteralPath $imported -Destination (Join-Path $config 'config.imported.json') -Force
        Write-Host 'A configuration was already there: kept; the import is config.imported.json'
    }
} else {
    Copy-Item -LiteralPath $imported -Destination $current
}
if (Test-Path (Join-Path $ConfigDir 'themes')) {
    Copy-Item -LiteralPath (Join-Path $ConfigDir 'themes') -Destination $config -Recurse -Force
}

$utf8 = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText((Join-Path $Target 'Запустить MistFences.cmd'),
    "@echo off`r`nchcp 65001 >nul`r`ncd /d `"%~dp0`"`r`nstart `"`" `"%~dp0mistfences.exe`" --portable`r`n", $utf8)
[IO.File]::WriteAllText((Join-Path $Target 'Восстановить рабочий стол.cmd'),
    "@echo off`r`nchcp 65001 >nul`r`nrem Quits MistFences, shows the Windows desktop icons and turns MistFences' autostart off.`r`n`"%~dp0mistfences-watchdog.exe`" --restore --no-autostart`r`n", $utf8)

Write-Host "MistFences is ready in $Target (nothing started, nothing registered)."
Get-ChildItem $Target | Format-Table Name, Length -AutoSize | Out-String | Write-Host
