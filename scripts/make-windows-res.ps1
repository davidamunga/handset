# Regenerates app/packaging/windows/handset.res, a compiled Windows resource holding the app icon.
# app/.cargo/config.toml hands it to the MSVC linker, so every build embeds the icon in handset.exe
# without a build script (which some machines block).
#   scripts\make-windows-res.ps1
param(
    [string]$Png = (Join-Path $PSScriptRoot '..\app\packaging\icon\handset-icon-512.png'),
    [string]$Out = (Join-Path $PSScriptRoot '..\app\packaging\windows\handset.res')
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$src = [System.Drawing.Image]::FromFile((Resolve-Path $Png).Path)
$images = foreach ($s in 16, 24, 32, 48, 64, 128, 256) {
    $bmp = New-Object System.Drawing.Bitmap $s, $s, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.DrawImage($src, 0, 0, $s, $s)
    $g.Dispose()
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    , @($s, $ms.ToArray())
}
$src.Dispose()

$res = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $res

# One .res record: sizes, numeric type and name, then the data padded to a dword.
function Add-Resource([uint16]$type, [uint16]$name, [byte[]]$data) {
    $w.Write([uint32]$data.Length); $w.Write([uint32]32)
    $w.Write([uint16]0xFFFF); $w.Write([uint16]$type)
    $w.Write([uint16]0xFFFF); $w.Write([uint16]$name)
    $w.Write([uint32]0)         # data version
    $w.Write([uint16]0x1030)    # moveable | pure | discardable
    $w.Write([uint16]1033)      # en-US
    $w.Write([uint32]0); $w.Write([uint32]0)
    $w.Write($data)
    $pad = (4 - ($data.Length % 4)) % 4
    if ($pad) { $w.Write([byte[]]::new($pad)) }
}

Add-Resource 0 0 ([byte[]]::new(0)) # the mandatory empty first record

$group = New-Object System.IO.MemoryStream
$gw = New-Object System.IO.BinaryWriter $group
$gw.Write([uint16]0); $gw.Write([uint16]1); $gw.Write([uint16]$images.Count)
$id = 1
foreach ($img in $images) {
    $size = $img[0]
    $data = [byte[]]$img[1]
    Add-Resource 3 $id $data # RT_ICON
    $dim = if ($size -ge 256) { 0 } else { $size }
    $gw.Write([byte]$dim); $gw.Write([byte]$dim); $gw.Write([byte]0); $gw.Write([byte]0)
    $gw.Write([uint16]1); $gw.Write([uint16]32); $gw.Write([uint32]$data.Length); $gw.Write([uint16]$id)
    $id++
}
Add-Resource 14 1 $group.ToArray() # RT_GROUP_ICON

New-Item -ItemType Directory -Force (Split-Path $Out) | Out-Null
[System.IO.File]::WriteAllBytes($Out, $res.ToArray())
Write-Host "wrote $Out"
