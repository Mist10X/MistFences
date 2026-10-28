# Compatibility entry point: the ZIP alone (scripts/mistfences/package.ps1 -Format Zip).
param(
  [string]$Version = "",
  [string]$Repository = "",
  [switch]$SkipBuild
)
$ErrorActionPreference = "Stop"
& "$PSScriptRoot/make-windows.ps1" -Format Portable @PSBoundParameters
