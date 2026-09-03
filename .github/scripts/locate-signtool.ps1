# Find Windows SDK signtool.exe and put a space-free copy on PATH.
#
# cargo-packager's built-in lookup only checks the 32-bit KitsRoot10 registry
# and x86/x64 folders, so it fails on ARM runners and some SDK 10.0.26100
# layouts ("ERROR SignTool not found"). Packager.toml [windows].sign-command
# then uses this copy (CI sets SIGNTOOL_PATH).
#
# Usage (GitHub Actions pwsh):
#   .github/scripts/locate-signtool.ps1
#
# Requires WINDOWS_CERTIFICATE or WINDOWS_CERTIFICATE_THUMBPRINT; otherwise
# exits 0 (unsigned MSI). Writes SIGNTOOL_PATH to GITHUB_ENV and the copy
# directory to GITHUB_PATH.

$ErrorActionPreference = "Stop"

$needSign = -not (
  [string]::IsNullOrWhiteSpace($env:WINDOWS_CERTIFICATE_THUMBPRINT) -and
  [string]::IsNullOrWhiteSpace($env:WINDOWS_CERTIFICATE)
)
if (-not $needSign) {
  Write-Host "no Authenticode cert; skipping SignTool"
  exit 0
}

function Find-SignTool {
  $found = [System.Collections.Generic.List[string]]::new()
  $kitRoots = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
    "${env:ProgramFiles}\Windows Kits\10\bin"
  )
  $archs = @("arm64", "x64", "x86")
  foreach ($root in $kitRoots) {
    if (-not (Test-Path -LiteralPath $root)) {
      continue
    }
    Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
      Sort-Object Name -Descending |
      ForEach-Object {
        foreach ($arch in $archs) {
          $p = Join-Path $_.FullName (Join-Path $arch "signtool.exe")
          if (Test-Path -LiteralPath $p) {
            $found.Add($p)
          }
        }
      }
    foreach ($arch in $archs) {
      $legacy = Join-Path $root (Join-Path $arch "signtool.exe")
      if (Test-Path -LiteralPath $legacy) {
        $found.Add($legacy)
      }
    }
  }
  foreach ($extra in @(
      "${env:ProgramFiles(x86)}\Windows Kits\10\App Certification Kit\signtool.exe",
      "${env:ProgramFiles}\Windows Kits\10\App Certification Kit\signtool.exe",
      "${env:ProgramFiles(x86)}\Microsoft SDKs\ClickOnce\SignTool\signtool.exe"
    )) {
    if (Test-Path -LiteralPath $extra) {
      $found.Add($extra)
    }
  }
  $onPath = Get-Command signtool.exe -ErrorAction SilentlyContinue
  if ($onPath) {
    $found.Add($onPath.Source)
  }
  return $found
}

$native = switch ($env:PROCESSOR_ARCHITECTURE) {
  "ARM64" { "arm64" }
  "AMD64" { "x64" }
  default { "x86" }
}

$candidates = Find-SignTool | Where-Object { $_ } | Select-Object -Unique
$signtool = $candidates |
  Sort-Object {
    if ($_ -match [regex]::Escape("\$native\")) { 0 } else { 1 }
  } |
  Select-Object -First 1

if (-not $signtool) {
  Write-Error @"
SignTool not found (needed to Authenticode-sign the Windows installer).
Searched Windows Kits bin (arm64/x64/x86), App Certification Kit, ClickOnce, PATH.
Install the Windows SDK "Signing Tools for Desktop Apps" component, or unset
WINDOWS_CERTIFICATE / WINDOWS_CERTIFICATE_THUMBPRINT to ship unsigned.
"@
  exit 1
}

Write-Host "found $signtool"

$destDir = if ($env:RUNNER_TEMP) {
  Join-Path $env:RUNNER_TEMP "signtool"
} else {
  Join-Path ([System.IO.Path]::GetTempPath()) "imprint-signtool"
}
New-Item -ItemType Directory -Force -Path $destDir | Out-Null
Copy-Item -LiteralPath $signtool -Destination (Join-Path $destDir "signtool.exe") -Force
Get-ChildItem -LiteralPath (Split-Path -Parent $signtool) -Filter *.dll -ErrorAction SilentlyContinue |
  Copy-Item -Destination $destDir -Force

$dest = Join-Path $destDir "signtool.exe"
Write-Host "using $dest"

if ($env:GITHUB_PATH) {
  Add-Content -LiteralPath $env:GITHUB_PATH -Value $destDir
}
if ($env:GITHUB_ENV) {
  Add-Content -LiteralPath $env:GITHUB_ENV -Value "SIGNTOOL_PATH=$dest"
} else {
  $env:SIGNTOOL_PATH = $dest
}
