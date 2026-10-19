"""Renders docs/images/digitize-*.png and convert-*.png from examples/digitize/*.png through openstitch-mcp.

Run from the repository root after `cargo build -p openstitch-mcp`:
    python3 -I tools/digitize_examples.py
Each example converts one sample image (made by
`cargo run -p openstitch-digitize --example samples`) and saves the MCP `preview` PNG.
"""
import base64
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path.cwd()
SERVER = ROOT / "target" / "debug" / "openstitch-mcp"

EXAMPLES = {
    "digitize-autopunch.png": [
        ("design_new", {"width": 130, "height": 110}),
        ("digitize_autoPunch", {"path": "badge.png", "colors": 6, "width": 120}),
    ],
    "digitize-cross-stitch.png": [
        ("design_new", {"width": 130, "height": 110}),
        ("digitize_crossStitch", {"path": "badge.png", "colors": 6, "cellSize": 2.5, "width": 120}),
    ],
    "digitize-photo-fill.png": [
        ("design_new", {"width": 110, "height": 90}),
        ("digitize_photoStitch", {"path": "sphere.png", "style": "fill", "colors": 8, "width": 100}),
    ],
    "digitize-photo-lines.png": [
        ("design_new", {"width": 110, "height": 90}),
        ("digitize_photoStitch", {"path": "sphere.png", "style": "lines", "mode": "mono", "width": 100}),
    ],
    # Conversions between data types, starting from the Auto Punch badge (objects 1-6).
    "convert-to-stitches.png": [
        ("design_new", {"width": 130, "height": 110}),
        ("digitize_autoPunch", {"path": "badge.png", "colors": 6, "width": 120}),
        ("object_toStitches", {"ids": [1, 2, 3, 4, 5, 6]}),
    ],
    "convert-to-blocks.png": [
        ("design_new", {"width": 130, "height": 110}),
        ("digitize_autoPunch", {"path": "badge.png", "colors": 6, "width": 120}),
        ("object_toStitches", {"ids": [1, 2, 3, 4, 5, 6]}),
        ("object_toShapes", {"ids": [1, 2, 3, 4, 5, 6], "mode": "blocks"}),
    ],
    "convert-region-to-line.png": [
        ("design_new", {"width": 130, "height": 110}),
        ("digitize_autoPunch", {"path": "badge.png", "colors": 6, "width": 120}),
        ("object_regionToLine", {"ids": [1, 2, 3, 4, 5, 6]}),
    ],
    "digitize-design-center.png": [
        ("design_new", {"width": 130, "height": 110}),
        ("digitize_designCenter", {"path": "drawing.png", "width": 120, "lineSew": "triple"}),
    ],
}


def main():
    proc = subprocess.Popen(
        [str(SERVER), "--root", str(ROOT / "examples" / "digitize")],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
    )
    ids = iter(range(1, 10_000))

    def call(method, params=None):
        msg = {"jsonrpc": "2.0", "id": next(ids), "method": method}
        if params is not None:
            msg["params"] = params
        proc.stdin.write(json.dumps(msg) + "\n")
        proc.stdin.flush()
        reply = json.loads(proc.stdout.readline())
        if "error" in reply:
            sys.exit(f"{method}: {reply['error']}")
        return reply["result"]

    call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                        "clientInfo": {"name": "digitize-examples", "version": "1"}})
    for name, steps in EXAMPLES.items():
        for tool, args in steps:
            r = call("tools/call", {"name": tool, "arguments": args})
            if r.get("isError"):
                sys.exit(f"{tool}: {r['content'][0]['text']}")
            print(tool, r["content"][0]["text"][:160])
        r = call("tools/call", {"name": "preview", "arguments": {"size": 900}})
        image = next(c for c in r["content"] if c["type"] == "image")
        (ROOT / "docs" / "images" / name).write_bytes(base64.b64decode(image["data"]))
        print("wrote", name)
    proc.stdin.close()
    proc.wait()


if __name__ == "__main__":
    main()
