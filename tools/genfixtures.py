"""Builds oracle fixtures with pyembroidery (independent MIT implementation)."""
import json, sys, os
import pyembroidery as pe
out = sys.argv[1]
CMD = {pe.STITCH:"stitch", pe.JUMP:"jump", pe.TRIM:"trim", pe.COLOR_CHANGE:"colorChange", pe.STOP:"stop", pe.END:"end"}

def build():
    """Two colour blocks: a zig-zag square, a trim + jump, then a long satin-like column."""
    p = pe.EmbPattern()
    p.add_thread({"hex": "#e01010", "description": "Red"})
    p.add_thread({"hex": "#1040c0", "description": "Blue"})
    p.metadata("name", "oracle")
    seq = []
    def st(x, y): p.add_stitch_absolute(pe.STITCH, x, y); seq.append([x, y, "stitch"])
    for i in range(0, 21):
        st(i * 10, 0 if i % 2 == 0 else 100)
    p.add_stitch_absolute(pe.TRIM, 200, 100); seq.append([200, 100, "trim"])
    p.add_stitch_absolute(pe.JUMP, 260, -50); seq.append([260, -50, "jump"])
    st(260, -50)
    p.add_stitch_absolute(pe.COLOR_CHANGE, 260, -50); seq.append([260, -50, "colorChange"])
    for i in range(0, 15):
        st(300 + (40 if i % 2 else 0), -50 + i * 25)
    st(300, 400)  # long stitch (> 121) forces DST interpolation
    p.add_stitch_absolute(pe.END, 300, 400); seq.append([300, 400, "end"])
    return p, seq

p, seq = build()
json.dump({"name": "oracle", "threads": ["#e01010", "#1040c0"], "stitches": seq}, open(os.path.join(out, "oracle_input.json"), "w"))
pe.write_dst(p, os.path.join(out, "oracle.dst"))
pe.write_pec(p, os.path.join(out, "oracle.pec"))
pe.write_pes(p, os.path.join(out, "oracle.pes"), {"version": "1t"})
for v in ("1", "6", "6t"):  # PES with the object section / thread table
    pe.write_pes(p, os.path.join(out, "oracle_v%s.pes" % v), {"version": v})
# PES v6 thread table with brand, catalogue number, description and chart all set (the chart is
# an arbitrary test string, not a real Brother chart name).
pc, _ = build()
for t, (catalog, brand, chart) in zip(pc.threadlist, (("800", "Acme", "Test chart A"), ("405", "Acme", "Test chart B"))):
    t.catalog_number, t.brand, t.chart = catalog, brand, chart
pe.write_pes(pc, os.path.join(out, "oracle_v6_chart.pes"), {"version": "6"})
pe.write_exp(p, os.path.join(out, "oracle.exp"))
pe.write_jef(p, os.path.join(out, "oracle.jef"), {"date": "20261018120000"})
pe.write_xxx(p, os.path.join(out, "oracle.xxx"))

# pyembroidery 1.5.1 reads PCS and SEW but cannot write them. These two fixtures are assembled
# here from the layout its readers expect (see docs/FILE_FORMATS.md); the oracle is the read.
import struct
from pyembroidery.EmbThreadSew import get_thread_set as sew_set
from pyembroidery.EmbThread import find_nearest_color_index, color_hex

def build_pcs(seq, threads):
    """Pfaff PCS: absolute 9-byte records in 1/6 mm, y up. Trims dropped, no end record."""
    b = bytearray(b"2")
    b.append(2 if max(r[0] for r in seq) - min(r[0] for r in seq) <= 800
             and max(r[1] for r in seq) - min(r[1] for r in seq) <= 800 else 3)
    b += struct.pack("<H", len(threads))
    for h in threads:
        c = color_hex(h); b += bytes([(c >> 16) & 255, (c >> 8) & 255, c & 255, 0])
    recs = bytearray(); n = 0; fx = fy = 0
    for x, y, cmd in seq:
        if cmd in ("stitch", "jump"):
            fx, fy = round(x * 3 / 5), round(-y * 3 / 5)
        elif cmd not in ("colorChange", "stop"):
            continue  # trim, end
        ctrl = {"stitch": 0, "jump": 4}.get(cmd, 1)
        recs += bytes([0]) + (fx & 0xFFFFFF).to_bytes(3, "little") + bytes([0]) + (fy & 0xFFFFFF).to_bytes(3, "little") + bytes([ctrl])
        n += 1
    return bytes(b + struct.pack("<H", n) + recs)

