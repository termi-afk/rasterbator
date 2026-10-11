# The Rasterbator — original standalone source (v1.0, 2005)

This directory contains the original C# source code of **The Rasterbator Standalone
version 1.0**, the desktop companion of the well-known web tool
The Rasterbator (<http://homokaasu.org/rasterbator/>), by Matias Ärje.

It is kept here as reference material for the Rust port in this repository,
so that the behavior of the original tool can be read next to the port.
It is not built or used by the Rust workspace.

## Origin

The source was published inside the official application archive, which contained
the application, its source code (`source/` folder, SharpDevelop project files)
and the bundled third-party libraries (iTextSharp, ICSharpCode.SharpZipLib).

- Original page (Wayback Machine, 12 Feb 2005):
  <https://web.archive.org/web/20050212014339/http://arje.net/rasterbator>
- Direct archive download (Wayback Machine):
  <https://web.archive.org/web/20050212014339if_/http://arje.net/files/Rasterbator_Standalone_1.0.zip>
- Archive: `Rasterbator_Standalone_1.0.zip` (831,748 bytes)
- SHA-256: `33CE39A66F3DCEEDBBFF4B7B9A2F359B49A55B580101A75444DCA49C408D0AAE`

The archive also contained `readme.html` (user documentation),
`Rasterbator.exe` (v1.0.1863.39291), `itextsharp.dll` (1.0.4.0) and
`ICSharpCode.SharpZipLib.dll`. Those binaries are not duplicated here; the
source files below are copied verbatim from the `source/` folder of the archive
(encoding: Windows-1252).

## License

    The Rasterbator
    Copyright (C) 2004-2005 Matias Ärje

    This program is free software; you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation; either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

The full text of the GNU General Public License version 2 is in
[`COPYING`](COPYING) next to this file.

**Licensing.** The files in this `original/`
directory remain under their original license, GPL-2.0-or-later, with the
copyright of Matias Ärje. The Rust reimplementation in `crates/` is a derived
work of that code and is released under **AGPL-3.0-or-later** (see the root
[`LICENSE`](../LICENSE)): GPL-2.0-or-later permits use under the GNU GPL
version 3, and GPLv3 section 13 allows conveying the combined work together
with a work under the GNU AGPL version 3, applying the AGPL's network clause
to the combination.

## Files

| File | Description |
| --- | --- |
| `source/Rasterbator.cs` | The rasterbation engine: source image sampling, dot grid, PDF page generation (iTextSharp). |
| `source/MainForm.cs` | The WinForms wizard UI (5 steps: source image → paper size → output size → options → save as). |
| `source/AssemblyInfo.cs` | Assembly metadata for v1.0. |
| `source/Rasterbator.prjx`, `source/Rasterbator.cmbx` | SharpDevelop project/combine files. |
| `source/Rasterbator.exe.manifest` | Application manifest (visual styles). |
| `source/rasterbator.ico` | Application icon. |
| `source/Rasterbator.MainForm.resources` | WinForms designer resources. |

## Notes for the Rust port

The executable analyzed in this repository (`Rasterbator.exe` at the repo root,
build `1.0.2693.15484`, compiled around May 2007) is a **later build** than this
1.0 source dump. The rasterbation *output* semantics are the same, with these
differences that the port follows the binary for:

- **Dot size units.** 2005 source: `MaxRadius = Convert.ToInt32(Attr["RasterSize"])`
  (dots sized directly in PDF units). 2007 binary: dot size is given in mm and
  converted at 144 dpi (`RasterSize / 25.4 * 144 / 2`).
- **Pixel access.** The 2007 binary adds a `LockBits`/pointer fast path
  (`GetSourcePixel`) for `Format32bppArgb` and `Format24bppRgb` bitmaps;
  other formats still go through `Bitmap.GetPixel`.
- **Loop exits.** The 2007 binary `break`s out of a page's scan when the page
  column/row is past the source area (2005 source used `continue`).
- Float rounding differs slightly in `SquareSize` (double-precision `sqrt(2)`
  division in the binary).

The Rust port replicates the 2007 binary, which was also verified against its
actual IL with a C# harness running the shipped assembly against iTextSharp
1.0.4.0 from this same archive.
