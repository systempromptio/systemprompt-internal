#!/usr/bin/env python3
"""Render a self-contained factsheet HTML document to PDF and page images.

This is the whole of the Python surface. It holds no template, no content, no
brand knowledge and no assets: it reads finished HTML on stdin and writes a
PDF, because WeasyPrint's layout engine is Python and has no Rust binding.
Everything that decides what a factsheet says lives in the Rust engine.

stdin   the complete HTML document, fonts and SVGs already inlined
stdout  {"page_count": N, "page_images": [...]}
stderr  diagnostics on failure
"""
import argparse
import json
import sys
from pathlib import Path

# Renders at 2x A4 for a legible on-screen preview without a huge payload.
PREVIEW_SCALE = 2.0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pdf", required=True, type=Path, help="output PDF path")
    parser.add_argument("--png-dir", type=Path, help="directory for page previews")
    parser.add_argument("--png-prefix", default="page", help="page image filename prefix")
    args = parser.parse_args()

    html = sys.stdin.read()
    if not html.strip():
        print("factsheet-render: empty HTML on stdin", file=sys.stderr)
        return 2

    try:
        from weasyprint import HTML
    except ImportError as exc:
        print(
            f"factsheet-render: WeasyPrint is not installed ({exc}). "
            "Install it with its Pango/HarfBuzz system libraries.",
            file=sys.stderr,
        )
        return 3

    args.pdf.parent.mkdir(parents=True, exist_ok=True)
    # No base_url: the document is self-contained by construction, and giving it
    # one would let a future template quietly start depending on the filesystem.
    document = HTML(string=html).render()
    page_count = len(document.pages)
    document.write_pdf(str(args.pdf))

    page_images = render_previews(args, page_count)

    json.dump({"page_count": page_count, "page_images": page_images}, sys.stdout)
    return 0


def render_previews(args, page_count: int) -> list:
    """Rasterise each page. A missing PyMuPDF costs previews, not the PDF."""
    if args.png_dir is None:
        return []
    try:
        import pymupdf
    except ImportError:
        try:
            import fitz as pymupdf  # PyMuPDF < 1.24 module name
        except ImportError:
            print(
                "factsheet-render: PyMuPDF not installed; skipping page previews",
                file=sys.stderr,
            )
            return []

    args.png_dir.mkdir(parents=True, exist_ok=True)
    images = []
    matrix = pymupdf.Matrix(PREVIEW_SCALE, PREVIEW_SCALE)
    with pymupdf.open(str(args.pdf)) as doc:
        for index in range(min(page_count, doc.page_count)):
            out = args.png_dir / f"{args.png_prefix}-p{index + 1}.png"
            doc.load_page(index).get_pixmap(matrix=matrix).save(str(out))
            images.append(str(out))
    return images


if __name__ == "__main__":
    sys.exit(main())
