#!/usr/bin/env python3
"""Scores how closely image-to-embroidery results resemble their source images (B16.1).

    cargo build --release -p stitchtool
    python -I tools/image_similarity.py [--only ID,ID] [--mode pe|openstitch|both]
        [--render flat|realistic] [--set NAME=JSON] [--no-dists] [--out FILE.json] [--renders DIR]

The metric is the one agreed for the image-similarity plan
(docs/research/beyond/image-similarity-plan.md, section 2):

1. **Render.** The design's stitches are drawn in sewing order as 0.4 mm thread on white fabric
   at 20 px/mm. `--render flat` (default until the sew-out calibration) draws each stitch in its
   thread colour; `--render realistic` adds the editor's realistic-view shading (an edge at
   0.55 x the colour and a highlight lifted 45 % towards white on the middle 60 % of a stitch).
2. **Viewing.** The source (composited on white) and the render are area-averaged in linear
   light to 6 px/mm (about 150 dpi, the design at actual size), long side 100 mm.
3. **Eye model.** S-CIELAB (Zhang and Wandell 1996): opponent channels filtered by the eye's
   contrast sensitivity at 40 cm, reflecting at the image edges, then CIELAB.
4. **Terms.** S = MS-SSIM (5 scales, sigma 1.5) on the filtered L*; C = max(0, 1 - dE00/60)
   from the mean CIEDE2000 (dE 3 scores 95 %); P = 1 - DISTS (Ding et al. 2020).
5. **Score** = 0.4 S + 0.4 C + 0.2 P, in percent.

**Sewability** is reported beside the score: needle points per mm2 of the design (needle points
over the image's width x height), the shortest stitch, stitches under 1 mm, trims and colours.

Each case converts one image of examples/images through `stitchtool serve`, as
`tools/image_eval.py` does (its CASES, plus the Design Center cases below). `--mode` runs each
case in OpenStitch's PE-DESIGN-compatible mode (`compatMode: peDesign`), in OpenStitch mode
(`openStitch`), or both; Cross Stitch and Design Center have no OpenStitch mode yet and run
once. Every result is OpenStitch's output: nothing here is PE-DESIGN's own.

Needs numpy, Pillow and scipy. The P term needs PyTorch and piq (`pip install torch piq`;
piq downloads the published DISTS weights on first use). DISTS is evaluation-only and never
shipped. `--no-dists` skips it: S and C are still reported, the score is not.
"""
import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy.ndimage import convolve1d, gaussian_filter

sys.path.insert(0, str(Path(__file__).resolve().parent))
import image_eval as ie  # noqa: E402

RES = 20.0  # render, px/mm
VIEW = 6.0  # viewing scale, px/mm
DIST_MM = 400.0  # viewing distance
THREAD_MM = 0.4
W_S, W_C, W_P = 0.4, 0.4, 0.2

CASES = dict(ie.CASES)
CASES.update({
    "g03-dc": ("g03-teapot-lineart.png", "digitize.designCenter", {}),
    "g04-dc": ("g04-butterfly-outline.png", "digitize.designCenter", {}),
})
# Commands with an OpenStitch mode (compatMode).
MODAL = {"digitize.autoPunch", "digitize.photoStitch"}
MODES = {"pe": "peDesign", "openstitch": "openStitch"}


# --- render ---------------------------------------------------------------------------------

def shades(rgb, realistic):
    """(edge, body, highlight) colours of a stitch, as the editor's realistic view draws it."""
    r = np.array(rgb, float)
    body = tuple(int(v) for v in r)
    if not realistic:
        return body, body, body
    edge = tuple(int(round(v)) for v in r * 0.55)
    light = tuple(int(round(v)) for v in r + (255 - r) * 0.45)
    return edge, body, light


def render(segments, w_mm, h_mm, realistic=False, fabric=(255, 255, 255)):
    """segments: (thread rgb, [(x_mm, y_mm), ...]) polylines in sewing order."""
    W, H = int(round(w_mm * RES)), int(round(h_mm * RES))
    im = Image.new("RGB", (W, H), fabric)
    d = ImageDraw.Draw(im)
    w = THREAD_MM * RES
    for rgb, pts in segments:
        edge, body, light = shades(rgb, realistic)
        P = [(x * RES, y * RES) for x, y in pts]
        for a, b in zip(P, P[1:]):
            d.line([a, b], fill=edge, width=max(1, round(w)))
            d.line([a, b], fill=body, width=max(1, round(w * 0.7)))
            dx, dy = b[0] - a[0], b[1] - a[1]
            d.line([(a[0] + dx * .2, a[1] + dy * .2), (b[0] - dx * .2, b[1] - dy * .2)],
                   fill=light, width=max(1, round(w * 0.25)))
    return im


