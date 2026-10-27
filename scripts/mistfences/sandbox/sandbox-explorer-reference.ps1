# Reference measurements of Explorer's own desktop icons INSIDE Windows Sandbox: screenshots,
# exact icon / label rectangles of its list view, the icon-title font and smoothing; then the
# same names as MistFences free icons, for comparison. Writes into C:\MF\out.
$ErrorActionPreference = 'Continue'
$in = 'C:\MF\in'; $out = 'C:\MF\out'; $app = 'C:\MistFences'
$log = Join-Path $out 'progress.log'
if (Test-Path -LiteralPath (Join-Path $out 'done.txt')) { exit }
function Log([string] $t) { Add-Content -LiteralPath $log -Value $t -Encoding UTF8 }
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Runtime.InteropServices;
public static class X {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t);
  [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint a, bool i, uint pid);
  [DllImport("kernel32.dll")] static extern IntPtr VirtualAllocEx(IntPtr p, IntPtr a, UIntPtr s, uint t, uint pr);
  [DllImport("kernel32.dll")] static extern bool VirtualFreeEx(IntPtr p, IntPtr a, UIntPtr s, uint t);
  [DllImport("kernel32.dll")] static extern bool ReadProcessMemory(IntPtr p, IntPtr a, byte[] b, UIntPtr s, out UIntPtr r);
  [DllImport("kernel32.dll")] static extern bool WriteProcessMemory(IntPtr p, IntPtr a, byte[] b, UIntPtr s, out UIntPtr r);
  [DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint a, uint p, byte[] v, uint w);
  [DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint a, uint p, ref int v, uint w);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool SystemParametersInfo(uint a, uint p, string v, uint w);
  [DllImport("user32.dll")] public static extern bool SetSysColors(int n, int[] e, int[] c);
  public static IntPtr List() { IntPtr pm = FindWindowEx(IntPtr.Zero, IntPtr.Zero, "Progman", null); IntPtr dv = FindWindowEx(pm, IntPtr.Zero, "SHELLDLL_DefView", null); IntPtr w = IntPtr.Zero; while (dv == IntPtr.Zero && (w = FindWindowEx(IntPtr.Zero, w, "WorkerW", null)) != IntPtr.Zero) dv = FindWindowEx(w, IntPtr.Zero, "SHELLDLL_DefView", null); return FindWindowEx(dv, IntPtr.Zero, "SysListView32", null); }
  public static int Count() { return (int)SendMessage(List(), 0x1004, IntPtr.Zero, IntPtr.Zero); }
  static byte[] Remote(uint msg, int index, byte[] input, int outLen, int textAt) {
    IntPtr lv = List(); uint pid; GetWindowThreadProcessId(lv, out pid);
    IntPtr p = OpenProcess(0x0008 | 0x0010 | 0x0020, false, pid);
    IntPtr mem = VirtualAllocEx(p, IntPtr.Zero, (UIntPtr)4096, 0x3000, 0x04);
    UIntPtr n; byte[] buf = new byte[Math.Max(outLen, input.Length)]; Array.Copy(input, buf, input.Length);
    if (textAt >= 0) { long addr = mem.ToInt64() + 1024; byte[] a = BitConverter.GetBytes(addr); Array.Copy(a, 0, buf, textAt, 8); }
    WriteProcessMemory(p, mem, buf, (UIntPtr)buf.Length, out n);
    SendMessage(lv, msg, (IntPtr)index, mem);
    byte[] res = new byte[textAt >= 0 ? 1024 + 520 : outLen];
    ReadProcessMemory(p, mem, res, (UIntPtr)res.Length, out n);
    VirtualFreeEx(p, mem, UIntPtr.Zero, 0x8000);
    return res;
  }
  public static int[] Rect(int index, int code) { byte[] b = Remote(0x100E, index, BitConverter.GetBytes(code), 16, -1); return new[] { BitConverter.ToInt32(b, 0), BitConverter.ToInt32(b, 4), BitConverter.ToInt32(b, 8), BitConverter.ToInt32(b, 12) }; }
  public static string Text(int index) {
    byte[] item = new byte[88]; BitConverter.GetBytes(0x1).CopyTo(item, 0); BitConverter.GetBytes(index).CopyTo(item, 4); BitConverter.GetBytes(260).CopyTo(item, 32);
    byte[] r = Remote(0x1073, index, item, 88, 24);
    string s = Encoding.Unicode.GetString(r, 1024, 520); int z = s.IndexOf('\0'); return z >= 0 ? s.Substring(0, z) : s; }
}
'@
New-Item -ItemType Directory -Force $out | Out-Null
Start-Sleep -Seconds 8
# A plain background, so cells at different places compare pixel for pixel.
[void][X]::SystemParametersInfo(0x14, 0, "", 3); [void][X]::SetSysColors(1, @(1), @(0x5A3A20)); Start-Sleep -Seconds 1
$desk = [Environment]::GetFolderPath('Desktop')
$names = 'Counter-Strike 2', 'Перезагрузить с VPN', 'Архив Chatterino by Mist1X', 'MistFences — техническое исследование', 'Resident Evil Requiem', 'Telegram', 'Logitech G HUB', 'Battlefield™ 6', 'Alpha Bravo Charlie Delta', 'Один два три четыре пять', 'Ab Cd Ef Gh Ij Kl Mn Op Qr', 'Steam Deck Docking Station', 'Visual Studio Code Insiders', 'Новая папка (2) копия', 'abcdefghijklmnopqrstuvwxyz', 'Документы и таблицы 2026'
foreach ($n in $names) { New-Item -ItemType Directory -Force (Join-Path $desk $n) | Out-Null }
Start-Sleep -Seconds 4
# The font and smoothing Explorer uses for icon titles.
$lf = New-Object byte[] 92; [void][X]::SystemParametersInfo(0x1F, 92, $lf, 0)
$h = [BitConverter]::ToInt32($lf, 0); $wgt = [BitConverter]::ToInt32($lf, 16); $q = $lf[26]; $face = [Text.Encoding]::Unicode.GetString($lf, 28, 64).Trim([char]0)
$sm = 0; [void][X]::SystemParametersInfo(0x004A, 0, [ref]$sm, 0); $smt = 0; [void][X]::SystemParametersInfo(0x200A, 0, [ref]$smt, 0)
Log "font: face=$face height=$h weight=$wgt quality=$q; smoothing=$sm type=$smt (2 = ClearType)"
for ($i = 0; $i -lt [X]::Count(); $i++) {
    $t = [X]::Text($i)
    Log ("item {0} '{1}': bounds {2} icon {3} label {4}" -f $i, $t, ([X]::Rect($i, 0) -join ','), ([X]::Rect($i, 1) -join ','), ([X]::Rect($i, 2) -join ','))
}
function Shot([string] $file, [int] $x, [int] $y, [int] $w, [int] $hh) { $b = New-Object Drawing.Bitmap $w, $hh; $g = [Drawing.Graphics]::FromImage($b); $g.CopyFromScreen($x, $y, 0, 0, (New-Object Drawing.Size $w, $hh)); $b.Save((Join-Path $out $file)); $g.Dispose(); $b.Dispose() }
Shot 'explorer.png' 0 0 300 1040
# The same names as MistFences free icons, placed by it.
if (Test-Path (Join-Path $in 'app')) {
    Copy-Item -LiteralPath (Join-Path $in 'app') -Destination $app -Recurse -Force
    New-Item -ItemType Directory -Force (Join-Path $app 'config') | Out-Null
    $cli = Join-Path $app 'mistfences-cli.exe'
    Start-Process (Join-Path $app 'mistfences.exe') -ArgumentList '--portable', '--exit-after', '300000'
    Start-Sleep -Seconds 6
    foreach ($n in $names) { & $cli item move (Join-Path $desk $n) --to inbox | Out-Null }
    Start-Sleep -Seconds 3
    $items = ((& $cli item list --fence inbox --compact) -join '' | ConvertFrom-Json) | ForEach-Object { $_ } | Where-Object deskRect
    foreach ($it in $items) { Log ("mistfences '{0}': cell {1}" -f $it.name, ($it.deskRect -join ',')) }
    Get-Content (Join-Path $env:LOCALAPPDATA 'MistFences\mistfences.log') -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_ -match 'desktop grid|lined up' } | ForEach-Object { Log $_ }
    Shot 'mistfences.png' 0 0 300 1040
    Get-Process mistfences -ErrorAction SilentlyContinue | Stop-Process -Force
}
Set-Content -LiteralPath (Join-Path $out 'results.json') -Value '[]'
Set-Content -LiteralPath (Join-Path $out 'done.txt') -Value 'done'
shutdown.exe /s /t 5
