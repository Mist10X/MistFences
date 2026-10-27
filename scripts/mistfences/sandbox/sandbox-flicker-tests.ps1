# Flicker tests INSIDE Windows Sandbox: free icons and a fence's icons must stay on screen while
# the pointer moves over one free icon, after a maximized window was minimized and the app sat
# idle long enough to trim its GPU caches (3 s), after Win+D, after Alt+Tab, and on repeated
# hovers. The screen is captured frame by frame (about every 15 ms) while it happens; each frame
# is compared with one taken before, cell by cell, leaving out the hovered icon.
$ErrorActionPreference = 'Continue'
$in = 'C:\MF\in'; $out = 'C:\MF\out'; $app = 'C:\MistFences'
$progress = Join-Path $out 'progress.log'
if (Test-Path -LiteralPath (Join-Path $out 'done.txt')) { exit }
$results = New-Object System.Collections.Generic.List[object]
function Log([string] $t) { Add-Content -LiteralPath $progress -Value ('{0:HH:mm:ss.fff} {1}' -f (Get-Date), $t) -Encoding UTF8 }
function Add-Result([string] $test, [bool] $ok, [string] $detail) {
    $results.Add([pscustomobject]@{ test = $test; passed = $ok; detail = $detail })
    Log ('{0} {1}: {2}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $test, $detail)
}
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System; using System.Drawing; using System.Drawing.Imaging; using System.Runtime.InteropServices;
public static class F {
  [DllImport("user32.dll")] static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] static extern void keybd_event(byte k, byte s, uint f, IntPtr e);
  [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool SystemParametersInfo(uint a, uint p, string v, uint w);
  [DllImport("user32.dll")] public static extern bool SetSysColors(int n, int[] e, int[] c);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  public static void Move(int x, int y) { int w = GetSystemMetrics(0), h = GetSystemMetrics(1); mouse_event(0x8001, x * 65535 / (w - 1), y * 65535 / (h - 1), 0, IntPtr.Zero); }
  public static void Keys(byte[] down) { foreach (var k in down) keybd_event(k, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(60); for (int i = down.Length - 1; i >= 0; i--) keybd_event(down[i], 0, 2, IntPtr.Zero); }
  public static Bitmap Grab(Rectangle r) { var b = new Bitmap(r.Width, r.Height, PixelFormat.Format32bppArgb); using (var g = Graphics.FromImage(b)) g.CopyFromScreen(r.Left, r.Top, 0, 0, r.Size); return b; }
  // Captures `r` for `ms` milliseconds, as fast as it can, moving the pointer to (tx, ty) after `moveAt` ms.
  public static System.Collections.Generic.List<object[]> Watch(Rectangle r, int ms, int moveAt, int tx, int ty) {
    var list = new System.Collections.Generic.List<object[]>(); var sw = System.Diagnostics.Stopwatch.StartNew(); bool moved = false;
    while (sw.ElapsedMilliseconds < ms) { if (!moved && sw.ElapsedMilliseconds >= moveAt) { Move(tx, ty); moved = true; }
      long t = sw.ElapsedMilliseconds; list.Add(new object[] { t, Grab(r) }); }
    return list; }
  // Mean absolute difference (0..255) of two equal-size captures inside `cell` (capture coordinates).
  public static double Diff(Bitmap a, Bitmap b, Rectangle cell) {
    var ra = new Rectangle(0, 0, a.Width, a.Height);
    var da = a.LockBits(ra, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb); var db = b.LockBits(ra, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
    long sum = 0; int n = 0; byte[] la = new byte[da.Stride], lb = new byte[db.Stride];
    for (int y = Math.Max(0, cell.Top); y < Math.Min(a.Height, cell.Bottom); y++) {
      Marshal.Copy(da.Scan0 + y * da.Stride, la, 0, da.Stride); Marshal.Copy(db.Scan0 + y * db.Stride, lb, 0, db.Stride);
      for (int x = Math.Max(0, cell.Left); x < Math.Min(a.Width, cell.Right); x++) { int o = x * 4; sum += Math.Abs(la[o] - lb[o]) + Math.Abs(la[o+1] - lb[o+1]) + Math.Abs(la[o+2] - lb[o+2]); n += 3; } }
    a.UnlockBits(da); b.UnlockBits(db); return n == 0 ? 0 : (double)sum / n; }
}
'@
function MfCli([string[]] $a) { (& (Join-Path $app 'mistfences-cli.exe') @a --compact 2>&1 | ForEach-Object { "$_" }) -join "`n" }
function Json([string[]] $a) { try { (MfCli $a) | ConvertFrom-Json } catch { $null } }
function Rect($r) { New-Object Drawing.Rectangle $r[0], $r[1], ($r[2] - $r[0]), ($r[3] - $r[1]) }

New-Item -ItemType Directory -Force $out | Out-Null
Log 'flicker tests start'
Start-Sleep -Seconds 8
# A plain background: any change in a captured cell is the icon itself.
[void][F]::SystemParametersInfo(0x14, 0, '', 3); [void][F]::SetSysColors(1, @(1), @(0x5A3A20))
Copy-Item -LiteralPath (Join-Path $in 'app') -Destination $app -Recurse -Force
New-Item -ItemType Directory -Force (Join-Path $app 'config') | Out-Null
$desk = [Environment]::GetFolderPath('Desktop')
# Shortcuts to programs with icons of their own: each has to be extracted again when the cache
# drops it, so a layer that lost its icons shows the gap long enough to be seen.
$targets = [ordered]@{ 'Блокнот' = 'notepad.exe'; 'Командная строка' = 'cmd.exe'; 'Paint' = 'mspaint.exe'; 'Редактор реестра' = 'regedit.exe'
    'Монитор ресурсов' = 'perfmon.exe'; 'Панель управления' = 'control.exe'; 'Сведения о системе' = 'msinfo32.exe'; 'Звукозапись' = 'SoundRecorder.exe' }
$shell = New-Object -ComObject WScript.Shell
$names = @()
foreach ($k in $targets.Keys) {
    $lnk = $shell.CreateShortcut((Join-Path $desk "$k.lnk")); $lnk.TargetPath = Join-Path $env:windir (Join-Path 'System32' $targets[$k]); $lnk.Save(); $names += "$k.lnk"
}
'x' | Set-Content (Join-Path $desk 'в группе.txt')

try {
    $env:RUST_LOG = 'info,mistfences::app::motion=debug,mistfences::desktop_layer=debug'
    Start-Process (Join-Path $app 'mistfences.exe') -ArgumentList '--portable', '--exit-after', '900000'
    Start-Sleep -Seconds 6
    foreach ($n in $names) { MfCli @('item', 'move', (Join-Path $desk $n), '--to', 'inbox') | Out-Null }
    Start-Sleep -Seconds 2
    $free = @($names | ForEach-Object { $n = $_; @(Json @('item', 'list')) | ForEach-Object { $_ } | Where-Object { $_.path -and (Split-Path $_.path -Leaf) -eq $n } | Select-Object -First 1 })
    # A fence with its own icon, to see whether fences are touched too.
    $fence = @(Json @('fence', 'list')) | ForEach-Object { $_ } | Where-Object { $_.kind -ne 'inbox' -and $_.windowRect } | Select-Object -First 1
    $grouped = @(Json @('item', 'list')) | ForEach-Object { $_ } | Where-Object { $_.path -and (Split-Path $_.path -Leaf) -eq 'в группе.txt' } | Select-Object -First 1
    Log ("free {0}; fence '{1}' {2}; 'в группе.txt' in '{3}'" -f ($free.Count), $fence.title, ($fence.windowRect | ConvertTo-Json -Compress), $grouped.fenceTitle)

    # Capture the union of the free cells; the fence separately.
    $cells = @($free | ForEach-Object { Rect $_.deskRect })
    $area = $cells[0]; foreach ($c in $cells) { $area = [Drawing.Rectangle]::Union($area, $c) }
    $fr = $fence.windowRect; $farea = New-Object Drawing.Rectangle $fr.x, $fr.y, ([Math]::Min($fr.w, 300)), ([Math]::Min($fr.h, 200))
    # One capture per frame (each costs a fixed ~30 ms in the sandbox): icons and fence together.
    $area = [Drawing.Rectangle]::Union($area, $farea)
    $local = @($cells | ForEach-Object { New-Object Drawing.Rectangle ($_.X - $area.X), ($_.Y - $area.Y), $_.Width, $_.Height })
    $flocal = New-Object Drawing.Rectangle ($farea.X - $area.X), ($farea.Y - $area.Y), $farea.Width, $farea.Height
    # Somewhere on the bare desktop: off the free icons and off every fence.
    $fences = @(Json @('fence', 'list')) | ForEach-Object { $_ } | Where-Object windowRect
    $away = $null
    foreach ($y in 300, 500, 700, 900) { foreach ($x in 400, 700, 1000, 1300, 1600) {
        if ($away -or $area.Contains($x, $y)) { continue }
        $hit = @($fences | Where-Object { $r = $_.windowRect; $x -ge $r.x - 40 -and $x -lt $r.x + $r.w + 40 -and $y -ge $r.y - 40 -and $y -lt $r.y + $r.h + 40 }).Count
        if (-not $hit) { $away = @($x, $y) } } }
    if (-not $away) { $away = @(($area.Right + 300), ($area.Bottom + 100)) }
    Log "pointer rests at $($away -join ',')"
    [F]::Move($away[0], $away[1]); Start-Sleep -Seconds 2
    $base = [F]::Grab($area)
    $base.Save((Join-Path $out 'flicker-base.png'))

    # Hovers the icon `hover` while capturing for `ms`; returns the worst frame of the other
    # icons (mean difference, 0..255) and of the fence, and how many frames went bad.
    function Watch-Hover([int] $hover, [int] $ms, [string] $tag) {
        $c = $cells[$hover]; $target = @(($c.X + [int]($c.Width / 2)), ($c.Y + 26))
        $frames = [F]::Watch($area, $ms, 40, $target[0], $target[1])
        $worst = 0.0; $fworst = 0.0; $bad = 0; $at = -1
        foreach ($f in $frames) {
            $d = 0.0
            for ($i = 0; $i -lt $local.Count; $i++) { if ($i -ne $hover) { $d = [Math]::Max($d, [F]::Diff($base, $f[1], $local[$i])) } }
            $fd = [F]::Diff($base, $f[1], $flocal)
            if ($d -gt 6 -or $fd -gt 6) { $bad++; if ($at -lt 0) { $at = $f[0]; $f[1].Save((Join-Path $out "flicker-$tag.png")) } }
            $worst = [Math]::Max($worst, $d); $fworst = [Math]::Max($fworst, $fd)
        }
        foreach ($f in $frames) { $f[1].Dispose() }
        [F]::Move($away[0], $away[1]); Start-Sleep -Milliseconds 600
        [pscustomobject]@{ frames = $frames.Count; worst = [Math]::Round($worst, 1); fence = [Math]::Round($fworst, 1); bad = $bad; firstBadMs = $at }
    }
    function Report([string] $test, $r) {
        Add-Result $test ($r.bad -eq 0 -and $r.frames -ge 15) ("{0} frames, other free icons worst diff {1}, fence worst diff {2}, bad frames {3}{4}" -f $r.frames, $r.worst, $r.fence, $r.bad, $(if ($r.bad) { " (first at $($r.firstBadMs) ms)" } else { '' }))
    }

    Report 'first hover: no other icon blinks' (Watch-Hover 0 700 'first')
    Report 'repeated hover' (Watch-Hover 1 700 'repeat')

    # A maximized window over the desktop, minimized; then idle past the 3 s GPU trim.
    $probe = Start-Process powershell.exe -PassThru -ArgumentList '-NoProfile', '-Command', 'Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form; $f.Text = ''MF flicker probe''; $f.BackColor = ''White''; [Windows.Forms.Application]::Run($f)'
    $w = [IntPtr]::Zero; for ($i = 0; $i -lt 60 -and $w -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 250; $w = [F]::FindWindow([NullString]::Value, 'MF flicker probe') }
    Log "test window $w"
    [void][F]::ShowWindow($w, 3); Start-Sleep -Seconds 3
    [void][F]::ShowWindow($w, 6); Start-Sleep -Seconds 5
    Report 'after minimizing a maximized window and 5 s idle' (Watch-Hover 2 900 'minimized')
    Report 'hovering again right after' (Watch-Hover 3 700 'minimized-again')

    # A normal (not maximized) window minimized the same way.
    [void][F]::ShowWindow($w, 1); [void][F]::SetForegroundWindow($w); Start-Sleep -Seconds 2
    [void][F]::ShowWindow($w, 6); Start-Sleep -Seconds 5
    Report 'after minimizing a normal window and 5 s idle' (Watch-Hover 0 900 'normal')

    # Win+D shows the desktop (the window comes back first, maximized).
    [void][F]::ShowWindow($w, 3); [void][F]::SetForegroundWindow($w); Start-Sleep -Seconds 2
    [F]::Keys([byte[]](0x5B, 0x44)); Start-Sleep -Seconds 5
    Report 'after Win+D and 5 s idle' (Watch-Hover 1 900 'wind')

    # Alt+Tab to the window and back to the desktop.
    [F]::Keys([byte[]](0x12, 0x09)); Start-Sleep -Seconds 2
    [void][F]::ShowWindow($w, 6); Start-Sleep -Seconds 5
    Report 'after Alt+Tab and minimizing, 5 s idle' (Watch-Hover 2 900 'alttab')
    Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue

    $base.Dispose()
    Get-Content (Join-Path $env:LOCALAPPDATA 'MistFences\mistfences.log') -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_ -match 'unneeded icons dropped|desktop grid' } | Select-Object -Last 12 | ForEach-Object { Log $_ }
} catch {
    Add-Result 'flicker tests ran' $false "$_"
} finally {
    Get-Process mistfences -ErrorAction SilentlyContinue | Stop-Process -Force
    $results | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $out 'results.json') -Encoding UTF8
    Set-Content -LiteralPath (Join-Path $out 'done.txt') -Value 'done'
    Log 'flicker tests end'
    shutdown.exe /s /t 5
}
