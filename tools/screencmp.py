"""Compare line-screen photo stitching (PE-DESIGN's Photo Stitch 2) with a source image (#129).

For each colour block of each PES file given, reports:
- the thread's PEC index and its stitch, jump and trim counts;
- the dominant line angle: the peak of a stitch-direction histogram weighted by stitch length,
  in degrees counter-clockwise on screen (0 = along x, 90 = along y), with its share of the
  thread's sewn length within 10 degrees of it;
- the line spacing: the first autocorrelation peak, between 0.8 and 6 mm, of the needle points'
  positions across that angle;
- needle points per cm2 in each colour area of the source image (the image is reduced to its
  most common colours; small blends at edges join the nearest).

Only documented PES/PEC fields are read (tools/pescmp.py's reader). The image is placed by its
top-left corner and width in mm, in the PES file's own coordinates; with --fit, the image is
placed over the file's stitch bounds instead (for files whose design origin is not known).

    python tools/screencmp.py IMAGE WIDTH_MM [--at X_MM Y_MM | --fit] [--colours N] PES [PES ...]
"""
import argparse
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
import pescmp  # noqa: E402


def areas(image_path, n):
    """The image as an index map into its `n` most common colours (nearest colour elsewhere)."""
    im = Image.open(image_path).convert("RGB")
    px = np.asarray(im, dtype=np.int32)
    flat = px.reshape(-1, 3)
    packed = flat[:, 0] << 16 | flat[:, 1] << 8 | flat[:, 2]
    values, counts = np.unique(packed, return_counts=True)
    top = values[np.argsort(-counts)[:n]]
    palette = np.stack([(top >> 16) & 255, (top >> 8) & 255, top & 255], axis=1)
    d = ((flat[:, None, :] - palette[None, :, :]) ** 2).sum(-1)
    labels = d.argmin(1).reshape(px.shape[:2])
    return labels, palette


def name(rgb):
    return "#%02x%02x%02x" % tuple(int(v) for v in rgb)


def dominant_angle(segments):
    """Peak of the length-weighted direction histogram (1-degree bins, 0..179) of `segments`
    (an (n, 2, 2) array of start and end points), and the share of the length within 10 degrees
    of it."""
    if len(segments) == 0:
        return None, 0.0
    d = segments[:, 1] - segments[:, 0]
    length = np.hypot(d[:, 0], d[:, 1])
    keep = length > 0.05
    d, length = d[keep], length[keep]
    if len(length) == 0:
        return None, 0.0
    ang = np.degrees(np.arctan2(-d[:, 1], d[:, 0])) % 180.0
    hist = np.zeros(180)
    np.add.at(hist, np.round(ang).astype(int) % 180, length)
    smooth = sum(np.roll(hist, k) for k in range(-2, 3))
    peak = int(smooth.argmax())
    diff = np.abs((ang - peak + 90.0) % 180.0 - 90.0)
    share = length[diff <= 10.0].sum() / length.sum()
    return peak, share


def spacing(points, angle):
    """First autocorrelation peak (0.8-6 mm) of needle positions across `angle`."""
    if angle is None or len(points) < 20:
        return None
    t = math.radians(angle)
    # The normal to a line at `angle` (y down on screen, so the angle's sine is negated).
    normal = np.array([math.sin(t), math.cos(t)])
    pos = points @ normal
    pos = pos - pos.min()
    bins = np.bincount(np.round(pos / 0.1).astype(int))
    bins = bins - bins.mean()
    ac = np.correlate(bins, bins, mode="full")[len(bins) - 1:]
    lo, hi = 8, min(60, len(ac) - 1)
    if hi <= lo:
        return None
    k = lo + int(ac[lo:hi].argmax())
    return k * 0.1


def report(pes_path, labels, palette, width_mm, at, fit):
    f = pescmp.read(pes_path)
    pts, kinds = f["pts"], f["kinds"]
    stitch = np.array([k == "stitch" for k in kinds])
    h, w = labels.shape
    if fit:
        sp = pts[stitch][:, :2]
        x0, y0 = sp.min(0)
        width_mm = sp[:, 0].max() - x0
    else:
        x0, y0 = at
    mm_per_px = width_mm / w
    area_cm2 = np.bincount(labels.ravel(), minlength=len(palette)) * (mm_per_px ** 2) / 100.0
    total = int(stitch.sum())
    print(f"== {Path(pes_path).name}: {total} stitches, {f['jumps']} jumps, {f['trims']} trims, "
          f"{f['colour_changes'] + 1} colour blocks")
    head = "  ".join(f"{name(c)}" for c in palette)
    print(f"   per cm2 in areas:          {head}")
    for block in range(f["colour_changes"] + 1):
        sel = (pts[:, 2] == block)
        seq = pts[sel][:, :2]
        ks = np.array(kinds)[sel]
        sewn = seq[ks == "stitch"]
        if len(sewn) == 0:
            continue
        # Sewn segments only: consecutive stitch records.
        runs, cur = [], []
        for p, k in zip(seq, ks):
            if k == "stitch":
                cur.append(p)
            elif cur:
                runs.append(np.array(cur))
                cur = []
        if cur:
            runs.append(np.array(cur))
        segs = [r for r in runs if len(r) > 1]
        allseg = np.concatenate([np.vstack([r[:-1], r[1:]]).reshape(2, -1, 2).transpose(1, 0, 2)
                                 for r in segs]) if segs else np.zeros((0, 2, 2))
        peak, share = dominant_angle(allseg)
        gap = spacing(sewn, peak)
        ix = np.floor((sewn[:, 0] - x0) / mm_per_px).astype(int)
        iy = np.floor((sewn[:, 1] - y0) / mm_per_px).astype(int)
        inside = (ix >= 0) & (ix < w) & (iy >= 0) & (iy < h)
        counts = np.bincount(labels[iy[inside], ix[inside]], minlength=len(palette))
        dens = "  ".join(f"{c / a:7.0f}" if a > 0 else "      -" for c, a in zip(counts, area_cm2))
        pec = f["palette"][block] if block < len(f["palette"]) else "?"
        angle = "-" if peak is None else f"{peak:3d} deg ({share:4.0%})"
        gap_s = "-" if gap is None else f"{gap:.1f} mm"
        print(f"   block {block} PEC {pec:>2}: {len(sewn):6d} st, angle {angle}, spacing {gap_s:>7}: {dens}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("image")
    ap.add_argument("width_mm", type=float)
    ap.add_argument("pes", nargs="+")
    ap.add_argument("--at", nargs=2, type=float, default=[0.0, 0.0], metavar=("X_MM", "Y_MM"))
    ap.add_argument("--fit", action="store_true")
    ap.add_argument("--colours", type=int, default=5)
    a = ap.parse_args()
    labels, palette = areas(a.image, a.colours)
    for p in a.pes:
        report(p, labels, palette, a.width_mm, a.at, a.fit)


if __name__ == "__main__":
    main()
