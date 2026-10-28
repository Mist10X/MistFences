<#
.SYNOPSIS
  Builds the MistFences release and packs it for other users: a setup program and a ZIP to
  extract and run, with the same files.
.DESCRIPTION
  1. Builds the release (build.ps1 -Release) unless -SkipBuild.
  2. Checks that the executables carry the workspace version.
  3. Stages the programs, WebView2Loader.dll, the user guide (README.md), LICENSE, NOTICE,
     the WebView2 Loader licence, THIRD-PARTY-LICENSES.txt (every locked Cargo dependency
     with its licence files, as upstream's write-license-notices.py does) and
     restore-desktop.cmd (the emergency way back for the ZIP edition).
  4. Writes dist\MistFences-v<version>-x64.zip and, with Inno Setup 7
     (packaging\inno\mistfences.iss), dist\MistFences-v<version>-x64-setup.exe, each with its
     SHA-256 next to it.
  Nothing is installed or registered.

  -Format All (default) builds both, or only the ZIP with a warning when Inno Setup 7 is not
  installed; Zip and Installer build one. -Iscc names ISCC.exe (default: the ISCC variable,
  PATH, then the usual install folders).
.EXAMPLE
  ./scripts/mistfences/package.ps1
  ./scripts/mistfences/package.ps1 -Format Installer -Iscc 'C:\Tools\Inno Setup 7\ISCC.exe'
#>
param(
    [switch] $SkipBuild,
    [ValidateSet('All', 'Zip', 'Installer')] [string] $Format = 'All',
    [string] $Iscc,
    # owner/name on GitHub, for the links in Windows' installed apps list and the guide shortcut.
    [string] $Repository = $(if ($env:GITHUB_REPOSITORY) { $env:GITHUB_REPOSITORY } else { 'Mist10X/MistFences' })
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Push-Location $repo
try {
    $version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
    if ($Repository -notmatch '^[A-Za-z0-9][A-Za-z0-9-]*/[A-Za-z0-9_.-]+$') { throw "-Repository must be owner/name, not '$Repository'" }
    # Before the build: a missing compiler should not cost a release build first.
    $setup = $Format -ne 'Zip'
    if ($setup) {
        . (Join-Path $repo 'scripts\find-iscc.ps1')
        try {
            $Iscc = Find-Iscc $Iscc
        } catch {
            if ($Format -eq 'Installer') { throw }
            Write-Warning "$($_.Exception.Message) Only the ZIP is built."
            $setup = $false
        }
    }
    if (-not $SkipBuild) {
        & (Join-Path $PSScriptRoot 'build.ps1') -Release
        if ($LASTEXITCODE) { throw 'release build failed' }
    }
    $release = Join-Path $repo 'target\release'
    $programs = 'mistfences.exe', 'mistfences-watchdog.exe', 'mistfences-cli.exe'
    foreach ($p in $programs) {
        $file = Join-Path $release $p
        if (-not (Test-Path -LiteralPath $file)) { throw "$p missing in $release" }
        $built = (Get-Item -LiteralPath $file).VersionInfo.ProductVersion
        if ($built -ne $version) { throw "$p is version '$built', the workspace is ${version}: rebuild" }
    }

    $name = "MistFences-v$version-x64"
    $dist = Join-Path $repo 'dist'
    $stage = Join-Path $dist $name
    if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
    New-Item -ItemType Directory -Force $stage | Out-Null
    foreach ($p in $programs) { Copy-Item -LiteralPath (Join-Path $release $p) -Destination $stage }
    Copy-Item -LiteralPath 'third_party\webview2\WebView2Loader.x64.dll' -Destination (Join-Path $stage 'WebView2Loader.dll')
    Copy-Item -LiteralPath 'third_party\webview2\LICENSE.txt' -Destination (Join-Path $stage 'LICENSE-WebView2Loader.txt')
    Copy-Item -LiteralPath 'LICENSE' -Destination $stage
    Copy-Item -LiteralPath 'NOTICE' -Destination $stage
    Copy-Item -LiteralPath 'docs\mistfences\USER-GUIDE.md' -Destination (Join-Path $stage 'README.md')
    # ASCII only: cmd.exe reads a batch file in the console code page.
    [IO.File]::WriteAllText((Join-Path $stage 'restore-desktop.cmd'), ((
        '@echo off',
        'rem MistFences: quits MistFences and shows the Windows desktop icons again.',
        'rem "restore-desktop.cmd --no-autostart" also turns off its start with Windows.',
        '"%~dp0mistfences-watchdog.exe" --restore %*',
        '') -join "`r`n"), (New-Object Text.ASCIIEncoding))

    # THIRD-PARTY-LICENSES.txt from the exact locked sources.
    $meta = cargo metadata --locked --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'cargo metadata failed' }
    $members = @($meta.workspace_members)
    $packages = $meta.packages | Where-Object { $members -notcontains $_.id } | Sort-Object name, version
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add('MistFences - third-party Rust dependencies')
    $lines.Add('Includes runtime and build/test dependencies from Cargo.lock.')
    $lines.Add('WebView2 Loader has its own LICENSE-WebView2Loader.txt.')
    $lines.Add('')
    $sections = [ordered]@{}
    $sha = [Security.Cryptography.SHA256]::Create()
    foreach ($p in $packages) {
        $label = "$($p.name) $($p.version)"
        $lines.Add("${label}: $(if ($p.license) { $p.license } else { 'see upstream license' })")
        $dir = Split-Path -Parent $p.manifest_path
        $files = @(Get-ChildItem -LiteralPath $dir -File | Where-Object { $_.Name -match '^(license|copying|notice)' })
        if ($p.license_file) { $files += Get-Item -LiteralPath (Join-Path $dir $p.license_file) -ErrorAction SilentlyContinue }
        foreach ($f in ($files | Sort-Object FullName -Unique)) {
            $text = ([IO.File]::ReadAllText($f.FullName)).Trim()
            if (-not $text) { continue }
            $key = [BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text)))
            if (-not $sections.Contains($key)) { $sections[$key] = @{ names = New-Object System.Collections.Generic.List[string]; text = $text } }
            $sections[$key].names.Add("$label / $($f.Name)")
        }
    }
    foreach ($s in $sections.Values) {
        $lines.Add(''); $lines.Add('=' * 72); $lines.Add(($s.names -join "`n")); $lines.Add('=' * 72); $lines.Add($s.text)
    }
    [IO.File]::WriteAllText((Join-Path $stage 'THIRD-PARTY-LICENSES.txt'), (($lines -join "`n") + "`n"), (New-Object Text.UTF8Encoding($false)))
    Write-Host "Bundled $($sections.Count) licence texts for $(@($packages).Count) dependencies"

    function Write-Checksum([string] $Path) {
        $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        Set-Content -LiteralPath "$Path.sha256" -Value "$hash  $(Split-Path -Leaf $Path)" -Encoding ASCII
        Write-Host "Package: $Path"
        Write-Host "SHA-256: $hash"
    }

    if ($Format -ne 'Installer') {
        $zip = Join-Path $dist "$name.zip"
        if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
        Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
        Write-Checksum $zip
    }
    if ($setup) {
        $installer = Join-Path $dist "$name-setup.exe"
        if (Test-Path -LiteralPath $installer) { Remove-Item -LiteralPath $installer -Force }
        & $Iscc /Q "/DPayloadDir=$stage" "/DAppVersion=$version" "/DRepository=$Repository" "/O$dist" "/F$name-setup" (Join-Path $repo 'packaging\inno\mistfences.iss')
        if ($LASTEXITCODE) { throw "Inno Setup failed ($LASTEXITCODE)" }
        Write-Checksum $installer
    }
} finally {
    Pop-Location
}
