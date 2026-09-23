param(
  [ValidateSet('debug','release')]
  [string]$Profile = 'debug'
)
$ErrorActionPreference = 'Stop'
$hostTriple = (rustc -vV | Select-String '^host: ' | ForEach-Object { $_.Line.Substring(6) }).Trim()
if ($LASTEXITCODE -ne 0 -or -not $hostTriple.EndsWith('-pc-windows-msvc')) {
  throw "unsupported Windows sidecar host: $hostTriple"
}
$profileArgs = @()
if ($Profile -eq 'release') { $profileArgs = @('--release') }
cargo build -p nyrva-hook --locked @profileArgs
if ($LASTEXITCODE -ne 0) { throw 'hook sidecar build failed' }
cargo build -p nyrva-core --bin nyrva-telemetry --locked @profileArgs
if ($LASTEXITCODE -ne 0) { throw 'telemetry sidecar build failed' }
$destinationDir = 'nyrva/binaries'
New-Item -ItemType Directory -Force -Path $destinationDir | Out-Null
$names = @{
  'nyrva-hook' = "nyrva-hook-$hostTriple.exe"
  'nyrva-telemetry' = "nyrva-telemetry-$hostTriple.exe"
}
foreach ($binary in @('nyrva-hook','nyrva-telemetry')) {
  $source = "target/$Profile/$binary.exe"
  $destination = Join-Path $destinationDir $names[$binary]
  Copy-Item -Force $source $destination
  Write-Host "prepared $destination from $source"
}
