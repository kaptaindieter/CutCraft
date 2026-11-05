<#
CutCraft - End-User Release Packager (Windows x64)

Goal: Create a self-contained ZIP so end users can run CutCraft without Rust/vcpkg.

Contents:
  - cutcraft.exe (copied/renamed from src\target\release\concat.exe)
  - required CutCraft native DLLs (onnxruntime, sherpa, DirectML, etc.)
  - required FFmpeg DLLs from vcpkg (avcodec/avformat/...) so ffmpeg loads
  - assets (optional)
  - optional VC++ Redistributable + Install.ps1

Output:
  .\dist\CutCraft-windows-x64.zip
#>

[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$NoVCRedist
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = $PSScriptRoot
$srcDir = Join-Path $root 'src'

$distRoot = Join-Path $root 'dist'
$stageRoot = Join-Path $distRoot '_stage'
$appFolderName = 'CutCraft'
$stage = Join-Path $stageRoot $appFolderName

$srcRelease = Join-Path $srcDir 'target\release'
$finalRelease = Join-Path $root 'target\release'

function Ensure-Dir([string]$p) { New-Item -ItemType Directory -Force $p | Out-Null }

function Get-VcpkgRoot {
    if ($env:VCPKG_ROOT -and (Test-Path $env:VCPKG_ROOT)) { return $env:VCPKG_ROOT }
    $fallback = 'C:\Users\JM\vcpkg'
    if (Test-Path (Join-Path $fallback 'vcpkg.exe')) { return $fallback }
    return $null
}

function Get-VcRedistPath {
    $cacheDir = Join-Path $distRoot '_cache'
    Ensure-Dir $cacheDir
    $cached = Join-Path $cacheDir 'vc_redist.x64.exe'
    if (Test-Path $cached) { return $cached }
    Write-Host '== Download VC++ Redistributable (x64) ==' -ForegroundColor Cyan
    $url = 'https://aka.ms/vs/17/release/vc_redist.x64.exe'
    Invoke-WebRequest -Uri $url -OutFile $cached -UseBasicParsing
    return $cached
}

function Write-InstallScript([string]$destPath) {
    $lines = @(
        '[CmdletBinding()]',
        'param(',
        "  [string]`$InstallDir = (Join-Path `$env:LOCALAPPDATA 'Programs\\ArtCraft\\CutCraft')",
        ')',
        '',
        'Set-StrictMode -Version Latest',
        "`$ErrorActionPreference = 'Stop'",
        '',
        'function Test-VcRuntime {',
        "  `$keys = @(",
        "    'HKLM:\\SOFTWARE\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\x64',",
        "    'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\x64'",
        '  )',
        '  foreach ($k in $keys) {',
        '    try {',
        '      $p = Get-ItemProperty -Path $k -ErrorAction Stop',
        '      if ($p.Installed -eq 1) { return $true }',
        '    } catch {}',
        '  }',
        '  return $false',
        '}',
        '',
        "Write-Host '== CutCraft Installer ==' -ForegroundColor Cyan",
        '',
        'if (-not (Test-VcRuntime)) {',
        "  `$redist = Join-Path `$PSScriptRoot 'vc_redist.x64.exe'",
        '  if (Test-Path $redist) {',
        "    Write-Host 'Installing Microsoft Visual C++ Redistributable (2015-2022) ...' -ForegroundColor Yellow",
        "    Start-Process -FilePath `$redist -ArgumentList '/install','/passive','/norestart' -Wait",
        '  } else {',
        "    Write-Host 'WARNING: VC++ runtime not detected and vc_redist.x64.exe not found next to this installer.' -ForegroundColor Yellow",
        "    Write-Host 'Download: https://aka.ms/vs/17/release/vc_redist.x64.exe' -ForegroundColor Yellow",
        '  }',
        '}',
        '',
        'New-Item -ItemType Directory -Force $InstallDir | Out-Null',
        'Write-Host ("Copying files to: {0}" -f $InstallDir) -ForegroundColor Cyan',
        'Copy-Item -LiteralPath (Join-Path $PSScriptRoot ''*'') -Destination $InstallDir -Recurse -Force',
        '',
        "Write-Host ''",
        "Write-Host 'Done. Start CutCraft via:' -ForegroundColor Green",
        "Write-Host (Join-Path `$InstallDir 'cutcraft.exe') -ForegroundColor Green"
    )
    Set-Content -LiteralPath $destPath -Value ($lines -join "`r`n") -Encoding UTF8
}

function Write-Readme([string]$destPath) {
    $lines = @(
        'CutCraft (Windows) - Quick Install',
        '==================================',
        '',
        '1) Unzip this folder anywhere (e.g. C:\ArtCraft\CutCraft).',
        '2) Run Install.ps1 (recommended). It installs the VC++ runtime if needed and copies files to LocalAppData.',
        '3) Start CutCraft via cutcraft.exe.',
        '',
        'Troubleshooting:',
        ' - If Windows reports missing DLLs (avcodec-*.dll / sherpa-onnx-*.dll), make sure all DLLs stay in the same folder as cutcraft.exe.'
    )
    Set-Content -LiteralPath $destPath -Value ($lines -join "`r`n") -Encoding UTF8
}

Write-Host '== CutCraft Release Pack ==' -ForegroundColor Cyan

Ensure-Dir $distRoot
Ensure-Dir $stageRoot
if (Test-Path $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
Ensure-Dir $stage

if (-not $SkipBuild) {
    Write-Host '== Build (cargo build --release) in src ==' -ForegroundColor Cyan
    Push-Location $srcDir
    try {
        cargo build --release --color never
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
    } finally { Pop-Location }
}

# Find the GUI exe
$candidate = @(
    (Join-Path $srcRelease 'cutcraft.exe'),
    (Join-Path $srcRelease 'concat.exe')
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $candidate) {
    throw "Could not find CutCraft executable in $srcRelease (expected concat.exe or cutcraft.exe)"
}

Write-Host '== Stage exe ==' -ForegroundColor Cyan
Copy-Item -LiteralPath $candidate -Destination (Join-Path $stage 'cutcraft.exe') -Force

Write-Host '== Stage CutCraft native DLLs (onnx/sherpa/etc.) ==' -ForegroundColor Cyan
Get-ChildItem -LiteralPath $srcRelease -Filter '*.dll' -File -ErrorAction SilentlyContinue |
    ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $stage -Force }

Write-Host '== Stage FFmpeg DLLs from vcpkg ==' -ForegroundColor Cyan
$vcpkg = Get-VcpkgRoot
if (-not $vcpkg) {
    Write-Host 'WARNING: VCPKG_ROOT not found. FFmpeg DLLs will NOT be included.' -ForegroundColor Yellow
} else {
    $ffBin = Join-Path $vcpkg 'installed\x64-windows\bin'
    if (-not (Test-Path $ffBin)) {
        Write-Host "WARNING: vcpkg ffmpeg bin dir not found: $ffBin" -ForegroundColor Yellow
    } else {
        # Copy only the FFmpeg runtime dlls (avoid unrelated tools like pkgconf-*.dll).
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
            Get-ChildItem -LiteralPath $ffBin -Filter $pat -File -ErrorAction SilentlyContinue |
                ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $stage -Force }
        }
    }
}

