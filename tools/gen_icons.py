#!/usr/bin/env python3
"""Writes every app icon format the packages need from the vector master.

    python tools/gen_icons.py

Reads assets/icons/openstitch.svg (made by tools/clean_logo.py from the Higgsfield logo) and
writes, in assets/icons/: PNGs 16-1024, openstitch.ico (Windows), openstitch.icns (macOS) and
openstitch-64.rgba (raw RGBA for the window icon, embedded by the app). Also writes
assets/brand/openstitch-wordmark*.png for places that can't show SVG. Needs Pillow and either
resvg or Inkscape on PATH (or in its default Windows folder) to rasterize the SVG.

At 32 px and below the needle and thread get a thicker outline in their own colour, so they
stay visible on a 16 px tab or taskbar icon.
"""

import pathlib
import re
import shutil
import subprocess
import tempfile

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "icons"
BRAND = ROOT / "assets" / "brand"
MASTER = OUT / "openstitch.svg"
TILE = "#1f3a5f"


def rasterizer():
    for exe in ("resvg", "inkscape", r"C:\Program Files\Inkscape\bin\inkscape.exe"):
        path = shutil.which(exe) or (exe if pathlib.Path(exe).is_file() else None)
        if path:
            return path
    raise SystemExit("needs resvg or Inkscape to rasterize the SVG")


def render(svg_text, out, width=None, height=None):
    exe = rasterizer()
    with tempfile.TemporaryDirectory() as tmp:
        src = pathlib.Path(tmp) / "in.svg"
        src.write_text(svg_text, encoding="utf-8")
        if "resvg" in pathlib.Path(exe).name:
            size = ["-w", str(width)] if width else ["-h", str(height)]
            subprocess.run([exe, *size, str(src), str(out)], check=True)
        else:
            size = [f"--export-width={width}"] if width else [f"--export-height={height}"]
            subprocess.run(
                [exe, str(src), "--export-type=png", f"--export-filename={out}", *size],
                check=True,
                stderr=subprocess.DEVNULL,
            )
    return Image.open(out).convert("RGBA")


def bold(svg_text, width):
    """The master with every non-tile shape outlined in its own colour."""
    return re.sub(
        r'<path fill="(#[0-9a-f]{6})"',
        lambda m: m.group(0)
        if m.group(1) == TILE
        else f'{m.group(0)} stroke="{m.group(1)}" stroke-width="{width}" stroke-linejoin="round"',
        svg_text,
    )


def main():
    svg = MASTER.read_text(encoding="utf-8")
    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        big = render(svg, tmp / "big.png", width=1024)
        small = {s: render(bold(svg, 3.0 * 32 / s * 16), tmp / f"s{s}.png", width=s * 8) for s in (16, 24, 32)}
        icon = {s: big.resize((s, s), Image.LANCZOS) for s in (48, 64, 128, 256, 512, 1024)}
        icon.update({s: im.resize((s, s), Image.LANCZOS) for s, im in small.items()})
        for s in (16, 32, 48, 64, 128, 256, 512, 1024):
            icon[s].save(OUT / f"openstitch-{s}.png", optimize=True)
        sizes = (16, 24, 32, 48, 64, 128, 256)
        icon[256].save(OUT / "openstitch.ico", sizes=[(s, s) for s in sizes],
                       append_images=[icon[s] for s in sizes if s != 256])
        big.save(OUT / "openstitch.icns")
        (OUT / "openstitch-64.rgba").write_bytes(icon[64].tobytes())
        for name in ("openstitch-wordmark", "openstitch-wordmark-dark"):
            text = (BRAND / f"{name}.svg").read_text(encoding="utf-8")
            render(text, tmp / "w.png", height=256).save(BRAND / f"{name}.png", optimize=True)


if __name__ == "__main__":
    main()
