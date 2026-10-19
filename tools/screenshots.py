#!/usr/bin/env python3
"""Regenerates the editor screenshots in docs/images and docs/images/readme with the real app.

    cargo build -p openstitch-app -p stitchtool
    python3 tools/screenshots.py [--only NAME,NAME]

Each scene runs `openstitch --screenshot` (1440x900 window) on a sample design built from
examples/ by `stitchtool exec`. The app runs inside a temporary folder and is given relative
paths, so the status bar never shows a local path. Images are downscaled and quantised to 256
colours (128 for the README copies): docs/images at 1600 px wide, docs/images/readme at
1280 px. A stand-in home folder keeps the user's folder names out of the images. Needs Pillow; macOS
renders with Metal (the documentation images are made there).
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "debug"
FIXTURES = ROOT / "crates" / "openstitch-formats" / "tests" / "fixtures"

# name -> (arguments, output files with their widths)
SCENES = {
    "overview": (["--demo", "--select", "2"],
                 [("docs/images/rust-editor-overview.png", 1600), ("docs/images/readme/editor.png", 1280)]),
    "appmenu": (["--demo", "--show", "appmenu"], [("docs/images/rust-editor-appmenu.png", 1600)]),
    "view-dark": (["--demo", "--ribbon", "view", "--pane", "color", "--select", "2", "--theme", "dark"],
                  [("docs/images/rust-editor-view-dark.png", 1600)]),
    "stitches-tab": (["badge.pes", "--select", "1", "--ribbon", "stitches"],
                     [("docs/images/rust-editor-stitches-tab.png", 1600)]),
    "floating": (["--demo", "--tool", "pen", "--show", "floating"],
                 [("docs/images/rust-editor-floating-panes.png", 1600)]),
    "realistic": (["--demo", "--show", "realistic", "--ribbon", "view", "--zoom", "1.2"],
                  [("docs/images/readme/realistic-view.png", 1280)]),
    "simulator": (["--demo", "--simulate", "0.55", "--ribbon", "view"],
                  [("docs/images/readme/stitch-simulator.png", 1280)]),
    "drawing": (["drawing.osd", "--select", "1"], [("docs/images/readme/drawing.png", 1280)]),
    "monogram": (["monograms.osd", "--select", "1", "--show", "monogram"],
                 [("docs/images/readme/monogram.png", 1280)]),
    "auto-punch": (["--image", "badge.png", "--convert", "autopunch", "--ribbon", "image"],
                   [("docs/images/readme/auto-punch.png", 1280)]),
    "design-database": (["--database", "home/Embroidery", "--db-view", "large"],
                        [("docs/images/readme/design-database.png", 1280)]),
    "stitch-creator": (["user-patterns.osd", "--pattern", "tulip.json"],
                       [("docs/images/readme/stitch-creator.png", 1280)]),
}


def run(args, cwd, env=None):
    subprocess.run(args, cwd=cwd, check=True, env=env)


def app_env(work):
    """A stand-in home folder, so the Design Database tree and the pattern library show
    neutral names instead of the user's own folders."""
    env = dict(os.environ)
    home = work / "home"
    env.update({"HOME": str(home), "APPDATA": str(home), "XDG_CONFIG_HOME": str(home / ".config"),
                "XDG_DATA_HOME": str(home / ".local" / "share"), "OPENSTITCH_PATTERNS": "patterns"})
    return env


def build_project(work, example, name):
    """Runs an example script and saves its design as NAME.osd in `work`."""
    steps = json.loads((ROOT / "examples" / example).read_text())
    steps = [s for s in steps if s.get("command") != "project.save"]
    steps.append({"command": "project.save", "params": {"path": str(work / f"{name}.osd")}})
    script = work / f"{name}.json"
    script.write_text(json.dumps(steps))
    run([str(BIN / "stitchtool"), "exec", str(script)], cwd=work)


def prepare(work):
    for example, name in [("drawing.json", "drawing"), ("monograms.json", "monograms"),
                          ("user-patterns.json", "user-patterns")]:
        build_project(work, example, name)
    patterns = json.loads((ROOT / "examples" / "user-patterns.json").read_text())
    tulip = next(s["params"]["pattern"] for s in patterns
                 if s.get("command") == "pattern.define" and s["params"]["pattern"]["id"] == "tulip")
    (work / "tulip.json").write_text(json.dumps(tulip))
    shutil.copy(ROOT / "examples" / "digitize" / "badge.png", work / "badge.png")
    shutil.copy(ROOT / "crates" / "openstitch-engine" / "tests" / "fixtures" / "badge-golden.pes",
                work / "badge.pes")
    (work / "patterns").mkdir()
    samples = work / "home" / "Embroidery"
    samples.mkdir(parents=True)
    for sub in ["Badges", "Flowers", "Lettering"]:
        (samples / sub).mkdir()
    for f in ["oracle.pes", "oracle.dst", "oracle.jef", "oracle.exp", "oracle.pec", "oracle.xxx",
              "oracle.sew", "oracle.pcs", "oracle_v1.pes"]:
        shutil.copy(FIXTURES / f, samples / f)
    shutil.copy(work / "drawing.osd", samples / "drawing.osd")
    shutil.copy(work / "monograms.osd", samples / "monograms.osd")


def shrink(src, dst, width):
    """Downscales and quantises; README copies stay under about 200 KB."""
    im = Image.open(src).convert("RGB")
    im = im.resize((width, round(im.height * width / im.width)), Image.LANCZOS)
    colors = 256 if width > 1280 else 128
    im = im.quantize(colors=colors, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    dst.parent.mkdir(parents=True, exist_ok=True)
    im.save(dst, optimize=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--only", help="comma-separated scene names")
    a = p.parse_args()
    names = a.only.split(",") if a.only else list(SCENES)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        prepare(work)
        for name in names:
            args, outputs = SCENES[name]
            shot = work / f"{name}.png"
            run([str(BIN / "openstitch"), *args, "--size", "1440x900", "--screenshot", str(shot)],
                cwd=work, env=app_env(work))
            for out, width in outputs:
                shrink(shot, ROOT / out, width)
                print(f"{out}: {(ROOT / out).stat().st_size // 1024} KB")


if __name__ == "__main__":
    sys.exit(main())
