<#
.SYNOPSIS
  Exports the public MistFences source tree (no development history) into a folder.
.DESCRIPTION
  Takes the committed tree of HEAD (git archive: tracked files only) and leaves out what
  belongs to PecoFence's own infrastructure or to this machine's migration:
  CI and issue templates (they build PecoFence packages), the website, Store and winget
  material, the feedback service, the promo video, upstream README translations, and the
  migration notes/scripts written for one PC (STAGE1-REPORT, SWITCH, scripts/mistfences/switch).
  skills/ stays: the CLI embeds its SKILL.md. README.md becomes
  docs/mistfences/PUBLIC-README.md. The folder is created fresh.
.EXAMPLE
  ./scripts/mistfences/export-public.ps1 -Target D:\Dev\MistFences-public
#>
param([Parameter(Mandatory)] [string] $Target)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (Test-Path -LiteralPath $Target) {
    if (@(Get-ChildItem -LiteralPath $Target -Force | Where-Object Name -ne '.git').Count) { throw "$Target is not empty" }
} else {
    New-Item -ItemType Directory $Target | Out-Null
}
$zip = Join-Path ([IO.Path]::GetTempPath()) ("mistfences-export-{0}.zip" -f [guid]::NewGuid())
git -C $repo archive --format=zip -o $zip HEAD
if ($LASTEXITCODE) { throw 'git archive failed' }
Expand-Archive -LiteralPath $zip -DestinationPath $Target
Remove-Item -LiteralPath $zip
$drop = '.github', '.wrangler', 'site', 'services', 'extras', 'packaging\msix',
    'docs\store', 'docs\STORE.md', 'docs\WEBSITE.md', 'docs\readme', 'CONTRIBUTING.md',
    'docs\mistfences\STAGE1-REPORT.md', 'docs\mistfences\SWITCH.md', 'scripts\mistfences\switch'
foreach ($d in $drop) {
    $p = Join-Path $Target $d
    if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Recurse -Force }
}
Move-Item -LiteralPath (Join-Path $Target 'docs\mistfences\PUBLIC-README.md') -Destination (Join-Path $Target 'README.md') -Force
Write-Host "Exported to $Target"
