# Verifies that the legacy mode still produces the same PDFs after changes:
# re-runs the comparison matrix (original vs port) and additionally compares
# the produced PDFs against the recorded 0.1.0 baseline hashes.
#
# The legacy writer embeds a live /CreationDate and stamps the crate
# version into /Producer, so hashes are computed over the bytes with
# both normalised (D:YYYYMMDDHHMMSSZ -> zeros, rasterbator-rs X.Y.Z).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Get-NormalizedHash([string]$path) {
    $text = [Text.Encoding]::Latin1.GetString([IO.File]::ReadAllBytes($path))
    $text = [regex]::Replace($text, 'D:\d{14}Z', 'D:00000000000000Z')
    $text = [regex]::Replace($text, 'rasterbator-rs \d+\.\d+\.\d+', 'rasterbator-rs X.Y.Z')
    $sha = [Security.Cryptography.SHA256]::Create()
    ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::Latin1.GetBytes($text))) -replace '-', '')
}

pwsh -NoProfile -File analysis\run_matrix.ps1
if ($LASTEXITCODE -ne 0) { Write-Error "matrix failed"; exit 1 }

$baselineFile = "analysis\refgen\baseline-0.1.0.sha256"
if (-not (Test-Path $baselineFile)) {
    Write-Output "no baseline recorded; skipping byte comparison"
    exit 0
}

$baseline = @{}
Get-Content $baselineFile | ForEach-Object {
    $parts = $_ -split "  ", 2
    if ($parts.Count -eq 2) { $baseline[$parts[1]] = $parts[0] }
}

$fail = 0
foreach ($name in $baseline.Keys) {
    $path = Join-Path "analysis\refgen\rust-outs" $name
    if (-not (Test-Path $path)) { Write-Output "MISSING $name"; $fail++; continue }
    $hash = Get-NormalizedHash $path
    if ($hash -ne $baseline[$name]) {
        Write-Output "CHANGED $name"
        $fail++
    }
}
if ($fail -eq 0) {
    Write-Output "BASELINE OK: all $($baseline.Count) legacy outputs are identical to 0.1.0 (dates and producer version normalised)"
} else {
    Write-Output "BASELINE DIFFERS in $fail file(s)"
    exit 1
}
