#!/usr/bin/env python3
"""Fit the Text Transform presets from PE-DESIGN 11.50 reference files.

    cargo build -p stitchtool
    python tools/fit_envelopes.py > crates/openstitch-design/src/envelope_presets.rs
    cargo fmt --all

Each reference (`compat/reference/text/text-transform-N.pes`, made by pe-gui) is the text
HHHHHHH in Lato Bold at 10 mm, centred, with preset N applied. The straight text's box is
x 12.7..87.3 mm, y 39.7..50.0 mm (PE's status bar: 74.60 x 10.30 mm). For every preset whose
letters keep their x positions, the stems of the H's give the envelope's top and bottom edge:
the highest and lowest needle point of each stem column. Each edge is a least-squares fit of
those samples (fractions of the straight box) on a constant, a slope and eight cosine terms,
dropping outliers; for curved presets each knot (u = k/32 along the text) is the outermost
of the samples near it, so peaks are kept, and the fit fills the gaps between letters. Slants
and fans are fitted from the letters' corners: a fan keeps one edge of the straight box and
scales the other about the text's centre, each letter sheared linearly between them. The arcs
(1 and 2) are not envelopes of this kind and are left out of the table.

The stitchtool binary: target/debug/stitchtool, or the path in STITCHTOOL.
"""

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOL = Path(os.environ.get("STITCHTOOL") or
            ROOT / "target" / "debug" / ("stitchtool.exe" if os.name == "nt" else "stitchtool"))
REF = ROOT / "compat" / "reference" / "text"
X0, X1, YT, YB = 12.7, 87.3, 39.7, 50.0
W, H = X1 - X0, YB - YT
KNOTS = 33
# Presets whose letters stay at their x positions; 9 and 10 slant.
COLUMN = [3, 4, 5, 6, 7, 8, 11, 12, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26]
SLANT = [9, 10]
# Fans: 13 shrinks the bottom edge about the centre, 14 the top edge.
FAN = [13, 14]
LETTER = 8.7


def needle_points(path):
    with tempfile.TemporaryDirectory() as d:
        (Path(d) / "a.pes").write_bytes(path.read_bytes())
        script = [
            {"command": "stitches.import", "params": {"path": "a.pes"}},
            {"command": "stitches.points", "params": {"id": 1}},
        ]
        (Path(d) / "s.json").write_text(json.dumps(script))
        out = subprocess.run([str(TOOL), "exec", "s.json"], cwd=d, capture_output=True,
                             text=True, check=True).stdout.splitlines()
    points = json.loads(out[1])["result"]["points"]
    letters, cur = [], []
    for p in points:
        if p["command"] != "stitch":
            if len(cur) > 20:
                letters.append(cur)
            cur = []
        else:
            cur.append((p["x"], p["y"]))
    if len(cur) > 20:
        letters.append(cur)
    return letters


def stem_samples(letters):
    """(u, top, bottom) per 0.3 mm column of each H stem, as fractions of the box."""
    out = []
    for pts in letters:
        bins = {}
        for x, y in pts:
            bins.setdefault(round(x / 0.3), []).append(y)
        tall = max(max(v) - min(v) for v in bins.values())
        for b, ys in sorted(bins.items()):
            # Stems run the letter's full height; the crossbar is a short band between them.
            if len(ys) >= 5 and max(ys) - min(ys) > 0.8 * tall:
                out.append(((b * 0.3 - X0) / W, (min(ys) - YT) / H, (max(ys) - YT) / H))
    return sorted(out)


LINEAR = {3, 4, 5, 6, 7, 8, 11, 12}
# Mirror pairs (the second is the first read right to left) and left-right symmetric presets:
# their samples are pooled, so both sides get the same fit.
PAIRS = [(3, 4), (5, 6), (7, 8), (11, 12), (19, 20)]
SYMMETRIC = {15, 16, 17, 18, 21, 22, 23, 24, 25, 26}


def basis(u, linear=False):
    """Constant and slope, plus eight cosine terms for bows, peaks, waves and double waves."""
    import math
    if linear:
        return [1.0, u]
    return [1.0, u] + [math.cos(j * math.pi * u) for j in range(1, 9)]


def solve(a, b):
    n = len(b)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for c in range(n):
        p = max(range(c, n), key=lambda r: abs(m[r][c]))
        m[c], m[p] = m[p], m[c]
        for r in range(n):
            if r != c and m[c][c] != 0:
                f = m[r][c] / m[c][c]
                m[r] = [x - f * y for x, y in zip(m[r], m[c])]
    return [m[i][n] / m[i][i] for i in range(n)]


def fit(samples, k, linear=False):
    """Least squares on `basis`, dropping samples 0.03 of the box off the fit, twice."""
    pts = [(u, v[k - 1]) for u, *v in samples]
    m = len(basis(0.0, linear))
    for _ in range(3):
        rows = [basis(u, linear) for u, _ in pts]
        a = [[sum(r[i] * r[j] for r in rows) for j in range(m)] for i in range(m)]
        b = [sum(r[i] * y for r, (_, y) in zip(rows, pts)) for i in range(m)]
        c = solve(a, b)
        f = lambda u, c=c: sum(ci * bi for ci, bi in zip(c, basis(u, linear)))
        kept = [(u, y) for u, y in pts if abs(f(u) - y) < 0.03]
        if len(kept) == len(pts) or len(kept) < 12:
            break
        pts = kept
    return f


