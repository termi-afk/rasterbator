#!/usr/bin/env python3
"""Numeric comparator for Rasterbator PDFs.

Compares two PDFs page by page:
  - page count and page sizes
  - drawing-op token streams (numbers compared with tolerance)
  - Info entries Title/Creator
Exit code 0 = MATCH, 1 = MISMATCH, 2 = error.

Usage: compare_pdfs.py <a.pdf> <b.pdf> [--tol 0.01]
"""
import re
import sys

from pypdf import PdfReader

NUM_RE = re.compile(rb"^[+-]?(\d+\.?\d*|\.\d+)$")


def tokens(data: bytes):
    """Split a PDF content stream into numbers and operators."""
    out = []
    for m in re.finditer(rb"[^\s\[\]()<>/]+", data):
        tok = m.group(0)
        if NUM_RE.match(tok):
            out.append(("n", float(tok)))
        else:
            out.append(("s", tok.decode("latin-1")))
    return out


def close(a: float, b: float, tol: float) -> bool:
    return abs(a - b) <= tol + 1e-9 * max(abs(a), abs(b))


def cmp_page(pa, pb, page_no: int, tol: float) -> bool:
    mb_a, mb_b = pa.mediabox, pb.mediabox
    if not (close(float(mb_a.width), float(mb_b.width), 0.01)
            and close(float(mb_a.height), float(mb_b.height), 0.01)):
        print(f"page {page_no}: size {mb_a} != {mb_b}")
        return False
    ta = tokens(pa.get_contents().get_data())
    tb = tokens(pb.get_contents().get_data())
    for i in range(max(len(ta), len(tb))):
        if i >= len(ta) or i >= len(tb):
            print(f"page {page_no}: token count {len(ta)} != {len(tb)} at #{i}")
            return False
        ka, va = ta[i]
        kb, vb = tb[i]
        if ka != kb:
            print(f"page {page_no}: token #{i} kind {ka} != {kb}")
            return False
        if ka == "n":
            if not close(va, vb, tol):
                print(f"page {page_no}: token #{i} number {va} != {vb}")
                return False
        elif va != vb:
            print(f"page {page_no}: token #{i} '{va}' != '{vb}'")
            return False
    return True


def main() -> int:
    args = sys.argv[1:]
    if len(args) < 2:
        print(__doc__)
        return 2
    a, b = args[0], args[1]
    tol = 0.01
    if "--tol" in args:
        tol = float(args[args.index("--tol") + 1])
    ra, rb = PdfReader(a), PdfReader(b)
    ok = True
    if len(ra.pages) != len(rb.pages):
        print(f"PAGE COUNT {len(ra.pages)} != {len(rb.pages)}")
        return 1
    for i, (pa, pb) in enumerate(zip(ra.pages, rb.pages), 1):
        if not cmp_page(pa, pb, i, tol):
            ok = False
            break
    ia, ib = ra.metadata or {}, rb.metadata or {}
    for key in ("/Title", "/Creator"):
        va, vb = ia.get(key), ib.get(key)
        if va != vb:
            print(f"info {key} '{va}' != '{vb}'")
    print("pages:", len(ra.pages), "| metadata:", dict(ia))
    print("MATCH" if ok else "MISMATCH")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
