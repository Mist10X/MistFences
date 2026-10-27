<#
.SYNOPSIS
  Drops onto free folders, shortcuts and the Recycle Bin, Windows' own file operations and
  dialogs, the drag image and the label rendering, tested with real mouse input INSIDE Windows
  Sandbox (run-sandbox-tests.ps1 -Script sandbox-folder-tests.ps1). Only test files are used.
#>
$ErrorActionPreference = 'Continue'
$in = 'C:\MF\in'; $out = 'C:\MF\out'; $app = 'C:\MistFences'
$progress = Join-Path $out 'progress.log'
$results = [System.Collections.Generic.List[object]]::new()
if (Test-Path -LiteralPath (Join-Path $out 'done.txt')) { exit }
function Log([string] $t) { Add-Content -LiteralPath $progress -Value ('{0:HH:mm:ss.fff} {1}' -f (Get-Date), $t) -Encoding UTF8 }
function Add-Result([string] $test, [bool] $ok, [string] $detail) {
    $results.Add([pscustomobject]@{ test = $test; passed = $ok; detail = $detail })
    Log ('{0} {1}: {2}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $test, $detail)
}
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class U {
    [DllImport("user32.dll")] static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
    [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
    public static void Move(int x, int y) { int w = GetSystemMetrics(0), h = GetSystemMetrics(1); mouse_event(0x8001, x * 65535 / (w - 1), y * 65535 / (h - 1), 0, IntPtr.Zero); }
    public static void Down() { mouse_event(0x0002, 0, 0, 0, IntPtr.Zero); }
    public static void Up() { mouse_event(0x0004, 0, 0, 0, IntPtr.Zero); }
    public static void Key(byte vk, bool down) { keybd_event(vk, 0, down ? 0u : 2u, IntPtr.Zero); }
    public static List<string> Visible() {
        var list = new List<string>();
        EnumWindows((h, l) => { if (IsWindowVisible(h)) { var c = new StringBuilder(256); GetClassName(h, c, 256); var t = new StringBuilder(256); GetWindowText(h, t, 256); list.Add(c + "|" + t + "|" + h); } return true; }, IntPtr.Zero);
        return list;
    }
}
'@
function MfCli([string[]] $a) { (& (Join-Path $app 'mistfences-cli.exe') @a --compact 2>&1 | ForEach-Object { "$_" }) -join "`n" }
function Json([string[]] $a) { try { (MfCli $a) | ConvertFrom-Json } catch { $null } }
function Wait-Until([scriptblock] $c, [int] $s) { $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.Elapsed.TotalSeconds -lt $s) { if (& $c) { return [int]$sw.Elapsed.TotalMilliseconds }; Start-Sleep -Milliseconds 100 }; -1 }
function Item([string] $file) { @(Json @('item', 'list')) | ForEach-Object { $_ } | Where-Object { $_.path -and [IO.Path]::GetFileName($_.path) -eq $file } | Select-Object -First 1 }
function Center($r) { @([int](($r[0] + $r[2]) / 2), [int]($r[1] + 26)) }
function Drag([int] $x1, [int] $y1, [int] $x2, [int] $y2, [byte] $key = 0) {
    [U]::Move($x1, $y1); Start-Sleep -Milliseconds 250; [U]::Down(); Start-Sleep -Milliseconds 150
    foreach ($i in 1..25) { [U]::Move([int]($x1 + ($x2 - $x1) * $i / 25), [int]($y1 + ($y2 - $y1) * $i / 25)); Start-Sleep -Milliseconds 25 }
    if ($key) { [U]::Key($key, $true); Start-Sleep -Milliseconds 150; [U]::Move($x2 + 1, $y2); Start-Sleep -Milliseconds 150; [U]::Move($x2, $y2); Start-Sleep -Milliseconds 150 }
    Start-Sleep -Milliseconds 300; [U]::Up(); if ($key) { [U]::Key($key, $false) }; Start-Sleep -Milliseconds 1500
}
function Shot([string] $file, [int] $x, [int] $y, [int] $w, [int] $h) { $b = New-Object Drawing.Bitmap $w, $h; $g = [Drawing.Graphics]::FromImage($b); $g.CopyFromScreen($x, $y, 0, 0, (New-Object Drawing.Size $w, $h)); $b.Save((Join-Path $out $file)); $g.Dispose(); $b.Dispose() }
function Free([string] $file) { MfCli @('item', 'move', (Join-Path $desk $file), '--to', 'inbox') | Out-Null }

New-Item -ItemType Directory -Force $out | Out-Null
Log 'folder tests start'
Start-Sleep -Seconds 8
Copy-Item -LiteralPath (Join-Path $in 'app') -Destination $app -Recurse -Force
New-Item -ItemType Directory -Force (Join-Path $app 'config') | Out-Null
$desk = [Environment]::GetFolderPath('Desktop')
$target = 'C:\MF\Цель ярлыка'; New-Item -ItemType Directory -Force $target | Out-Null
$folder = Join-Path $desk 'Папка-цель'; New-Item -ItemType Directory -Force $folder | Out-Null
$denied = Join-Path $desk 'Закрытая'; New-Item -ItemType Directory -Force $denied | Out-Null
& icacls.exe $denied /deny '*S-1-1-0:(OI)(CI)(W)' | Out-Null
'move me' | Set-Content (Join-Path $desk 'файл-для-папки.txt')
'copy me' | Set-Content (Join-Path $desk 'копия.txt')
'link me' | Set-Content (Join-Path $desk 'через ярлык.txt')
'bin me' | Set-Content (Join-Path $desk 'в корзину.txt')
'desk version' | Set-Content (Join-Path $desk 'дубль.txt')
'folder version' | Set-Content (Join-Path $folder 'дубль.txt')
'locked out' | Set-Content (Join-Path $desk 'нельзя.txt')
$ws = New-Object -ComObject WScript.Shell; $lnk = $ws.CreateShortcut((Join-Path $desk 'Ярлык папки.lnk')); $lnk.TargetPath = $target; $lnk.Save()
$src = 'C:\MF\Из проводника'; New-Item -ItemType Directory -Force $src | Out-Null; 'from explorer' | Set-Content (Join-Path $src 'снаружи.txt')

try {
    $env:RUST_LOG = 'info,mistfences::desktop_layer=debug'
    Start-Process -FilePath (Join-Path $app 'mistfences.exe') -ArgumentList '--portable', '--exit-after', '1800000'
    $null = Wait-Until { $s = Json @('status'); $s -and $s.pid } 45; Start-Sleep -Seconds 3
    foreach ($f in 'Папка-цель', 'Закрытая', 'файл-для-папки.txt', 'копия.txt', 'через ярлык.txt', 'в корзину.txt', 'дубль.txt', 'нельзя.txt', 'Ярлык папки.lnk') { Free $f }
    Start-Sleep -Seconds 2
    $grid = Get-Content (Join-Path $env:LOCALAPPDATA 'MistFences\mistfences.log') -Encoding UTF8 | Where-Object { $_ -match 'desktop grid' } | Select-Object -Last 1
    Log "grid: $grid"
    $cells = @(Json @('item', 'list', '--fence', 'inbox')) | ForEach-Object { $_ } | Where-Object deskRect
    $w = $cells[0].deskRect[2] - $cells[0].deskRect[0]; $h = $cells[0].deskRect[3] - $cells[0].deskRect[1]
    # The grid starts where Explorer's own icons start (read from its folder view at start).
    $ox = 0; $oy = 0; if ("$grid" -match 'origin=\(([\d.]+), ([\d.]+)\)') { $ox = [int][double]$matches[1]; $oy = [int][double]$matches[2] }
    $aligned = @($cells | Where-Object { (($_.deskRect[0] - $ox) % $w) -ne 0 -or (($_.deskRect[1] - $oy) % $h) -ne 0 }).Count -eq 0
    Add-Result 'free icons sit on Explorer''s grid' $aligned "cell ${w}x${h}, origin $ox,$oy; $(@($cells).Count) icons"
    Shot 'labels.png' 0 0 460 760

    $fold = Center (Item 'Папка-цель').deskRect
    # 1. Move into a free folder.
    $f1 = Center (Item 'файл-для-папки.txt').deskRect
    Drag $f1[0] $f1[1] $fold[0] $fold[1]
    $ok = (Wait-Until { Test-Path (Join-Path $folder 'файл-для-папки.txt') } 8) -ge 0 -and -not (Test-Path (Join-Path $desk 'файл-для-папки.txt'))
    Add-Result 'drag a file into a free folder moves it' $ok "in folder: $(Test-Path (Join-Path $folder 'файл-для-папки.txt')), still on desktop: $(Test-Path (Join-Path $desk 'файл-для-папки.txt'))"
    # 2. Ctrl: copy.
    $f2 = Center (Item 'копия.txt').deskRect
    Drag $f2[0] $f2[1] $fold[0] $fold[1] 0x11
    $ok = (Wait-Until { Test-Path (Join-Path $folder 'копия.txt') } 8) -ge 0 -and (Test-Path (Join-Path $desk 'копия.txt'))
    Add-Result 'Ctrl+drag copies into the folder' $ok "copy in folder: $(Test-Path (Join-Path $folder 'копия.txt')), original kept: $(Test-Path (Join-Path $desk 'копия.txt'))"
    # 3. Onto a shortcut to a folder.
    $lk = Center (Item 'Ярлык папки.lnk').deskRect
    $f3 = Center (Item 'через ярлык.txt').deskRect
    Drag $f3[0] $f3[1] $lk[0] $lk[1]
    $ok = (Wait-Until { Test-Path (Join-Path $target 'через ярлык.txt') } 8) -ge 0
    Add-Result 'drop on a folder shortcut goes into its folder' $ok "in target: $ok"
    # 4. Recycle Bin.
    $bin = @(Json @('item', 'list', '--fence', 'inbox')) | ForEach-Object { $_ } | Where-Object { $_.kind -eq 'namespace' -and $_.fileName -match 'Корзина|Recycle' } | Select-Object -First 1
    $f4 = Center (Item 'в корзину.txt').deskRect
    $bc = Center $bin.deskRect
    Drag $f4[0] $f4[1] $bc[0] $bc[1]
    $null = Wait-Until { -not (Test-Path (Join-Path $desk 'в корзину.txt')) } 8
    $inBin = @((New-Object -ComObject Shell.Application).Namespace(10).Items() | Where-Object { $_.Name -match 'в корзину' }).Count -gt 0
    Add-Result 'drop on the Recycle Bin recycles' ((-not (Test-Path (Join-Path $desk 'в корзину.txt'))) -and $inBin) "gone from desktop, in the bin: $inBin"
    # 5. A name conflict: Windows asks; Escape leaves both files as they were.
    $before = [U]::Visible()
    $f5 = Center (Item 'дубль.txt').deskRect
    Drag $f5[0] $f5[1] $fold[0] $fold[1]
    Start-Sleep -Seconds 2
    $new = @([U]::Visible() | Where-Object { $before -notcontains $_ })
    [U]::Key(0x1B, $true); [U]::Key(0x1B, $false); Start-Sleep -Seconds 1
    $kept = ((Get-Content (Join-Path $folder 'дубль.txt')) -eq 'folder version') -and (Test-Path (Join-Path $desk 'дубль.txt'))
    $sw = [Diagnostics.Stopwatch]::StartNew(); $alive = Json @('status'); Log "status after the conflict: $([int]$sw.ElapsedMilliseconds) ms, pid $($alive.pid)"
    Add-Result 'name conflict: Windows dialog, nothing lost on cancel' ($new.Count -gt 0 -and $kept) "dialogs: $(($new | ForEach-Object { ($_ -split '\|')[0..1] -join ':' }) -join ', '); both files intact: $kept"
    # 6. A folder without write access: the shell reports it; the file stays.
    $before = [U]::Visible()
    $dn = Center (Item 'Закрытая').deskRect
    $f6 = Center (Item 'нельзя.txt').deskRect
    Drag $f6[0] $f6[1] $dn[0] $dn[1]
    Start-Sleep -Seconds 2
    $new = @([U]::Visible() | Where-Object { $before -notcontains $_ })
    [U]::Key(0x1B, $true); [U]::Key(0x1B, $false); Start-Sleep -Seconds 1
    $stay = (Test-Path (Join-Path $desk 'нельзя.txt')) -and -not (Test-Path (Join-Path $denied 'нельзя.txt'))
    Add-Result 'access denied: reported, file stays' $stay "dialogs: $(($new | ForEach-Object { ($_ -split '\|')[0..1] -join ':' }) -join ', '); stayed: $stay"
    # 7. From an Explorer window into a free folder.
    Start-Process explorer.exe -ArgumentList "`"$src`""; Start-Sleep -Seconds 4
    # Beside the free icons, not over them.
    $ew = (Get-Process explorer | Where-Object { $_.MainWindowTitle -match 'Из проводника' } | Select-Object -First 1).MainWindowHandle
    if ($ew) { [void][U]::SetWindowPos($ew, [IntPtr]::Zero, 760, 80, 700, 500, 0x0040); Start-Sleep -Seconds 1 }
    Log "explorer window $ew"
    $root = [Windows.Automation.AutomationElement]::RootElement
    $cond = New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty), 'снаружи.txt'
    $el = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, $cond)
    if (-not $el) { $cond = New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty), 'снаружи'; $el = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, $cond) }
    if ($el) {
        $r = $el.Current.BoundingRectangle
        Log "explorer item '$($el.Current.Name)' ($($el.Current.ControlType.ProgrammaticName)) at $r"
        Drag ([int]($r.X + 30)) ([int]($r.Y + $r.Height / 2)) $fold[0] $fold[1]
        $ok = (Wait-Until { Test-Path (Join-Path $folder 'снаружи.txt') } 8) -ge 0
        Add-Result 'drag from Explorer into a free folder' $ok "in folder: $ok"
    } else {
        Add-Result 'drag from Explorer into a free folder' $false 'the file was not found in the Explorer window'
    }
    Get-Process explorer | Where-Object { $_.MainWindowTitle } | ForEach-Object { $null = $_.CloseMainWindow() }
    Start-Sleep -Seconds 1
    # 8. Moving free icons still only moves icons (no file operation).
    $files = (Get-ChildItem $desk | Sort-Object Name | ForEach-Object { $_.Name }) -join '|'
    $k = Item 'копия.txt'; $kc = Center $k.deskRect
    Drag $kc[0] $kc[1] ($kc[0] + 160) ($kc[1] + 220)
    $k2 = Item 'копия.txt'
    $same = $files -eq ((Get-ChildItem $desk | Sort-Object Name | ForEach-Object { $_.Name }) -join '|')
    Add-Result 'moving an icon on the desktop touches no file' ($same -and (($k2.deskRect -join ',') -ne ($k.deskRect -join ','))) "moved to $($k2.deskRect -join ','), files same: $same"
} catch {
    Add-Result 'script error' $false $_.Exception.Message
} finally {
    Get-Process mistfences -ErrorAction SilentlyContinue | Stop-Process -Force
    New-Item -ItemType Directory -Force (Join-Path $out 'logs') | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA 'MistFences') -File -ErrorAction SilentlyContinue | Copy-Item -Destination (Join-Path $out 'logs') -Force
    $results | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $out 'results.json') -Encoding UTF8
    Log 'folder tests end'
    Set-Content -LiteralPath (Join-Path $out 'done.txt') -Value 'done' -Encoding ASCII
    shutdown.exe /s /t 10
}
