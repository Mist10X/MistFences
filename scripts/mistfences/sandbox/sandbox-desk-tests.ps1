<#
.SYNOPSIS
  Free desktop icons and instant hover peeks, tested with real mouse input INSIDE Windows
  Sandbox (run-sandbox-tests.ps1 -Script sandbox-desk-tests.ps1). Never on a real desktop: it
  drags icons, opens menus and folders, deletes a fence and moves the mouse by itself.
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

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class U {
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern void mouse_event(uint f, int dx, int dy, uint data, IntPtr extra);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
    public static void Move(int x, int y) {
        int w = GetSystemMetrics(0), h = GetSystemMetrics(1);
        mouse_event(0x8001, x * 65535 / (w - 1), y * 65535 / (h - 1), 0, IntPtr.Zero);
    }
    public static void Down() { mouse_event(0x0002, 0, 0, 0, IntPtr.Zero); }
    public static void Up() { mouse_event(0x0004, 0, 0, 0, IntPtr.Zero); }
    public static void RDown() { mouse_event(0x0008, 0, 0, 0, IntPtr.Zero); }
    public static void RUp() { mouse_event(0x0010, 0, 0, 0, IntPtr.Zero); }
    public static void Esc() { keybd_event(0x1B, 0, 0, IntPtr.Zero); keybd_event(0x1B, 0, 2, IntPtr.Zero); }
    public static string ClassAt(int x, int y) {
        var p = new POINT { X = x, Y = y };
        IntPtr h = GetAncestor(WindowFromPoint(p), 2);
        var sb = new StringBuilder(256); GetClassName(h, sb, 256); return sb.ToString();
    }
    public static bool MenuOpen() { IntPtr m = FindWindow("#32768", null); return m != IntPtr.Zero && IsWindowVisible(m); }
    public static List<int[]> Windows(string cls) {
        var list = new List<int[]>();
        EnumWindows((h, l) => {
            var sb = new StringBuilder(256); GetClassName(h, sb, 256);
            if (sb.ToString() == cls && IsWindowVisible(h)) { RECT r; GetWindowRect(h, out r); list.Add(new[] { r.L, r.T, r.R, r.B, (int)h }); }
            return true; }, IntPtr.Zero);
        return list;
    }
    public static List<string> Visible() {
        var list = new List<string>();
        EnumWindows((h, l) => {
            if (IsWindowVisible(h)) { var sb = new StringBuilder(256); GetClassName(h, sb, 256); list.Add(sb.ToString() + "|" + h); }
            return true; }, IntPtr.Zero);
        return list;
    }
    public static void Close(int hwnd) { PostMessage((IntPtr)hwnd, 0x0010, IntPtr.Zero, IntPtr.Zero); }
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    public static uint Owner(int hwnd) { uint pid; GetWindowThreadProcessId((IntPtr)hwnd, out pid); return pid; }
    public static void Key(byte vk) { keybd_event(vk, 0, 0, IntPtr.Zero); keybd_event(vk, 0, 2, IntPtr.Zero); }
    public static void Ctrl(byte vk) { keybd_event(0x11, 0, 0, IntPtr.Zero); keybd_event(vk, 0, 0, IntPtr.Zero); keybd_event(vk, 0, 2, IntPtr.Zero); keybd_event(0x11, 0, 2, IntPtr.Zero); }
    public static string ForegroundClass() { var sb = new StringBuilder(256); GetClassName(GetForegroundWindow(), sb, 256); return sb.ToString(); }
    [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] static extern int GetMenuItemCount(IntPtr m);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetMenuString(IntPtr m, uint id, StringBuilder s, int n, uint f);
    [DllImport("user32.dll")] static extern int GetWindowLong(IntPtr h, int i);
    [StructLayout(LayoutKind.Sequential)] public struct GUITHREADINFO { public int cbSize, flags; public IntPtr hwndActive, hwndFocus, hwndCapture, hwndMenuOwner, hwndMoveSize, hwndCaret; public RECT rcCaret; }
    [DllImport("user32.dll")] static extern bool GetGUIThreadInfo(uint tid, ref GUITHREADINFO g);
    // The items of the open popup menu (#32768), whichever process owns it.
    public static string MenuText() {
        IntPtr w = FindWindow("#32768", null); if (w == IntPtr.Zero) return "";
        IntPtr m = SendMessage(w, 0x01E1, IntPtr.Zero, IntPtr.Zero); if (m == IntPtr.Zero) return "";
        var parts = new List<string>();
        for (int i = 0; i < GetMenuItemCount(m); i++) { var sb = new StringBuilder(256); GetMenuString(m, (uint)i, sb, 256, 0x400); if (sb.Length > 0) parts.Add(sb.ToString()); }
        return string.Join(" | ", parts);
    }
    // The focused window of the foreground thread: class, window rect and styles.
    public static string Focused() {
        var g = new GUITHREADINFO(); g.cbSize = Marshal.SizeOf(g); uint pid;
        uint tid = GetWindowThreadProcessId(GetForegroundWindow(), out pid);
        if (!GetGUIThreadInfo(tid, ref g) || g.hwndFocus == IntPtr.Zero) return "none";
        var sb = new StringBuilder(256); GetClassName(g.hwndFocus, sb, 256); RECT r; GetWindowRect(g.hwndFocus, out r);
        return string.Format("{0} {1},{2},{3},{4} style=0x{5:X8}", sb, r.L, r.T, r.R, r.B, GetWindowLong(g.hwndFocus, -16));
    }
    public static void Type(string latin) { foreach (char c in latin.ToUpperInvariant()) { keybd_event((byte)c, 0, 0, IntPtr.Zero); keybd_event((byte)c, 0, 2, IntPtr.Zero); System.Threading.Thread.Sleep(30); } }
}
'@

