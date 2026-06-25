# PHPM installer for Windows.
#   powershell -ExecutionPolicy ByPass -c "irm https://github.com/lemesdaniel/phpm/releases/latest/download/install.ps1 | iex"
#
# Environment overrides:
#   $env:PHPM_INSTALL_DIR  install location (default: %LOCALAPPDATA%\phpm\bin)
#   $env:PHPM_VERSION      a specific release tag (default: latest), e.g. v0.1.0
$ErrorActionPreference = 'Stop'

$repo = 'lemesdaniel/phpm'
$bin = 'phpm'
$installDir = if ($env:PHPM_INSTALL_DIR) { $env:PHPM_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'phpm\bin' }

$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { 'x86_64' }
    'ARM64' { 'aarch64' }
    default { throw "phpm: unsupported architecture: $($env:PROCESSOR_ARCHITECTURE)" }
}
$target = "$arch-pc-windows-msvc"
$asset = "$bin-$target.zip"

$version = if ($env:PHPM_VERSION) { $env:PHPM_VERSION } else { 'latest' }
$url = if ($version -eq 'latest') {
    "https://github.com/$repo/releases/latest/download/$asset"
} else {
    "https://github.com/$repo/releases/download/$version/$asset"
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Write-Host "phpm: downloading $url"
    $zip = Join-Path $tmp $asset
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
    Expand-Archive -Path $zip -DestinationPath $tmp -Force

    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item -Path (Join-Path $tmp "$bin.exe") -Destination (Join-Path $installDir "$bin.exe") -Force
    Write-Host "phpm: installed to $installDir\$bin.exe"

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -notlike "*$installDir*") {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$installDir", 'User')
        Write-Host "phpm: added $installDir to your user PATH (restart your shell)"
    }
    Write-Host "phpm: run 'phpm --help' to get started (requires composer, php, and git on PATH)"
}
finally {
    Remove-Item -Recurse -Force $tmp
}