def to_view(img, w_mm, h_mm):
    """Linear-light RGB, area-averaged to the viewing scale."""
    W, H = int(round(w_mm * VIEW)), int(round(h_mm * VIEW))
    a = np.asarray(img.convert("RGB")).astype(np.float64) / 255
    lin = np.where(a <= 0.04045, a / 12.92, ((a + 0.055) / 1.055) ** 2.4)
    ch = []
    for c in range(3):
        f = Image.fromarray(lin[..., c].astype(np.float32), "F").resize((W, H), Image.BOX)
        ch.append(np.asarray(f))
    return np.stack(ch, -1)


# --- eye model and colour ---------------------------------------------------------------------

M_XYZ = np.array([[0.4124, 0.3576, 0.1805], [0.2126, 0.7152, 0.0722], [0.0193, 0.1192, 0.9505]])
WHITE = np.array([0.95047, 1.0, 1.08883])
M_OPP = np.array([[0.279, 0.72, -0.107], [-0.449, 0.29, -0.077], [0.086, -0.59, 0.501]])


def lab_from_xyz(xyz):
    t = xyz / WHITE
    f = np.where(t > 216 / 24389, np.cbrt(np.maximum(t, 0)), (24389 / 27 * t + 16) / 116)
    return np.stack([116 * f[..., 1] - 16, 500 * (f[..., 0] - f[..., 1]),
                     200 * (f[..., 1] - f[..., 2])], -1)


def scielab(lin):
    """S-CIELAB: the opponent channels filtered with the eye's spatial CSF, as CIELAB."""
    opp = (lin @ M_XYZ.T) @ M_OPP.T
    spd = DIST_MM * np.tan(np.radians(1)) * VIEW  # samples per degree
    params = [[(0.0283, 0.921), (0.133, 0.105), (4.336, -0.1080)],
              [(0.0392, 0.531), (0.494, 0.330)],
              [(0.0536, 0.488), (0.386, 0.371)]]
    out = np.empty_like(opp)
    for c, ps in enumerate(params):
        tot = sum(wt for _, wt in ps)
        acc = np.zeros(opp.shape[:2])
        for spread, wt in ps:
            acc += wt / tot * gaussian_filter(opp[..., c], max(spread * spd, 0.3), mode="reflect")
        out[..., c] = acc
    return lab_from_xyz(np.clip(out @ np.linalg.inv(M_OPP).T, 0, None))


def de2000(lab1, lab2):
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
    RT = (-2 * np.sqrt(Cbp7 / (Cbp7 + 25.0 ** 7))
          * np.sin(np.radians(60 * np.exp(-((hbp - 275) / 25) ** 2))))
    return np.sqrt((dLp / SL) ** 2 + (dCp / SC) ** 2 + (dHp / SH) ** 2
                   + RT * (dCp / SC) * (dHp / SH))


# --- structure and perceptual terms ---------------------------------------------------------

def _ssim_parts(x, y, sigma=1.5, L=100.0):
    C1, C2 = (0.01 * L) ** 2, (0.03 * L) ** 2

    def g(a):
        return gaussian_filter(a, sigma, mode="reflect", truncate=3.5)

    mx, my = g(x), g(y)
    sxx, syy, sxy = g(x * x) - mx * mx, g(y * y) - my * my, g(x * y) - mx * my
    lum = (2 * mx * my + C1) / (mx * mx + my * my + C1)
    cs = (2 * sxy + C2) / (sxx + syy + C2)
    return lum.mean(), cs.mean()


