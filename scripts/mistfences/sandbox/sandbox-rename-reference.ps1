# Explorer's own rename box on the desktop, INSIDE Windows Sandbox: its window class, styles,
# font, rectangle against the item's label, and screenshots while it is open (a short name and
# a long one). Then the same for MistFences' free icons. Writes into C:\MF\out.
$ErrorActionPreference = 'Continue'
$in = 'C:\MF\in'; $out = 'C:\MF\out'; $app = 'C:\MistFences'
$log = Join-Path $out 'progress.log'
if (Test-Path -LiteralPath (Join-Path $out 'done.txt')) { exit }
function Log([string] $t) { Add-Content -LiteralPath $log -Value $t -Encoding UTF8 }
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Runtime.InteropServices;
public static class R {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] static extern void keybd_event(byte k, byte s, uint f, IntPtr e);
  [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool GetGUIThreadInfo(uint tid, ref GUITHREADINFO g);
  [StructLayout(LayoutKind.Sequential)] public struct GUITHREADINFO { public int cbSize, flags; public IntPtr hwndActive, hwndFocus, hwndCapture, hwndMenuOwner, hwndMoveSize, hwndCaret; public RECT rcCaret; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern int GetWindowLong(IntPtr h, int i);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("gdi32.dll")] static extern int GetObject(IntPtr h, int n, byte[] b);
  public static void Move(int x, int y) { int w = GetSystemMetrics(0), h = GetSystemMetrics(1); mouse_event(0x8001, x * 65535 / (w - 1), y * 65535 / (h - 1), 0, IntPtr.Zero); }
  public static void Click() { mouse_event(2, 0, 0, 0, IntPtr.Zero); mouse_event(4, 0, 0, 0, IntPtr.Zero); }
  public static void Key(byte k) { keybd_event(k, 0, 0, IntPtr.Zero); keybd_event(k, 0, 2, IntPtr.Zero); }
  public static string Focused() {
    var g = new GUITHREADINFO(); g.cbSize = Marshal.SizeOf(g); uint pid;
    uint tid = GetWindowThreadProcessId(GetForegroundWindow(), out pid);
    if (!GetGUIThreadInfo(tid, ref g) || g.hwndFocus == IntPtr.Zero) return "none";
    IntPtr h = g.hwndFocus; var sb = new StringBuilder(256); GetClassName(h, sb, 256);
    RECT r; GetWindowRect(h, out r);
    int style = GetWindowLong(h, -16), ex = GetWindowLong(h, -20);
    IntPtr font = SendMessage(h, 0x31, IntPtr.Zero, IntPtr.Zero); byte[] lf = new byte[92]; string face = "";
    int height = 0, weight = 0;
    if (font != IntPtr.Zero && GetObject(font, 92, lf) > 0) { height = BitConverter.ToInt32(lf, 0); weight = BitConverter.ToInt32(lf, 16); face = Encoding.Unicode.GetString(lf, 28, 64).Split('\0')[0]; }
    return string.Format("class={0} pid={1} rect={2},{3},{4},{5} style=0x{6:X8} ex=0x{7:X8} font={8} {9} {10}", sb, pid, r.L, r.T, r.R, r.B, style, ex, face, height, weight);
  }
}
'@
function Shot([string] $file, [int] $x, [int] $y, [int] $w, [int] $hh) { $b = New-Object Drawing.Bitmap $w, $hh; $g = [Drawing.Graphics]::FromImage($b); $g.CopyFromScreen($x, $y, 0, 0, (New-Object Drawing.Size $w, $hh)); $b.Save((Join-Path $out $file)); $g.Dispose(); $b.Dispose() }
New-Item -ItemType Directory -Force $out | Out-Null
Start-Sleep -Seconds 8
$desk = [Environment]::GetFolderPath('Desktop')
'x' | Set-Content (Join-Path $desk 'Короткое.txt')
'x' | Set-Content (Join-Path $desk 'Очень длинное имя файла для проверки переноса.txt')
Start-Sleep -Seconds 4
# Explorer: the two new files take the next cells of the first column (Recycle Bin, Edge,
# Edge, then these). Click each, F2, measure, shoot, Esc.
$i = 0
foreach ($y in 3, 4) {
    $cy = 5 + $y * 103 + 26
    [R]::Move(37, $cy); Start-Sleep -Milliseconds 300; [R]::Click(); Start-Sleep -Milliseconds 500
    [R]::Key(0x71); Start-Sleep -Milliseconds 900
    Log ("explorer row {0}: {1}" -f $y, [R]::Focused())
    Shot "rename-explorer-$y.png" 0 ($cy - 40) 200 160
    [R]::Key(0x1B); Start-Sleep -Milliseconds 500
    [R]::Move(900, 700); Start-Sleep -Milliseconds 300; [R]::Click(); Start-Sleep -Milliseconds 800
}
if (Test-Path (Join-Path $in 'app')) {
    Copy-Item -LiteralPath (Join-Path $in 'app') -Destination $app -Recurse -Force
    New-Item -ItemType Directory -Force (Join-Path $app 'config') | Out-Null
    $cli = Join-Path $app 'mistfences-cli.exe'
    Start-Process (Join-Path $app 'mistfences.exe') -ArgumentList '--portable', '--exit-after', '300000'
    Start-Sleep -Seconds 7
    foreach ($n in 'Короткое.txt', 'Очень длинное имя файла для проверки переноса.txt') {
        & $cli item move (Join-Path $desk $n) --to inbox | Out-Null
    }
    Start-Sleep -Seconds 3
    $items = ((& $cli item list --fence inbox --compact) -join '' | ConvertFrom-Json) | ForEach-Object { $_ } | Where-Object deskRect
    foreach ($n in 'Короткое', 'Очень длинное имя файла для проверки переноса') {
        $it = $items | Where-Object { $_.name -like "$n*" } | Select-Object -First 1
        $r = $it.deskRect; $cx = [int](($r[0] + $r[2]) / 2); $cy = $r[1] + 26
        [R]::Move($cx, $cy); Start-Sleep -Milliseconds 300; [R]::Click(); Start-Sleep -Milliseconds 500
        [R]::Key(0x71); Start-Sleep -Milliseconds 900
        Log ("mistfences '{0}' cell {1}: {2}" -f $n, ($r -join ','), [R]::Focused())
        Shot "rename-mistfences-$($n.Length).png" ($r[0] - 60) ($cy - 40) 200 160
        [R]::Key(0x1B); Start-Sleep -Milliseconds 500
        [R]::Move(900, 700); Start-Sleep -Milliseconds 300; [R]::Click(); Start-Sleep -Milliseconds 800
    }
    Get-Process mistfences -ErrorAction SilentlyContinue | Stop-Process -Force
}
Set-Content -LiteralPath (Join-Path $out 'results.json') -Value '[]'
Set-Content -LiteralPath (Join-Path $out 'done.txt') -Value 'done'
shutdown.exe /s /t 5
