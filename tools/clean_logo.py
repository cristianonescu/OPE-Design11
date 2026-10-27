#!/usr/bin/env python3
"""Turn the SVGs generated for the OpenStitch logo into the masters in assets/.

The mark (a needle whose thread sweeps round it in a C, closes into a small o-loop and ends in a
running stitch, so the letters are hinted rather than written) and the
"OpenStitch" lettering were generated with Higgsfield (Recraft V4.1, vector output) from the
prompts in assets/brand/README.md. This script keeps their shapes and only: strips the
generator's metadata and backgrounds, turns the needle's eye and the letters' counters into
holes (so they show whatever is behind them), centres and scales the mark on a new rounded tile,
and lays out the wordmark.

    python tools/clean_logo.py MARK.svg LETTERING.svg

Writes assets/icons/openstitch.svg (the app icon), assets/brand/openstitch-mark.svg (no tile),
assets/brand/openstitch-wordmark.svg and assets/brand/openstitch-wordmark-dark.svg (for dark
backgrounds). Then run tools/gen_icons.py for the PNG, ICO, ICNS and RGBA files.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
NAVY, CORAL, CREAM, WHITE = "#1f3a5f", "#e2445c", "#f6f1e7", "#ffffff"
RIM = "#3a5f8f"  # tile edge, so the tile keeps its shape on dark backgrounds


def paths(svg):
    """(fill, d) for every path, in drawing order, colours snapped to the palette."""
    out = []
    for m in re.finditer(r'<path[^>]*fill="rgb\(([0-9,]+)\)"[^>]*d="([^"]*)"', svg):
        c = tuple(int(v) for v in m.group(1).split(","))
        fill = min(
            (NAVY, CORAL, CREAM, WHITE),
            key=lambda h: sum((a - int(h[i : i + 2], 16)) ** 2 for a, i in zip(c, (1, 3, 5))),
        )
        out.append((fill, re.sub(r"(\d+\.\d)\d+", r"\1", m.group(2).strip())))
    return out


def bbox(d):
    n = [float(v) for v in re.findall(r"-?\d+\.?\d*", d)]
    return min(n[0::2]), min(n[1::2]), max(n[0::2]), max(n[1::2])


def inside(a, b):
    return a[0] >= b[0] and a[1] >= b[1] and a[2] <= b[2] and a[3] <= b[3]


def punch(shapes, hole_fill):
    """Merges every `hole_fill` shape into the shape around it as an even-odd hole."""
    kept = [[f, d] for f, d in shapes if f != hole_fill]
    for f, d in shapes:
        if f == hole_fill:
            outer = next(s for s in reversed(kept) if inside(bbox(d), bbox(s[1])))
            outer[1] += " " + d
    return kept


def union_box(shapes):
    boxes = [bbox(d) for _, d in shapes]
    return tuple(f(b[i] for b in boxes) for i, f in enumerate((min, min, max, max)))


def group(shapes, transform, recolour=None):
    recolour = recolour or {}
    body = "\n".join(
        f'<path fill="{recolour.get(f, f)}" fill-rule="evenodd" d="{d}"/>' for f, d in shapes
    )
    return f'<g transform="{transform}">\n{body}\n</g>'


def fit(box, cx, cy, size):
    """Transform that scales `box` to `size` wide (or high) and centres it on (cx, cy)."""
    k = size / max(box[2] - box[0], box[3] - box[1])
    tx = cx - k * (box[0] + box[2]) / 2
    ty = cy - k * (box[1] + box[3]) / 2
    return f"translate({tx:.1f} {ty:.1f}) scale({k:.4f})"


def tile(x, y, s):
    r = s * 0.225
    return (
        f'<rect x="{x}" y="{y}" width="{s}" height="{s}" rx="{r:.0f}" fill="{NAVY}"/>\n'
        f'<rect x="{x + 6}" y="{y + 6}" width="{s - 12}" height="{s - 12}" rx="{r - 6:.0f}" '
        f'fill="none" stroke="{RIM}" stroke-width="12"/>'
    )


def write(name, view, body):
    text = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view}">\n<title>OpenStitch</title>\n'
        f"{body}\n</svg>\n"
    )
    (ROOT / name).parent.mkdir(parents=True, exist_ok=True)
    (ROOT / name).write_text(text, encoding="utf-8", newline="\n")


def main():
    mark_src, word_src = (pathlib.Path(a).read_text(encoding="utf-8") for a in sys.argv[1:3])

    # Mark: drop the white background, the generated tile (the large navy shape) and the
    # anti-aliasing slivers at the needle's tip; a navy eye becomes a hole in the needle.
    def size(d):
        b = bbox(d)
        return max(b[2] - b[0], b[3] - b[1])

    shapes = [s for s in paths(mark_src)[1:] if 20 < size(s[1]) and (s[0] != NAVY or size(s[1]) < 600)]
    mark = punch(shapes, NAVY)
    box = union_box(mark)

    write("assets/icons/openstitch.svg", "0 0 1024 1024",
          tile(48, 48, 928) + "\n" + group(mark, fit(box, 512, 512, 700)))
    write("assets/brand/openstitch-mark.svg", "0 0 1024 1024", group(mark, fit(box, 512, 512, 960)))

    # Lettering: drop the white background; the white counters become holes.
    letters = punch(paths(word_src)[1:], WHITE)
    lb = union_box(letters)
    h = 512
    k = 0.52 * h / (lb[3] - lb[1])
    x0 = h + 0.14 * h
    width = x0 + k * (lb[2] - lb[0]) + 0.04 * h
    place = f"translate({x0 - k * lb[0]:.1f} {0.24 * h - k * lb[1]:.1f}) scale({k:.4f})"
    icon = tile(0, 0, h) + "\n" + group(mark, fit(box, h / 2, h / 2, h * 700 / 928))
    for name, ink in (("openstitch-wordmark", NAVY), ("openstitch-wordmark-dark", CREAM)):
        write(f"assets/brand/{name}.svg", f"0 0 {width:.0f} {h}",
              icon + "\n" + group(letters, place, {NAVY: ink}))


if __name__ == "__main__":
    main()
