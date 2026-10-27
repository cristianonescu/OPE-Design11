#!/usr/bin/env python3
"""Turn the SVGs generated for the command icons into the files in assets/icons/commands.

The icons were generated with Higgsfield (Recraft V4.1, vector output) from the prompts in
assets/icons/commands/prompts.tsv. This script strips the generator's metadata and the white
background square, drops the fixed size, and snaps near-palette fills onto the palette so the
app can swap the charcoal outline for the theme's text colour.

    python tools/clean_icons.py RAW_DIR assets/icons/commands
"""

import pathlib
import re
import sys

# Charcoal outline (recoloured by the app), white, light grey and the accent colours.
PALETTE = [
    (0x2B, 0x2F, 0x36),
    (0xFF, 0xFF, 0xFF),
    (0xA0, 0xA6, 0xAE),
    (0x2A, 0x7A, 0xD8),
    (0xD8, 0x43, 0x3A),
    (0x2E, 0x9D, 0x58),
    (0xE0, 0xA2, 0x1B),
    (0x8A, 0x52, 0xC8),
    (0xF2, 0x8C, 0xB8),
]
SNAP = 70


def snap(m):
    c = tuple(int(v) for v in m.group(1).split(","))
    best = min(PALETTE, key=lambda p: sum((a - b) ** 2 for a, b in zip(c, p)))
    if sum((a - b) ** 2 for a, b in zip(c, best)) <= SNAP * SNAP:
        c = best
    return 'fill="#%02x%02x%02x"' % c


def clean(s):
    s = re.sub(r"<metadata>.*?</metadata>", "", s, flags=re.S)
    s = re.sub(r'<path[^>]*d="M 0 0 L 2048 0 L 2048 2048 L 0 2048 L 0 0 z"/>', "", s)
    s = re.sub(r"<svg [^>]*>", '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2048 2048">', s, count=1)
    s = s.replace(' transform="translate(0,0)"', "")
    s = re.sub(r'fill="rgb\(([0-9,]+)\)"', snap, s)
    s = re.sub(r"(\d+\.\d)\d+", r"\1", s)
    s = s.replace("/><", "/>\n<")
    return re.sub(r"\n\s*\n", "\n", s).strip() + "\n"


def main():
    src, dst = map(pathlib.Path, sys.argv[1:3])
    dst.mkdir(parents=True, exist_ok=True)
    for f in sorted(src.glob("*.svg")):
        (dst / f.name).write_text(clean(f.read_text(encoding="utf-8")), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
