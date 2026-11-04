# Windows reproduction harness for Cortex (run by .github/workflows/windows-repro.yml).
#
# Runs on a GitHub Windows runner's interactive desktop:
#   0. sets the screen to 1920x1080 and the display scale to -Scale percent (DisplayConfig,
#      no sign-out needed); falls back to WebView2 --force-device-scale-factor emulation
#   a. a 4 s test-pattern recording (CORTEX_DEBUG_TEST_PATTERN), ffprobe on the file
#   b. a 4 s real display recording (CORTEX_DEBUG_TARGET=display:1), ffprobe on the file
#   c. a normal launch: screenshots every 2 s, window rects and responsiveness every 250 ms
#   d. clicks the Settings gear (found through UI Automation) and looks for a Settings window
# Everything lands in -Out: screenshots, stdout/stderr, rects.csv, summary.json.
param(
  [Parameter(Mandatory)] [string] $Exe,
  [Parameter(Mandatory)] [string] $Out,
  [int] $Scale = 100
)

$ErrorActionPreference = "Continue"
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$summary = [ordered]@{ scale_requested = $Scale }
function Log($m) { $line = "[{0:HH:mm:ss.fff}] {1}" -f (Get-Date), $m; Write-Host $line; Add-Content "$Out/harness.log" $line }

Add-Type -AssemblyName System.Drawing, System.Windows.Forms, UIAutomationClient, UIAutomationTypes
Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class W {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsHungAppWindow(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint msg, IntPtr w, IntPtr l, uint flags, uint ms, out IntPtr res);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll")] public static extern uint GetDpiForSystem();

  public static List<string> Windows(uint pid) {
    var list = new List<string>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid) return true;
      var sb = new StringBuilder(256); GetWindowText(h, sb, 256);
      RECT r; GetWindowRect(h, out r);
      IntPtr res;
      var ok = SendMessageTimeout(h, 0, IntPtr.Zero, IntPtr.Zero, 2, 1000, out res) != IntPtr.Zero;
      list.Add(String.Format("{0}|{1}|{2}|{3},{4},{5},{6}|dpi={7}|responds={8}|hung={9}",
        h.ToInt64(), sb, IsWindowVisible(h), r.L, r.T, r.R - r.L, r.B - r.T, GetDpiForWindow(h), ok, IsHungAppWindow(h)));
      return true;
    }, IntPtr.Zero);
    return list;
  }

  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  // Screen point of a client-area point given in CSS pixels (scaled by the window's DPI).
  public static POINT ClientCss(IntPtr h, int fromRightCss, int fromTopCss) {
    RECT r; GetClientRect(h, out r);
    double k = GetDpiForWindow(h) / 96.0;
    var p = new POINT { X = r.R - (int)(fromRightCss * k), Y = (int)(fromTopCss * k) };
    ClientToScreen(h, ref p);
    return p;
  }

  public static void Click(int x, int y) {
    SetCursorPos(x, y);
    System.Threading.Thread.Sleep(80);
    mouse_event(0x0002, 0, 0, 0, IntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    mouse_event(0x0004, 0, 0, 0, IntPtr.Zero);
  }

  // Display scale through DisplayConfig (what the Settings app does; applies at once).
  [DllImport("user32.dll")] static extern int GetDisplayConfigBufferSizes(uint f, out uint np, out uint nm);
  [DllImport("user32.dll")] static extern int QueryDisplayConfig(uint f, ref uint np, byte[] p, ref uint nm, byte[] m, IntPtr t);
  [DllImport("user32.dll")] static extern int DisplayConfigGetDeviceInfo(byte[] p);
  [DllImport("user32.dll")] static extern int DisplayConfigSetDeviceInfo(byte[] p);
  static readonly int[] Steps = { 100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500 };

  static byte[] Header(int type, int size, byte[] path) {
    var b = new byte[size];
    BitConverter.GetBytes(type).CopyTo(b, 0);
    BitConverter.GetBytes(size).CopyTo(b, 4);
    Array.Copy(path, 0, b, 8, 12); // source adapterId (LUID) + source id
    return b;
  }

  public static string SetScale(int percent) {
    uint np, nm;
    if (GetDisplayConfigBufferSizes(2, out np, out nm) != 0) return "GetDisplayConfigBufferSizes failed";
    var paths = new byte[np * 72]; var modes = new byte[nm * 64];
    if (QueryDisplayConfig(2, ref np, paths, ref nm, modes, IntPtr.Zero) != 0) return "QueryDisplayConfig failed";
    var log = new StringBuilder();
    for (int i = 0; i < np; i++) {
      var path = new byte[20]; Array.Copy(paths, i * 72, path, 0, 20);
      var get = Header(-3, 32, path);
      if (DisplayConfigGetDeviceInfo(get) != 0) { log.Append("get failed; "); continue; }
      int min = BitConverter.ToInt32(get, 20), cur = BitConverter.ToInt32(get, 24), max = BitConverter.ToInt32(get, 28);
      // Relative steps are counted from the recommended scale (rel 0 = -min steps above 100%).
      int recIdx = -min;
      int want = Array.IndexOf(Steps, percent);
      if (want < 0) return "unknown scale";
      int rel = want - recIdx;
      log.AppendFormat("path {0}: min {1} cur {2} max {3} -> rel {4}; ", i, min, cur, max, rel);
      if (rel < min || rel > max) { log.Append("out of range; "); continue; }
      var set = Header(-4, 24, path);
      BitConverter.GetBytes(rel).CopyTo(set, 20);
      log.AppendFormat("set={0}; ", DisplayConfigSetDeviceInfo(set));
    }
    return log.ToString();
  }
}
"@