def build_sew(seq, threads):
    """Janome SEW: chart indices, zero pad to 0x1D78, JEF-style relative records, 80 10 end."""
    chart = sew_set(); chart[0] = None
    b = bytearray(struct.pack("<H", len(threads)))
    for h in threads:
        b += struct.pack("<H", find_nearest_color_index(color_hex(h), chart))
    b += bytes(0x1D78 - len(b))
    xx = yy = 0
    for x, y, cmd in seq:
        if cmd == "trim":
            continue
        if cmd == "end":
            break
        dx, dy = x - xx, y - yy
        assert abs(dx) <= 127 and abs(dy) <= 127
        xx, yy = x, y
        prefix = {"stitch": b"", "jump": b"\x80\x02", "colorChange": b"\x80\x01", "stop": b"\x80\x01"}[cmd]
        b += prefix + bytes([dx & 255, -dy & 255])
    return bytes(b + b"\x80\x10")

from pyembroidery.EmbCompress import compress as hus_compress
from pyembroidery.EmbThreadHus import get_thread_set as hus_set

def hus_records(seq):
    """HUS/VIP records (cmd, dx, dy) with y up; moves over 127 split into jumps, END appended."""
    recs = []; xx = yy = 0
    for x, y, cmd in seq:
        if cmd == "end":
            break
        if cmd == "trim":
            recs.append((0x88, 0, 0)); continue
        dx, dy = x - xx, y - yy
        while abs(dx) > 127 or abs(dy) > 127:
            sx, sy = max(-127, min(127, dx)), max(-127, min(127, dy))
            recs.append((0x81, sx, sy)); xx += sx; yy += sy; dx -= sx; dy -= sy
        recs.append(({"stitch": 0x80, "jump": 0x81}.get(cmd, 0x84), dx, dy)); xx, yy = x, y
    return recs + [(0x90, 0, 0)]

def hus_streams(seq):
    """The three streams in pyembroidery's literal-only 'compressed' form (EmbCompress.compress)."""
    recs = hus_records(seq)
    return len(recs), [hus_compress(bytearray(r[i] & 255 if i < 2 else -r[2] & 255 for r in recs))
                       for i in (0, 1, 2)]

def hus_header(magic, seq, n_colors, first):
    count, streams = hus_streams(seq)
    xs = [r[0] for r in seq if r[2] in ("stitch", "jump")]; ys = [r[1] for r in seq if r[2] in ("stitch", "jump")]
    h = struct.pack("<III", magic, count, n_colors) + struct.pack("<hhhh", max(xs), -min(ys), min(xs), -max(ys))
    h += struct.pack("<III", first, first + len(streams[0]), first + len(streams[0]) + len(streams[1])) + bytes(10)
    return h, b"".join(streams)

def build_hus(seq, threads):
    """Husqvarna HUS, assembled from the layout pyembroidery's HusReader expects."""
    h, body = hus_header(0x00C8AF5B, seq, len(threads), 0x2A + 2 * len(threads))
    for t in threads:
        h += struct.pack("<H", find_nearest_color_index(color_hex(t), hus_set()))
    return h + body

from pyembroidery.EmbThreadShv import get_thread_set as shv_set

