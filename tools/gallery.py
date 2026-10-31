#!/usr/bin/env python3
"""Regenerates the README's "From image to embroidery" gallery from the test images in
examples/images (docs/research/test-images.md).

    cargo build -p openstitch-app -p stitchtool
    python3 tools/gallery.py [--only ID,ID]

Each entry converts one image with `stitchtool exec` (a Design Page sized to the image, the
long side 100 mm, the wizard at the settings named in GALLERY), opens the result in the editor's
Realistic view (`openstitch --show realistic --screenshot`, 1440x900 window), crops the design
area and writes docs/images/readme/gallery/<id>.png (480 px wide, 128 colours). The README
table between the `gallery:start` and `gallery:end` markers is rewritten with the source, the
result and a caption (wizard, settings, stitches, trims, colours). The images in MODES are also
converted in both modes (`compatMode`) for the table between the `gallery-modes` markers. `--release` uses the release build. Needs Pillow.
"""

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / ("release" if "--release" in sys.argv else "debug")
IMAGES = ROOT / "examples" / "images"
OUT = ROOT / "docs" / "images" / "readme" / "gallery"
README = ROOT / "README.md"
# The design area of a 1440x900 editor window: below the top ruler and above the zoom readout.
# Its left and right edges move with the widths of the Sewing Order and attribute panes.
TOP, BOTTOM = 174, 765
PANE = (243, 244, 246)
LONG_SIDE_MM = 100

# id -> (image, wizard caption, command, parameters)
GALLERY = {
    "p01": ("p01-portrait.jpg", "Photo Stitch 1, colour, defaults",
            "digitize.photoStitch", {"style": "fill"}),
    "p02": ("p02-mono-portrait.jpg", "Photo Stitch 2, mono, defaults",
            "digitize.photoStitch", {"style": "lines", "mode": "mono"}),
    "p04": ("p04-cat-fur.jpg", "Photo Stitch 1, colour, defaults",
            "digitize.photoStitch", {"style": "fill"}),
    "p06": ("p06-sunset.jpg", "Photo Stitch 2, Auto Select",
            "digitize.photoStitch", {"style": "lines"}),
    "p08": ("p08-poppies.jpg", "Photo Stitch 2, Color Set CMYK",
            "digitize.photoStitch", {"style": "lines", "colorSet": "cmyk"}),
    "p12": ("p12-backlit-tree.jpg", "Photo Stitch 1, mono, defaults",
            "digitize.photoStitch", {"style": "fill", "mode": "mono"}),
    "p03": ("p03-storybook.jpg", "Photo Stitch 1, colour, defaults",
            "digitize.photoStitch", {"style": "fill"}),
    "g01": ("g01-emblem.png", "Auto Punch, defaults",
            "digitize.autoPunch", {}),
    "g02": ("g02-balloon.png", "Auto Punch, defaults",
            "digitize.autoPunch", {}),
    "g05": ("g05-robot-cartoon.png", "Auto Punch, defaults",
            "digitize.autoPunch", {}),
    "g08": ("g08-lettering.png", "Auto Punch, defaults",
            "digitize.autoPunch", {}),
    "g04": ("g04-butterfly-outline.png", "Design Center, running line",
            "digitize.designCenter", {}),
    "g07": ("g07-pixel-strawberry.png", "Cross Stitch, defaults",
            "digitize.crossStitch", {}),
    "g15": ("g15-folk-bird.png", "Cross Stitch, defaults",
            "digitize.crossStitch", {}),
}

# PE-DESIGN mode next to OpenStitch mode (`compatMode`, B16,
# docs/research/beyond/image-digitizing.md): id -> (image, wizard, command, parameters),
# rendered as <id>-pe and <id>-os.
MODES = {
    "g05": ("g05-robot-cartoon.png", "Auto Punch", "digitize.autoPunch", {}),
    "g06": ("g06-watercolour-lemons.jpg", "Auto Punch", "digitize.autoPunch", {}),
    "p03": ("p03-storybook.jpg", "Auto Punch", "digitize.autoPunch", {}),
    "p01": ("p01-portrait.jpg", "Photo Stitch 1", "digitize.photoStitch", {"style": "fill"}),
    "p06": ("p06-sunset.jpg", "Photo Stitch 1", "digitize.photoStitch", {"style": "fill"}),
    "p08": ("p08-poppies.jpg", "Photo Stitch 2", "digitize.photoStitch", {"style": "lines"}),
    "p04": ("p04-cat-fur.jpg", "Photo Stitch 2", "digitize.photoStitch", {"style": "lines"}),
}
for _key, (_image, _wizard, _command, _params) in MODES.items():
    GALLERY[f"{_key}-pe"] = (_image, f"{_wizard}, PE-DESIGN mode", _command, _params)
    GALLERY[f"{_key}-os"] = (_image, f"{_wizard}, OpenStitch mode", _command,
                             {**_params, "compatMode": "openStitch"})


def run(args, cwd):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout


