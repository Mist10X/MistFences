# A new user's first start INSIDE Windows Sandbox: the released files run from a plain folder
# with no arguments (settings in %APPDATA%), as someone who downloaded the ZIP would. Checks
# that it starts, hides Windows' icons and shows the desktop's items itself, writes its
# startup entry, shows fences, and that Exit gives Windows' icons back. Writes into C:\MF\out.
$ErrorActionPreference = 'Continue'
$in = 'C:\MF\in'; $out = 'C:\MF\out'; $app = 'C:\Programs\MistFences'
$progress = Join-Path $out 'progress.log'
if (Test-Path -LiteralPath (Join-Path $out 'done.txt')) { exit }
$results = New-Object System.Collections.Generic.List[object]
function Log([string] $t) { Add-Content -LiteralPath $progress -Value ('{0:HH:mm:ss.fff} {1}' -f (Get-Date), $t) -Encoding UTF8 }
function Add-Result([string] $test, [bool] $ok, [string] $detail) {
    $results.Add([pscustomobject]@{ test = $test; passed = $ok; detail = $detail })
    Log ('{0} {1}: {2}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $test, $detail)
}
function MfCli([string[]] $a) { (& (Join-Path $app 'mistfences-cli.exe') @a --compact 2>&1 | ForEach-Object { "$_" }) -join "`n" }
function Json([string[]] $a) { try { (MfCli $a) | ConvertFrom-Json } catch { $null } }
function IconsHidden { (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name HideIcons -ErrorAction SilentlyContinue).HideIcons -eq 1 }
Add-Type -AssemblyName System.Drawing

New-Item -ItemType Directory -Force $out | Out-Null
Start-Sleep -Seconds 8
New-Item -ItemType Directory -Force $app | Out-Null
Copy-Item -Path (Join-Path $in 'app\*') -Destination $app -Recurse -Force
$desk = [Environment]::GetFolderPath('Desktop')
'n' | Set-Content (Join-Path $desk 'Заметки.txt')
New-Item -ItemType Directory -Force (Join-Path $desk 'Проекты') | Out-Null
try {
    Add-Result 'nothing hidden before' (-not (IconsHidden)) "HideIcons=$((Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced').HideIcons)"
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Start-Process (Join-Path $app 'mistfences.exe')
    $status = $null
    while ($sw.Elapsed.TotalSeconds -lt 45 -and -not $status) { Start-Sleep -Milliseconds 300; $status = Json @('status'); if ($status -and -not $status.pid) { $status = $null } }
    Add-Result 'starts from a plain folder' ($null -ne $status) "ready after $([int]$sw.Elapsed.TotalMilliseconds) ms; version $($status.appVersion); config $($status.configPath)"
    Start-Sleep -Seconds 4
    Add-Result 'settings in %APPDATA%' ("$($status.configPath)" -like "$env:APPDATA*") "$($status.configPath)"
    Add-Result 'Windows icons hidden, MistFences shows them' ((IconsHidden) -and $status.desktopIconsHidden) "HideIcons: $(IconsHidden), status: $($status.desktopIconsHidden)"
    $items = @(Json @('item', 'list')) | ForEach-Object { $_ }
    $shown = @($items | Where-Object { $_.name -like 'Заметки*' -or $_.name -eq 'Проекты' })
    Add-Result 'desktop items are there' ($shown.Count -eq 2) (($shown | ForEach-Object { "$($_.name) in '$($_.fenceTitle)' $($_.deskRect -join ',')" }) -join '; ')
    $fences = @(Json @('fence', 'list')) | ForEach-Object { $_ }
    Add-Result 'fences shown' (@($fences | Where-Object { $_.kind -ne 'inbox' }).Count -ge 1) (($fences | ForEach-Object { "$($_.title) ($($_.kind))" }) -join ', ')
    $run = (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name MistFences -ErrorAction SilentlyContinue).MistFences
    Add-Result 'starts with Windows' ("$run" -like "*$app\mistfences.exe*") "Run entry: $run"
    Add-Type -AssemblyName System.Windows.Forms; $sb = [Windows.Forms.Screen]::PrimaryScreen.Bounds; $b = New-Object Drawing.Bitmap $sb.Width, $sb.Height; $g = [Drawing.Graphics]::FromImage($b); $g.CopyFromScreen(0, 0, 0, 0, $sb.Size); $b.Save((Join-Path $out 'firstrun.png')); $g.Dispose(); $b.Dispose()
    MfCli @('quit') | Out-Null
    $sw.Restart(); while ($sw.Elapsed.TotalSeconds -lt 15 -and (Get-Process mistfences -ErrorAction SilentlyContinue)) { Start-Sleep -Milliseconds 200 }
    Start-Sleep -Seconds 1
    Add-Result 'Exit gives Windows its icons back' (-not (IconsHidden) -and -not (Get-Process mistfences -ErrorAction SilentlyContinue)) "HideIcons: $(IconsHidden)"
    Add-Result 'files untouched' ((Test-Path (Join-Path $desk 'Заметки.txt')) -and (Test-Path (Join-Path $desk 'Проекты'))) 'Заметки.txt, Проекты'
} catch {
    Add-Result 'script error' $false $_.Exception.Message
} finally {
    Get-Process mistfences -ErrorAction SilentlyContinue | Stop-Process -Force
    $results | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $out 'results.json') -Encoding UTF8
    Set-Content -LiteralPath (Join-Path $out 'done.txt') -Value 'done'
    Log 'first run tests end'
    shutdown.exe /s /t 5
}
