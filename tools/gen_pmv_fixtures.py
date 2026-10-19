#!/usr/bin/env python3
"""PMV oracle fixtures for openstitch_design::sewing::pmv_bytes.

pyembroidery's PmvWriter (MIT) is the only public description of Brother's PMV sewing-stitch
format. This writes a few stitch lists with PmvWriter.write directly (no encoder, so long
moves are not split) and records the stitch lists next to the files, so the Rust writer can be
compared byte for byte.

Run with pyembroidery installed: python3 -I tools/gen_pmv_fixtures.py
"""

import json
import math
import os

import pyembroidery as pe
from pyembroidery import PmvWriter

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates",
                   "openstitch-design", "tests", "fixtures", "pmv")

CASES = {
    # x along the feed, y across, 0.1 mm, pyembroidery orientation.
    "zigzag": [(i * 5.0, 30.0 if i % 2 else -30.0) for i in range(21)],
    "wide": [(i * 10.0, 45.0 * math.sin(i * 0.7)) for i in range(30)],
    "flat": [(i * 12.5, 0.0) for i in range(9)],
    "wave": [(i * 3.3, 20.0 * math.sin(i / 3.0) - 7.0) for i in range(100)],
}


def main():
    os.makedirs(OUT, exist_ok=True)
    for name, pts in CASES.items():
        pattern = pe.EmbPattern()
        for x, y in pts:
            pattern.add_stitch_absolute(pe.STITCH, x, y)
        with open(os.path.join(OUT, name + ".pmv"), "wb") as f:
            PmvWriter.write(pattern, f)
        with open(os.path.join(OUT, name + ".json"), "w") as f:
            json.dump([[x, y] for x, y in pts], f)
            f.write("\n")


if __name__ == "__main__":
    main()
