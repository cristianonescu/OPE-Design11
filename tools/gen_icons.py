#!/usr/bin/env python3
"""Draws the OpenStitch app icon (original artwork) and writes every format the packages need.

    python3 tools/gen_icons.py

Output in assets/icons/: openstitch.svg (Linux scalable icon, same geometry), PNGs 16-1024,
openstitch.ico (Windows), openstitch.icns (macOS) and openstitch-64.rgba (raw RGBA for the
window icon, embedded by the app). Needs Pillow.

The artwork: an embroidery hoop on a blue tile, holding fabric with a satin-stitched red star,
and a needle trailing thread. Nothing is taken from Brother or any other product.
"""

import math
import pathlib

from PIL import Image, ImageDraw, ImageFilter

OUT = pathlib.Path(__file__).resolve().parent.parent / "assets" / "icons"
S = 1024  # design units; drawn at 4x and downsampled

BG_TOP = (0x24, 0x4A, 0x7A)
BG_BOTTOM = (0x16, 0x2C, 0x4D)
WOOD = (0xD8, 0xA2, 0x5E)
WOOD_DARK = (0x9C, 0x6A, 0x32)
FABRIC = (0xF6, 0xF1, 0xE7)
THREAD = (0xD7, 0x26, 0x3D)
THREAD_DARK = (0x9E, 0x15, 0x29)
THREAD_LIGHT = (0xF2, 0x7A, 0x88)
STEEL = (0xD5, 0xDB, 0xE3)

HOOP_C = (512, 548)
HOOP_R = 352
HOOP_W = 50
FABRIC_R = 318
STAR_R, STAR_IN = 230, 98
NEEDLE = ((612, 300), (880, 104))  # point, eye end


def star_points(cx, cy, r, inner):
    pts = []
    for i in range(10):
        a = -math.pi / 2 + i * math.pi / 5
        rr = r if i % 2 == 0 else inner
        pts.append((cx + rr * math.cos(a), cy + rr * math.sin(a)))
    return pts


STAR = star_points(512, 562, STAR_R, STAR_IN)
THREAD_PATH = [(842, 132), (935, 255), (900, 410), (790, 470), (722, 489)]


def smooth(points, n=24):
    """Catmull-Rom through `points`."""
    out = []
    p = [points[0]] + points + [points[-1]]
    for i in range(1, len(p) - 2):
        p0, p1, p2, p3 = p[i - 1], p[i], p[i + 1], p[i + 2]
        for k in range(n):
            t = k / n
            out.append(tuple(
                0.5 * ((2 * p1[j]) + (-p0[j] + p2[j]) * t
                       + (2 * p0[j] - 5 * p1[j] + 4 * p2[j] - p3[j]) * t * t
                       + (-p0[j] + 3 * p1[j] - 3 * p2[j] + p3[j]) * t * t * t)
                for j in range(2)))
    out.append(points[-1])
    return out


def draw(scale=4):
    z = S * scale
    sc = lambda pts: [(x * scale, y * scale) for x, y in pts]
    img = Image.new("RGBA", (z, z), (0, 0, 0, 0))

    # Tile with a vertical gradient.
    grad = Image.new("RGBA", (1, z))
    for y in range(z):
        t = y / (z - 1)
        grad.putpixel((0, y), tuple(round(a + (b - a) * t) for a, b in zip(BG_TOP, BG_BOTTOM)) + (255,))
    grad = grad.resize((z, z))
    mask = Image.new("L", (z, z), 0)
    ImageDraw.Draw(mask).rounded_rectangle((40 * scale, 40 * scale, 984 * scale, 984 * scale),
                                           radius=210 * scale, fill=255)
    img.paste(grad, (0, 0), mask)

    d = ImageDraw.Draw(img)
    cx, cy = HOOP_C
    # Drop shadow under the hoop.
    shadow = Image.new("RGBA", (z, z), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).ellipse(
        ((cx - HOOP_R) * scale, (cy - HOOP_R + 18) * scale, (cx + HOOP_R) * scale, (cy + HOOP_R + 18) * scale),
        fill=(0, 0, 0, 90))
    img.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(14 * scale)))
    d = ImageDraw.Draw(img)

    # Fabric, then the hoop ring over its edge.
    d.ellipse(((cx - FABRIC_R) * scale, (cy - FABRIC_R) * scale, (cx + FABRIC_R) * scale, (cy + FABRIC_R) * scale),
              fill=FABRIC)
    for r, w, c in ((HOOP_R, HOOP_W, WOOD_DARK), (HOOP_R - 6, HOOP_W - 12, WOOD)):
        d.ellipse(((cx - r) * scale, (cy - r) * scale, (cx + r) * scale, (cy + r) * scale),
                  outline=c, width=w * scale)
    # Clamp at the top of the hoop.
    d.rounded_rectangle(((cx - 46) * scale, (cy - HOOP_R - 34) * scale, (cx + 46) * scale, (cy - HOOP_R + 30) * scale),
                        radius=12 * scale, fill=WOOD_DARK)
    d.rounded_rectangle(((cx - 34) * scale, (cy - HOOP_R - 24) * scale, (cx + 34) * scale, (cy - HOOP_R + 20) * scale),
                        radius=9 * scale, fill=WOOD)
    d.ellipse(((cx - 13) * scale, (cy - HOOP_R - 15) * scale, (cx + 13) * scale, (cy - HOOP_R + 11) * scale),
              fill=STEEL, outline=WOOD_DARK, width=3 * scale)

    # Satin-stitched star: diagonal stitches clipped to the star, each with a highlight.
    stitches = Image.new("RGBA", (z, z), (0, 0, 0, 0))
    sd = ImageDraw.Draw(stitches)
    step = 15
    for k in range(-60, 60):
        off = k * step
        a = (512 - 400 + off, 562 + 400)
        b = (512 + 400 + off, 562 - 400)
        sd.line(sc([a, b]), fill=THREAD_DARK, width=13 * scale)
        sd.line(sc([a, b]), fill=THREAD, width=9 * scale)
        sd.line(sc([(a[0] + 3, a[1] - 3), (b[0] + 3, b[1] - 3)]), fill=THREAD_LIGHT, width=3 * scale)
    star_mask = Image.new("L", (z, z), 0)
    ImageDraw.Draw(star_mask).polygon(sc(STAR), fill=255)
    img.paste(stitches, (0, 0), star_mask)
    d = ImageDraw.Draw(img)
    d.line(sc(STAR + [STAR[0]]), fill=THREAD_DARK, width=7 * scale, joint="curve")

    # Thread from the star to the needle's eye, then the needle.
    d.line(sc(smooth(THREAD_PATH)), fill=THREAD, width=9 * scale, joint="curve")
    (px, py), (ex, ey) = NEEDLE
    d.line(sc([(px, py), (ex, ey)]), fill=(0x8A, 0x93, 0xA0), width=18 * scale)
    d.line(sc([(px, py), (ex, ey)]), fill=STEEL, width=12 * scale)
    d.ellipse(((ex - 32) * scale, (ey - 10) * scale, (ex - 10) * scale, (ey + 12) * scale), fill=BG_TOP)
    return img.resize((S, S), Image.LANCZOS)