# Physical pixels everywhere (screenshots and rects).
[W]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) | Out-Null

function Shot($name) {
  try {
    $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
    $bmp.Save("$Out/$name.png", [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
  } catch { Log "screenshot $name failed: $_" }
}

function Start-Cortex($name, [hashtable] $envs) {
  $psi = New-Object System.Diagnostics.ProcessStartInfo $Exe
  $psi.UseShellExecute = $false
  $psi.CreateNoWindow = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  foreach ($k in $envs.Keys) { $psi.Environment[$k] = [string]$envs[$k] }
  # Skip the first-run tour so the main window (and its Settings gear) is in front.
  $dataDir = $envs["CORTEX_DEBUG_DATA_DIR"]
  if ($dataDir) {
    New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
    $settingsFile = Join-Path $dataDir "settings.json"
    if (-not (Test-Path $settingsFile)) { Set-Content $settingsFile '{"version":2,"onboardingCompleted":true}' }
  }
  if ($script:WebviewArgs) { $psi.Environment["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = $script:WebviewArgs }
  $p = [System.Diagnostics.Process]::Start($psi)
  # Async reads so a full pipe never blocks the app.
  $p | Add-Member -NotePropertyName OutTask -NotePropertyValue $p.StandardOutput.ReadToEndAsync()
  $p | Add-Member -NotePropertyName ErrTask -NotePropertyValue $p.StandardError.ReadToEndAsync()
  $p | Add-Member -NotePropertyName Tag -NotePropertyValue $name
  Log "started $name (pid $($p.Id))"
  return $p
}

function Stop-Cortex($p) {
  if (-not $p.HasExited) { $p.Kill($true); $p.WaitForExit(10000) | Out-Null }
  Set-Content "$Out/$($p.Tag).stdout.txt" $p.OutTask.Result
  Set-Content "$Out/$($p.Tag).stderr.txt" $p.ErrTask.Result
  Log "$($p.Tag) exit code $($p.ExitCode)"
}

function Probe($file) {
  if (-not $file -or -not (Test-Path $file)) { return $null }
  $j = & ffprobe -v error -show_entries format=duration,size:stream=codec_type,codec_name,width,height,avg_frame_rate -of json $file 2>&1
  return ($j -join "`n")
}

function Record-Run($name, [hashtable] $envs) {
  $dir = Join-Path $env:RUNNER_TEMP "cortex-$name"
  Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force $dir | Out-Null
  $envs["CORTEX_DEBUG_DATA_DIR"] = $dir
  $envs["CORTEX_DEBUG_RECORD_SECS"] = "4"
  $envs["CORTEX_DEBUG_TRACE"] = "1"
  $envs["CORTEX_DEBUG_PREVIEW_STATS"] = "1"
  $p = Start-Cortex $name $envs
  Start-Sleep 5; Shot "$name-during"
  $exited = $p.WaitForExit(60000)
  Stop-Cortex $p
  $stdout = Get-Content "$Out/$name.stdout.txt" -Raw
  $file = if ($stdout -match "cortex: recorded (.+)") { $Matches[1].Trim() } else { $null }
  $probe = Probe $file
  if ($file -and (Test-Path $file)) { Copy-Item $file "$Out/$name$([IO.Path]::GetExtension($file))" }
  Set-Content "$Out/$name.ffprobe.json" ([string]$probe)
  $ok = [bool]($probe -and $probe -match '"codec_type": "video"')
  Log "$name exited=$exited file=$file valid=$ok"
  return [ordered]@{ exited_in_60s = $exited; exit_code = $p.ExitCode; file = $file; valid_video = $ok }
}

# 0. Screen size and scale.
try { Set-DisplayResolution -Width 1920 -Height 1080 -Force -ErrorAction Stop; Log "resolution set to 1920x1080" }
catch { Log "Set-DisplayResolution failed: $_" }
Start-Sleep 2
$summary.scale_set_log = [W]::SetScale($Scale)
Log "SetScale($Scale): $($summary.scale_set_log)"
Start-Sleep 3
$summary.system_dpi = [W]::GetDpiForSystem()
$summary.screen = "{0}x{1}" -f [System.Windows.Forms.SystemInformation]::VirtualScreen.Width, [System.Windows.Forms.SystemInformation]::VirtualScreen.Height
# GetDpiForSystem is fixed at sign-in; the per-monitor value shows in the windows' dpi= field.
Log "system dpi $($summary.system_dpi), screen $($summary.screen)"
$script:WebviewArgs = $null
Shot "00-desktop"

# a. Test pattern recording.
$summary.a_test_pattern = Record-Run "a-test-pattern" @{ CORTEX_DEBUG_TEST_PATTERN = "1" }
# b. Real display recording.
$summary.b_display = Record-Run "b-display" @{ CORTEX_DEBUG_TARGET = "display:1" }

# c + d. Normal launch, sampled; then the gear.
function Launch-Run($name) {
  $dir = Join-Path $env:RUNNER_TEMP "cortex-$name"
  Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force $dir | Out-Null
  $p = Start-Cortex $name @{ CORTEX_DEBUG_DATA_DIR = $dir; CORTEX_DEBUG_TRACE = "1"; CORTEX_DEBUG_CAPTURABLE = "1" }
  $csv = "$Out/$name-rects.csv"
  Set-Content $csv "t_ms,responding,window"
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $nextShot = 0; $i = 0
  while ($sw.ElapsedMilliseconds -lt 20000 -and -not $p.HasExited) {
    $p.Refresh()
    foreach ($w in [W]::Windows([uint32]$p.Id)) {
      if ($w -match '\|True\|') { Add-Content $csv ("{0},{1},""{2}""" -f $sw.ElapsedMilliseconds, $p.Responding, $w) }
    }
    if ($sw.ElapsedMilliseconds -ge $nextShot) { Shot ("{0}-{1:D2}" -f $name, $i); $i++; $nextShot += 2000 }
    Start-Sleep -Milliseconds 250
  }
  $res = [ordered]@{ exited_early = $p.HasExited }
  # Shaking: how many distinct rects each visible window had.
  $rows = Import-Csv $csv
  $res.rects = @($rows | Group-Object { ($_.window -split '\|')[0] + ' ' + ($_.window -split '\|')[1] } | ForEach-Object {
    $distinct = @($_.Group | ForEach-Object { ($_.window -split '\|')[3] } | Select-Object -Unique)
    [ordered]@{ window = $_.Name; samples = $_.Count; distinct_rects = $distinct.Count; rects = ($distinct | Select-Object -First 12) -join ' ; ' }
  })
  $res.not_responding_samples = @($rows | Where-Object { $_.responding -eq 'False' -or $_.window -match 'responds=False|hung=True' }).Count
  $res.windows_before_click = @([W]::Windows([uint32]$p.Id) | Where-Object { $_ -match '\|True\|' })

  # d. The gear, by its accessible name.
  $main = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
  $cond = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::NameProperty), "Settings"
  $gear = $null
  if ($main) { $gear = $main.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond) }
  if ($gear) {
    $r = $gear.Current.BoundingRectangle
    $x = [int]($r.X + $r.Width / 2); $y = [int]($r.Y + $r.Height / 2)
    $res.gear_rect = "$($r.X),$($r.Y),$($r.Width),$($r.Height)"
    Log "$name gear at $x,$y"
    [W]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 300
    Shot "$name-before-click"
    [W]::Click($x, $y)
    Start-Sleep 4
    Shot "$name-after-click"
  } else {
    # WebView2 content is not always in the UI Automation tree: click where the title bar
    # draws the gear (right edge, vertically centred in the 44 px bar).
    $pt = [W]::ClientCss($p.MainWindowHandle, 26, 22)
    $res.gear_rect = "by-position $($pt.X),$($pt.Y)"
    Log "$name gear not found through UI Automation; clicking its position $($pt.X),$($pt.Y)"
    [W]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 300
    Shot "$name-before-click"
    [W]::Click($pt.X, $pt.Y)
    Start-Sleep 4
    Shot "$name-after-click"
  }
  $after = @([W]::Windows([uint32]$p.Id) | Where-Object { $_ -match '\|True\|' })
  $res.windows_after_click = $after
  $res.settings_opened = [bool]($after | Where-Object { $_ -match '\|Settings\|' })
  Stop-Cortex $p
  return $res
}

$summary.c_d_native = Launch-Run "c-native"
# e. WebView2 scale emulation on top (only meaningful when the real scale did not apply).
$script:WebviewArgs = "--force-device-scale-factor={0}" -f ($Scale / 100)
$summary.e_emulated = Launch-Run "e-emulated"

$summary | ConvertTo-Json -Depth 6 | Set-Content "$Out/summary.json"
Get-Content "$Out/summary.json"
