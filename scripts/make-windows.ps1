#requires -Version 5.1
# Upstream's packaging entry point. MistFences packs its ZIP and its setup program from the
# same staged files in scripts/mistfences/package.ps1; this keeps the old name and -Format
# values working (Portable = the ZIP).
[CmdletBinding()]
param(
  [ValidateSet("All", "Portable", "Installer")][string]$Format = "All",
  [string]$Version = "",
  [string]$Repository = "",
  [string]$Iscc = "",
  [switch]$SkipBuild
)
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$workspaceVersion = (Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
if ($Version -and $Version -cne $workspaceVersion) { throw "Package version must match Cargo.toml ($workspaceVersion)" }
$arguments = @{ Format = @{ All = "All"; Portable = "Zip"; Installer = "Installer" }[$Format] }
if ($Repository) { $arguments.Repository = $Repository }
if ($Iscc) { $arguments.Iscc = $Iscc }
if ($SkipBuild) { $arguments.SkipBuild = $true }
& (Join-Path $PSScriptRoot "mistfences/package.ps1") @arguments
