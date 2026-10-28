"""Machine-stream comparer: what a Brother machine reads from a PES (the PEC section), for an
OpenStitch export vs the PE-DESIGN 11.50 file of the same design.

Documented fields only (pyembroidery's PEC reader/writer, the Embroidery_format_PES wiki page):
  PES: '#PES' + version, u32 LE PEC offset.
  PEC: 'LA:' + 16-char label + CR; 0x22 icon stride/height; 0x30 colour count - 1, then one PEC
  chart index per colour block; 0x200 stitch block (2 zero bytes, u24 length, 31 FF F0, u16 width,
  u16 height, u16 0x1E0, u16 0x1B0, stitch records, FF); then (colours + 1) 48x38 1-bit icons.
  Stitch records: 7-bit short form, or 12-bit long form with bit 0x80 and flags 0x10 (jump) and
  0x20 (trim); FE B0 xx = colour change; FF 00 = end. Units 0.1 mm.

Usage: peccmp.py ours.pes theirs.pes [--json] [--records N]
       peccmp.py --batch <work-dir> <reference-dir> [--json out.json]
"""
import json
import math
import struct
import sys
from pathlib import Path

ICON_BYTES = 6 * 38


def s12(v):
    v &= 0xFFF
    return v - 0x1000 if v > 0x7FF else v


def s7(v):
    return v - 0x80 if v > 0x3F else v


def read_pec(d, pec):
    """Decode the PEC section starting at offset `pec` of bytes `d`."""
    out = {}
    out["label"] = d[pec + 3:pec + 3 + 16].decode("latin-1")
    out["label_terminator"] = d[pec + 0x13]
    out["icon_stride"] = d[pec + 0x22]
    out["icon_height"] = d[pec + 0x23]
    ncol = d[pec + 0x30] + 1
    out["colour_count"] = ncol
    out["palette"] = list(d[pec + 0x31:pec + 0x31 + ncol])
    # bytes between palette and the stitch block should be spaces
    pad = d[pec + 0x31 + ncol:pec + 0x200]
    out["palette_pad_ok"] = all(b == 0x20 for b in pad)
    blk = pec + 0x200
    out["block_zero"] = list(d[blk:blk + 2])
    out["block_len"] = d[blk + 2] | d[blk + 3] << 8 | d[blk + 4] << 16
    out["block_magic"] = d[blk + 5:blk + 8].hex()
    out["width"], out["height"], out["f1e0"], out["f1b0"] = struct.unpack_from("<HHHH", d, blk + 8)
    p = blk + 16
    x = y = 0
    recs = []  # (kind, x, y, dx, dy, raw)
    colour_bytes = []
    while p + 1 < len(d):
        a, b = d[p], d[p + 1]
        if a == 0xFF:
            # end marker: one byte (the next byte is the first icon row, which is 0x00)
            p += 1
            break
        if a == 0xFE and b == 0xB0:
            colour_bytes.append(d[p + 2])
            recs.append(("colour", x, y, 0, 0, d[p + 2]))
            p += 3
            continue
        start = p
        p += 2
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
        recs.append((kind, x, y, dx, dy, d[start:p].hex()))
    out["stream_end"] = p
    out["records"] = recs
    out["colour_bytes"] = colour_bytes
    # icons: (ncol + 1) * 228 bytes after the stream
    icons = ncol + 1
    out["icons_expected"] = icons
    rest = len(d) - p
    out["icons_bytes_after_stream"] = rest
    out["icons_present"] = rest >= icons * ICON_BYTES
    out["block_len_matches"] = (out["block_len"] == p - blk)
    return out


def read_file(path):
    d = Path(path).read_bytes()
    info = {"file": str(path), "bytes": len(d)}
    if d[:4] == b"#PES":
        info["pes_version"] = d[4:8].decode("latin-1")
        pec = struct.unpack_from("<I", d, 8)[0]
        info["pec_offset"] = pec
    elif d[:8] == b"#PEC0001":
        info["pes_version"] = None
        pec = 8
    else:
        raise ValueError(f"{path}: not PES/PEC")
    info["pec"] = read_pec(d, pec)
    return info