def build_shv(seq, threads, name="oracle"):
    """Husqvarna SHV (big-endian), assembled from the layout pyembroidery's ShvReader expects:
    0x56-byte text header, name, a blank 4x4 preview, colour table with per-block word counts,
    records starting 2 bytes before the end of the table. y down; jumps as 80 01 + i16 pairs,
    80 02 ends jump mode, trims as 80 03 (skipped by readers), colour changes implicit."""
    blocks = [bytearray()]; xx = yy = 0; in_jump = False
    for x, y, cmd in seq:
        if cmd == "end":
            break
        if cmd in ("colorChange", "stop"):
            blocks.append(bytearray()); continue
        if cmd == "trim":
            blocks[-1] += b"\x80\x03"; continue
        dx, dy = x - xx, y - yy; xx, yy = x, y
        if cmd == "jump":
            blocks[-1] += b"\x80\x01" + struct.pack(">hh", dx, dy); in_jump = True; continue
        if in_jump:
            blocks[-1] += b"\x80\x02"; in_jump = False
        assert abs(dx) <= 127 and abs(dy) <= 127
        blocks[-1] += bytes([dx & 255, dy & 255])
    b = bytearray(b"OpenStitch test fixture".ljust(0x56, b" "))
    b += bytes([len(name)]) + name.encode() + bytes([4, 4]) + bytes(4 + 2 * 4)
    b += bytes([len(threads)]) + bytes(18)
    for blk, t in zip(blocks, threads):
        b += struct.pack(">I", len(blk) // 2) + bytes([find_nearest_color_index(color_hex(t), shv_set())]) + bytes(9)
    return bytes(b[:-2] + b"".join(blocks))

def build_phc(pec_path):
    """Brother PHC, assembled from the layout pyembroidery's PhcReader expects around the stitch
    stream and colour list of a PEC file (icons left blank)."""
    pec = open(pec_path, "rb").read()[8:]
    n = pec[0x30] + 1
    colors = pec[0x31:0x31 + n]
    stream = pec[0x210:0x200 + int.from_bytes(pec[0x202:0x205], "little")]
    b = bytearray(b"#PHC0001".ljust(0x4A, b"\0"))
    b += bytes([38, 0, 6]) + struct.pack("<H", n) + colors + bytes(6 * 38 * (n + 1))
    pec_offset, pec_add = len(b), 4
    b[0x2B] = pec_add; b[0x30:0x32] = struct.pack("<H", pec_offset)
    b += bytes(pec_add) + struct.pack("<H", 2) + bytes(2) + struct.pack("<I", 8) + bytes(8) + bytes(10)
    b += bytes([n]) + bytes(n + 0x1D) + stream
    return bytes(b)

open(os.path.join(out, "oracle.phc"), "wb").write(build_phc(os.path.join(out, "oracle.pec")))
open(os.path.join(out, "oracle.shv"), "wb").write(build_shv(seq, ["#e01010", "#1040c0"]))
open(os.path.join(out, "oracle.hus"), "wb").write(build_hus(seq, ["#e01010", "#1040c0"]))
open(os.path.join(out, "oracle.pcs"), "wb").write(build_pcs(seq, ["#e01010", "#1040c0"]))
open(os.path.join(out, "oracle.sew"), "wb").write(build_sew(seq, ["#e01010", "#1040c0"]))
def dump_read(name, json_name):
    r = pe.read(os.path.join(out, name))
    json.dump({"threads": [t.hex_color() for t in r.threadlist],
               "stitches": [[s[0], s[1], CMD.get(s[2] & pe.COMMAND_MASK, str(s[2]))] for s in r.stitches]},
              open(os.path.join(out, json_name), "w"))

for ext in ("dst", "pec", "pes", "exp", "jef", "pcs", "sew", "xxx", "hus", "shv", "phc"):
    dump_read("oracle." + ext, "oracle_%s_read.json" % ext)
# Files written by OpenStitch itself (tests regenerate them with OPENSTITCH_REGEN=1), read here by
# the independent decoder.
for v in ("1", "6", "6t"):
    dump_read("oracle_v%s.pes" % v, "oracle_v%s_pes_read.json" % v)
for ext in ("hus",):
    if os.path.exists(os.path.join(out, "openstitch." + ext)):
        dump_read("openstitch." + ext, "openstitch_%s_read.json" % ext)
print("ok", sorted(os.listdir(out)))
