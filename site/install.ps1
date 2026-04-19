# Mosaic CLI installer — Windows
# Usage: irm https://mosaicvideo.github.io/mosaic/install.ps1 | iex
# Params:
#   -InstallDir  target dir (default: $env:LOCALAPPDATA\Programs\mosaic-cli)
#   -Version     tag or 'latest' (default: latest)

[CmdletBinding()]
param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\mosaic-cli",
    [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"
# Disables Invoke-WebRequest's progress bar. On PS 5.1 (Windows default) the
# progress-rendering path cuts download speed by ~10x.
$ProgressPreference = "SilentlyContinue"
$Repo = "mosaicvideo/mosaic"
$GhApi = "https://api.github.com/repos/$Repo"
$DocsUrl = "https://mosaicvideo.github.io/mosaic/cli.html"

function Die($msg) {
    Write-Host "mosaic-cli: $msg" -ForegroundColor Red
    exit 1
}

function Info($msg) {
    Write-Host "mosaic-cli: $msg"
}

# 1. Detect arch
$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
    "AMD64" { $asset = "mosaic-cli-windows-x86_64.exe" }
    "ARM64" { $asset = "mosaic-cli-windows-aarch64.exe" }
    default { Die "Unsupported Windows arch: $arch" }
}

# 2. Resolve version
if ($Version -eq "latest") {
    Info "resolving latest release tag..."
    try {
        $resp = Invoke-RestMethod -UseBasicParsing -Uri "$GhApi/releases/latest"
        $tag = $resp.tag_name
    } catch {
        Die "could not resolve latest release tag: $_"
    }
} else {
    $tag = $Version
}
if (-not $tag) { Die "empty tag from release API" }

# 3. Download to temp dir
$tmp = Join-Path $env:TEMP "mosaic-cli-install-$([guid]::NewGuid())"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
    $baseUrl = "https://github.com/$Repo/releases/download/$tag"
    Info "downloading $asset ($tag)"
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/$asset" -OutFile (Join-Path $tmp $asset)
    } catch {
        Die "download failed: $baseUrl/$asset"
    }
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/SHA256SUMS" -OutFile (Join-Path $tmp "SHA256SUMS")
    } catch {
        Die "download failed: $baseUrl/SHA256SUMS (this release may predate SHA256SUMS; try -Version v0.1.5 or later)"
    }

    # 4. Verify checksum
    Info "verifying checksum..."
    $sumsPath = Join-Path $tmp "SHA256SUMS"
    $line = Get-Content $sumsPath | Where-Object { $_ -match "\s+$([regex]::Escape($asset))$" } | Select-Object -First 1
    if (-not $line) { Die "asset $asset not listed in SHA256SUMS" }
    $expected = ($line -split "\s+")[0].ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $tmp $asset)).Hash.ToLower()
    if ($expected -ne $actual) {
        Die "checksum mismatch for $asset (expected $expected, got $actual)"
    }

    # 5. Install
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $dest = Join-Path $InstallDir "mosaic-cli.exe"
    Move-Item -Force -Path (Join-Path $tmp $asset) -Destination $dest

    # 6. Sanity probe
    try {
        $ver = & $dest --version 2>$null
        if ($LASTEXITCODE -ne 0) { throw "nonzero exit" }
    } catch {
        Die "installed binary failed to run: $dest"
    }

    # 7. PATH: add InstallDir to user PATH if not already present
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not ($userPath -split ";" | Where-Object { $_ -eq $InstallDir })) {
        $newPath = if ([string]::IsNullOrEmpty($userPath)) { $InstallDir } else { "$userPath;$InstallDir" }
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        Write-Host ""
        Write-Host "Added $InstallDir to user PATH."
        Write-Host "Restart your terminal for the change to take effect."
    }

    Write-Host ""
    Write-Host "Installed $ver"
    Write-Host "  -> $dest"
    Write-Host ""
    Write-Host "Enable PowerShell completions (add to `$PROFILE to persist):"
    Write-Host "  mosaic-cli completions powershell | Out-String | Invoke-Expression"
    Write-Host ""
    Write-Host "Docs: $DocsUrl"
} finally {
    Remove-Item -Recurse -Force -Path $tmp -ErrorAction SilentlyContinue
}