def ms_ssim(x, y):
    weights = [0.0448, 0.2856, 0.3001, 0.2363, 0.1333]
    out = 1.0
    for i, wi in enumerate(weights):
        lum, cs = _ssim_parts(x, y)
        if i == len(weights) - 1:
            out *= max(lum * cs, 1e-6) ** wi
        else:
            out *= max(cs, 1e-6) ** wi
            xi = Image.fromarray(x.astype(np.float32), "F")
            yi = Image.fromarray(y.astype(np.float32), "F")
            size = (max(1, xi.width // 2), max(1, xi.height // 2))
            x = np.asarray(xi.resize(size, Image.BOX), np.float64)
            y = np.asarray(yi.resize(size, Image.BOX), np.float64)
    return float(out)


def gmsd(x, y, c=170 / 255 * 100 / 255 * 100):
    """Gradient-magnitude similarity on L*: (mean, deviation); a detail diagnostic only."""
    def gm(a):
        gx = convolve1d(convolve1d(a, [1 / 3, 1 / 3, 1 / 3], 0), [1, 0, -1], 1) / 2
        gy = convolve1d(convolve1d(a, [1 / 3, 1 / 3, 1 / 3], 1), [1, 0, -1], 0) / 2
        return np.hypot(gx, gy)

    g1, g2 = gm(x), gm(y)
    gms = (2 * g1 * g2 + c) / (g1 ** 2 + g2 ** 2 + c)
    return float(gms.mean()), float(gms.std())


_DISTS = None


def dists(lin_a, lin_b):
    """DISTS on the viewing-scale sRGB images; 0 means identical. Deterministic, one thread."""
    global _DISTS
    import piq
    import torch
    torch.use_deterministic_algorithms(True)
    torch.set_num_threads(1)
    if _DISTS is None:
        _DISTS = piq.DISTS(reduction="none")

    def tensor(lin):
        s = np.where(lin <= 0.0031308, lin * 12.92,
                     1.055 * np.clip(lin, 0, 1) ** (1 / 2.4) - 0.055)
        return torch.tensor(np.clip(s, 0, 1).transpose(2, 0, 1)[None], dtype=torch.float32)

    with torch.no_grad():
        return float(_DISTS(tensor(lin_a), tensor(lin_b)).item())


def score(src_img, out_img, w_mm, h_mm, with_dists=True):
    A, B = to_view(src_img, w_mm, h_mm), to_view(out_img, w_mm, h_mm)
    a, b = scielab(A), scielab(B)
    de = de2000(a, b)
    r = {"dE": float(de.mean()), "dE90": float(np.percentile(de, 90)),
         "msssim": ms_ssim(a[..., 0], b[..., 0])}
    r["gms"], r["gmsd"] = gmsd(a[..., 0], b[..., 0])
    if with_dists:
        r["dists"] = dists(A, B)
        c = max(0.0, 1 - r["dE"] / 60)
        p = max(0.0, 1 - r["dists"])
        r["score"] = W_S * r["msssim"] + W_C * c + W_P * p
    return r


# --- the wizards ------------------------------------------------------------------------------

def hexrgb(c):
    if isinstance(c, str):
        c = c.lstrip("#")
        return tuple(int(c[i:i + 2], 16) for i in (0, 2, 4))
    return tuple(c)


def stitch_runs(tool):
    """(thread rgb, points) runs of consecutive needle points of one colour, in sewing order;
    a jump, trim or colour change ends a run."""
    runs = []
    for o in tool("object.list"):
        pts = tool("stitches.points", id=o["id"])
        threads = [hexrgb(t) for t in pts["threads"]]
        cur, col = [], None
        for p in pts["points"]:
            if p["command"] == "stitch":
                if col is not None and p["color"] != col and cur:
                    runs.append((threads[col], cur))
                    cur = []
                col = p["color"]
                cur.append((p["x"], p["y"]))
            else:
                if len(cur) > 1:
                    runs.append((threads[col], cur))
                cur = []
        if col is not None and len(cur) > 1:
            runs.append((threads[col], cur))
    return runs


def sewability(runs, stats, w_mm, h_mm):
    lengths = [np.hypot(b[0] - a[0], b[1] - a[1])
               for _, pts in runs for a, b in zip(pts, pts[1:])]
    moving = [s for s in lengths if s > 1e-6]
    return {
        "stitches": stats["stitches"],
        "trims": stats["trims"],
        "colours": len(stats["colors"]),
        "pointsPerMm2": stats["stitches"] / (w_mm * h_mm),
        "shortestMm": float(min(moving)) if moving else 0.0,
        "under1mm": sum(1 for s in moving if s < 1.0),
    }


def run_case(tool, image, command, params, realistic, with_dists):
    w, h = Image.open(ie.IMAGES / image).size
    mm_per_px = ie.LONG_SIDE_MM / max(w, h)
    W, H = w * mm_per_px, h * mm_per_px
    tool("design.new", width=round(W + 0.5, 1), height=round(H + 0.5, 1))
    tool(command, path=str(ie.IMAGES / image), width=W, x=0, y=0, **params)
    stats = tool("design.info")["stitchStats"]
    ids = [o["id"] for o in tool("object.list")]
    try:
        tool("object.toStitches", ids=ids)
    except RuntimeError as e:
        if "no shape or text objects" not in str(e):
            raise
    runs = stitch_runs(tool)
    out = render(runs, W, H, realistic)
    src = Image.fromarray(ie.source_rgb(ie.IMAGES / image))
    r = score(src, out, W, H, with_dists)
    r.update(sewability(runs, stats, W, H))
    return r, out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--only", help="comma-separated case ids")
    ap.add_argument("--mode", choices=["pe", "openstitch", "both"], default="both")
    ap.add_argument("--render", choices=["flat", "realistic"], default="flat")
    ap.add_argument("--set", action="append", default=[], metavar="NAME=JSON",
                    help="extra digitize parameter for every case")
    ap.add_argument("--no-dists", action="store_true", help="skip DISTS (no PyTorch needed)")
    ap.add_argument("--out", help="write the results as JSON")
    ap.add_argument("--renders", help="save each render (quarter size) to this folder")
    a = ap.parse_args()

    if not ie.BIN.exists():
        sys.exit(f"{ie.BIN} not found: cargo build --release -p stitchtool")
    with_dists = not a.no_dists
    if with_dists:
        try:
            import piq  # noqa: F401
            import torch  # noqa: F401
        except ImportError:
            sys.exit("DISTS needs PyTorch and piq: pip install torch piq (or pass --no-dists)")

    extra = {k: json.loads(v) for k, v in (s.split("=", 1) for s in a.set)}
    keys = a.only.split(",") if a.only else list(CASES)
    unknown = [k for k in keys if k not in CASES]
    if unknown:
        sys.exit(f"unknown case(s): {', '.join(unknown)}")
    modes = ["pe", "openstitch"] if a.mode == "both" else [a.mode]

    tool = ie.Tool(ie.ROOT)
    results = {}
    print(f"render: {a.render}; every result is OpenStitch's output "
          f"(pe = PE-DESIGN-compatible mode)")
    print(f"{'case':9} {'mode':10} {'score':>6} {'dE':>6} {'msssim':>7} {'dists':>6} "
          f"{'pts/mm2':>7} {'short':>6} {'<1mm':>6} {'stitch':>7} {'trims':>5} {'col':>3}")
    try:
        for k in keys:
            image, command, params = CASES[k]
            for m in (modes if command in MODAL else ["pe"]):
                p = {**params, **extra}
                if command in MODAL:
                    p["compatMode"] = MODES[m]
                try:
                    r, out = run_case(tool, image, command, p, a.render == "realistic",
                                      with_dists)
                except RuntimeError as e:
                    print(f"{k:9} {m:10} failed: {e}", flush=True)
                    continue
                results.setdefault(k, {})[m] = r
                if a.renders:
                    folder = Path(a.renders)
                    folder.mkdir(parents=True, exist_ok=True)
                    out.resize((out.width // 4, out.height // 4), Image.BOX).save(
                        folder / f"{k}-{m}-{a.render}.png")
                s = f"{r['score'] * 100:6.1f}" if "score" in r else f"{'-':>6}"
                d = f"{r['dists']:6.3f}" if "dists" in r else f"{'-':>6}"
                print(f"{k:9} {m:10} {s} {r['dE']:6.2f} {r['msssim']:7.4f} {d} "
                      f"{r['pointsPerMm2']:7.2f} {r['shortestMm']:6.2f} {r['under1mm']:6} "
                      f"{r['stitches']:7} {r['trims']:5} {r['colours']:3}", flush=True)
    finally:
        tool.close()

    summary = {}
    for k, by_mode in results.items():
        tool_name = CASES[k][1].split(".")[1]
        if CASES[k][1] == "digitize.photoStitch":
            tool_name = "photoStitch2" if CASES[k][2].get("style") == "lines" else "photoStitch1"
        for m, r in by_mode.items():
            if "score" in r:
                summary.setdefault(tool_name, {}).setdefault(m, []).append(r["score"] * 100)
    if summary:
        print("\nmean score per tool:")
        for t, by_mode in summary.items():
            parts = ", ".join(f"{m} {np.mean(v):.1f} ({len(v)})" for m, v in by_mode.items())
            print(f"  {t:13} {parts}")
    if a.out:
        Path(a.out).write_text(json.dumps({"render": a.render, "results": results}, indent=1))


if __name__ == "__main__":
    main()
