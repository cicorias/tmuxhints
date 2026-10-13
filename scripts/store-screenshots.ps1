# Captures the real tmuxhints popup and centers it on a 1920x1080 background for the
# Microsoft Store listing (minimum 1366x768). Uses a throwaway config dir (TMUXHINTS_CONFIG_DIR)
# so your own settings are never touched. Run via `mise run store:screenshots`.
param([string]$OutDir = (Join-Path $PSScriptRoot "..\docs\store"))
$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot "..\target\debug\tmuxhints.exe"
New-Item -ItemType Directory -Force $OutDir | Out-Null

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Shot {
  [StructLayout(LayoutKind.Sequential)] public struct R { public int L, T, Rt, B; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  // DWMWA_EXTENDED_FRAME_BOUNDS = 9: the visible window, without the invisible resize/shadow margins.
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out R r, int size);
}
'@

function Capture([string]$name, [string]$prefix) {
  $cfg = Join-Path $env:TEMP "tmuxhints-shot-$name"
  New-Item -ItemType Directory -Force $cfg | Out-Null
  Set-Content (Join-Path $cfg 'settings.json') "{ `"hotkey`": `"Super+Ctrl+K`", `"prefix`": `"$prefix`" }"
  $env:TMUXHINTS_CONFIG_DIR = $cfg
  try {
    $app = Start-Process $exe -PassThru -WindowStyle Hidden
    Start-Sleep 3
    & $exe --show; Start-Sleep 2
    $h = [Shot]::FindWindow([NullString]::Value, "tmux cheat sheet")
    if (-not [Shot]::IsWindowVisible($h)) { throw "popup not visible for $name" }
    $r = New-Object Shot+R
    [void][Shot]::DwmGetWindowAttribute($h, 9, [ref]$r, [Runtime.InteropServices.Marshal]::SizeOf($r))
    $w = $r.Rt - $r.L; $hgt = $r.B - $r.T
    $popup = New-Object System.Drawing.Bitmap $w, $hgt
    [System.Drawing.Graphics]::FromImage($popup).CopyFromScreen($r.L, $r.T, 0, 0, $popup.Size)
  } finally {
    & $exe --quit; Start-Sleep 2
    if (-not $app.HasExited) { $app | Stop-Process }
    Remove-Item Env:\TMUXHINTS_CONFIG_DIR
  }

  # 1920x1080 canvas: dark diagonal gradient, soft shadow, popup centered.
  $canvas = New-Object System.Drawing.Bitmap 1920, 1080
  $g = [System.Drawing.Graphics]::FromImage($canvas)
  $g.SmoothingMode = 'AntiAlias'
  $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush (New-Object System.Drawing.Rectangle 0, 0, 1920, 1080), ([System.Drawing.Color]::FromArgb(255, 14, 32, 22)), ([System.Drawing.Color]::FromArgb(255, 10, 12, 18)), 35
  $g.FillRectangle($bg, 0, 0, 1920, 1080)
  $x = [int]((1920 - $w) / 2); $y = [int]((1080 - $hgt) / 2)
  for ($i = 24; $i -ge 1; $i -= 3) {
    $shadow = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(10, 0, 0, 0))
    $g.FillRectangle($shadow, $x - $i, $y - $i + 8, $w + 2 * $i, $hgt + 2 * $i)
  }
  $g.DrawImage($popup, $x, $y, $w, $hgt)
  $out = Join-Path $OutDir "$name.png"
  $canvas.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
  "{0}  popup {1}x{2} -> 1920x1080" -f $out, $w, $hgt
}

if (Get-Process tmuxhints -ErrorAction SilentlyContinue) { throw "quit tmuxhints first" }
Capture "1-cheatsheet-default-prefix" "Ctrl+b"
Capture "2-cheatsheet-custom-prefix" "Ctrl+a"