def interpolate(samples, u, k):
    if u <= samples[0][0]:
        return samples[0][k]
    if u >= samples[-1][0]:
        return samples[-1][k]
    for a, b in zip(samples, samples[1:]):
        if a[0] <= u <= b[0]:
            t = 0 if b[0] == a[0] else (u - a[0]) / (b[0] - a[0])
            return a[k] + t * (b[k] - a[k])
    return samples[-1][k]


def samples_for(n):
    """The stem samples of preset n, pooled with its mirror twin's or its own mirror image."""
    own = stem_samples(needle_points(REF / f"text-transform-{n}.pes"))
    for a, b in PAIRS:
        if n in (a, b):
            twin = stem_samples(needle_points(REF / f"text-transform-{b if n == a else a}.pes"))
            return sorted(own + [(1 - u, t, bt) for u, t, bt in twin])
    if n in SYMMETRIC:
        return sorted(own + [(1 - u, t, bt) for u, t, bt in own])
    return own


def column_preset(n):
    s = samples_for(n)
    us = [k / (KNOTS - 1) for k in range(KNOTS)]
    ft, fb = fit(s, 1, n in LINEAR), fit(s, 2, n in LINEAR)
    if n in LINEAR:
        return [(u, ft(u)) for u in us], [(u, fb(u)) for u in us]
    return [(u, knot(s, 1, ft, u)) for u in us], [(u, knot(s, 2, fb, u)) for u in us]


def knot(samples, k, smooth, u):
    """The outermost (highest top, lowest bottom) of the samples within 1/64 of u that lie
    near the smooth fit (the crossbars and stem edges do not); the smooth fit where there
    are none."""
    near = [v[k - 1] for su, *v in samples
            if abs(su - u) <= 1 / 64 and abs(v[k - 1] - smooth(su)) < 0.06]
    if not near:
        return smooth(u)
    return min(near) if k == 1 else max(near)


def slant_preset(n):
    """A parallelogram: the top edge shifted by d of the width against the bottom edge."""
    letters = needle_points(REF / f"text-transform-{n}.pes")
    first = letters[0]
    left, right = min(x for x, _ in first), max(x for x, _ in first)
    # The straight letter is 8.7 mm wide; slanted it spans 8.7 (1 - d) + d W.
    d = round(((right - left) - 8.7) / (W - 8.7), 4)
    us = [k / (KNOTS - 1) for k in range(KNOTS)]
    if n == 9:  # forward: the tops lean right
        top = [(d + (1 - d) * u, 0.0) for u in us]
        bottom = [((1 - d) * u, 1.0) for u in us]
    else:  # back: the tops lean left
        top = [((1 - d) * u, 0.0) for u in us]
        bottom = [(d + (1 - d) * u, 1.0) for u in us]
    return top, bottom


def corner_scales(letters):
    """(top, bottom) scale of the letters' corners about the text's centre, least squares over
    every letter's two top and two bottom corners against the straight text's."""
    pitch = (W - LETTER) / (len(letters) - 1)
    c = X0 + W / 2
    num = {1: 0.0, 2: 0.0}
    den = {1: 0.0, 2: 0.0}
    for i, pts in enumerate(letters):
        top, bottom = min(y for _, y in pts), max(y for _, y in pts)
        for k, edge in ((1, top), (2, bottom)):
            near = [x for x, y in pts if abs(y - edge) < 0.4]
            for straight, moved in ((X0 + i * pitch, min(near)), (X0 + i * pitch + LETTER, max(near))):
                num[k] += (moved - c) * (straight - c)
                den[k] += (straight - c) ** 2
    return num[1] / den[1], num[2] / den[2]


def fan_preset(n):
    """A trapezoid: each edge is the straight text scaled about its centre (PE-DESIGN 11.50:
    13 keeps the top and shrinks the bottom to 0.70, 14 the other way round at 0.69)."""
    st, sb = corner_scales(needle_points(REF / f"text-transform-{n}.pes"))
    us = [k / (KNOTS - 1) for k in range(KNOTS)]
    top = [(round(0.5 + (u - 0.5) * st, 4), 0.0) for u in us]
    bottom = [(round(0.5 + (u - 0.5) * sb, 4), 1.0) for u in us]
    return top, bottom


def fmt(edge):
    return "[" + ", ".join(f"({x:.4f}, {y:.4f})" for x, y in edge) + "]"


def main():
    if not TOOL.exists():
        sys.exit(f"build stitchtool first: {TOOL}")
    print("//! Text Transform envelopes, generated by `tools/fit_envelopes.py` from PE-DESIGN 11.50")
    print("//! reference files (compat/reference/text/text-transform-N.pes). Do not edit by hand.")
    print("//! Each preset's top and bottom edge at u = k/32 along the text, as fractions of the straight")
    print("//! text's box: (x, y), x from its left, y from its top (y down).")
    print()
    print("// Fitted values, some of which happen to be close to well-known constants.")
    print("#![allow(clippy::approx_constant)]")
    print()
    print("pub type Edge = [(f64, f64); 33];")
    print()
    print("/// (preset number, top edge, bottom edge).")
    print("pub static ENVELOPES: &[(u8, Edge, Edge)] = &[")
    for n in sorted(COLUMN + SLANT + FAN):
        if n in COLUMN:
            top, bottom = column_preset(n)
        elif n in SLANT:
            top, bottom = slant_preset(n)
        else:
            top, bottom = fan_preset(n)
        print(f"    ({n}, {fmt(top)}, {fmt(bottom)}),")
    print("];")


if __name__ == "__main__":
    main()
