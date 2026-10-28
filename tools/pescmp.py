"""Compare an OpenStitch PES with a PE-DESIGN PES of the same design, using only documented fields.

PES header: '#PES' + 4-char version, PEC offset (u32 LE). Object classes are found by name.
PEC: colour count and palette indices, then the stitch stream (7-bit short / 12-bit long form with
jump 0x10 and trim 0x20 flags, FE B0 xx colour change, FF 00 end), in 0.1 mm units.
"""
import json
import re
import struct
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

CLASSES = [b"CEmbOne", b"CSewSeg", b"CEmbRect", b"CEmbCirc", b"CEmbLine", b"CEmbPunch",
           b"CEmbText", b"CEmbOutline", b"CEmbPolygon"]


def s12(v):
    v &= 0xFFF
    return v - 0x1000 if v > 0x7FF else v


def s7(v):
    return v - 0x80 if v > 0x3F else v


def read(path):
    d = Path(path).read_bytes()
    version = d[4:8].decode("latin-1")
    pec = struct.unpack_from("<I", d, 8)[0]
    classes = sorted({m.decode() for m in re.findall(rb"C(?:Emb|Sew)[A-Za-z]+", d[:pec])})
    p = pec + 3 + 16 + 0xF + 2 + 0xC
    ncol = d[p] + 1
    palette = list(d[p + 1:p + 1 + ncol])
    p = p + 1 + ncol + (0x1D0 - (ncol - 1))
    p += 3 + 0x0B
    x = y = 0
    pts, kinds, colour = [], [], 0
    jumps = trims = 0
    while p + 1 < len(d):
        a, b = d[p], d[p + 1]
        p += 2
        if a == 0xFF and b == 0x00:
            break
        if a == 0xFE and b == 0xB0:
            p += 1
            colour += 1
            continue
        jump = trim = False
        if a & 0x80:
            jump |= bool(a & 0x10)
            trim |= bool(a & 0x20)
            dx = s12((a << 8) | b)
            b = d[p]
            p += 1
        else:
            dx = s7(a)
        if b & 0x80:
            jump |= bool(b & 0x10)
            trim |= bool(b & 0x20)
            dy = s12((b << 8) | d[p])
            p += 1
        else:
            dy = s7(b)
        x += dx
        y += dy
        kind = "trim" if trim else "jump" if jump else "stitch"
        jumps += kind == "jump"
        trims += kind == "trim"
        pts.append((x / 10.0, y / 10.0, colour))
        kinds.append(kind)
    a = np.array(pts, dtype=float)
    return {"version": version, "size": len(d), "classes": classes, "colours": ncol,
            "palette": palette, "pts": a, "kinds": kinds, "jumps": jumps, "trims": trims,
            "colour_changes": colour}


def needle(f):
    m = np.array([k == "stitch" for k in f["kinds"]])
    return f["pts"][m][:, :2]


def nearest(a, b):
    out = np.empty(len(a))
    for i in range(0, len(a), 2000):
        c = a[i:i + 2000, None, :] - b[None, :, :]
        out[i:i + 2000] = np.sqrt((c ** 2).sum(-1)).min(1)
    return out


def raster(f, origin, extent, scale=5):
    """Stitch lines drawn 2 px wide at `scale` px/mm. A stitch is drawn the same way whichever
    way it is sewn: PIL puts a 2 px line's second pixel on a side that depends on the drawing
    direction, so the endpoints are put in a fixed order first (without this, a design and its
    own reversed stream overlap by only 0.55). Pixel positions are rounded from the file's
    0.1 mm units, so equal coordinates land on equal pixels whatever the origin."""
    img = Image.new("L", (int(extent[0] * scale) + 4, int(extent[1] * scale) + 4), 0)
    g = ImageDraw.Draw(img)
    pts, kinds = f["pts"], f["kinds"]

    def px(v, o):
        return int(np.floor(round((v - o) * 10.0) * scale / 10.0 + 0.5)) + 2

    for i in range(1, len(pts)):
        if kinds[i] != "stitch" or kinds[i - 1] == "trim":
            continue
        a = (px(pts[i - 1][0], origin[0]), px(pts[i - 1][1], origin[1]))
        b = (px(pts[i][0], origin[0]), px(pts[i][1], origin[1]))
        g.line(sorted([a, b]), fill=255, width=2)
    return np.array(img) > 0


def compare(ours, theirs, overlay=None):
    a, b = read(ours), read(theirs)
    na, nb = needle(a), needle(b)
    oa, ob = na.min(0), nb.min(0)
    na0, nb0 = na - oa, nb - ob
    ea, eb = na0.max(0), nb0.max(0)
    da, db = nearest(na0, nb0), nearest(nb0, na0)
    ext = np.maximum(ea, eb)
    ra, rb = raster(a, oa, ext), raster(b, ob, ext)
    h = min(ra.shape[0], rb.shape[0]); w = min(ra.shape[1], rb.shape[1])
    ra, rb = ra[:h, :w], rb[:h, :w]
    iou = float((ra & rb).sum() / max(1, (ra | rb).sum()))
    if overlay:
        img = np.full((h, w, 3), 255, np.uint8)
        img[rb] = (230, 60, 60)            # PE-DESIGN only: red
        img[ra] = (40, 90, 220)            # OpenStitch only: blue
        img[ra & rb] = (90, 40, 120)       # both: purple
        Image.fromarray(img).save(overlay)
    def side(f, n, e):
        return {"version": f["version"], "bytes": f["size"], "classes": f["classes"],
                "colours": f["colours"], "palette": f["palette"], "stitches": int(len(n)),
                "jumps": f["jumps"], "trims": f["trims"], "size_mm": [round(float(e[0]), 2), round(float(e[1]), 2)]}
    return {"openstitch": side(a, na, ea), "pe_design": side(b, nb, eb),
            "needle_to_nearest_mm": {
                "ours_mean": round(float(da.mean()), 3), "ours_p95": round(float(np.percentile(da, 95)), 3),
                "pe_mean": round(float(db.mean()), 3), "pe_p95": round(float(np.percentile(db, 95)), 3)},
            "coverage_iou": round(iou, 3)}


if __name__ == "__main__":
    print(json.dumps(compare(sys.argv[1], sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)))