Write-Host '== Stage assets (optional) ==' -ForegroundColor Cyan
if (Test-Path (Join-Path $root 'assets')) {
    Copy-Item -LiteralPath (Join-Path $root 'assets') -Destination (Join-Path $stage 'assets') -Recurse -Force
}

Write-Host '== Stage docs/licenses (optional) ==' -ForegroundColor Cyan
foreach ($pat in @('LICENSE*','NOTICE*','THIRD_PARTY_NOTICES*','README*')) {
    Get-ChildItem -LiteralPath $root -File -Filter $pat -ErrorAction SilentlyContinue |
        ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $stage -Force }
}

# Also keep the "normalized" release folder for your own shortcutter scripts
Ensure-Dir $finalRelease
Copy-Item -LiteralPath (Join-Path $stage 'cutcraft.exe') -Destination (Join-Path $finalRelease 'cutcraft.exe') -Force

# Copy the staged DLLs + assets next to the exe as well (fixes missing-DLL system errors when launched from target\release)
Get-ChildItem -LiteralPath $stage -Filter '*.dll' -File -ErrorAction SilentlyContinue |
    ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $finalRelease -Force }
if (Test-Path (Join-Path $stage 'assets')) {
    Copy-Item -LiteralPath (Join-Path $stage 'assets') -Destination (Join-Path $finalRelease 'assets') -Recurse -Force
}

Write-InstallScript -destPath (Join-Path $stage 'Install.ps1')
Write-Readme -destPath (Join-Path $stage 'README.txt')

if (-not $NoVCRedist) {
    $redist = Get-VcRedistPath
    Copy-Item -LiteralPath $redist -Destination (Join-Path $stage 'vc_redist.x64.exe') -Force
}

$zip = Join-Path $distRoot 'CutCraft-windows-x64.zip'
if (Test-Path $zip) { Remove-Item -LiteralPath $zip -Force }

Write-Host "== Create ZIP: $zip ==" -ForegroundColor Cyan
Compress-Archive -LiteralPath $stage -DestinationPath $zip -Force

Write-Host ''
Write-Host 'DONE.' -ForegroundColor Green
Write-Host "ZIP: $zip" -ForegroundColor Green