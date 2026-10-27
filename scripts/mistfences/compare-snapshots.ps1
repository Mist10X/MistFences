<#
.SYNOPSIS
  Compares two snapshot-system.ps1 files. Exit code 0 = the protected state is unchanged.

.DESCRIPTION
  Protected (a difference fails): HideIcons, the desktop view flags, the Run entries, the
  user and Public Desktop listings, the data folders outside the preview. Informative
  (reported, never fails): the Stardock Fences key and backups, Explorer's icon layout blob;
  Fences and Explorer rewrite those on their own.
#>
param(
    [Parameter(Mandatory)] [string] $Before,
    [Parameter(Mandatory)] [string] $After
)

$a = Get-Content -LiteralPath $Before -Raw | ConvertFrom-Json
$b = Get-Content -LiteralPath $After -Raw | ConvertFrom-Json
$failed = $false

function Check([string] $what, $x, $y) {
    $left = ($x | ConvertTo-Json -Depth 5 -Compress)
    $right = ($y | ConvertTo-Json -Depth 5 -Compress)
    if ($left -eq $right) {
        Write-Host "  OK       $what"
    } else {
        Write-Host "  CHANGED  $what" -ForegroundColor Red
        Write-Host "           before: $left"
        Write-Host "           after:  $right"
        $script:failed = $true
    }
}

function Note([string] $what, $x, $y) {
    $same = ($x | ConvertTo-Json -Depth 5 -Compress) -eq ($y | ConvertTo-Json -Depth 5 -Compress)
    Write-Host ('  {0}  {1} (informative)' -f $(if ($same) { 'same    ' } else { 'differs ' }), $what)
}

Write-Host "Protected state ($($a.taken) -> $($b.taken)):"
Check 'Show desktop icons (HideIcons)' $a.hideIcons $b.hideIcons
Check 'Desktop view flags' $a.desktopViewFlags $b.desktopViewFlags
Check 'Startup entries (HKCU Run)' $a.runEntries $b.runEntries
Check 'User Desktop files' $a.userDesktop $b.userDesktop
Check 'Public Desktop files' $a.publicDesktop $b.publicDesktop
Check 'Data folders outside the preview' $a.dataFolders $b.dataFolders
Write-Host 'Other programs:'
Note 'Stardock Fences registry key' $a.informative.fencesRegistrySha256 $b.informative.fencesRegistrySha256
Note 'Stardock Fences backups' $a.informative.fencesBackups $b.informative.fencesBackups
Note 'Explorer icon layout blob' $a.informative.desktopIconLayoutSha256 $b.informative.desktopIconLayoutSha256

if ($failed) { Write-Host 'RESULT: protected state CHANGED' -ForegroundColor Red; exit 1 }
Write-Host 'RESULT: protected state unchanged' -ForegroundColor Green
exit 0
