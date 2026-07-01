#!/usr/bin/env python3
"""Regenerate the Astound Bridge raster icons from the master `assets/icon.svg`.

Produces, idempotently, from the single Astound "A" master:
  - window-icon-1024.png  (1024x1024, GUI window + macOS .icns source)
  - tray-icon.png         (44x44, A on the rounded dark square, for the system tray)
  - app-icon.ico          (multi-resolution 16/32/48/256, embedded into the .exe)

Requires cairosvg and Pillow (both already present in this environment):
    python3 bridge/scripts/render-icons.py
"""

import io
from pathlib import Path

import cairosvg
from PIL import Image

ASSETS = Path(__file__).resolve().parent.parent / "assets"
MASTER = ASSETS / "icon.svg"


def render(svg_bytes: bytes, size: int) -> Image.Image:
    png = cairosvg.svg2png(bytestring=svg_bytes, output_width=size, output_height=size)
    return Image.open(io.BytesIO(png)).convert("RGBA")


def main() -> None:
    master = MASTER.read_bytes()

    render(master, 1024).save(ASSETS / "window-icon-1024.png")
    # Render the tray icon from the same master (A on the rounded dark square) so
    # it stays legible on both the dark macOS menu bar and a light Windows tray —
    # a bare monochrome glyph would vanish against one or the other.
    render(master, 44).save(ASSETS / "tray-icon.png")

    # Render at full resolution and let the ICO writer emit every frame. The base
    # image must be the largest size — Pillow's ICO writer silently drops any
    # requested size larger than the source image.
    ico_sizes = [16, 32, 48, 256]
    render(master, max(ico_sizes)).save(
        ASSETS / "app-icon.ico",
        format="ICO",
        sizes=[(s, s) for s in ico_sizes],
    )

    print("Regenerated:", ", ".join(
        f.name for f in (
            ASSETS / "window-icon-1024.png",
            ASSETS / "tray-icon.png",
            ASSETS / "app-icon.ico",
        )
    ))


if __name__ == "__main__":
    main()