def convert(work, key):
    image, _, command, params = GALLERY[key]
    w, h = Image.open(IMAGES / image).size
    scale = LONG_SIDE_MM / max(w, h)
    width, height = round(w * scale, 1), round(h * scale, 1)
    steps = [
        {"command": "design.new", "params": {"width": width + 10, "height": height + 10}},
        {"command": command, "params": {"path": image, "width": width, **params}},
        {"command": "design.info"},
        {"command": "project.save", "params": {"path": f"{key}.osd"}},
    ]
    (work / f"{key}.json").write_text(json.dumps(steps), encoding="utf-8")
    replies = [json.loads(line) for line in run([str(BIN / "stitchtool"), "exec", f"{key}.json"], work).splitlines()
               if line.strip()]
    return replies[2]["result"]["stitchStats"]


def shoot(work, key):
    shot = work / f"{key}.png"
    run([str(BIN / "openstitch"), f"{key}.osd", "--show", "realistic", "--no-splash",
         "--size", "1440x900", "--screenshot", str(shot)], work)
    im = Image.open(shot).convert("RGB")
    im = im.crop(design_box(im))
    im = im.resize((480, round(im.height * 480 / im.width)), Image.LANCZOS)
    OUT.mkdir(parents=True, exist_ok=True)
    dst = OUT / f"{key}.png"
    im.quantize(colors=128, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE).save(dst, optimize=True)
    return dst


def design_box(im):
    """The stitched area of the canvas, with a small margin: pixels that are coloured or dark
    (thread), not the light grey fabric, the grey surround or the rulers."""
    # Just above the canvas the Sewing Order pane's background runs up to the pane splitter,
    # the vertical ruler (22 px) follows it, and the top ruler runs on to the attribute pane.
    left = next(x for x in range(200, im.width) if im.getpixel((x, TOP - 4)) != PANE) + 24
    right = next(x for x in range(left, im.width) if im.getpixel((x, TOP - 4)) == PANE) - 2
    xs, ys = [], []
    for yy in range(TOP, BOTTOM, 2):
        for xx in range(left, right, 2):
            r, g, b = im.getpixel((xx, yy))
            if max(r, g, b) - min(r, g, b) > 60 or max(r, g, b) < 120:
                xs.append(xx)
                ys.append(yy)
    if not xs:
        return (left, TOP, right, BOTTOM)
    pad = 12
    return (max(left, min(xs) - pad), max(TOP, min(ys) - pad),
            min(right, max(xs) + pad), min(BOTTOM, max(ys) + pad))


def result(s):
    if not s:
        return ""
    n = len(s["colors"])
    return f"{s['stitches']:,} stitches, {s['trims']} trims, {n} colour{'' if n == 1 else 's'}"


def table(stats):
    rows = ["| Image | OpenStitch (Realistic view) | Wizard and result |", "|---|---|---|"]
    for key, (image, wizard, _, _) in GALLERY.items():
        if key[-3:] in ("-pe", "-os"):
            continue
        rows.append(f"| <img src=\"examples/images/{image}\" width=\"200\"> "
                    f"| <img src=\"docs/images/readme/gallery/{key}.png\" width=\"260\"> "
                    f"| **{wizard}**<br>{result(stats.get(key))} |")
    return "\n".join(rows)


def modes_table(stats):
    rows = ["| Image | PE-DESIGN mode | OpenStitch mode |", "|---|---|---|"]
    for key, (image, wizard, _, _) in MODES.items():
        rows.append(f"| <img src=\"examples/images/{image}\" width=\"200\"><br>{wizard} "
                    f"| <img src=\"docs/images/readme/gallery/{key}-pe.png\" width=\"260\"><br>"
                    f"{result(stats.get(key + '-pe'))} "
                    f"| <img src=\"docs/images/readme/gallery/{key}-os.png\" width=\"260\"><br>"
                    f"{result(stats.get(key + '-os'))} |")
    return "\n".join(rows)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--only", help="comma-separated ids")
    p.add_argument("--release", action="store_true", help="use the release build (faster)")
    a = p.parse_args()
    keys = a.only.split(",") if a.only else list(GALLERY)
    cache = OUT / "stats.json"
    stats = json.loads(cache.read_text(encoding="utf-8")) if cache.exists() else {}
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        for key in keys:
            shutil.copy(IMAGES / GALLERY[key][0], work / GALLERY[key][0])
            s = convert(work, key)
            stats[key] = {"stitches": s["stitches"], "colors": s["colors"], "trims": s["trims"]}
            dst = shoot(work, key)
            print(f"{key}: {s['stitches']} stitches, {len(s['colors'])} colours, {dst.stat().st_size // 1024} KB")
    cache.write_text(json.dumps(stats, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    text = README.read_text(encoding="utf-8")
    for marker, make in (("gallery", table), ("gallery-modes", modes_table)):
        text, n = re.subn(rf"(<!-- {marker}:start -->\n).*?(<!-- {marker}:end -->)",
                          lambda m: m.group(1) + make(stats) + "\n" + m.group(2), text, flags=re.S)
        if n != 1:
            sys.exit(f"README.md has no {marker}:start / {marker}:end markers")
    README.write_text(text, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    sys.exit(main())
