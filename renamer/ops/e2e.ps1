<#
e2e.ps1 — UIA-based E2E driver for the dev app (Batch File Renamer).

Usage:
  powershell -NoProfile -ExecutionPolicy Bypass -File renamer/ops/e2e.ps1 -scanDir "C:\path"

Flow:
  1. Restore + focus the app window (DPI-aware coordinates)
  2. ValuePattern.SetValue on the scan-path input (Chromium fires the input
     event so Svelte's bind updates state and the Rescan button enables)
  3. InvokePattern.Invoke on the Rescan button
  4. Report backend scan lines + visible file rows in the preview table

Notes:
  - Writes only to the in-memory workspace of the dev instance; no disk changes.
  - DPI-awareness must be set BEFORE querying UIA, otherwise clicks land off.
#>
param(
  [string]$scanDir = "C:\Users\HippoNoi\Temp\br-ci-test",
  [switch]$skipScan,   # only dump current UI state (rows/buttons) without scanning
  [switch]$clear,      # after optionally scanning, press the Clear-list button (BTN[6]) and confirm via OK
  [switch]$dump        # dumps every Button control with geometry and exits
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class E2E {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
}
'@
[E2E]::SetProcessDPIAware() | Out-Null

$root = [System.Windows.Automation.AutomationElement]::RootElement
$all = $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
$win = $null
foreach ($w in $all) { if ($w.Current.Name -match "Batch [Ff]ile [Rr]enamer") { $win = $w; break } }
if (-not $win) { Write-Output "WINDOW_NOT_FOUND"; exit 1 }
$hw = [IntPtr]$win.Current.NativeWindowHandle
[E2E]::ShowWindow($hw, 9) | Out-Null
[E2E]::SetForegroundWindow($hw) | Out-Null
Start-Sleep -Milliseconds 500

# two passes: the Windows accessibility tree is built lazily on first query
$els = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
Start-Sleep -Milliseconds 300
$els = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)

if ($dump) {
  $i = 0
  foreach ($el in $els) {
    if ($el.Current.ControlType.ProgrammaticName -match "Button") {
      $r = $el.Current.BoundingRectangle
      Write-Output ("BTN[" + $i + "] x=" + [int]$r.Left + " y=" + [int]$r.Top + " w=" + [int]($r.Right-$r.Left) + " h=" + [int]($r.Bottom-$r.Top) + " enabled=" + $el.Current.IsEnabled)
      $i++
    }
  }
  exit 0
}

if (-not $skipScan) {
  $edit = $null
  foreach ($el in $els) { if ($el.Current.AutomationId -eq "scan-path") { $edit = $el; break } }
  if (-not $edit) { Write-Output "EDIT_NOT_FOUND"; exit 2 }
  $vp = $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
  $vp.SetValue($scanDir)

  # Rescan button = index 5 among Button controls in the fresh toolbar layout
  $i = 0; $btn = $null
  foreach ($el in $els) { if ($el.Current.ControlType.ProgrammaticName -match "Button") { if ($i -eq 5) { $btn = $el; break }; $i++ } }
  if (-not $btn) { Write-Output "RESCAN_BTN_NOT_FOUND"; exit 3 }
  if (-not $btn.Current.IsEnabled) { Write-Output "RESCAN_DISABLED (bind did not fire?)"; exit 4 }
  ($btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)).Invoke()
  Start-Sleep -Milliseconds 2500
  Write-Output "SCAN_TRIGGERED"
}

if ($clear) {
  # press Clear-list (button index 6 in toolbar order), then BM_CLICK id=1 on the
  # native confirm dialog (Win32 child button — the Svelte confirm() dialog)
  $i = 0; $clr = $null
  foreach ($el in $els) { if ($el.Current.ControlType.ProgrammaticName -match "Button") { if ($i -eq 6) { $clr = $el; break }; $i++ } }
  if (-not $clr) { Write-Output "CLEAR_BTN_NOT_FOUND"; exit 5 }
  if ($clr.Current.IsEnabled) {
    ($clr.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)).Invoke()
    Start-Sleep -Milliseconds 800
    Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ClrDlg {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc p, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
}
'@
    $pid2 = $win.Current.ProcessId
    $dlg = [IntPtr]::Zero
    $cb = [ClrDlg+EnumProc]{ param($h, $l)
      $p = 0; [ClrDlg]::GetWindowThreadProcessId($h, [ref]$p) | Out-Null
      if ($p -eq $pid2 -and [ClrDlg]::IsWindowVisible($h)) {
        $cn = New-Object System.Text.StringBuilder 64
        [ClrDlg]::GetClassName($h, $cn, 64) | Out-Null
        if ($cn.ToString() -eq "#32770") { $script:dlg = $h; return $false }
      }
      return $true
    }
    $dlg = [IntPtr]::Zero
    [ClrDlg]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
    if ($dlg -eq [IntPtr]::Zero) { Write-Output "CONFIRM_DLG_NOT_FOUND"; exit 6 }
    $ok = [IntPtr]::Zero
    $cb2 = [ClrDlg+EnumProc]{ param($h, $l)
      $cn = New-Object System.Text.StringBuilder 64
      [ClrDlg]::GetClassName($h, $cn, 64) | Out-Null
      if ($cn.ToString() -match "Button" -and [ClrDlg]::GetDlgCtrlID($h) -eq 1) { $script:ok = $h; return $false }
      return $true
    }
    $ok = [IntPtr]::Zero
    [ClrDlg]::EnumChildWindows($dlg, $cb2, [IntPtr]::Zero) | Out-Null
    if ($ok -eq [IntPtr]::Zero) { Write-Output "OK_BTN_NOT_FOUND"; exit 7 }
    [ClrDlg]::SendMessageW($ok, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null  # BM_CLICK
    Write-Output "CLEAR_CONFIRMED"
  } else { Write-Output "CLEAR_DISABLED" }
}

# table row visibility check (labels appear for every rendered table cell)
Start-Sleep -Milliseconds 500
$els2 = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
Start-Sleep -Milliseconds 300
$els2 = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
$hit = 0; $sample = @()
foreach ($el in $els2) {
  $n = $el.Current.Name
  if ($n -and $n -match "file_") {
    $hit++
    if ($sample.Count -lt 5) { $sample += $n }
  }
}
Write-Output ("VISIBLE_TEXT_HITS=" + $hit)
$sample | ForEach-Object { Write-Output ("  sample: " + $_) }