def svg():
    """The same artwork as a scalable SVG (Linux hicolor/scalable)."""
    pts = lambda p: " ".join(f"{x:.1f},{y:.1f}" for x, y in p)
    cx, cy = HOOP_C
    hexc = lambda c: "#%02x%02x%02x" % c
    lines = []
    step = 15
    for k in range(-60, 60):
        off = k * step
        a = (512 - 400 + off, 562 + 400)
        b = (512 + 400 + off, 562 - 400)
        lines.append(f'<line x1="{a[0]}" y1="{a[1]}" x2="{b[0]}" y2="{b[1]}"/>')
    th = smooth(THREAD_PATH)
    (px, py), (ex, ey) = NEEDLE
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <title>OpenStitch</title>
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="{hexc(BG_TOP)}"/><stop offset="1" stop-color="{hexc(BG_BOTTOM)}"/>
    </linearGradient>
    <clipPath id="star"><polygon points="{pts(STAR)}"/></clipPath>
  </defs>
  <rect x="40" y="40" width="944" height="944" rx="210" fill="url(#bg)"/>
  <circle cx="{cx}" cy="{cy + 18}" r="{HOOP_R}" fill="#000" opacity="0.25"/>
  <circle cx="{cx}" cy="{cy}" r="{FABRIC_R}" fill="{hexc(FABRIC)}"/>
  <circle cx="{cx}" cy="{cy}" r="{HOOP_R - HOOP_W / 2}" fill="none" stroke="{hexc(WOOD_DARK)}" stroke-width="{HOOP_W}"/>
  <circle cx="{cx}" cy="{cy}" r="{HOOP_R - HOOP_W / 2}" fill="none" stroke="{hexc(WOOD)}" stroke-width="{HOOP_W - 12}"/>
  <rect x="{cx - 46}" y="{cy - HOOP_R - 34}" width="92" height="64" rx="12" fill="{hexc(WOOD_DARK)}"/>
  <rect x="{cx - 34}" y="{cy - HOOP_R - 24}" width="68" height="44" rx="9" fill="{hexc(WOOD)}"/>
  <circle cx="{cx}" cy="{cy - HOOP_R - 2}" r="13" fill="{hexc(STEEL)}" stroke="{hexc(WOOD_DARK)}" stroke-width="3"/>
  <g clip-path="url(#star)">
    <rect x="0" y="0" width="1024" height="1024" fill="{hexc(THREAD)}"/>
    <g stroke="{hexc(THREAD_DARK)}" stroke-width="4">{"".join(lines)}</g>
  </g>
  <polygon points="{pts(STAR)}" fill="none" stroke="{hexc(THREAD_DARK)}" stroke-width="7" stroke-linejoin="round"/>
  <polyline points="{pts(th)}" fill="none" stroke="{hexc(THREAD)}" stroke-width="9" stroke-linecap="round"/>
  <line x1="{px}" y1="{py}" x2="{ex}" y2="{ey}" stroke="#8a93a0" stroke-width="18" stroke-linecap="round"/>
  <line x1="{px}" y1="{py}" x2="{ex}" y2="{ey}" stroke="{hexc(STEEL)}" stroke-width="12" stroke-linecap="round"/>
  <circle cx="{ex - 21}" cy="{ey + 1}" r="11" fill="{hexc(BG_TOP)}"/>
</svg>
"""


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    big = draw()
    sizes = [16, 32, 48, 64, 128, 256, 512, 1024]
    for s in sizes:
        big.resize((s, s), Image.LANCZOS).save(OUT / f"openstitch-{s}.png", optimize=True)
    big.save(OUT / "openstitch.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    big.save(OUT / "openstitch.icns")
    (OUT / "openstitch-64.rgba").write_bytes(big.resize((64, 64), Image.LANCZOS).tobytes())
    (OUT / "openstitch.svg").write_text(svg())


if __name__ == "__main__":
    main()
