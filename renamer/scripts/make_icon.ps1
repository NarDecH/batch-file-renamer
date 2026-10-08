# Generates icons/icon.ico (multiple sizes) using System.Drawing
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$target = Join-Path $root "src-tauri\icons"
New-Item -ItemType Directory -Force -Path $target | Out-Null

function Make-PngStream([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAlias

    # Background gradient-ish: solid rounded square
    $brushBg = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 30, 41, 59))
    $g.FillRectangle($brushBg, 0, 0, $size, $size)

    # Letter "R" centered
    $fontSize = [int]($size * 0.55)
    $font = New-Object System.Drawing.Font ("Segoe UI", $fontSize, [System.Drawing.FontStyle]::Bold)
    $brushFg = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 96, 165, 250))
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = [System.Drawing.StringAlignment]::Center
    $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $rect = New-Object System.Drawing.RectangleF (0, ($size * 0.02), $size, $size)
    $g.DrawString("R", $font, $brushFg, $rect, $sf)
    $g.Dispose()

    # Encode as PNG into a memory stream
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return ,$ms.ToArray()
}

function Write-Ico([string]$path, [int[]]$sizes) {
    $streams = @()
    foreach ($s in $sizes) { $streams += ,(Make-PngStream $s) }

    $fs = [System.IO.File]::Create($path)
    $bw = New-Object System.IO.BinaryWriter $fs

    # ICONDIR
    $bw.Write([UInt16]0)          # reserved
    $bw.Write([UInt16]1)          # type: icon
    $bw.Write([UInt16]$sizes.Count)

    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $s = $sizes[$i]
        $data = $streams[$i]
        $bw.Write([Byte]($(if ($s -ge 256) { 0 } else { $s })))  # width
        $bw.Write([Byte]($(if ($s -ge 256) { 0 } else { $s })))  # height
        $bw.Write([Byte]0)      # palette
        $bw.Write([Byte]0)      # reserved
        $bw.Write([UInt16]1)    # color planes
        $bw.Write([UInt16]32)   # bpp
        $bw.Write([UInt32]$data.Length)
        $bw.Write([UInt32]$offset)
        $offset += $data.Length
    }
    foreach ($data in $streams) { $bw.Write($data) }
    $bw.Flush(); $bw.Dispose(); $fs.Dispose()
}

Write-Ico (Join-Path $target "icon.ico") @(16, 32, 48, 256)
Write-Output "icon.ico written to $target"
