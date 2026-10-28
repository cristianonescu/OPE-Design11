#!/usr/bin/env python3
"""Regenerates the editor screenshots in docs/images and docs/images/readme with the real app.

    cargo build -p openstitch-app -p stitchtool
    python3 tools/screenshots.py [--only NAME,NAME]

Each scene runs `openstitch --screenshot` (1440x900 window) on a sample design built from
examples/ by `stitchtool exec`. The app runs inside a temporary folder and is given relative
paths, so the status bar never shows a local path. Images are downscaled and quantised to 256
colours (fewer if a busy scene would pass about 250 KB; 128 for the README copies): docs/images
at 1600 px wide, docs/images/readme at 1280 px. A stand-in home folder keeps the user's folder
names out of the images and holds a sample user thread chart. Needs Pillow; macOS renders with
Metal (the documentation images are made there).
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


def IMG(name):
    return (f"docs/images/rust-editor-{name}.png", 1600)


# name -> (arguments, output files with their widths)
SCENES = {
    "overview": (["--demo", "--select", "2"],
                 [("docs/images/rust-editor-overview.png", 1600), ("docs/images/readme/editor.png", 1280)]),
    "appmenu": (["--demo", "--show", "appmenu"], [("docs/images/rust-editor-appmenu.png", 1600)]),
    "update": (["--demo", "--show", "update"], [("docs/images/rust-editor-update.png", 1600)]),
    "close-prompt": (["--demo", "--show", "close-prompt"], [("docs/images/rust-editor-close-prompt.png", 1600)]),
    "view-dark": (["--demo", "--ribbon", "view", "--pane", "color", "--select", "2", "--theme", "dark"],
                  [("docs/images/rust-editor-view-dark.png", 1600)]),
    "stitches-tab": (["badge.pes", "--select", "1", "--ribbon", "stitches"],
                     [("docs/images/rust-editor-stitches-tab.png", 1600)]),
    "floating": (["--demo", "--tool", "pen", "--show", "floating"],
                 [("docs/images/rust-editor-floating-panes.png", 1600)]),
    "realistic": (["--demo", "--show", "realistic", "--ribbon", "view", "--zoom", "1.2"],
                  [("docs/images/rust-editor-realistic.png", 1600),
                   ("docs/images/readme/realistic-view.png", 1280)]),
    "simulator": (["--demo", "--simulate", "0.55", "--ribbon", "view"],
                  [("docs/images/rust-editor-sim.png", 1600),
                   ("docs/images/readme/stitch-simulator.png", 1280)]),
    "drawing-readme": (["drawing.osd", "--select", "1"], [("docs/images/readme/drawing.png", 1280)]),
    "monogram": (["monograms.osd", "--select", "1", "--show", "monogram"],
                 [("docs/images/rust-editor-monogram.png", 1600),
                  ("docs/images/readme/monogram.png", 1280)]),
    "auto-punch": (["--image", "badge.png", "--convert", "autopunch", "--ribbon", "image"],
                   [("docs/images/rust-editor-autopunch.png", 1600),
                    ("docs/images/readme/auto-punch.png", 1280)]),
    "design-database": (["--database", "home/Embroidery/oracle.pes", "--db-view", "large"],
                        [("docs/images/database-browse.png", 1600),
                         ("docs/images/readme/design-database.png", 1280)]),
    "database-details": (["--database", "home/Embroidery/oracle.pes", "--db-view", "details"],
                         [("docs/images/database-details.png", 1600)]),
    "database-search": (["--database", "home/Embroidery", "--db-dialog", "search"],
                        [("docs/images/database-search.png", 1600)]),
    "database-convert": (["--database", "home/Embroidery/oracle.pes", "--db-dialog", "convert"],
                         [("docs/images/database-convert.png", 1600)]),
    "database-catalogue": (["--database", "home/Embroidery", "--db-dialog", "catalog"],
                           [("docs/images/database-catalogue.png", 1600)]),
    "stitch-creator": (["user-patterns.osd", "--pattern", "tulip.json"],
                       [("docs/images/rust-editor-stitch-creator.png", 1600),
                        ("docs/images/readme/stitch-creator.png", 1280)]),
    "stitch-factory": (["sewing-stitches.osd", "--show", "sdf"],
                       [("docs/images/rust-editor-stitch-design-factory.png", 1600)]),
    # Feature screenshots for docs/FEATURE_MATRIX.md and docs/ARCHITECTURE.md.
    "draw": (["--ribbon", "rectangle", "--script",
              "type:r;drag:12,12,48,38;type:e;drag:56,14,88,46;type:p;click:15,55;click:45,52;"
              "click:52,82;click:22,86;key:Enter;type:r"],
             [IMG("draw")]),
    "pen": (["--script", "type:b;click:20,70;drag:50,30,70,30;click:80,70;click:20,70;"
             "type:b;click:20,85;drag:50,78,62,90;key:Enter;type:a;click:50,60"],
            [IMG("pen")]),
    "points": (["--demo", "--select", "2", "--tool", "point", "--ribbon", "shapes"], [IMG("points")]),
    "punch": (["--ribbon", "punch", "--script",
               "type:u;click:20,30;click:30,15;click:40,40;click:48,25;click:60,50;click:72,42;"
               "click:62,72;click:78,72;key:Enter"],
              [IMG("punch")]),
    "needles": (["badge.pes", "--select", "1", "--tool", "point", "--ribbon", "stitches"],
                [IMG("needles")]),
    "drawing": (["drawing.osd", "--select", "13", "--ribbon", "shapes"],
                [IMG("drawing")]),
    "arrange": (["--demo", "--select", "2", "--show", "arrange"], [IMG("arrange")]),
    "picker": (["--demo", "--select", "2", "--pane", "color"], [IMG("picker")]),
    "direction": (["sewing-direction.osd", "--select", "1", "--ribbon", "shapes"],
                  [IMG("direction")]),
    "ornaments": (["ornaments.osd", "--select", "2", "--ribbon", "shapes"], [IMG("ornaments")]),
    "stamps": (["stamps.osd", "--select", "3"], [IMG("stamps")]),
    "imports": (["imports.osd", "--ribbon", "image"], [IMG("imports")]),
    "image-wizard": (["--image", "badge.png", "--ribbon", "image"], [IMG("image-wizard")]),
    "background-fill": (["background-fill.osd", "--select", "1", "--wizard", "background"],
                        [IMG("background-fill")]),
    "applique": (["applique.osd", "--select", "17", "--wizard", "applique"], [IMG("applique")]),
    "patch": (["patch.osd", "--select", "1", "--wizard", "patch"], [IMG("patch")]),
    "cutwork": (["cutwork.osd", "--select", "21", "--wizard", "cutwork"], [IMG("cutwork")]),
    "lettering-fonts": (["lettering.osd", "--select", "3", "--ribbon", "text", "--show", "text"],
                        [IMG("lettering-fonts")]),
    "name-drop": (["name-drop.osd", "--select", "2", "--show", "namedrop"], [IMG("name-drop")]),
    "template-wizard": (["template-wizard.osd", "--show", "templates"], [IMG("template-wizard")]),
    "user-mapped-text": (["user-mapped-text.osd", "--select", "4", "--show", "fonts"],
                         [IMG("user-mapped-text")]),
    "font-creator": (["font-creator.osd", "--show", "fonts"], [IMG("font-creator")]),
    "split": (["split-banner.osd", "--show", "settings"], [IMG("split")]),
    "workspace": (["--demo", "--show", "settings,reference,measure", "--script",
                   "drag:22,22,78,58"],
                  [IMG("workspace")]),
    "thread-charts": (["--demo", "--show", "charts"], [IMG("thread-charts")]),
    "print": (["--demo", "--show", "print"], [IMG("print")]),
    "usb": (["--demo", "--show", "usb"], [IMG("usb")]),
}


def run(args, cwd, env=None):
    subprocess.run(args, cwd=cwd, check=True, env=env)


def app_env(work):
    """A stand-in home folder, so the Design Database tree and the pattern library show
    neutral names instead of the user's own folders."""
    env = dict(os.environ)
    home = work / "home"
    env.update({"HOME": str(home), "USERPROFILE": str(home), "APPDATA": str(home), "XDG_CONFIG_HOME": str(home / ".config"),
                "XDG_DATA_HOME": str(home / ".local" / "share"), "OPENSTITCH_PATTERNS": "patterns"})
    return env


