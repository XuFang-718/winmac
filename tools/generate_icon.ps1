param(
    [string]$OutputDir = (Join-Path $PSScriptRoot "..\\assets")
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing

$null = New-Item -ItemType Directory -Force -Path $OutputDir
$tempDir = Join-Path $env:TEMP "winmac-icon-build"
$null = New-Item -ItemType Directory -Force -Path $tempDir

function New-RoundedPath {
    param(
        [float]$X,
        [float]$Y,
        [float]$Width,
        [float]$Height,
        [float]$Radius
    )

    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $diameter = $Radius * 2
    $path.AddArc($X, $Y, $diameter, $diameter, 180, 90)
    $path.AddArc($X + $Width - $diameter, $Y, $diameter, $diameter, 270, 90)
    $path.AddArc($X + $Width - $diameter, $Y + $Height - $diameter, $diameter, $diameter, 0, 90)
    $path.AddArc($X, $Y + $Height - $diameter, $diameter, $diameter, 90, 90)
    $path.CloseFigure()
    return $path
}

function Save-WinMacPng {
    param(
        [int]$Size,
        [string]$Path
    )

    $bitmap = New-Object System.Drawing.Bitmap $Size, $Size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $graphics.Clear([System.Drawing.Color]::Transparent)

    $outerInset = [Math]::Max(1, [int]($Size * 0.035))
    $outerRadius = [Math]::Max(4, [int]($Size * 0.24))
    $outerPath = New-RoundedPath $outerInset $outerInset ($Size - $outerInset * 2) ($Size - $outerInset * 2) $outerRadius

    $baseBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        ([System.Drawing.PointF]::new(0, 0)),
        ([System.Drawing.PointF]::new($Size, $Size)),
        ([System.Drawing.Color]::FromArgb(255, 238, 240, 244)),
        ([System.Drawing.Color]::FromArgb(255, 72, 76, 88))
    )
    $graphics.FillPath($baseBrush, $outerPath)

    $glossHeight = [Math]::Max(6, [int]($Size * 0.34))
    $glossPath = New-RoundedPath ($outerInset + 2) ($outerInset + 2) ($Size - ($outerInset + 2) * 2) $glossHeight ([Math]::Max(4, [int]($Size * 0.18)))
    $glossBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        ([System.Drawing.PointF]::new(0, 0)),
        ([System.Drawing.PointF]::new(0, $glossHeight)),
        ([System.Drawing.Color]::FromArgb(190, 255, 255, 255)),
        ([System.Drawing.Color]::FromArgb(20, 255, 255, 255))
    )
    $graphics.FillPath($glossBrush, $glossPath)

    $borderPen = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(115, 255, 255, 255)), ([Math]::Max(1, $Size * 0.02))
    $graphics.DrawPath($borderPen, $outerPath)

    $chipSize = [Math]::Max(8, [int]($Size * 0.56))
    $chipX = [int](($Size - $chipSize) / 2)
    $chipY = [int](($Size - $chipSize) / 2)
    $chipRect = [System.Drawing.Rectangle]::new($chipX, $chipY, $chipSize, $chipSize)
    $chipBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $chipRect,
        ([System.Drawing.Color]::FromArgb(255, 32, 34, 39)),
        ([System.Drawing.Color]::FromArgb(255, 12, 14, 18)),
        90
    )
    $graphics.FillEllipse($chipBrush, $chipRect)

    $chipBorder = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(96, 255, 255, 255)), ([Math]::Max(1, $Size * 0.015))
    $graphics.DrawEllipse($chipBorder, $chipRect)

    $commandSize = [Math]::Max(8, [int]($Size * 0.31))
    $font = New-Object System.Drawing.Font("Segoe UI Symbol", $commandSize, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel))
    $format = New-Object System.Drawing.StringFormat
    $format.Alignment = [System.Drawing.StringAlignment]::Center
    $format.LineAlignment = [System.Drawing.StringAlignment]::Center
    $textBrush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 250, 252, 255))
    $commandRect = [System.Drawing.RectangleF]::new([float]$chipX, [float]($chipY - $Size * 0.01), [float]$chipSize, [float]$chipSize)
    $graphics.DrawString([char]0x2318, $font, $textBrush, $commandRect, $format)

    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)

    $textBrush.Dispose()
    $format.Dispose()
    $font.Dispose()
    $chipBorder.Dispose()
    $chipBrush.Dispose()
    $borderPen.Dispose()
    $glossBrush.Dispose()
    $glossPath.Dispose()
    $baseBrush.Dispose()
    $outerPath.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
}

$sizes = @(16, 20, 24, 32, 40, 48, 64, 128, 256)
$pngPaths = @()

foreach ($size in $sizes) {
    $pngPath = Join-Path $tempDir ("winmac-{0}.png" -f $size)
    Save-WinMacPng -Size $size -Path $pngPath
    $pngPaths += $pngPath
}

$previewSource = Join-Path $tempDir "winmac-256.png"
Copy-Item $previewSource (Join-Path $OutputDir "winmac-preview.png") -Force

$icoPath = Join-Path $OutputDir "winmac.ico"
$file = [System.IO.File]::Open($icoPath, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write)
$writer = New-Object System.IO.BinaryWriter $file

$writer.Write([UInt16]0)
$writer.Write([UInt16]1)
$writer.Write([UInt16]$pngPaths.Count)

$offset = 6 + (16 * $pngPaths.Count)
$payloads = @()

foreach ($pngPath in $pngPaths) {
    $bytes = [System.IO.File]::ReadAllBytes($pngPath)
    $payloads += ,$bytes
    $size = [int]([System.IO.Path]::GetFileNameWithoutExtension($pngPath).Split('-')[-1])
    $dimension = if ($size -ge 256) { [byte]0 } else { [byte]$size }

    $writer.Write($dimension)
    $writer.Write($dimension)
    $writer.Write([byte]0)
    $writer.Write([byte]0)
    $writer.Write([UInt16]1)
    $writer.Write([UInt16]32)
    $writer.Write([UInt32]$bytes.Length)
    $writer.Write([UInt32]$offset)
    $offset += $bytes.Length
}

foreach ($bytes in $payloads) {
    $writer.Write($bytes)
}

$writer.Flush()
$writer.Close()
$file.Close()

Write-Output ("Generated {0}" -f $icoPath)
