<#
.SYNOPSIS
  Backs up everything the switch from Fences 6 to MistFences could touch, then verifies it.

.DESCRIPTION
  Read-only for the sources. Each run writes a new folder D:\MistFences-Backups\<time> (never
  overwrites an older one) with:
    fences\        copy of %APPDATA%\Stardock\Fences (snapshots, backups, logs)
    fences-local\  copy of %LOCALAPPDATA%\Stardock\Fences6 (if present)
    registry\      .reg exports: Stardock\Fences, Explorer desktop view (Bags\1\Desktop,
                   icon layout), Explorer\Advanced (HideIcons), Run (startup entries)
    mistfences\    copy of %APPDATA%\MistFences and %LOCALAPPDATA%\MistFences (if present)
    inventory\     listing of the user and Public Desktop: every file and folder (recursive)
                   with size, times and SHA-256 of files up to 200 MB
    manifest.json  SHA-256 of every backup file
  Verification: every copied file hash equals its source, every XML snapshot parses, every
  .reg export starts with the registry header and contains its key, the inventory covers the
  same number of top-level items as the desktop.

.EXAMPLE
  ./scripts/mistfences/backup.ps1
#>
param([string] $Root = 'D:\MistFences-Backups')

$ErrorActionPreference = 'Stop'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$dest = Join-Path $Root $stamp
if (Test-Path $dest) { throw "$dest already exists" }
New-Item -ItemType Directory -Force $dest | Out-Null
$problems = [System.Collections.Generic.List[string]]::new()

function Copy-Tree([string] $from, [string] $to) {
    if (-not (Test-Path -LiteralPath $from)) { return $null }
    New-Item -ItemType Directory -Force $to | Out-Null
    $count = 0
    Get-ChildItem -LiteralPath $from -Recurse -Force -File | ForEach-Object {
        $rel = $_.FullName.Substring($from.TrimEnd('\').Length + 1)
        $target = Join-Path $to $rel
        New-Item -ItemType Directory -Force (Split-Path $target) | Out-Null
        Copy-Item -LiteralPath $_.FullName -Destination $target -Force
        $a = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
        $b = (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash
        if ($a -ne $b) { $problems.Add("copy differs: $rel") }
        $count++
    }
    return $count
}

# 1-2. Fences data and snapshots
$fencesCount = Copy-Tree (Join-Path $env:APPDATA 'Stardock\Fences') (Join-Path $dest 'fences')
$fencesLocal = Copy-Tree (Join-Path $env:LOCALAPPDATA 'Stardock\Fences6') (Join-Path $dest 'fences-local')
$snapshots = @(Get-ChildItem -LiteralPath (Join-Path $dest 'fences\Backups') -Filter '*.xml' -ErrorAction SilentlyContinue)
foreach ($s in $snapshots) {
    try { [xml](Get-Content -LiteralPath $s.FullName -Raw -Encoding UTF8) | Out-Null }
    catch { $problems.Add("snapshot does not parse: $($s.Name)") }
}
if ($snapshots.Count -eq 0) { $problems.Add('no Fences XML snapshot found') }

# 3-4. Registry
$regDir = Join-Path $dest 'registry'
New-Item -ItemType Directory -Force $regDir | Out-Null
$keys = [ordered]@{
    'stardock-fences.reg' = 'HKCU\Software\Stardock\Fences'
    'explorer-desktop-view.reg' = 'HKCU\Software\Microsoft\Windows\Shell\Bags\1\Desktop'
    'explorer-advanced.reg' = 'HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced'
    'run.reg' = 'HKCU\Software\Microsoft\Windows\CurrentVersion\Run'
    'run-hklm.reg' = 'HKLM\Software\Microsoft\Windows\CurrentVersion\Run'
    'stardock-hklm.reg' = 'HKLM\Software\WOW6432Node\Stardock'
}
foreach ($name in $keys.Keys) {
    $file = Join-Path $regDir $name
    & reg.exe export $keys[$name] $file /y *> $null
    if (-not (Test-Path $file)) { if ($name -notlike '*hklm*') { $problems.Add("registry export failed: $($keys[$name])") }; continue }
    $text = Get-Content -LiteralPath $file -Raw
    if ($text -notmatch '^\uFEFF?Windows Registry Editor Version 5\.00' -or $text -notmatch [regex]::Escape(($keys[$name] -replace '^HKCU', 'HKEY_CURRENT_USER' -replace '^HKLM', 'HKEY_LOCAL_MACHINE'))) {
        $problems.Add("registry export unreadable: $name")
    }
}

# 5. MistFences
$mf1 = Copy-Tree (Join-Path $env:APPDATA 'MistFences') (Join-Path $dest 'mistfences\roaming')
$mf2 = Copy-Tree (Join-Path $env:LOCALAPPDATA 'MistFences') (Join-Path $dest 'mistfences\local')

# 6. Desktop inventory
$invDir = Join-Path $dest 'inventory'
New-Item -ItemType Directory -Force $invDir | Out-Null
foreach ($pair in @(@('user-desktop', [Environment]::GetFolderPath('Desktop')), @('public-desktop', [Environment]::GetFolderPath('CommonDesktopDirectory')))) {
    $name, $dir = $pair
    $rows = Get-ChildItem -LiteralPath $dir -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object {
        $hash = ''
        if (-not $_.PSIsContainer -and $_.Length -le 200MB) { try { $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash } catch { $hash = 'unreadable' } }
        [pscustomobject]@{
            path = $_.FullName.Substring($dir.Length + 1)
            kind = $(if ($_.PSIsContainer) { 'dir' } else { 'file' })
            size = $(if ($_.PSIsContainer) { '' } else { $_.Length })
            created = $_.CreationTimeUtc.ToString('o')
            modified = $_.LastWriteTimeUtc.ToString('o')
            attributes = [string]$_.Attributes
            sha256 = $hash
        }
    }
    $rows | Export-Csv -LiteralPath (Join-Path $invDir "$name.csv") -NoTypeInformation -Encoding UTF8
    $top = @(Get-ChildItem -LiteralPath $dir -Force).Count
    $topInv = @(Import-Csv -LiteralPath (Join-Path $invDir "$name.csv") | Where-Object { $_.path -notmatch '\\' }).Count
    if ($top -ne $topInv) { $problems.Add("inventory of $name lists $topInv of $top top-level items") }
}

# Manifest
$manifest = Get-ChildItem -LiteralPath $dest -Recurse -File | ForEach-Object {
    [pscustomobject]@{ path = $_.FullName.Substring($dest.Length + 1); size = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
}
$summary = [ordered]@{
    created = (Get-Date).ToString('o')
    computer = $env:COMPUTERNAME
    fencesFiles = $fencesCount
    fencesLocalFiles = $fencesLocal
    fencesSnapshots = @($snapshots | ForEach-Object Name)
    mistfencesFiles = @($mf1, $mf2)
    problems = $problems
    files = $manifest
}
$summary | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $dest 'manifest.json') -Encoding UTF8

Write-Host "Backup: $dest"
Write-Host "  Fences files: $fencesCount, snapshots: $($snapshots.Count), MistFences files: $(($mf1 + $mf2))"
Write-Host "  Backup files: $(@($manifest).Count), total $([math]::Round((($manifest | Measure-Object size -Sum).Sum)/1MB, 1)) MB"
if ($problems.Count) { $problems | ForEach-Object { Write-Host "  PROBLEM: $_" -ForegroundColor Red }; exit 1 }
Write-Host '  Verified: copies match their sources, snapshots parse, registry exports readable, inventory complete.' -ForegroundColor Green