def build_project(work, example, name):
    """Runs an example script and saves its design as NAME.osd in `work`."""
    steps = json.loads((ROOT / "examples" / example).read_text(encoding="utf-8"))
    steps = [s for s in steps if s.get("command") != "project.save"]
    steps.append({"command": "project.save", "params": {"path": str(work / f"{name}.osd")}})
    script = work / f"{name}.json"
    script.write_text(json.dumps(steps), encoding="utf-8")
    run([str(BIN / "stitchtool"), "exec", str(script)], cwd=work)


def prepare(work):
    shutil.copytree(ROOT / "examples" / "digitize", work / "digitize")
    shutil.copytree(ROOT / "examples" / "imports", work / "imports")
    for example in ["drawing", "monograms", "user-patterns", "sewing-stitches", "applique",
                    "background-fill", "cutwork", "patch", "font-creator", "imports", "lettering",
                    "name-drop", "ornaments", "sewing-direction", "split-banner", "stamps",
                    "template-wizard", "user-mapped-text"]:
        build_project(work, f"{example}.json", example)
    patterns = json.loads((ROOT / "examples" / "user-patterns.json").read_text(encoding="utf-8"))
    tulip = next(s["params"]["pattern"] for s in patterns
                 if s.get("command") == "pattern.define" and s["params"]["pattern"]["id"] == "tulip")
    (work / "tulip.json").write_text(json.dumps(tulip), encoding="utf-8")
    shutil.copy(ROOT / "examples" / "digitize" / "badge.png", work / "badge.png")
    shutil.copy(ROOT / "crates" / "openstitch-engine" / "tests" / "fixtures" / "badge-golden.pes",
                work / "badge.pes")
    (work / "patterns").mkdir()
    # A user thread chart for the Thread Charts window (the stand-in home's settings file).
    chart = [(0xc0392b, "101", "Poppy"), (0x2471a3, "102", "Lake"), (0xf4d03f, "103", "Straw"),
             (0x1e8449, "104", "Fern"), (0x6c3483, "105", "Plum")]
    threads = [{"red": c >> 16, "green": (c >> 8) & 255, "blue": c & 255, "name": n,
                "catalogNumber": code, "brand": "Studio"} for c, code, n in chart]
    user = {"format": "openstitch.user", "version": 1,
            "settings": {"threadCharts": [{"name": "Studio", "threads": threads}]}}
    for settings in [work / "home" / "Library" / "Application Support" / "OpenStitch",
                     work / "home" / "OpenStitch", work / "home" / ".config" / "openstitch"]:
        settings.mkdir(parents=True)
        (settings / "user.json").write_text(json.dumps(user), encoding="utf-8")
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
    """Downscales and quantises; docs/images stay under about 250 KB (fewer colours when a
    busy scene needs it), README copies under about 200 KB."""
    im = Image.open(src).convert("RGB")
    im = im.resize((width, round(im.height * width / im.width)), Image.LANCZOS)
    dst.parent.mkdir(parents=True, exist_ok=True)
    for colors in ([256, 224, 192, 160, 128] if width > 1280 else [128]):
        q = im.quantize(colors=colors, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
        q.save(dst, optimize=True)
        if dst.stat().st_size <= 250 * 1024:
            break


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
