# Builds dist/windows/Handset-<version>-windows.zip: handset.exe (with its icon), the licence files and,
# when -ToolsDir is given, adb and ffmpeg for a self-contained download. The zip is unsigned.
#   scripts\package-windows.ps1 [-ToolsDir <folder holding platform-tools\ and ffmpeg.exe>]
param([string]$ToolsDir)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$version = (Select-String -Path "$root\app\Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$out = Join-Path $root 'dist\windows'
$stage = Join-Path $out "Handset-$version"

Push-Location "$root\app"
try {
    $ErrorActionPreference = 'Continue' # cargo reports progress on stderr
    & cargo rustc --release --bin handset -- -C link-arg=packaging/windows/handset.res
    $status = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($status -ne 0) { throw 'cargo build failed' }
} finally { Pop-Location }

Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "$root\app\target\release\handset.exe", "$root\LICENSE", "$root\NOTICE" $stage
if ($ToolsDir) {
    $adb = Join-Path $ToolsDir 'platform-tools'
    if (Test-Path $adb) { Copy-Item $adb (Join-Path $stage 'platform-tools') -Recurse }
    $ffmpeg = Join-Path $ToolsDir 'ffmpeg.exe'
    if (Test-Path $ffmpeg) { Copy-Item $ffmpeg $stage }
}

$zip = Join-Path $out "Handset-$version-windows.zip"
Compress-Archive -Path $stage -DestinationPath $zip
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
"$hash  $(Split-Path $zip -Leaf)" | Set-Content -Encoding ascii (Join-Path $out 'SHA256SUMS')
Remove-Item -Recurse -Force $stage
Write-Host "built $zip"
Get-ChildItem $out | Format-Table Name, Length
