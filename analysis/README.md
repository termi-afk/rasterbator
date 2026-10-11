# Analysis and verification tooling

Reverse engineering artifacts for `../Rasterbator.exe` and the harness that
verifies the Rust port against the original engine.

## What's here

| Path | Description |
| --- | --- |
| `decompiled/` | ILSpy decompilation of `Rasterbator.exe` (C# sources + reconstructed project). |
| `Rasterbator_IL.txt` | Full IL dump of the `Rasterbator` class (used to verify arithmetic). |
| `ByteBuffer104.txt`, `PdfContentByte104.txt` | Decompiled `iTextSharp 1.0.4.0` internals: the float formatter and the drawing primitives the port replicates. |
| `PdfReader104.txt` | Decompiled `PdfReader` (API reference used by the harness). |
| `refgen/` | C# oracle harness. `refgen render` runs the **shipped** `Rasterbator.exe` engine (referenced as an assembly) with the original iTextSharp 1.0.4.0 to produce reference PDFs; `refgen make-images` creates the test images. |
| `oracle/orig/` | The original 2005 distribution (`Rasterbator_Standalone_1.0.zip` from the Wayback Machine) including `itextsharp.dll` 1.0.4.0 and the GPL sources. |
| `oracle/nuget/` | iTextSharp 5.5.13.3 from NuGet (unused fallback, kept for reference). |
| `compare_pdfs.py` | pypdf-based comparator: per-page content-stream token diff (numbers with tolerance), page count, media boxes, Title/Creator. |
| `run_matrix.ps1` | Runs the full 16-scenario matrix (original vs port) and reports pass/fail. |
| `refs/`, `rust-outs/` | Generated PDFs (reference and port) for the last matrix run. |

## Reproducing the verification

```powershell
# 1. Build the oracle harness (needs the .NET SDK; the original engine runs
#    on .NET 10 with the iTextSharp 1.0.4.0 copied from oracle/orig/).
cd analysis/refgen
dotnet build -c Release

# 2. Build the port.
cd ../..
cargo build --release -p rasterbator-cli

# 3. Test images (once), then run the matrix.
cd analysis/refgen
./bin/Release/net10.0-windows/refgen.exe make-images --out images
cd ../..
pwsh analysis/run_matrix.ps1          # uses python + pypdf
```

Single comparison:

```powershell
python analysis/compare_pdfs.py analysis/refgen/refs/s01_gradient_a4_black.pdf `
                               analysis/refgen/rust-outs/s01_gradient_a4_black.pdf --tol 0.011
```

## Methodology notes

- The oracle executes the original *IL* on a modern runtime; the shipped binary
  itself targets .NET 1.1 and cannot run as-is on current Windows. The engine
  class (`Rasterbator.Rasterbator`) only uses `System.Drawing` bitmap access
  and iTextSharp, so this reproduces its behavior closely; differences in
  GDI+ decoders across runtimes are the main residual uncertainty.
- `compare_pdfs.py` compares tokens, so it also catches op-order and color
  changes, not just geometry. Numbers are compared with tolerance because the
  original quantizes coordinates to 2 decimals (see
  `crates/rasterbator-core/src/numfmt.rs`); the port quantizes identically.
- The matrix intentionally avoids cases where the harness and the port would
  use different image decoders as *ground truth* (e.g. exotic formats); PNG
  and GIF scenarios are exact, JPEG scenarios are tolerated at 0.5 pt but
  happen to match at 0.011 pt as well.