function MfCli([string[]] $a) { (& (Join-Path $app 'mistfences-cli.exe') @a --compact 2>&1 | ForEach-Object { "$_" }) -join "`n" }
function Json([string[]] $a) { try { (MfCli $a) | ConvertFrom-Json } catch { $null } }
function Wait-Until([scriptblock] $c, [int] $s) { $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.Elapsed.TotalSeconds -lt $s) { if (& $c) { return [int]$sw.Elapsed.TotalMilliseconds }; Start-Sleep -Milliseconds 100 }; -1 }
function Start-Mf { Start-Process -FilePath (Join-Path $app 'mistfences.exe') -ArgumentList '--portable', '--exit-after', '1800000' -PassThru }
function Ready { Wait-Until { $s = Json @('status'); $s -and $s.pid } 45 }
function Items([string] $fence) { @(Json @('item', 'list', '--fence', $fence)) | ForEach-Object { $_ } }
function Item([string] $name) { @(Json @('item', 'list')) | ForEach-Object { $_ } | Where-Object { $_.path -and [IO.Path]::GetFileNameWithoutExtension($_.path) -eq $name } | Select-Object -First 1 }
# The middle of the icon of a desk cell (the cell's centre falls between icon and label).
function Center($r) { @([int][Math]::Floor(($r[0] + $r[2]) / 2), ($r[1] + 26)) }
function Drag([int] $x1, [int] $y1, [int] $x2, [int] $y2) {
    [U]::Move($x1, $y1); Start-Sleep -Milliseconds 250; [U]::Down(); Start-Sleep -Milliseconds 150
    foreach ($i in 1..25) { [U]::Move([int]($x1 + ($x2 - $x1) * $i / 25), [int]($y1 + ($y2 - $y1) * $i / 25)); Start-Sleep -Milliseconds 25 }
    Start-Sleep -Milliseconds 300; [U]::Up(); Start-Sleep -Milliseconds 1200
}
function Fences { @(Json @('fence', 'list')) | ForEach-Object { $_ } }
function FenceRect($f) { $r = $f.windowRect; @($r.x, $r.y, ($r.x + $r.w), ($r.y + $r.h)) }

New-Item -ItemType Directory -Force $out | Out-Null
Log 'desk tests start'
Start-Sleep -Seconds 8
Copy-Item -LiteralPath (Join-Path $in 'app') -Destination $app -Recurse -Force
New-Item -ItemType Directory -Force (Join-Path $app 'config') | Out-Null
$desk = [Environment]::GetFolderPath('Desktop')
'a' | Set-Content (Join-Path $desk 'alpha.txt'); 'b' | Set-Content (Join-Path $desk 'beta.txt'); 'g' | Set-Content (Join-Path $desk 'gamma.txt')
New-Item -ItemType Directory -Force (Join-Path $desk 'deltaDir') | Out-Null
$filesBefore = (Get-ChildItem $desk | Sort-Object Name | ForEach-Object { $_.Name }) -join '|'

