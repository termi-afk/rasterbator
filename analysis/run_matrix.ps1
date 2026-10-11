# Compares the ORIGINAL Rasterbator engine (run through refgen with the real
# Rasterbator.exe + iTextSharp 1.0.4) against the Rust port for a matrix of
# scenarios, via analysis/compare_pdfs.py (numeric content-stream diff).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$refgen = "analysis\refgen\bin\Release\net10.0-windows\refgen.exe"
$rust = "target\release\rasterbator.exe"
$imgdir = "analysis\refgen\images"
$refs = "analysis\refgen\refs"
$outs = "analysis\refgen\rust-outs"
New-Item -ItemType Directory -Force -Path $refs, $outs | Out-Null

# n = name, img = source, ref = refgen args, rust = cli args, tol = number tolerance
$scenarios = @(
  @{ n = "s01_gradient_a4_black"; img = "gradient.png"; tol = "0.011"
     ref = @("--iw", "210", "--ih", "140", "--pw", "210", "--ph", "297", "--dot", "10", "--color", "black", "--cutout", "false")
     rust = @("--size", "210x140", "--paper", "210x297", "--dot-size", "10", "--color", "black") }
  @{ n = "s02_gradient_cutout"; img = "gradient.png"; tol = "0.011"
     ref = @("--iw", "420", "--ih", "297", "--pw", "210", "--ph", "297", "--dot", "10", "--color", "black", "--cutout", "true")
     rust = @("--size", "420x297", "--paper", "210x297", "--dot-size", "10", "--color", "black", "--cutout") }
  @{ n = "s03_solid_avg"; img = "solid.png"; tol = "0.011"
     ref = @("--iw", "210", "--ih", "157", "--pw", "210", "--ph", "297", "--dot", "10", "--color", "avg", "--cutout", "false")
     rust = @("--size", "210x157", "--paper", "210x297", "--dot-size", "10", "--color", "avg") }
  @{ n = "s04_alpha_black"; img = "alpha.png"; tol = "0.011"
     ref = @("--iw", "210", "--ih", "210", "--pw", "210", "--ph", "297", "--dot", "8", "--color", "black", "--cutout", "false")
     rust = @("--size", "210x210", "--paper", "210x297", "--dot-size", "8", "--color", "black") }
  @{ n = "s05_palette"; img = "palette.gif"; tol = "0.011"
     ref = @("--iw", "240", "--ih", "180", "--pw", "210", "--ph", "297", "--dot", "12", "--color", "black", "--cutout", "false")
     rust = @("--size", "240x180", "--paper", "210x297", "--dot-size", "12", "--color", "black") }
  @{ n = "s06_tall_multirow"; img = "tall.png"; tol = "0.011"
     ref = @("--iw", "105", "--ih", "315", "--pw", "210", "--ph", "297", "--dot", "10", "--color", "black", "--cutout", "false")
     rust = @("--size", "105x315", "--paper", "210x297", "--dot-size", "10", "--color", "black") }
  @{ n = "s07_photo_dot20"; img = "photo.jpg"; tol = "0.5"
     ref = @("--iw", "420", "--ih", "315", "--pw", "210", "--ph", "297", "--dot", "20", "--color", "black", "--cutout", "false")
     rust = @("--size", "420x315", "--paper", "210x297", "--dot-size", "20", "--color", "black") }
  @{ n = "s08_photo_custom_color"; img = "photo.jpg"; tol = "0.5"
     ref = @("--iw", "420", "--ih", "315", "--pw", "210", "--ph", "297", "--dot", "10", "--color", "ff3300", "--cutout", "true")
     rust = @("--size", "420x315", "--paper", "210x297", "--dot-size", "10", "--color", "ff3300", "--cutout") }
  @{ n = "s09_tiny_image"; img = "tiny.png"; tol = "0.011"
     ref = @("--iw", "210", "--ih", "210", "--pw", "210", "--ph", "297", "--dot", "5", "--color", "black", "--cutout", "true")
     rust = @("--size", "210x210", "--paper", "210x297", "--dot-size", "5", "--color", "black", "--cutout") }
  @{ n = "s10_custom_paper_dot25"; img = "gradient.png"; tol = "0.011"
     ref = @("--iw", "400", "--ih", "200", "--pw", "200", "--ph", "200", "--dot", "25", "--color", "black", "--cutout", "false")
     rust = @("--size", "400x200", "--paper", "200x200", "--dot-size", "25", "--color", "black") }
  @{ n = "s11_landscape_avg"; img = "alpha.png"; tol = "0.011"
     ref = @("--iw", "297", "--ih", "210", "--pw", "297", "--ph", "210", "--dot", "8", "--color", "avg", "--cutout", "false")
     rust = @("--size", "297x210", "--paper", "a4", "--landscape", "--dot-size", "8", "--color", "avg") }
  @{ n = "s12_crop"; img = "photo.jpg"; tol = "0.5"
     ref = @("--iw", "300", "--ih", "250", "--pw", "210", "--ph", "297", "--dot", "15", "--color", "black", "--cutout", "false", "--crop", "100,50,700,550")
     rust = @("--size", "300x250", "--paper", "210x297", "--dot-size", "15", "--color", "black", "--crop", "100,50,700,550") }
  @{ n = "s13_sheets_width"; img = "gradient.png"; tol = "0.011"
     ref = @("--iw", "1050", "--ih", "787", "--pw", "210", "--ph", "297", "--dot", "10", "--color", "black", "--cutout", "false")
     rust = @("-w", "5", "--paper", "a4", "--dot-size", "10", "--color", "black") }
  @{ n = "s14_sheets_height"; img = "tall.png"; tol = "0.011"
     ref = @("--iw", "396", "--ih", "1188", "--pw", "210", "--ph", "297", "--dot", "10", "--color", "black", "--cutout", "false")
     rust = @("--height-sheets", "4", "--paper", "a4", "--dot-size", "10", "--color", "black") }
  @{ n = "s15_dot_fractional"; img = "gradient.png"; tol = "0.011"
     ref = @("--iw", "210", "--ih", "140", "--pw", "210", "--ph", "297", "--dot", "7.5", "--color", "black", "--cutout", "false")
     rust = @("--size", "210x140", "--paper", "210x297", "--dot-size", "7.5", "--color", "black") }
  @{ n = "s16_letter_avg_cutout"; img = "photo.jpg"; tol = "0.5"
     ref = @("--iw", "300", "--ih", "225", "--pw", "216", "--ph", "279", "--dot", "15", "--color", "avg", "--cutout", "true")
     rust = @("--size", "300x225", "--paper", "letter", "--dot-size", "15", "--color", "avg", "--cutout") }
)

$pass = 0
$fail = 0
foreach ($s in $scenarios) {
  $img = Join-Path $imgdir $s.img
  $refPdf = Join-Path $refs "$($s.n).pdf"
  $rustPdf = Join-Path $outs "$($s.n).pdf"
  & $refgen render --input $img --output $refPdf @($s.ref) | Out-Null
  & $rust --quiet $img -o $rustPdf @($s.rust)
  if ($LASTEXITCODE -ne 0) { Write-Host "$($s.n): RUST FAILED" -ForegroundColor Red; $fail++; continue }
  $cmp = python analysis\compare_pdfs.py $refPdf $rustPdf --tol $s.tol 2>&1
  if ($cmp -match "MATCH") {
    Write-Host "$($s.n): MATCH" -ForegroundColor Green
    $pass++
  }
  else {
    Write-Host "$($s.n): MISMATCH" -ForegroundColor Red
    $cmp | Select-Object -First 6 | ForEach-Object { Write-Host "    $_" }
    $fail++
  }
}
Write-Host ""
Write-Host "passed $pass, failed $fail"
if ($fail -gt 0) { exit 1 }