def summarize(info):
    pec = info["pec"]
    recs = pec["records"]
    kinds = [r[0] for r in recs]
    stitches = [r for r in recs if r[0] == "stitch"]
    lengths = [math.hypot(r[3], r[4]) for r in stitches]
    jump_lengths = [math.hypot(r[3], r[4]) for r in recs if r[0] in ("jump", "trim")]
    # jump runs: consecutive jump/trim records
    runs = []
    cur = []
    for r in recs:
        if r[0] in ("jump", "trim"):
            cur.append(r)
        else:
            if cur:
                runs.append(cur)
                cur = []
    if cur:
        runs.append(cur)
    first = recs[0] if recs else None
    last_needle = stitches[-1] if stitches else None
    last = recs[-1] if recs else None
    s = {
        "pes_version": info.get("pes_version"),
        "bytes": info["bytes"],
        "label": pec["label"].rstrip(),
        "label_raw": pec["label"],
        "colour_count": pec["colour_count"],
        "palette": pec["palette"],
        "colour_change_bytes": pec["colour_bytes"],
        "width": pec["width"], "height": pec["height"],
        "f1e0": pec["f1e0"], "f1b0": pec["f1b0"],
        "block_len_matches": pec["block_len_matches"],
        "icons_present": pec["icons_present"],
        "icons_bytes_after_stream": pec["icons_bytes_after_stream"],
        "icons_expected": pec["icons_expected"],
        "records": len(recs),
        "stitches": len(stitches),
        "jumps": kinds.count("jump"),
        "trims": kinds.count("trim"),
        "colour_changes": kinds.count("colour"),
        "zero_length_stitches": sum(1 for r in stitches if r[3] == 0 and r[4] == 0),
        "longest_stitch_01mm": max(lengths) if lengths else 0,
        "longest_jump_01mm": max(jump_lengths) if jump_lengths else 0,
        "jump_runs": len(runs),
        "jump_run_lengths": [len(r) for r in runs][:40],
        "first_record": first[:5] if first else None,
        "first_needle": stitches[0][1:3] if stitches else None,
        "last_needle": last_needle[1:3] if last_needle else None,
        "last_record": last[:5] if last else None,
        "min_xy": [min(r[1] for r in stitches), min(r[2] for r in stitches)] if stitches else None,
        "max_xy": [max(r[1] for r in stitches), max(r[2] for r in stitches)] if stitches else None,
    }
    # trim run shapes: for each trim record, the length of the move and what follows
    s["trim_moves_01mm"] = [round(math.hypot(r[3], r[4]), 1) for r in recs if r[0] == "trim"][:40]
    s["jump_moves_01mm"] = [round(math.hypot(r[3], r[4]), 1) for r in recs if r[0] == "jump"][:40]
    # what precedes each colour change and what follows it
    pre_post = []
    for i, r in enumerate(recs):
        if r[0] == "colour":
            pre = recs[i - 1][0] if i > 0 else None
            post = recs[i + 1][0] if i + 1 < len(recs) else None
            post2 = recs[i + 2][0] if i + 2 < len(recs) else None
            pre_post.append([pre, "colour", post, post2])
    s["around_colour_changes"] = pre_post[:20]
    # the end: records before the end marker
    s["tail_kinds"] = kinds[-4:]
    s["head_kinds"] = kinds[:4]
    return s


def diff_records(a, b, limit=10):
    """First `limit` divergences, record by record (absolute positions, kinds)."""
    ra, rb = a["pec"]["records"], b["pec"]["records"]
    out = []
    n = min(len(ra), len(rb))
    for i in range(n):
        x, y = ra[i], rb[i]
        if x[0] != y[0] or x[1] != y[1] or x[2] != y[2]:
            out.append({"i": i, "ours": x[:5], "pe": y[:5]})
            if len(out) >= limit:
                break
    return {"first_divergence": out[0]["i"] if out else (n if len(ra) != len(rb) else None),
            "divergences_shown": out, "len_ours": len(ra), "len_pe": len(rb)}


def compare(ours, theirs):
    a, b = read_file(ours), read_file(theirs)
    sa, sb = summarize(a), summarize(b)
    keys = ["label", "colour_count", "palette", "colour_change_bytes", "width", "height", "f1e0",
            "f1b0", "icons_present", "block_len_matches", "stitches", "jumps", "trims",
            "colour_changes", "zero_length_stitches", "longest_stitch_01mm", "longest_jump_01mm",
            "jump_runs", "head_kinds", "tail_kinds", "around_colour_changes", "first_needle",
            "last_needle", "min_xy", "max_xy"]
    differs = {k: {"ours": sa[k], "pe": sb[k]} for k in keys if sa[k] != sb[k]}
    return {"ours": sa, "pe": sb, "differs": differs, "records": diff_records(a, b)}


def fmt(v):
    return json.dumps(v)


def report(name, r):
    lines = [f"## {name}", f"ours: {r['ours']['pes_version']}  pe: {r['pe']['pes_version']}"]
    for k, v in r["differs"].items():
        lines.append(f"  {k:24} ours={fmt(v['ours'])}  pe={fmt(v['pe'])}")
    rd = r["records"]
    lines.append(f"  records ours={rd['len_ours']} pe={rd['len_pe']} first divergence={rd['first_divergence']}")
    for d in rd["divergences_shown"][:3]:
        lines.append(f"    #{d['i']}: ours={d['ours']} pe={d['pe']}")
    return "\n".join(lines)


def batch(work, refdir, json_out=None):
    work, refdir = Path(work), Path(refdir)
    refs = {p.stem: p for p in refdir.rglob("*.pes")}
    results = {}
    for spec_dir in sorted(work.iterdir()):
        if not spec_dir.is_dir():
            continue
        ref = refs.get(spec_dir.name)
        if ref is None:
            continue
        ours = None
        for cand in ["a.pes", "design.pes", "out.pes"]:
            if (spec_dir / cand).exists():
                ours = spec_dir / cand
                break
        if ours is None:
            pes = sorted(spec_dir.glob("*.pes"))
            ours = next((p for p in pes if not p.stem.endswith("t")), pes[0] if pes else None)
        if ours is None:
            continue
        try:
            r = compare(ours, ref)
        except Exception as e:  # noqa: BLE001
            print(f"## {spec_dir.name}: ERROR {e}")
            continue
        r["ours_file"] = str(ours)
        r["pe_file"] = str(ref)
        results[spec_dir.name] = r
        print(report(spec_dir.name, r))
        print()
    if json_out:
        Path(json_out).write_text(json.dumps(results, indent=1, default=str))
    return results


if __name__ == "__main__":
    args = sys.argv[1:]
    if args and args[0] == "--batch":
        jo = None
        if "--json" in args:
            jo = args[args.index("--json") + 1]
        batch(args[1], args[2], jo)
    elif args and args[0] == "--dump":
        info = read_file(args[1])
        print(json.dumps(summarize(info), indent=1))
        n = int(args[2]) if len(args) > 2 else 60
        for i, r in enumerate(info["pec"]["records"][:n]):
            print(i, r)
    else:
        r = compare(args[0], args[1])
        print(report(f"{args[0]} vs {args[1]}", r))
        if "--json" in args:
            print(json.dumps(r, indent=1, default=str))