try {
    $p = Start-Mf; $null = Ready; Start-Sleep -Seconds 3
    foreach ($n in 'alpha.txt', 'beta.txt') { MfCli @('item', 'move', (Join-Path $desk $n), '--to', 'inbox') | Out-Null }
    MfCli @('item', 'move', (Join-Path $desk 'deltaDir'), '--to', 'inbox') | Out-Null
    Start-Sleep -Seconds 2
    $alpha = Item 'alpha'; $beta = Item 'beta'; $delta = Item 'deltaDir'
    $layers = [U]::Windows('MistFences.DesktopLayer')
    Add-Result 'free icons shown on the desktop' ($alpha.deskRect -and $beta.deskRect -and $delta.deskRect -and $layers.Count -ge 1) "alpha $($alpha.deskRect -join ','), beta $($beta.deskRect -join ','), layer windows $($layers.Count)"
    $regular = @(Fences | Where-Object { $_.kind -ne 'inbox' }).Count
    $fenceWindows = @([U]::Windows('MistFences.Fence')).Count
    Add-Result 'no visible desktop group' ($fenceWindows -eq $regular) "fence windows $fenceWindows, regular fences $regular"

    $c = Center $alpha.deskRect
    Add-Result 'a free icon takes the mouse' ([U]::ClassAt($c[0], $c[1]) -eq 'MistFences.DesktopLayer') "class at alpha: $([U]::ClassAt($c[0], $c[1]))"

    # Move alpha by mouse.
    $target = @(($c[0] + 380), ($c[1] + 300))
    Drag $c[0] $c[1] $target[0] $target[1]
    $alpha2 = Item 'alpha'; $c2 = Center $alpha2.deskRect
    $ok = $alpha2.fence -eq $alpha.fence -and [Math]::Abs($c2[0] - $target[0]) -le 75 -and [Math]::Abs($c2[1] - $target[1]) -le 99
    Add-Result 'drag a free icon to another place' $ok "target $($target -join ','), now at $($c2 -join ',')"

    # Empty pixels between free icons belong to the desktop.
    $layers = [U]::Windows('MistFences.DesktopLayer'); $lr = $layers[0]
    $gap = @([int](($c2[0] + (Center $beta.deskRect)[0]) / 2), [int](($c2[1] + (Center $beta.deskRect)[1]) / 2))
    $inCell = { param($r, $pt) $pt[0] -ge $r[0] -and $pt[0] -lt $r[2] -and $pt[1] -ge $r[1] -and $pt[1] -lt $r[3] }
    $gapFree = -not (& $inCell $alpha2.deskRect $gap) -and -not (& $inCell $beta.deskRect $gap) -and -not (& $inCell $delta.deskRect $gap)
    $cls = [U]::ClassAt($gap[0], $gap[1])
    Add-Result 'empty desktop between icons is click-through' ($gapFree -and $cls -ne 'MistFences.DesktopLayer') "layer $($lr[0..3] -join ','), point $($gap -join ',') -> $cls"
    $shown = [U]::Visible()
    [U]::Move($gap[0], $gap[1]); Start-Sleep -Milliseconds 200; [U]::RDown(); [U]::RUp(); Start-Sleep -Milliseconds 1500
    $new = @([U]::Visible() | Where-Object { $shown -notcontains $_ })
    $menu = [U]::MenuOpen() -or @($new | Where-Object { $_ -match '^(#32768|Microsoft.UI.Content.PopupWindowSiteBridge|Xaml_WindowedPopupClass|PopupHost)' }).Count -gt 0
    [U]::Esc(); Start-Sleep -Milliseconds 700
    Add-Result 'desktop context menu on empty space' $menu "new windows: $(($new | ForEach-Object { ($_ -split '\|')[0] }) -join ', ')"

    [U]::Move($c2[0], $c2[1]); Start-Sleep -Milliseconds 200; [U]::RDown(); [U]::RUp(); Start-Sleep -Milliseconds 1500
    $menu = [U]::MenuOpen(); [U]::Esc(); Start-Sleep -Milliseconds 500
    Add-Result 'item context menu on a free icon' $menu "menu open: $menu"

    $dc = Center $delta.deskRect
    $before = @([U]::Windows('CabinetWClass')).Count
    [U]::Move($dc[0], $dc[1]); Start-Sleep -Milliseconds 200; [U]::Down(); [U]::Up(); Start-Sleep -Milliseconds 80; [U]::Down(); [U]::Up()
    $opened = Wait-Until { @([U]::Windows('CabinetWClass')).Count -gt $before } 10
    Add-Result 'double-click opens a free folder' ($opened -ge 0) "Explorer window after $opened ms"
    foreach ($w in [U]::Windows('CabinetWClass')) { [U]::Close($w[4]) }
    Start-Sleep -Seconds 1

    # From a fence to the desktop.
    $gamma = Item 'gamma'
    $gf = Fences | Where-Object id -eq $gamma.fence
    $fr = FenceRect $gf
    $cols = [Math]::Max(1, [Math]::Floor(($fr[2] - $fr[0]) / 80)); $pitch = ($fr[2] - $fr[0]) / $cols
    $from = @([int]($fr[0] + $pitch / 2), [int]($fr[1] + 36 + 8 + 8 + 24))
    $to = @(($c2[0] - 300), ($c2[1]))
    Drag $from[0] $from[1] $to[0] $to[1]
    $gamma2 = Item 'gamma'
    $inbox = (Fences | Where-Object kind -eq 'inbox').id
    $gc = if ($gamma2.deskRect) { Center $gamma2.deskRect } else { @(0, 0) }
    Add-Result 'drag from a fence onto the desktop frees the icon' ($gamma2.fence -eq $inbox -and [Math]::Abs($gc[0] - $to[0]) -le 75 -and [Math]::Abs($gc[1] - $to[1]) -le 99) "from $($from -join ',') in '$($gf.title)' to $($to -join ','): fence $($gamma2.fenceTitle), at $($gc -join ',')"

    # From the desktop into a fence.
    $target = Fences | Where-Object { $_.kind -eq 'virtual' } | Select-Object -First 1
    $tc = Center (FenceRect $target)
    $bc = Center $beta.deskRect
    Drag $bc[0] $bc[1] $tc[0] $tc[1]
    $beta2 = Item 'beta'
    Add-Result 'drag a free icon into a fence' ($beta2.fence -eq $target.id -and -not $beta2.deskRect) "beta now in '$($beta2.fenceTitle)'"

    # Deleting that fence leaves its icons on the desktop.
    MfCli @('fence', 'delete', $target.id) | Out-Null; Start-Sleep -Seconds 2
    $beta3 = Item 'beta'
    $gone = -not (Fences | Where-Object id -eq $target.id)
    Add-Result 'deleting a fence frees its icons' ($gone -and $beta3.fence -eq $inbox -and $beta3.deskRect) "fence gone=$gone, beta at $($beta3.deskRect -join ',')"

    # Places survive a restart.
    $list = @(Items 'inbox'); Log "inbox items $($list.Count), with a desk cell $(@($list | Where-Object deskRect).Count)"
    $before = @(Items 'inbox' | Where-Object deskRect | ForEach-Object { "$($_.name)=$($_.deskRect -join ',')" } | Sort-Object) -join ';'
    MfCli @('quit') | Out-Null; $null = Wait-Until { -not (Get-Process mistfences -ErrorAction SilentlyContinue) } 20
    $p = Start-Mf; $null = Ready
    # No change on the desktop after the start: the icons must show by themselves.
    $null = Wait-Until { @(Items 'inbox' | Where-Object deskRect).Count -gt 0 } 15
    $after = @(Items 'inbox' | Where-Object deskRect | ForEach-Object { "$($_.name)=$($_.deskRect -join ',')" } | Sort-Object) -join ';'
    Add-Result 'free icon places survive a restart' ($before -eq $after -and $before) "before: $before / after: $after"

    # Instant hover: start of the roll after the pointer arrives / leaves.
    $f = Fences | Where-Object { $_.kind -eq 'virtual' } | Select-Object -First 1
    MfCli @('fence', 'roll', $f.id) | Out-Null; Start-Sleep -Seconds 1
    $r = FenceRect (Fences | Where-Object id -eq $f.id)
    $h0 = $r[3] - $r[1]
    $far = @(5, ($r[3] + 400))
    [U]::Move($far[0], $far[1]); Start-Sleep -Milliseconds 500
    $sw = [Diagnostics.Stopwatch]::StartNew()
    [U]::Move([int](($r[0] + $r[2]) / 2), ($r[1] + 12))
    $open = -1
    while ($sw.ElapsedMilliseconds -lt 1500) { $g = [U]::Windows('MistFences.Fence') | Where-Object { $_[0] -eq $r[0] -and $_[1] -eq $r[1] } | Select-Object -First 1; if ($g -and ($g[3] - $g[1]) -gt $h0) { $open = $sw.ElapsedMilliseconds; break } }
    Start-Sleep -Milliseconds 600
    $g = [U]::Windows('MistFences.Fence') | Where-Object { $_[0] -eq $r[0] -and $_[1] -eq $r[1] } | Select-Object -First 1; $hOpen = $g[3] - $g[1]
    $sw.Restart(); [U]::Move($far[0], $far[1])
    $close = -1
    while ($sw.ElapsedMilliseconds -lt 1500) { $g = [U]::Windows('MistFences.Fence') | Where-Object { $_[0] -eq $r[0] -and $_[1] -eq $r[1] } | Select-Object -First 1; if ($g -and ($g[3] - $g[1]) -lt $hOpen) { $close = $sw.ElapsedMilliseconds; break } }
    Add-Result 'hover opens at once' ($open -ge 0 -and $open -le 120) "roll started $open ms after the pointer reached the title"
    Add-Result 'leaving closes at once' ($close -ge 0 -and $close -le 120) "roll back started $close ms after the pointer left"

    # Back over the title while it rolls up: it opens again and stays open.
    Start-Sleep -Milliseconds 600
    [U]::Move([int](($r[0] + $r[2]) / 2), ($r[1] + 12)); Start-Sleep -Milliseconds 700
    [U]::Move($far[0], $far[1]); Start-Sleep -Milliseconds 60
    [U]::Move([int](($r[0] + $r[2]) / 2), ($r[1] + 12)); Start-Sleep -Milliseconds 900
    $g = [U]::Windows('MistFences.Fence') | Where-Object { $_[0] -eq $r[0] -and $_[1] -eq $r[1] } | Select-Object -First 1
    Add-Result 'returning mid-collapse reopens it' (($g[3] - $g[1]) -ge $hOpen - 2) "height $($g[3] - $g[1]) of $hOpen"
    [U]::Move($far[0], $far[1]); Start-Sleep -Milliseconds 900

    $filesAfter = (Get-ChildItem $desk | Sort-Object Name | ForEach-Object { $_.Name }) -join '|'
    Add-Result 'no file moved or deleted' ($filesBefore -eq $filesAfter) $filesAfter

    # A bare spot with room around it: off every fence and free icon.
    $fr = @(Fences | Where-Object windowRect | ForEach-Object { FenceRect $_ })
    $icons = @(@(Json @('item', 'list', '--fence', 'inbox')) | ForEach-Object { $_ } | Where-Object deskRect | ForEach-Object { $_.deskRect })
    $bare = $null
    foreach ($y in 380, 560, 740) { foreach ($x in 500, 800, 1100, 1400) {
        if ($bare) { continue }
        $box = @(($x - 20), ($y - 20), ($x + 260), ($y + 200))
        $hit = @($fr + $icons | Where-Object { $_[0] -lt $box[2] -and $_[2] -gt $box[0] -and $_[1] -lt $box[3] -and $_[3] -gt $box[1] }).Count
        if (-not $hit) { $bare = @($x, $y) } } }
    Log "bare spot $($bare -join ',')"
    $menus = {
        $new = @([U]::Visible() | Where-Object { $shown -notcontains $_ })
        @($new | Where-Object { $_ -match '^(#32768|Microsoft.UI.Content.PopupWindowSiteBridge|Xaml_WindowedPopupClass|PopupHost)' } | ForEach-Object {
            $h = [int](($_ -split '\|')[1]); "$(($_ -split '\|')[0]):$([U]::Owner($h))" })
    }
    $mfPid = (Get-Process mistfences | Select-Object -First 1).Id

    # Right-button rectangle: MistFences offers a new fence, Explorer's menu stays shut.
    $shown = [U]::Visible()
    [U]::Move($bare[0], $bare[1]); Start-Sleep -Milliseconds 250; [U]::RDown(); Start-Sleep -Milliseconds 150
    foreach ($i in 1..20) { [U]::Move(($bare[0] + 12 * $i), ($bare[1] + 8 * $i)); Start-Sleep -Milliseconds 25 }
    Start-Sleep -Milliseconds 300; [U]::RUp(); Start-Sleep -Milliseconds 1500
    $m = @(& $menus)
    $ours = @($m | Where-Object { $_ -like "#32768:$mfPid" }).Count; $theirs = @($m | Where-Object { $_ -notlike "*:$mfPid" }).Count
    [U]::Esc(); Start-Sleep -Milliseconds 700
    Add-Result 'right-button rectangle offers a new fence' ($ours -ge 1 -and $theirs -eq 0) "menus: $($m -join ', ') (MistFences pid $mfPid)"

    # A plain right click there still opens Explorer's desktop menu.
    $shown = [U]::Visible()
    [U]::Move($bare[0], $bare[1]); Start-Sleep -Milliseconds 250; [U]::RDown(); Start-Sleep -Milliseconds 80; [U]::RUp(); Start-Sleep -Milliseconds 1500
    $m = @(& $menus)
    [U]::Esc(); Start-Sleep -Milliseconds 700
    Add-Result 'plain right click: Explorer menu' (@($m | Where-Object { $_ -notlike "*:$mfPid" }).Count -ge 1) "menus: $($m -join ', ')"

    # A left-button rectangle on the bare desktop only selects: no offer.
    $shown = [U]::Visible()
    Drag $bare[0] $bare[1] ($bare[0] + 240) ($bare[1] + 160)
    $m = @(& $menus)
    if ($m) { [U]::Esc(); Start-Sleep -Milliseconds 700 }
    Add-Result 'left-button rectangle offers nothing' ($m.Count -eq 0) "menus: $($m -join ', ')"

    # Delete on a selected free icon: to the Recycle Bin, as on the desktop.
    'd' | Set-Content (Join-Path $desk 'удалить меня.txt')
    Start-Sleep -Seconds 2
    MfCli @('item', 'move', (Join-Path $desk 'удалить меня.txt'), '--to', 'inbox') | Out-Null
    Start-Sleep -Seconds 2
    $victim = Item 'удалить меня'
    $vc = Center $victim.deskRect
    [U]::Move($vc[0], $vc[1]); Start-Sleep -Milliseconds 250; [U]::Down(); Start-Sleep -Milliseconds 80; [U]::Up(); Start-Sleep -Milliseconds 500
    [U]::Key(0x2E); Start-Sleep -Seconds 3

    # Taking the keyboard does not lift the icons over a window that covers part of one.
    $cover = Item 'gamma'; $cr = $cover.deskRect
    $probe = Start-Process powershell.exe -PassThru -ArgumentList '-NoProfile', '-Command', ("Add-Type -AssemblyName System.Windows.Forms; `$f = New-Object Windows.Forms.Form; `$f.Text = 'MF cover'; `$f.StartPosition = 'Manual'; `$f.FormBorderStyle = 'None'; `$f.Left = {0}; `$f.Top = {1}; `$f.Width = 400; `$f.Height = 300; `$f.TopMost = `$false; [Windows.Forms.Application]::Run(`$f)" -f ($cr[0] + 40), ($cr[1] - 20))
    Start-Sleep -Seconds 4
    [U]::Move(($cr[0] + 15), ($cr[1] + 26)); Start-Sleep -Milliseconds 250; [U]::Down(); Start-Sleep -Milliseconds 80; [U]::Up(); Start-Sleep -Milliseconds 600
    $under = [U]::ClassAt(($cr[0] + 55), ($cr[1] + 26))
    Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue
    Add-Result 'a clicked icon stays under the window covering it' ($under -like 'WindowsForms*') "class over the covered half: $under"

    # Rename with F2 in Explorer's desktop box: the file, the label and dragging follow.
    'r' | Set-Content (Join-Path $desk 'переименуй.txt')
    Start-Sleep -Seconds 2
    MfCli @('item', 'move', (Join-Path $desk 'переименуй.txt'), '--to', 'inbox') | Out-Null
    Start-Sleep -Seconds 2
    $ren = Center (Item 'переименуй').deskRect
    Drag $ren[0] $ren[1] ($bare[0] + 140) ($bare[1] + 60)
    $rr = (Item 'переименуй').deskRect; $rc = Center $rr
    [U]::Move($rc[0], $rc[1]); Start-Sleep -Milliseconds 250; [U]::Down(); Start-Sleep -Milliseconds 80; [U]::Up(); Start-Sleep -Milliseconds 500
    [U]::Key(0x71); Start-Sleep -Milliseconds 900
    $box = [U]::Focused()
    [U]::Type('2026'); Start-Sleep -Milliseconds 200; [U]::Key(0x0D); Start-Sleep -Seconds 3
    $renamed = (Test-Path (Join-Path $desk '2026.txt')) -and -not (Test-Path (Join-Path $desk 'переименуй.txt'))
    $after = Item '2026'
    # Explorer's box: a multi-line, centred EDIT (ES_MULTILINE | ES_CENTER), 80 px wide on a 75 px cell.
    $boxOk = $box -match '^Edit (-?\d+),(-?\d+),(-?\d+),(-?\d+) style=0x([0-9A-F]+)' -and (([Convert]::ToInt32($matches[5], 16) -band 0x5) -eq 0x5) -and ([int]$matches[3] - [int]$matches[1]) -eq 80
    Add-Result 'F2 opens Explorer''s rename box' $boxOk $box
    Add-Result 'rename changes the file and the free icon' ($renamed -and $after -and $after.deskRect) "file renamed: $renamed, icon: $($after.name) at $($after.deskRect -join ',')"
    if ($after) {
        $ac = Center $after.deskRect
        Drag $ac[0] $ac[1] ($ac[0] - 120) ($ac[1] + 110)
        $moved = Item '2026'
        Add-Result 'a renamed icon still drags' (($moved.deskRect -join ',') -ne ($after.deskRect -join ',')) "from $($after.deskRect -join ',') to $($moved.deskRect -join ',')"
        $after = $moved
    }

    # A free icon's menu is Explorer's own desktop menu: no MistFences items (a folder: it has
    # 「Открыть」 in the sandbox too).
    $ac = Center (Item 'deltaDir').deskRect
    [U]::Move($ac[0], $ac[1]); Start-Sleep -Milliseconds 250; [U]::RDown(); Start-Sleep -Milliseconds 80; [U]::RUp(); Start-Sleep -Milliseconds 1500
    $menuText = [U]::MenuText().Replace('&', '')
    [U]::Esc(); Start-Sleep -Milliseconds 700
    $native = $menuText -match '(Открыть|Open)' -and $menuText -match '(Свойства|Properties)' -and $menuText -notmatch '(栅栏|области|fence)'
    Add-Result 'free icon menu is Explorer''s' $native $menuText

    # A right-button rectangle over it selects it and opens that menu (Windows).
    $ar = $after.deskRect
    $from = @(($ar[0] - 25), ($ar[1] - 12)); $to = @(($ar[2] + 20), ($ar[3] - 30))
    [U]::Move($from[0], $from[1]); Start-Sleep -Milliseconds 250; [U]::RDown(); Start-Sleep -Milliseconds 150
    foreach ($i in 1..20) { [U]::Move([int]($from[0] + ($to[0] - $from[0]) * $i / 20), [int]($from[1] + ($to[1] - $from[1]) * $i / 20)); Start-Sleep -Milliseconds 25 }
    Start-Sleep -Milliseconds 300; [U]::RUp(); Start-Sleep -Milliseconds 1500
    $menuText = [U]::MenuText().Replace('&', '')
    [U]::Esc(); Start-Sleep -Milliseconds 700
    Add-Result 'right-button rectangle selects icons and opens their menu' ($menuText -match '(Свойства|Properties)') $menuText

    # Delete after a left-button rectangle selected it.
    [U]::Move(($bare[0] + 600), ($bare[1] + 300)); Start-Sleep -Milliseconds 200; [U]::Down(); [U]::Up(); Start-Sleep -Milliseconds 500
    Drag $from[0] $from[1] $to[0] $to[1]
    [U]::Key(0x2E); Start-Sleep -Seconds 3
    $gone = -not (Test-Path (Join-Path $desk '2026.txt'))
    Add-Result 'Delete after a rectangle selection' $gone "2026.txt gone: $gone, foreground: $([U]::ForegroundClass())"

    # Ctrl+C on a free icon puts the file on the clipboard as Explorer does (Telegram, a browser
    # or a folder pastes it); Ctrl+V pastes the clipboard's files onto the desktop.
    Add-Type -AssemblyName System.Windows.Forms
    'c' | Set-Content (Join-Path $desk 'скопируй.txt')
    Start-Sleep -Seconds 2
    MfCli @('item', 'move', (Join-Path $desk 'скопируй.txt'), '--to', 'inbox') | Out-Null
    Start-Sleep -Seconds 2
    $cp = Center (Item 'скопируй').deskRect
    [Windows.Forms.Clipboard]::Clear()
    [U]::Move($cp[0], $cp[1]); Start-Sleep -Milliseconds 250; [U]::Down(); Start-Sleep -Milliseconds 80; [U]::Up(); Start-Sleep -Milliseconds 500
    [U]::Ctrl(0x43); Start-Sleep -Milliseconds 1500
    $list = @([Windows.Forms.Clipboard]::GetFileDropList())
    Add-Result 'Ctrl+C copies a free icon''s file' ($list -contains (Join-Path $desk 'скопируй.txt')) "clipboard files: $($list -join '; ')"

    $src = 'C:\MF\paste-source'; New-Item -ItemType Directory -Force $src | Out-Null
    'p' | Set-Content (Join-Path $src 'вставка.txt')
    $drop = New-Object System.Collections.Specialized.StringCollection; [void]$drop.Add((Join-Path $src 'вставка.txt'))
    [Windows.Forms.Clipboard]::SetFileDropList($drop)
    [U]::Move($cp[0], $cp[1]); Start-Sleep -Milliseconds 250; [U]::Down(); Start-Sleep -Milliseconds 80; [U]::Up(); Start-Sleep -Milliseconds 500
    [U]::Ctrl(0x56); Start-Sleep -Seconds 4
    $pasted = Test-Path (Join-Path $desk 'вставка.txt')
    Add-Result 'Ctrl+V pastes files onto the desktop' ($pasted -and (Test-Path (Join-Path $src 'вставка.txt'))) "on the desktop: $pasted, source kept: $(Test-Path (Join-Path $src 'вставка.txt'))"
    $gone = -not (Test-Path (Join-Path $desk 'удалить меня.txt'))
    $inBin = @((New-Object -ComObject Shell.Application).Namespace(10).Items() | Where-Object { $_.Name -like 'удалить меня*' }).Count -gt 0
    Add-Result 'Delete key sends a free icon to the Recycle Bin' ($gone -and $inBin) "gone from desktop: $gone, in the bin: $inBin, foreground: $([U]::ForegroundClass())"
} catch {
    Add-Result 'script error' $false $_.Exception.Message
} finally {
    Get-Process mistfences -ErrorAction SilentlyContinue | Stop-Process -Force
    New-Item -ItemType Directory -Force (Join-Path $out 'logs') | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA 'MistFences') -File -ErrorAction SilentlyContinue | Copy-Item -Destination (Join-Path $out 'logs') -Force
    Copy-Item (Join-Path $app 'config\config.json') (Join-Path $out 'config.json') -ErrorAction SilentlyContinue
    $results | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $out 'results.json') -Encoding UTF8
    Log 'desk tests end'
    Set-Content -LiteralPath (Join-Path $out 'done.txt') -Value 'done' -Encoding ASCII
    shutdown.exe /s /t 10
}
