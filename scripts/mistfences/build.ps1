<#
.SYNOPSIS
  Builds MistFences (debug by default) and puts WebView2Loader.dll next to the executables.

.DESCRIPTION
  Runs Cargo inside the Visual Studio Build Tools environment (vcvars64.bat), so it works from
  any PowerShell window. Nothing is installed or registered; the output stays in target\.

.EXAMPLE
  ./scripts/mistfences/build.ps1
  ./scripts/mistfences/build.ps1 -Release
#>
param(
    [switch] $Release,
    # vcvars64.bat of the Build Tools. Found through vswhere when not given.
    [string] $VcVars
)

$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..\..')

if (-not $VcVars) {
    $candidates = @('D:\devtools\BuildTools\VC\Auxiliary\Build\vcvars64.bat')
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($vs) { $candidates = @("$vs\VC\Auxiliary\Build\vcvars64.bat") + $candidates }
    }
    $VcVars = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $VcVars) { throw 'vcvars64.bat not found: pass -VcVars <path>' }

$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }
$profileArgs = if ($Release) { '--release' } else { '' }
# Two builds: built together, Cargo would enable the CLI's `describe` feature (JSON Schema
# export through schemars) in the crates the app shares with it, and link it into the app.
$build = "`"$cargo`" build --locked $profileArgs"
$command = "call `"$VcVars`" >nul 2>nul && cd /d `"$repo`" && $build -p pecofence -p pecofence-watchdog && $build -p pecofence-cli"
# Cargo reports progress on stderr: not an error for PowerShell.
$ErrorActionPreference = 'Continue'
cmd /c "$command 2>&1"
$ErrorActionPreference = 'Stop'
if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }

$out = Join-Path $repo $(if ($Release) { 'target\release' } else { 'target\debug' })
Copy-Item (Join-Path $repo 'third_party\webview2\WebView2Loader.x64.dll') (Join-Path $out 'WebView2Loader.dll') -Force
Write-Host "Built: $out\mistfences.exe"
