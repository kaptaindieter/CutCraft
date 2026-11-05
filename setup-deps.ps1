param(
    [switch]$BuildRelease
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$DepsDir = Join-Path $RepoRoot "build-deps"
$FfmpegDir = Join-Path $DepsDir "ffmpeg"
$OrtDir = Join-Path $DepsDir "onnxruntime"

New-Item -ItemType Directory -Force -Path $DepsDir | Out-Null

if (-not (Test-Path (Join-Path $FfmpegDir "bin"))) {
    Write-Host "=== Downloading FFmpeg Shared Dev Build (Windows x64) ==="
    $ffZip = Join-Path $DepsDir "ffmpeg-shared.zip"
    $ffUrl = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl-shared.zip"
    Invoke-WebRequest -Uri $ffUrl -OutFile $ffZip
    Expand-Archive -Path $ffZip -DestinationPath $DepsDir -Force
    $extracted = Get-ChildItem -Path $DepsDir -Directory -Filter "ffmpeg-master-*" | Select-Object -First 1
    if ($extracted) {
        if (Test-Path $FfmpegDir) { Remove-Item -LiteralPath $FfmpegDir -Recurse -Force }
        Rename-Item -LiteralPath $extracted.FullName -NewName "ffmpeg"
    }
    Remove-Item -LiteralPath $ffZip -Force
}

if (-not (Test-Path (Join-Path $OrtDir "lib"))) {
    Write-Host "=== Downloading ONNX Runtime (Windows x64) ==="
    $ortVer = "1.20.1"
    $ortZip = Join-Path $DepsDir "onnxruntime.zip"
    $ortUrl = "https://github.com/microsoft/onnxruntime/releases/download/v$ortVer/onnxruntime-win-x64-$ortVer.zip"
    Invoke-WebRequest -Uri $ortUrl -OutFile $ortZip
    Expand-Archive -Path $ortZip -DestinationPath $DepsDir -Force
    $extracted = Get-ChildItem -Path $DepsDir -Directory -Filter "onnxruntime-win-x64-*" | Select-Object -First 1
    if ($extracted) {
        if (Test-Path $OrtDir) { Remove-Item -LiteralPath $OrtDir -Recurse -Force }
        Rename-Item -LiteralPath $extracted.FullName -NewName "onnxruntime"
    }
    Remove-Item -LiteralPath $ortZip -Force
}

$env:FFMPEG_DIR = $FfmpegDir
$env:ORT_DIR = $OrtDir
$env:ORT_LIB_LOCATION = Join-Path $OrtDir "lib"
$env:ORT_PREFER_DYNAMIC_LINK = "1"

Write-Host "[OK] Environment configured:"
Write-Host "  FFMPEG_DIR      = $env:FFMPEG_DIR"
Write-Host "  ORT_LIB_LOCATION= $env:ORT_LIB_LOCATION"

if ($BuildRelease) {
    Write-Host "=== Building CutCraft (cargo build --release) ==="
    cargo build --release -p concat --manifest-path (Join-Path $RepoRoot "src\Cargo.toml")
    $outRel = Join-Path $RepoRoot "target\release"
    New-Item -ItemType Directory -Force -Path $outRel | Out-Null
    $srcExe = Join-Path $RepoRoot "src\target\release\concat.exe"
    if (Test-Path $srcExe) {
        Copy-Item -LiteralPath $srcExe -Destination (Join-Path $outRel "cutcraft.exe") -Force
    }
    Copy-Item "$FfmpegDir\bin\*.dll" -Destination $outRel -Force
    Copy-Item "$OrtDir\lib\*.dll" -Destination $outRel -Force
    gh release download v1.0.0 --repo kaptaindieter/CutCraft --dir $outRel --clobber 2>$null
    Write-Host "[SUCCESS] CutCraft built and staged in: $outRel\cutcraft.exe"
}
