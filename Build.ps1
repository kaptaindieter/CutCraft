# ============================================================================
# CutCraft - Eigenständiges Build- & Setup-Skript
# ============================================================================
$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$srcDir = Join-Path $root "src"
$targetRelease = Join-Path $srcDir "target\release"
$finalRelease = Join-Path $root "target\release"

function Get-VcpkgRoot {
    if ($env:VCPKG_ROOT -and (Test-Path $env:VCPKG_ROOT)) { return $env:VCPKG_ROOT }
    $fallback = 'C:\Users\JM\vcpkg'
    if (Test-Path (Join-Path $fallback 'vcpkg.exe')) { return $fallback }
    return $null
}

Write-Host "== Prüfe Toolchain (Rust & Cargo) ==" -ForegroundColor Cyan
if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    Write-Host "FEHLT: Rust/Cargo ist nicht installiert!" -ForegroundColor Red
    exit 1
}

Write-Host "== Baue CutCraft (Release) ==" -ForegroundColor Cyan
Push-Location $srcDir

# Ensure we build against vcpkg FFmpeg (not a random system/scoop FFmpeg).
$oldFfmpegDir = $env:FFMPEG_DIR
$oldVcpkgRoot = $env:VCPKG_ROOT
try {
    $vcpkg = Get-VcpkgRoot
    if ($vcpkg) {
        $env:VCPKG_ROOT = $vcpkg
        $env:FFMPEG_DIR = Join-Path $vcpkg 'installed\x64-windows'
    }
    cargo build --release --color never
}
finally {
    $env:FFMPEG_DIR = $oldFfmpegDir
    $env:VCPKG_ROOT = $oldVcpkgRoot
}
if ($LASTEXITCODE -ne 0) {
    Write-Host "FEHLGESCHLAGEN beim Kompilieren!" -ForegroundColor Red
    Pop-Location
    exit 1
}
Pop-Location

Write-Host "== Kopiere Exe & DLLs in den Zielordner ==" -ForegroundColor Cyan
New-Item -ItemType Directory -Force $finalRelease | Out-Null

if (Test-Path "$targetRelease\cutcraft.exe") {
    Copy-Item "$targetRelease\cutcraft.exe" "$finalRelease\cutcraft.exe" -Force
} elseif (Test-Path "$targetRelease\concat.exe") {
    Copy-Item "$targetRelease\concat.exe" "$finalRelease\cutcraft.exe" -Force
}

# Alle DLLs (Sherpa, ONNX, FFmpeg) automatisch mitnehmen
Get-ChildItem -Path $targetRelease -Filter "*.dll" -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
    Copy-Item $_.FullName $finalRelease -Force -ErrorAction SilentlyContinue
}

# Prefer vcpkg FFmpeg runtime DLLs (most reliable for end-user Windows builds)
$vcpkg = Get-VcpkgRoot
if ($vcpkg) {
    $ffBin = Join-Path $vcpkg 'installed\x64-windows\bin'
    if (Test-Path $ffBin) {
        $ffPatterns = @(
            'avcodec-*.dll',
            'avdevice-*.dll',
            'avfilter-*.dll',
            'avformat-*.dll',
            'avutil-*.dll',
            'swresample-*.dll',
            'swscale-*.dll'
        )
        foreach ($pat in $ffPatterns) {
            Get-ChildItem -Path $ffBin -Filter $pat -File -ErrorAction SilentlyContinue | ForEach-Object {
                Copy-Item $_.FullName $finalRelease -Force -ErrorAction SilentlyContinue
            }
        }
    }
}

# Fallback: if user provided a custom FFMPEG_DIR, also copy from there
if ($env:FFMPEG_DIR -and (Test-Path $env:FFMPEG_DIR)) {
    Get-ChildItem -Path $env:FFMPEG_DIR -Filter "*.dll" -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
        Copy-Item $_.FullName $finalRelease -Force -ErrorAction SilentlyContinue
    }
}

Write-Host ""
Write-Host "FERTIG! CutCraft ist einsatzbereit in: $finalRelease\cutcraft.exe" -ForegroundColor Green