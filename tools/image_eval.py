#!/usr/bin/env python3
"""Measures image-to-embroidery results on the test images (docs/research/test-images.md):
how far the sewn colours are from the image, and what the design costs to sew.

    cargo build --release -p stitchtool
    python -I tools/image_eval.py [--only ID,ID] [--set NAME=JSON] [--out FILE.json]

Each case converts one image of examples/images with `stitchtool serve` at the settings named
in CASES: a Design Page the size of the image (long side 100 mm), the image placed at its
top-left corner. The objects are converted to stitches and drawn as 0.4 mm thread lines on
white fabric, at the image's own pixel scale. The measures:

- **dE**: CIEDE2000 between the image and the drawing, both averaged over 2 mm cells; mean
  and 90th percentile over the cells. Cells the image shows as background and the wizard
  leaves out count too (fabric against the image's colour). **sewn**: the mean over the cells
  at least 30 % covered by thread, the colour error of what is sewn.
- **stitches**, **trims**, **colours**: from `design.info`.
- **objects** and **specks**: the objects sewn, and those whose stitches cover less than
  2 mm² (an estimate from their needle points' footprint).

`--set` adds digitize parameters to every case, for comparing settings or modes, e.g.
`--set compatMode='"openStitch"'`. Needs numpy and Pillow.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "stitchtool"
IMAGES = ROOT / "examples" / "images"
LONG_SIDE_MM = 100.0
THREAD_MM = 0.4
CELL_MM = 2.0
SPECK_MM2 = 2.0

# id -> (image, command, parameters)
CASES = {
    "g01-ap": ("g01-emblem.png", "digitize.autoPunch", {}),
    "g02-ap": ("g02-balloon.png", "digitize.autoPunch", {}),
    "g05-ap": ("g05-robot-cartoon.png", "digitize.autoPunch", {}),
    "g06-ap": ("g06-watercolour-lemons.jpg", "digitize.autoPunch", {}),
    "g08-ap": ("g08-lettering.png", "digitize.autoPunch", {}),
    "g11-ap": ("g11-mandala.png", "digitize.autoPunch", {}),
    "g13-ap": ("g13-mosaic.png", "digitize.autoPunch", {}),
    "g15-ap": ("g15-folk-bird.png", "digitize.autoPunch", {}),
    "p03-ap": ("p03-storybook.jpg", "digitize.autoPunch", {}),
    "d01-ap": ("d01-fruit-artefacts.jpg", "digitize.autoPunch", {}),
    "g07-cs": ("g07-pixel-strawberry.png", "digitize.crossStitch", {}),
    "g15-cs": ("g15-folk-bird.png", "digitize.crossStitch", {}),
    "g02-cs": ("g02-balloon.png", "digitize.crossStitch", {}),
    "g05-cs": ("g05-robot-cartoon.png", "digitize.crossStitch", {}),
    "g12-cs": ("g12-deer-stencil.png", "digitize.crossStitch", {}),
    "p03-cs": ("p03-storybook.jpg", "digitize.crossStitch", {}),
    "p04-cs": ("p04-cat-fur.jpg", "digitize.crossStitch", {}),
    "p01-ps1": ("p01-portrait.jpg", "digitize.photoStitch", {"style": "fill"}),
    "p04-ps1": ("p04-cat-fur.jpg", "digitize.photoStitch", {"style": "fill"}),
    "p07-ps1": ("p07-fruit.jpg", "digitize.photoStitch", {"style": "fill"}),
    "p03-ps1": ("p03-storybook.jpg", "digitize.photoStitch", {"style": "fill"}),
    "p05-ps1": ("p05-dog.jpg", "digitize.photoStitch", {"style": "fill"}),
    "p06-ps1": ("p06-sunset.jpg", "digitize.photoStitch", {"style": "fill"}),
    "p10-ps1": ("p10-night-street.jpg", "digitize.photoStitch", {"style": "fill"}),
    "p11-ps1": ("p11-high-key.jpg", "digitize.photoStitch", {"style": "fill"}),
    "g09-ps1": ("g09-gradient.jpg", "digitize.photoStitch", {"style": "fill"}),
    "p08-ps2": ("p08-poppies.jpg", "digitize.photoStitch", {"style": "lines"}),
    "g09-ps2": ("g09-gradient.jpg", "digitize.photoStitch", {"style": "lines"}),
    "p01-ps2": ("p01-portrait.jpg", "digitize.photoStitch", {"style": "lines"}),
    "p04-ps2": ("p04-cat-fur.jpg", "digitize.photoStitch", {"style": "lines"}),
    "p06-ps2": ("p06-sunset.jpg", "digitize.photoStitch", {"style": "lines"}),
    "p07-ps2": ("p07-fruit.jpg", "digitize.photoStitch", {"style": "lines"}),
    "p08-cmyk": ("p08-poppies.jpg", "digitize.photoStitch", {"style": "lines", "colorSet": "cmyk"}),
}


# --- colour science -----------------------------------------------------------------------

def srgb_to_lab(rgb):
    """sRGB 0-255 array (..., 3) to CIELAB (D65)."""
    c = rgb.astype(np.float64) / 255.0
    c = np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    m = np.array([[0.4124, 0.3576, 0.1805], [0.2126, 0.7152, 0.0722], [0.0193, 0.1192, 0.9505]])
    xyz = c @ m.T / np.array([0.95047, 1.0, 1.08883])
    f = np.where(xyz > 216 / 24389, np.cbrt(xyz), (24389 / 27 * xyz + 16) / 116)
    return np.stack([116 * f[..., 1] - 16, 500 * (f[..., 0] - f[..., 1]),
                     200 * (f[..., 1] - f[..., 2])], axis=-1)


def de2000(lab1, lab2):
    """CIEDE2000 between two Lab arrays of the same shape (..., 3)."""
    L1, a1, b1 = lab1[..., 0], lab1[..., 1], lab1[..., 2]
    L2, a2, b2 = lab2[..., 0], lab2[..., 1], lab2[..., 2]
    C1, C2 = np.hypot(a1, b1), np.hypot(a2, b2)
    Cb7 = ((C1 + C2) / 2) ** 7
    G = 0.5 * (1 - np.sqrt(Cb7 / (Cb7 + 25.0 ** 7)))
    a1p, a2p = (1 + G) * a1, (1 + G) * a2
    C1p, C2p = np.hypot(a1p, b1), np.hypot(a2p, b2)
    h1p = np.degrees(np.arctan2(b1, a1p)) % 360
    h2p = np.degrees(np.arctan2(b2, a2p)) % 360
    dLp, dCp = L2 - L1, C2p - C1p
    dh = h2p - h1p
    dh = np.where(dh > 180, dh - 360, np.where(dh < -180, dh + 360, dh))
    dh = np.where(C1p * C2p == 0, 0, dh)
    dHp = 2 * np.sqrt(C1p * C2p) * np.sin(np.radians(dh / 2))
    Lbp, Cbp = (L1 + L2) / 2, (C1p + C2p) / 2
    hs = h1p + h2p
    hbp = np.where(np.abs(h1p - h2p) > 180, np.where(hs < 360, hs + 360, hs - 360), hs) / 2
    hbp = np.where(C1p * C2p == 0, hs, hbp)
    T = (1 - 0.17 * np.cos(np.radians(hbp - 30)) + 0.24 * np.cos(np.radians(2 * hbp))
         + 0.32 * np.cos(np.radians(3 * hbp + 6)) - 0.20 * np.cos(np.radians(4 * hbp - 63)))
    SL = 1 + 0.015 * (Lbp - 50) ** 2 / np.sqrt(20 + (Lbp - 50) ** 2)
    SC, SH = 1 + 0.045 * Cbp, 1 + 0.015 * Cbp * T
    Cbp7 = Cbp ** 7
    RT = -2 * np.sqrt(Cbp7 / (Cbp7 + 25.0 ** 7)) * np.sin(np.radians(60 * np.exp(-((hbp - 275) / 25) ** 2)))
    return np.sqrt((dLp / SL) ** 2 + (dCp / SC) ** 2 + (dHp / SH) ** 2 + RT * (dCp / SC) * (dHp / SH))


def cells(img, cell_px):
    """Mean colour of every cell_px × cell_px cell (partial cells at the edges dropped)."""
    h, w = img.shape[0] // cell_px * cell_px, img.shape[1] // cell_px * cell_px
    a = img[:h, :w].reshape(h // cell_px, cell_px, w // cell_px, cell_px, 3)
    return a.mean(axis=(1, 3))


# --- conversion ---------------------------------------------------------------------------

class Tool:
    def __init__(self, cwd):
        self.p = subprocess.Popen([str(BIN), "serve"], cwd=cwd, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, text=True, encoding="utf-8")

    def __call__(self, command, **params):
        self.p.stdin.write(json.dumps({"command": command, "params": params}) + "\n")
        self.p.stdin.flush()
        reply = json.loads(self.p.stdout.readline())
        if not reply.get("ok"):
            raise RuntimeError(f"{command}: {reply.get('error')}")
        return reply["result"]

    def close(self):
        self.p.stdin.close()
        self.p.wait()


def source_rgb(path):
    """The image as RGB on white (transparent pixels show the fabric)."""
    im = Image.open(path).convert("RGBA")
    bg = Image.new("RGBA", im.size, (255, 255, 255, 255))
    return np.asarray(Image.alpha_composite(bg, im).convert("RGB"))


def run_case(tool, image, command, params):
    w, h = Image.open(IMAGES / image).size
    mm_per_px = LONG_SIDE_MM / max(w, h)
    width, height = w * mm_per_px, h * mm_per_px
    tool("design.new", width=round(width + 0.5, 1), height=round(height + 0.5, 1))
    tool(command, path=str(IMAGES / image), width=width, x=0, y=0, **params)
    info = tool("design.info")
    stats = info["stitchStats"]
    ids = [o["id"] for o in tool("object.list")]
    try:
        tool("object.toStitches", ids=ids)
    except RuntimeError as e:  # Cross Stitch and Photo Stitch make stitch patterns already
        if "no shape or text objects" not in str(e):
            raise
    draw_scale = 1 / mm_per_px  # px per mm
    canvas = Image.new("RGB", (w, h), (255, 255, 255))
    d = ImageDraw.Draw(canvas)
    sewn = Image.new("L", (w, h), 0)
    ds = ImageDraw.Draw(sewn)
    line = max(1, round(THREAD_MM * draw_scale))
    specks = 0
    objects = 0
    for o in tool("object.list"):
        pts = tool("stitches.points", id=o["id"])
        threads = pts["threads"]
        prev = None
        xs, ys = [], []
        for p in pts["points"]:
            here = (p["x"] * draw_scale, p["y"] * draw_scale)
            if p["command"] == "stitch":
                if prev is not None:
                    d.line([prev, here], fill=threads[p["color"]], width=line)
                    ds.line([prev, here], fill=255, width=line)
                prev = here
                xs.append(p["x"])
                ys.append(p["y"])
            else:
                prev = None if p["command"] in ("trim", "colorChange") else here
        if xs:
            objects += 1
            if (max(xs) - min(xs)) * (max(ys) - min(ys)) < SPECK_MM2:
                specks += 1
    src = source_rgb(IMAGES / image)
    out = np.asarray(canvas)
    cell_px = max(1, round(CELL_MM * draw_scale))
    de = de2000(srgb_to_lab(cells(src, cell_px)), srgb_to_lab(cells(out, cell_px)))
    cover = cells(np.repeat(np.asarray(sewn)[..., None], 3, axis=2), cell_px)[..., 0] / 255
    sewn_de = de[cover >= 0.3]
    return {
        "dE": round(float(de.mean()), 2), "dE90": round(float(np.percentile(de, 90)), 2),
        "dEsewn": round(float(sewn_de.mean()), 2) if sewn_de.size else None,
        "stitches": stats["stitches"], "trims": stats["trims"], "colours": len(stats["colors"]),
        "objects": objects, "specks": specks,
    }, canvas


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", help="comma-separated case ids")
    ap.add_argument("--set", action="append", default=[], help="NAME=JSON parameter for every case")
    ap.add_argument("--out", help="write the results as JSON")
    ap.add_argument("--renders", help="folder for the drawings")
    a = ap.parse_args()
    extra = {k: json.loads(v) for k, v in (s.split("=", 1) for s in a.set)}
    keys = a.only.split(",") if a.only else list(CASES)
    tool = Tool(ROOT)
    results = {}
    print(f"{'case':10} {'dE':>6} {'dE90':>6} {'sewn':>6} {'stitches':>9} {'trims':>6} {'col':>4} {'obj':>5} {'specks':>6}")
    try:
        for key in keys:
            image, command, params = CASES[key]
            try:
                r, canvas = run_case(tool, image, command, {**params, **extra})
            except RuntimeError as e:
                print(f"{key:10} failed: {e}")
                continue
            results[key] = r
            if a.renders:
                Path(a.renders).mkdir(parents=True, exist_ok=True)
                canvas.save(Path(a.renders) / f"{key}.png")
            print(f"{key:10} {r['dE']:6.2f} {r['dE90']:6.2f} {r['dEsewn'] or 0:6.2f} {r['stitches']:9} {r['trims']:6} "
                  f"{r['colours']:4} {r['objects']:5} {r['specks']:6}")
    finally:
        tool.close()
    if a.out:
        Path(a.out).write_text(json.dumps(results, indent=1) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    sys.exit(main())
