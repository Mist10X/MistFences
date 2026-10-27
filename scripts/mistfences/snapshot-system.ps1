<#
.SYNOPSIS
  Records the Windows state the MistFences preview must leave alone, as JSON.

.DESCRIPTION
  Read-only. Run it before and after a preview session and compare the two files with
  compare-snapshots.ps1. It records:
    - the "Show desktop icons" setting (HideIcons) and the Explorer desktop view flags;
    - every value of HKCU\...\CurrentVersion\Run (startup entries);
    - the file listing (name, size, write time) of the user and Public Desktop folders;
    - which data folders of MistFences / PecoFence exist outside the preview;
    - a hash of the Stardock Fences configuration key and its backup folder listing
      (informative: Fences itself rewrites them while it runs).

.EXAMPLE
  ./snapshot-system.ps1 -Out before.json
#>
param([Parameter(Mandatory)] [string] $Out)

$ErrorActionPreference = 'Stop'

function Get-Listing([string] $dir) {
    if (-not (Test-Path -LiteralPath $dir)) { return @() }
    Get-ChildItem -LiteralPath $dir -Force | Sort-Object Name | ForEach-Object {
        '{0}|{1}|{2:o}' -f $_.Name, ($(if ($_.PSIsContainer) { 'dir' } else { $_.Length })), $_.LastWriteTimeUtc
    }
}

function Get-RegistryText([string] $key) {
    if (-not (Test-Path -LiteralPath $key)) { return '' }
    $lines = foreach ($k in @(Get-Item -LiteralPath $key) + @(Get-ChildItem -LiteralPath $key -Recurse)) {
        $k.Name
        foreach ($name in ($k.GetValueNames() | Sort-Object)) {
            $value = $k.GetValue($name, $null, 'DoNotExpandEnvironmentNames')
            if ($value -is [byte[]]) { $value = [Convert]::ToBase64String($value) }
            elseif ($value -is [string[]]) { $value = $value -join "`n" }
            "  $name=$value"
        }
    }
    $lines -join "`n"
}

function Get-Hash([string] $text) {
    $sha = [Security.Cryptography.SHA256]::Create()
    ($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text)) | ForEach-Object { $_.ToString('x2') }) -join ''
}

$advanced = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced'
$bags = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\Shell\Bags\1\Desktop' -ErrorAction SilentlyContinue
$run = Get-Item 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$userDesktop = [Environment]::GetFolderPath('Desktop')
$publicDesktop = [Environment]::GetFolderPath('CommonDesktopDirectory')

$snapshot = [ordered]@{
    taken = (Get-Date).ToString('o')
    hideIcons = $advanced.HideIcons
    desktopViewFlags = $bags.FFlags
    runEntries = [ordered]@{}
    userDesktop = @(Get-Listing $userDesktop)
    publicDesktop = @(Get-Listing $publicDesktop)
    dataFolders = [ordered]@{
        'APPDATA\MistFences' = Test-Path (Join-Path $env:APPDATA 'MistFences')
        'APPDATA\PecoFence' = Test-Path (Join-Path $env:APPDATA 'PecoFence')
        'LOCALAPPDATA\PecoFence' = Test-Path (Join-Path $env:LOCALAPPDATA 'PecoFence')
    }
    informative = [ordered]@{
        fencesRegistrySha256 = Get-Hash (Get-RegistryText 'HKCU:\Software\Stardock\Fences')
        fencesBackups = @(Get-Listing (Join-Path $env:APPDATA 'Stardock\Fences\Backups'))
        desktopIconLayoutSha256 = Get-Hash ([string]$(if ($bags.IconLayouts) { [Convert]::ToBase64String($bags.IconLayouts) }))
    }
}
foreach ($name in ($run.GetValueNames() | Sort-Object)) { $snapshot.runEntries[$name] = $run.GetValue($name) }

$snapshot | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $Out -Encoding utf8
Write-Host "snapshot written to $Out"
