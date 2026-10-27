<#
.SYNOPSIS
  Builds the MistFences release and packs it for other users: a ZIP to extract and run.
.DESCRIPTION
  1. Builds the release (build.ps1 -Release) unless -SkipBuild.
  2. Checks that the executables carry the workspace version.
  3. Stages the programs, WebView2Loader.dll, the user guide (README.md), LICENSE, NOTICE,
     the WebView2 Loader licence and THIRD-PARTY-LICENSES.txt (every locked Cargo
     dependency with its licence files, as upstream's write-license-notices.py does).
  4. Writes dist\MistFences-v<version>-x64.zip and its SHA-256 next to it.
  Nothing is installed or registered.
.EXAMPLE
  ./scripts/mistfences/package.ps1
#>
param([switch] $SkipBuild)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Push-Location $repo
try {
    $version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
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

    $zip = Join-Path $dist "$name.zip"
    if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
    $hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath "$zip.sha256" -Value "$hash  $name.zip" -Encoding ASCII
    Write-Host "Package: $zip"
    Write-Host "SHA-256: $hash"
} finally {
    Pop-Location
}
