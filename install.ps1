# Install ACE from GitHub releases (Windows).
#
# Usage:
#   powershell -c "irm https://ace-rs.dev/install.ps1 | iex"
#
# Installs the latest release binary to %LOCALAPPDATA%\ace\ace.exe.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'  # ~100x faster Invoke-WebRequest on PS 5.x
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo = 'ace-rs/ace'
$LatestUrl = 'https://ace-rs.dev/latest'
$InstallDir = Join-Path $env:LOCALAPPDATA 'ace'
$InstallPath = Join-Path $InstallDir 'ace.exe'

# --- Detect architecture ------------------------------------------------------

switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { $TripleArch = 'x86_64' }
    default {
        Write-Error "Unsupported architecture: $env:PROCESSOR_ARCHITECTURE"
        exit 1
    }
}

$Target = "$TripleArch-pc-windows-gnu"

# --- Resolve latest release ---------------------------------------------------

Write-Host 'Fetching latest release...'
$Tag = (Invoke-WebRequest -Uri $LatestUrl -UseBasicParsing).Content.Trim()

if (-not $Tag) {
    Write-Error "Could not determine latest release tag from $LatestUrl."
    exit 1
}

# --- Download binary ----------------------------------------------------------

$AssetUrl = "https://github.com/$Repo/releases/download/$Tag/ace-$Target.exe"
$TmpFile = New-TemporaryFile

try {
    Write-Host "Downloading ace $Tag ($Target)..."
    Invoke-WebRequest -Uri $AssetUrl -OutFile $TmpFile.FullName -UseBasicParsing

    if ((Get-Item $TmpFile.FullName).Length -eq 0) {
        Write-Error 'Download failed or produced empty file.'
        exit 1
    }

    # --- Install --------------------------------------------------------------

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir | Out-Null
    }
    Move-Item -Force $TmpFile.FullName $InstallPath
} finally {
    if (Test-Path $TmpFile.FullName) { Remove-Item $TmpFile.FullName }
}

Write-Host "Installed ace $Tag to $InstallPath"

# --- PATH hint ----------------------------------------------------------------

$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not ($UserPath -split ';' | Where-Object { $_ -eq $InstallDir })) {
    Write-Host ''
    Write-Host "Note: $InstallDir is not on your PATH."
    Write-Host 'Add it with:'
    Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"`$([Environment]::GetEnvironmentVariable('Path','User'));$InstallDir`", 'User')"
    Write-Host 'Then open a new shell.'
}
