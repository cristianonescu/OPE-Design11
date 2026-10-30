#!/usr/bin/env python3
"""Draws an examples/*.json design as a docs/images/engine-*.png stitch plot.

    cargo build -p stitchtool
    python3 tools/engine_image.py examples/programmable-fill.json docs/images/engine-programmable-fill.png

The script runs with `stitchtool exec` in a temporary folder, the design is exported as PES and
its stitches are read back (tools/pescmp.py's reader). Each colour block is drawn in the thread
colour of the object it belongs to (the example's shapes in order, one block each), stitches as
lines and needle points as black dots, on white, at the width given (default 1600 px) with a
small margin. Jumps and trims are not drawn. Needs numpy and Pillow.
"""

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "tools"))
import pescmp  # noqa: E402

EXE = ROOT / "target" / "debug" / ("stitchtool.exe" if sys.platform == "win32" else "stitchtool")
MARGIN = 16


def run(script: Path, work: Path):
    steps = json.loads(script.read_text(encoding="utf-8"))
    ids = [i + 1 for i, s in enumerate(s for s in steps if s.get("command", "").startswith("shape."))]
    steps = steps + [{"command": "design.export", "params": {"path": "out.pes"}}]
    steps += [{"command": "object.get", "params": {"id": i}, "id": f"object-{i}"} for i in ids]
    (work / "script.json").write_text(json.dumps(steps), encoding="utf-8")
    out = subprocess.run([str(EXE), "exec", str(work / "script.json")], capture_output=True, text=True, check=True)
    replies = [json.loads(line) for line in out.stdout.splitlines() if line.strip()]
    colours = []
    for r in replies[-len(ids):]:
        obj = r.get("result", {})
        t = (obj.get("region") or obj.get("line") or {}).get("thread", {})
        colours.append((t.get("red", 0), t.get("green", 0), t.get("blue", 0)))
    return pescmp.read(work / "out.pes"), colours


def draw(f, colours, width):
    pts, kinds = f["pts"], f["kinds"]
    sewn = [i for i, k in enumerate(kinds) if k == "stitch"]
    xs = [pts[i][0] for i in sewn]
    ys = [pts[i][1] for i in sewn]
    x0, y0 = min(xs), min(ys)
    scale = (width - 2 * MARGIN) / (max(xs) - x0)
    height = int(round((max(ys) - y0) * scale)) + 2 * MARGIN
    img = Image.new("RGB", (width, height), "white")
    d = ImageDraw.Draw(img)
    at = lambda i: (MARGIN + (pts[i][0] - x0) * scale, MARGIN + (pts[i][1] - y0) * scale)  # noqa: E731
    for a, b in zip(range(len(kinds)), range(1, len(kinds))):
        if kinds[a] == "stitch" and kinds[b] == "stitch" and pts[a][2] == pts[b][2]:
            block = int(pts[b][2])
            d.line([at(a), at(b)], fill=colours[min(block, len(colours) - 1)], width=1)
    for i in sewn:
        x, y = at(i)
        d.ellipse([x - 1.6, y - 1.6, x + 1.6, y + 1.6], fill="black")
    return img


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("script", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--width", type=int, default=1600)
    a = ap.parse_args()
    work = Path(tempfile.mkdtemp(prefix="engine-image-"))
    try:
        f, colours = run(a.script.resolve(), work)
        img = draw(f, colours, a.width)
        img.quantize(colors=64).save(a.out, optimize=True)
        print(a.out, img.size, len(f["kinds"]), "records")
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
