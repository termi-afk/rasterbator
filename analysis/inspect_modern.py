"""Structural inspection of the modern-mode smoke PDFs."""
import os
from pypdf import PdfReader

D = "analysis/refgen/modern-check"
print(f"{'file':18} {'pages':>5} {'media (pt)':>17} trim bleed noScale dct labels bytes")
for f in sorted(os.listdir(D)):
    p = os.path.join(D, f)
    r = PdfReader(p)
    raw = open(p, "rb").read().decode("latin1")
    pg = r.pages[0]
    media = f"{float(pg.mediabox.width):.2f}x{float(pg.mediabox.height):.2f}"
    trim = "/TrimBox" in raw
    bleed = "/BleedBox" in raw
    nos = "/PrintScaling /None" in raw
    dct = "DCTDecode" in raw
    labels = "/PageLabels" in raw
    print(f"{f:18} {len(r.pages):>5} {media:>17} {str(trim):>4} {str(bleed):>5} {str(nos):>7} {str(dct):>3} {str(labels):>6} {os.path.getsize(p)}")
