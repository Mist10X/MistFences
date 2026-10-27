<#
.SYNOPSIS
  Checks an imported MistFences configuration against the Fences 6 snapshot it came from.

.DESCRIPTION
  Independent of MistFences' own parser: reads the snapshot with .NET's XML reader and the
  configuration as JSON, then compares every Fences group with its fence: title, monitor
  (device id), position and size (px at 100 %, title bar included, clamped to the work area),
  and the items in Fences' order (row by row). Loose icons must each be in one of the
  "Рабочий стол" fences. Read-only: changes nothing.

.EXAMPLE
  ./scripts/mistfences/check-import.ps1 -Snapshot D:\MistFences-Backups\...\DailySnapshot6.xml -ConfigDir D:\Dev\MistFences\.review\config
#>
param(
    [Parameter(Mandatory)] [string] $Snapshot,
    [Parameter(Mandatory)] [string] $ConfigDir,
    # Work-area bottom (px from the monitor's top) per monitor model, for the height clamp.
    [hashtable] $WorkBottom = @{ AOC3202 = 1392; AOC2702 = 1032 }
)
$ErrorActionPreference = 'Stop'

[xml] $x = Get-Content -LiteralPath $Snapshot -Raw -Encoding UTF8
$root = $x.SnapshotInfo
$config = Get-Content -LiteralPath (Join-Path $ConfigDir 'config.json') -Raw -Encoding UTF8 | ConvertFrom-Json

# Monitors: \\.\DISPLAYn -> device id and origin.
$monitors = @{}
foreach ($m in ($root.ResolutionFingerprintEx -split ';' | Where-Object { $_ })) {
    $f = $m.Trim('[', ']') -split '\|'
    $iface = $f | Where-Object { $_ -like '\\?\DISPLAY#*' } | Select-Object -First 1
    $parts = $iface -split '#'
    $monitors[$f[5]] = [pscustomobject]@{ Id = "$($parts[1])#$($parts[2])"; Left = [int]$f[0]; Top = [int]$f[1]; Model = $parts[1] }
}

$layout = $config.layouts | Sort-Object shown -Descending | Select-Object -First 1
$items = @{}
foreach ($p in $config.items.PSObject.Properties) { $items[$p.Name] = $p.Value }
function Members($fence) {
    $fence.items | Sort-Object { [int]$_.manualIndex } | ForEach-Object {
        $it = $items[$_.itemId]
        if ($it.path) { $it.path } else { $it.key.path }
    }
}

$icons = @()
foreach ($a in $root.Icons.ArrayOfString) {
    $v = $a.string[1] -split '\|'
    $icons += [pscustomobject]@{ Path = $a.string[0]; Group = [int]$v[0]; X = [int]$v[1]; Y = [int]$v[2]; RelX = [int]$v[3]; RelY = [int]$v[4] }
}
$icons = $icons | Where-Object { (Split-Path $_.Path -Leaf) -ne 'desktop.ini' }

$failures = 0
function Fail($text) { $script:failures++; Write-Host "FAIL  $text" -ForegroundColor Red }
function Pass($text) { Write-Host "ok    $text" }

$groups = $root.Groups.ArrayOfString | Where-Object { $_.string[0] -ne '' -and $_.string[0] -is [string] -and $_.string[1] -match '^\d+\|' }
foreach ($g in $groups) {
    $f = $g.string[1] -split '\|'
    $id = [int]$f[0]; $gx = [int]$f[1]; $gy = [int]$f[2]; $gw = [int]$f[3]; $gh = [int]$f[4]
    $title = ($f[6] -replace '` $', '') -replace '`', ' '
    $display = ($f[7] -replace '` $', '')
    $mon = $monitors[$display]
    $fence = $layout.fences | Where-Object { $_.title -ceq $title }
    if (-not $fence) { Fail "group '$title': no fence"; continue }
    if ($fence.geometry.monitor -ne $mon.Id) { Fail "'$title': monitor $($fence.geometry.monitor), expected $($mon.Id)" }
    $top = $gy - 36 - $mon.Top
    $bottom = [Math]::Min($gy + $gh - $mon.Top, $WorkBottom[$mon.Model])
    $want = @(($gx - $mon.Left), $top, $gw, ($bottom - $top))
    $got = @([int]$fence.geometry.x, [int]$fence.geometry.y, [int]$fence.geometry.w, [int]$fence.geometry.h)
    if (($want -join ',') -ne ($got -join ',')) { Fail "'$title': geometry $($got -join ',') expected $($want -join ',')" }
    $expected = $icons | Where-Object Group -eq $id | Where-Object { Test-Path -LiteralPath $_.Path } | Sort-Object RelY, RelX | ForEach-Object Path
    $members = @(Members $fence)
    if ((($expected | ForEach-Object { $_.ToLower() }) -join '|') -ne (($members | ForEach-Object { $_.ToLower() }) -join '|')) {
        Fail "'$title': items differ (expected $(@($expected).Count), got $($members.Count))"
    } else {
        Pass ("'{0}' on {1}: at {2}, rolled={3}, {4} items in order" -f $title, $mon.Model, ($got -join ','), $fence.rolledUp, $members.Count)
    }
}

# Loose icons: each in one of the "Рабочий стол" fences, none in a group fence.
$looseFences = $layout.fences | Where-Object { $_.title -like 'Рабочий стол*' }
$looseMembers = @($looseFences | ForEach-Object { Members $_ } | ForEach-Object { $_.ToLower() })
$loose = $icons | Where-Object Group -lt 0
$lost = 0
foreach ($i in $loose) {
    $p = $i.Path.ToLower()
    $exists = $p.StartsWith('::') -or (Test-Path -LiteralPath $i.Path)
    if ($exists -and $looseMembers -notcontains $p) { $lost++; Fail "loose icon not placed: $($i.Path)" }
}
if ($lost -eq 0) { Pass "$(@($loose).Count) loose icons all in the 'Рабочий стол' fences ($(@($looseFences).Count) fences)" }

# Every desktop item is in exactly one fence.
$all = @($layout.fences | ForEach-Object { $_.items } | ForEach-Object { $_.itemId })
$dupes = $all | Group-Object | Where-Object Count -gt 1
if ($dupes) { Fail "$(@($dupes).Count) items are in more than one fence" } else { Pass "$($all.Count) memberships, no item in two fences" }
$orphans = $items.Keys | Where-Object { $all -notcontains $_ }
if ($orphans) { Fail "$(@($orphans).Count) desktop items in no fence" } else { Pass "every desktop item is in a fence" }

if ($failures) { Write-Host "$failures check(s) failed" -ForegroundColor Red; exit 1 }
Write-Host 'all checks passed' -ForegroundColor Green
