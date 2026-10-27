# File formats

Units are 0.1 mm throughout. OpenStitch's internal convention: absolute coordinates,
+x right, +y down (the PEC/PES convention).

**Implementation:** the Rust crate `crates/openstitch-formats` (one module per format plus
`FileType` for extension lookup and dispatch), tested against the oracle fixtures in
`crates/openstitch-formats/tests/fixtures`. Every reader treats input as hostile: truncation, out-of-file offsets and colour counts larger than the data return a
`FormatError`, never a panic (checked by truncation, corruption and random-byte tests and one
cargo-fuzz target per reader under `crates/openstitch-formats/fuzz`).

Notes that do not change the fixture bytes:
- PEC label: the first 8 characters are kept as whole UTF-8 characters that fit in 16 bytes,
  so non-ASCII names never misalign the 512-byte header.
- JEF date: a supplied stamp must be 14 ASCII digits; the default is the current UTC time.

| Format | Read | Write | Evidence level | Notes |
|---|---|---|---|---|
| PES | ✅ all versions, via the PEC section; v4+ metadata, v5+ RGB thread table, `CEmbOne`/`CSewSeg` objects | ✅ v1 and v6, truncated or with one `CEmbOne`/`CSewSeg` pair | Oracle-verified (pyembroidery), byte-identical for 1t, 1, 6, 6t | Shape, text and other PE-DESIGN object classes: no public description (Blocked). |
| PEC | ✅ | ✅ | Oracle-verified | Byte-identical round trip of the oracle file. |
| DST | ✅ | ✅ | Oracle-verified | Byte-identical round trip of the oracle file. |
| EXP | ✅ | ✅ | Oracle-verified | Byte-identical round trip. |
| JEF | ✅ | ✅ | Oracle-verified | Byte-identical round trip (fixed header date). |
| XXX | ✅ | ✅ | Oracle-verified | Byte-identical round trip of a pyembroidery-written file. |
| PCS | ✅ | ✅ | Oracle-verified read; writer self-consistent | pyembroidery has no PCS writer: the fixture is assembled from its reader's layout. Trims dropped, 1/6 mm grid. |
| SEW | ✅ | ✅ | Oracle-verified read; writer self-consistent | pyembroidery has no SEW writer: same approach as PCS. Janome chart colours, trims dropped. |
| HUS | ✅ | ✅ | Oracle-verified read; writer output decoded identically by the oracle | Own LZ77 + Huffman compressor (`lzh`); 29-colour HUS chart. |
| VIP | ✅ | ✅ | Spec-derived (libembroidery); self round trip; header and colour table confirmed by libembroidery's reader | No independent stitch decoder exists (pyembroidery has no VIP module; libembroidery's decompressor is a stub). |
| SHV | ✅ | ❌ (read only) | Oracle-verified read | No public writer or description of the unknown header bytes; `FileType::Shv.can_write()` is false. |
| CSD | ✅ (spec-derived) | ❌ | Spec-derived (libembroidery); synthetic round-trip tests only | Fixed-key obfuscation; no public writer or reference files. See "CSD" below. |
| PHC | ✅ | — (read only, as in PE-DESIGN) | Oracle-verified read (pyembroidery) | PEC stitch data in a Brother wrapper. |
| PHX | ❌ | — | **Blocked** | No public description or open-source reader; studying PE-DESIGN output would be reverse engineering (LEGAL_AND_LICENSING.md). |
| PEN | ❌ | — | **Blocked** | Brother's encrypted format for designs bought from iBroidery; decrypting it would circumvent a protection measure. Out of scope. |
| FCM / VCM (ScanNCut) | ❌ | ❌ | **Blocked** (owner decision) | No Brother specification; see "FCM" below. |
| PE-DESIGN project (.pes as project, .pem?) | ❌ | ❌ | Not investigated | Needs reference files saved by PE-DESIGN. |

**Authoritative list (Brother online guide, "Saving" and "Converting Embroidery Design
Files"):** PE-DESIGN 11 saves `.pes` and exports `.dst .hus .exp .pcs .vip .sew .jef .csd .xxx .shv`.
Design Database converts between `.pes .dst .exp .pcs .hus .vip .shv .jef .sew .csd .xxx`.
`.phc .phx .pen` cannot be produced. VP3 is not in the list.

## PEC

| Offset | Size | Content |
|---|---|---|
| 0x000 | 20 | `LA:` + label (first 8 chars, space padded to 16) + CR |
| 0x014 | 12 | spaces |
| 0x020 | 2 | `FF 00` |
| 0x022 | 1 | icon byte stride (6 → 48 px) |
| 0x023 | 1 | icon height (38) |
| 0x024 | 12 | spaces |
| 0x030 | 1 | colour blocks − 1 |
| 0x031 | n | PEC chart index (1–64) per colour block |
| … | | space padding to 0x200 |
| 0x200 | 2 | `00 00` |
| 0x202 | 3 | u24 stitch-block length, measured from 0x200 |
| 0x205 | 3 | `31 FF F0` |
| 0x208 | 2+2 | u16 design width, height |
| 0x20C | 2+2 | u16 `0x1E0`, `0x1B0` (constant in all observed files; meaning unknown) |
| 0x210 | … | stitch stream, terminated by `FF` |
| end | 228 × (n+1) | 1-bit icons: whole design, then one per colour block |

Stitch stream: each axis value is either one byte (7-bit two's complement, used when
−64 < d < 63) or two bytes `1 f f f v v v v  v v v v v v v v` (bit 7 set; bit 5 = trim,
bit 4 = jump; 12-bit two's complement). Colour change: `FE B0` then `02`/`01` alternating.

Colour mapping: PEC stores indexes into Brother's fixed 64-entry chart, not RGB. Writer
assigns each distinct design colour the nearest still-unused chart entry ("red-mean"
distance), so distinct colours stay distinct on the machine.

## PES

`#PESvvvv` signature, u32 offset of the PEC section, version-dependent object section, PEC
section. Default output (`pes::WriteOptions::default`, `FileType::Pes.write`, and so
`design.export` without `pesVersion`, Name Drop, USB transfer and Design Database convert) is
`#PES0060` with one `CEmbOne`/`CSewSeg` stitch group and the RGB thread table, byte-identical to
pyembroidery's `6`. `pesVersion` = `1t`, `1`, `6t` selects the others; `1t` (`#PES0001`, offset
0x16, 10 zero bytes, no objects) stays as the smallest, machine-only file. **Unverified:**
whether every Brother machine and PE-DESIGN 11 open these files; needs a black-box open test on
the owner's machine (docs/COMPATIBILITY_AUDIT.md item 2).

### Save as a PES version: what each keeps

| Written as | Stitches + PEC chart colours | Exact RGB, thread names, brands | Name, author, keywords… (metadata) | Stitch-group object (opens in PE-DESIGN as a grouped stitch pattern) | Editable shapes / text |
|---|---|---|---|---|---|
| `1t` (machine only, smallest) | ✅ | ❌ (nearest of the 64 Brother colours) | ❌ (8-character PEC label only) | ❌ | ❌ |
| `1` | ✅ | ❌ | ❌ | ✅ (colours as PEC indexes) | ❌ |
| `6t` | ✅ | ✅ | ✅ | ❌ | ❌ |
| `6` (default) | ✅ | ✅ | ✅ | ✅ | ❌ |
| 4, 5 (5.5, 5.6), 7–10 | **Blocked** | | | | |

Editor: File ▸ Export ▸ PES offers the four. Engine: `design.export` `pesVersion`.
**Why v4, v5 and v7–v10 are not written:** no public writer exists. pyembroidery's reader only
gives each version's field *lengths* (v5 has 24 fixed bytes where v6 has 36; v8 has 38; v9/v10
add a hoop name). It does not say which fields were added or removed, so a v5 header written
by analogy with v6 would be a guess that PE-DESIGN might misread. They are read, not written.
Unblocks it: reference files saved by PE-DESIGN's "Save As" in each version (needs a licensed
installation and the legal opinion).

Object section as pyembroidery 1.5.1 reads and writes it (little-endian):

| Part | Content |
|---|---|
| v1 header | u16 scale-to-fit (1), u16 hoop (1), u16 object count |
| v4+ header | u16 hoop, 2 ASCII digits, then five u8-length strings: name, category, author, keywords, comments. Strings are Windows-1252 with CR LF line breaks, as PE-DESIGN 11 writes them; OpenStitch reads valid UTF-8 (its earlier files, pyembroidery) as UTF-8 |
| v5–v10 | fixed fields (36 bytes in v6: hoop/page/grid settings, written with pyembroidery's values), u8-length image file name, six f32 (identity transform), u16 counts of programmable fills, motifs, feather patterns, u16 thread count, threads |
| thread (v5+) | u8-string catalogue no., R G B, u8 0, u32 colour type (`0x0A` custom colour), u8-strings name, brand, chart |
| objects | u16 object count, `FFFF 0000`, then objects named by u16-length class strings, separated by `FFFF 0000`, ended by `0000 0000` |
| `CEmbOne` | 16 zero bytes, six f32 transform (a b c d tx ty; pyembroidery places the group at `tx = 1000 − w/2`, `ty = 1000 + h/2`), u16 1, 0, 0, width, height, 8 zero bytes, u16 section count |
| `CSewSeg` | sections separated by `03 80`: u16 flag (0 stitches, 1 jump), u16 colour (PEC index in v1, thread-table index in v6), u16 n, n × (i16 x, i16 y) relative to the bottom-left corner; then u16 n and n × (section, colour) colour log. No trim type: a trim is a jump segment, and the PEC section's `0x20` flag carries the trim. Like PE-DESIGN 11, OpenStitch writes no lead-in jump segment before the first stitch run |
| v6 tail | after the objects u32 0, u32 0 and (i, 0) per colour-log entry; after the PEC section the PEC colour list padded to 128 bytes, 0x90 zero bytes per thread, u24 RGB per thread, u16 0 |

Thread-table strings are u8-length single-byte strings, or MFC Unicode strings (`FF FE FF`, length in UTF-16 units, UTF-16LE), which PE-DESIGN 11 writes; the reader takes both. The v6 thread table carries exact RGB, names, catalogue numbers, brands, chart names and the
colour type (`Thread::chart`, `Thread::kind`). Chart and type are kept only as a file supplied
them: OpenStitch writes an empty chart and type `0x0A` for its own colours and never invents
Brother chart strings; a non-`0x0A` type read from a file is written back unchanged
(`pes_v6_thread_chart_and_kind_round_trip_byte_identically`, fixture `oracle_v6_chart.pes`). EduTechWiki's
other type codes (1 list colour, 2 appliqué, B Brother) are unverified. The reader maps the table
onto colour blocks as pyembroidery does (one to one when it covers every block, else by first
use of each PEC index). The v4+ metadata strings become pattern metadata (`name`, `category`,
`author`, `keywords`, `comments`, plus `pes.hoopName`, `pes.imageFile`). The design's Design
Property fields (`Design.properties`, `design.setProperties`) are copied into that metadata when
compiling, so v4+ output carries them; `stitches.import` offers the file's fields and adopts them
into a design that has none (`adoptProperties` overrides).
`pes::read_objects` exposes the parsed objects (transform, size, stitch runs, colour log).

PE-DESIGN's editable objects have no public description. Their mapping, from the owner's own
PE-DESIGN files, is in "PES 0110 object list" below. The reader reads `CEmbLine` shapes and
stops at the first other class (`ObjectSection::unparsed`). OpenStitch still writes its shapes
as stitches (a `CEmbOne` stitch group). Unverified: whether PE-DESIGN accepts several `CEmbOne`
objects in one file, so the writer emits one for the whole design.

### PES 0110 object list

Facts only, from PE-DESIGN 11.50 files the owner made, compared with each other. The files are
minimal designs that vary one property at a time; PE-DESIGN's program files are not analysed
(project policy, [LEGAL_AND_LICENSING.md](LEGAL_AND_LICENSING.md)). Evidence:
`crates/openstitch-formats/tests/fixtures/pe11` (provenance and SHA-256 in its `PROVENANCE.md`)
and the round-1 compat references. Reader: `crates/openstitch-formats/src/pes11.rs`; each fact
below is pinned by a test in `tests/pes11.rs`.

- **Only `#PES0110` keeps editable objects.** PE-DESIGN's Ver.6.0 save of a rectangle is a
  `CEmbOne`/`CSewSeg` stitch group (it warns that the data is saved as stitch data).
- **The object section has no stitches.** Changing Fill to Satin changes one byte of the object
  section while the PEC stitches change entirely: objects hold geometry and attributes, and
  PE-DESIGN regenerates stitches from them.
- **Object list:** a u16 object count, then MFC `CArchive` objects. The first object of a class
  starts with `FFFF`, a u16 schema (0) and the u16-length ASCII class name. A later object of an
  already named class starts with `0x8000 | n`, where n is the class's entry in the archive
  map. MFC numbers classes and objects in one sequence from 1: in r14 (`AB`), `CEmbNText` is
  1, the text object 2, `CTTFLetter` 3 and the letter `A` 4, so `B` starts `03 80`; in r16 the
  second rectangle starts `01 80`. The reader finds the list by the first `FFFF 0000` + class name after the
  header strings; the hoop, page and thread-table fields in between are not all mapped.
- **Thread table** (right before the object list): u16 count, then per thread the catalogue
  number, RGB and a pad byte, a u32 colour type (10), the PEC chart index as a string ("16" for
  005 SILVER, "5" for 800 RED, matching the PEC colour blocks), the brand ("EMBROIDERY") and the
  chart name. Strings are MFC Unicode strings (`FF FE FF`, u8 length in UTF-16 units,
  UTF-16LE). The entries are in sewing order, and the table can list colours no block sews (an
  unused attribute's default, below), so `pes::read` keeps PEC chart colours for 0110 files and
  the table stays in `ObjectSection::threads`. The fields between the header strings and the
  table include a machine-model string (`nv880e-M`) and are not all mapped, so the reader
  locates the table as the one that parses to exactly where the object list starts.
- **Coordinates** are 0.1 mm with the page's top-left corner at (500, 500), y down, in every
  file so far (all on a 100 × 100 mm page).
- Classes seen: `CEmbLine` (Shape tools: rectangle, ellipse, open and closed lines), `CEmbNText`
  with one `CTTFLetter` per letter (text), `CEmbSpecialShape` (Shapes gallery presets).

`CEmbLine` body, offsets from the byte after the class name or tag (u16 unless noted):

| Offset | Field | Evidence (each file differs from the baseline only here) |
|---|---|---|
| 0 | bounds `x0 y0 x1 y1` (i16) | r02 width, r03 position |
| 8 | sewn extent `x0 y0 x1 y1` (i16): the bounds plus half the line width | r07 (equals the bounds when the line is not sewn) |
| 16 | transform, 6 × f32 (identity so far) | all |
| 40 | 0 closed, 1 open | r11 line |
| 42 | line sewn 1, not sewn 0 | r07 |
| 44 | line sew type: 0 Zigzag, 1 Running, 3 E or V stitch, 4 Triple, 9 while not sewn | r07, r19, the `line-*-default` references |
| 46 | line colour, a thread-table index | r17 (red 1 → silver 0) |
| 48 | zigzag width, 0.1 mm | r21 (2.0 → 3.0 mm) |
| 50 | zigzag density, 0.1 line/mm | r01 (4.5) |
| 52 | running stitch pitch, 0.1 mm; kept while another line type is chosen | r20 (2.0 → 3.0 mm) |
| 60 | line under sewing 1 on, 0 off | r22 |
| 134 | region sew type: 0 Satin, 1 Fill, 2 Programmable fill (11 on the inner copy of a hole-sewing pair) | r04, programmable-fill-45, hole-sewing-pentagon-star |
| 136 | region density, 0.1 line/mm | r09 (4.5 → 6.0) |
| 138 | gradation 0 off, 1 on | region-travel-u-gradation |
| 162 | a thread-table index for an attribute not identified yet: it points at 800 RED (PE's default colour) in every file, even when nothing sews red | r17, r18, round-1 references |
| 168 | region direction, degrees | r08 (45 → 90) |
| 170 | region colour, a thread-table index | shape-start-points (three shapes: 0, 2, 3 = 800, 007, 027), hole-sewing-pentagon-star |
| 174 | Fill Frequency, % (default 30) | fill-frequency-0/25/50 |
| 180 | region under sewing 1 on, 0 off | r06 |
| 186 | Running Stitch Path: 0 Inside, 1 Along outline, 2 On outline | region-travel-u-* |
| 4432 | geometry: an 84-byte header with three handle marks (f64 and f32 x, y each), a u32 vertex count n, then n vertices | all |

Each vertex is 28 bytes: x, y and two cubic Bézier handles (6 × f32), then a u32 kind (0 corner,
`0x11` smooth). A rectangle starts in the middle of its bottom edge and runs round the corners;
an ellipse is four smooth vertices whose handles lie 0.5523 × the radius along the tangent. The
record ends after its last vertex. After the list come 52 or more bytes not mapped yet.

Not mapped yet: the fields that differ between E and V, the line's run times, the three handle marks, `CEmbNText`/`CTTFLetter`
(text, font name, scale), `CEmbSpecialShape`, Manual Punch blocks (a `CEmbLine` variant the
reader refuses), and the trailer.

## DST

512-byte header of `KEY:value` CR-terminated fields (`LA ST CO +X -X +Y -Y AX AY MX MY PD`),
`0x1A`, space padding. Records are 3 bytes encoding dx, dy ∈ [−121, 121] in balanced ternary
(powers 1, 3, 9, 27, 81); byte 2 bits: 7 = jump, 6+7 = colour change, `F3` = end. DST y points
up. DST has no trim command: a trim is written as three jumps (+2,+2) (−4,−4) (+2,+2), and the
reader treats ≥3 consecutive jumps as a trim followed by the net move.

## DST trims

PE-DESIGN exposes "Number of jumps for trim" for DST import and export. OpenStitch takes
`trimJumps` on read and write (default 3). The reader marks a trim once a jump run reaches
that count, and clips zero-displacement runs.

## EXP

No header. 2-byte signed (dx, dy) records with y up; `80 04 dx dy` jump, `80 80 07 00` trim,
`80 01 00 00` colour change. No colour data. Max delta ±127.

## JEF

| Offset | Content |
|---|---|
| 0x00 | u32 offset of stitch data (`0x74 + 8 × colours`) |
| 0x04 | u32 `0x14` |
| 0x08 | 14-char date `yyyyMMddHHmmss`, 2 zero bytes |
| 0x18 | u32 colour count (stops included) |
| 0x1C | u32 point count |
| 0x20 | u32 hoop code (0 = 110×110, 1 = 50×50, 2 = 140×200, 3 = 126×110, 4 = 200×200) |
| 0x24 | 2 × (half width, half height) |
| 0x34 | 4 × hoop-edge distances (left, top, right, bottom; −1 if it does not fit) |
| 0x74 | u32 Janome chart index per colour (0 = stop), then u32 `0x0D` per colour |

Records: 2-byte signed (dx, dy), y up; `80 01` colour change, `80 02` jump, `80 10` end.
JEF has no trim command. The reader infers a trim when a jump run moves more than 3 mm.

## XXX (Singer)

| Offset | Content |
|---|---|
| 0x17 | u32 record count (end marker excluded) |
| 0x27 | u32 colour count (readers use the low 16 bits), u16 0 |
| 0x2D | i16 width, height, end x, −end y, −min x, max y (extents over every record) |
| 0xF6 | u16 `0x20` |
| 0xFC | u32 file offset of the end marker |
| 0x100 | records |

Records: 2-byte signed (dx, dy) with y up; `7D dx(i16) dy(i16)` long form (written for a stitch
of ±124 or more; the reference reader turns it into a move, so a long stitch reads back as a
jump); `7F 01 dx dy` jump; `7F 03 00 00` trim (a non-zero delta adds a move on read);
`7F 08 dx dy` colour change (`0A`–`17` also read as colour change); `7F 7F 02 14` end. Then
`00 00`, `00 R G B` per colour, zero entries up to 21 colours, `00 FF FF FF`, `00 01`.
Moves are split at 124. Stops are written as colour changes.

## PCS (Pfaff)

| Offset | Content |
|---|---|
| 0x00 | version byte; OpenStitch writes `'2'` (`0x32`), as libembroidery does |
| 0x01 | hoop: 2 = 80×80 mm, 3 = large (115×120 per libembroidery); written as 2 when the design fits 80×80 |
| 0x02 | u16 colour count, then `R G B 00` per colour |
| … | u16 record count, then 9-byte records |

Record: `00 x(i24le) 00 y(i24le) ctrl`, absolute, y up, in 1/6 mm (pyembroidery's 5/3 ratio
to 0.1 mm). `ctrl` 0x00 stitch, bit 0x01 colour change, bit 0x04 jump (libembroidery calls it a
trim; pyembroidery a move). There is no trim or end record: the reference reader stops at end of
file. On write, trims are dropped and a stop becomes a colour change to the same thread.
Coordinates are rounded to the 1/6 mm grid, so a read gives fractional 0.1 mm values.

pyembroidery 1.5.1 has no PCS writer. `oracle.pcs` is assembled in `tools/genfixtures.py` from
the layout its reader expects, and its reader produces `oracle_pcs_read.json`. The byte-identity
test therefore pins OpenStitch's writer to that layout; it is not an independent writer check.
Open questions for the reference tier: the version byte PE-DESIGN writes, whether it pads the
colour table to 16 entries, and its hoop choice.

## SEW (Janome)

| Offset | Content |
|---|---|
| 0x00 | u16 colour count, then u16 Janome SEW chart index per colour (`SewThreadChart`, 79 entries) |
| … | zero padding |
| 0x1D78 | records |

Records as JEF: 2-byte signed (dx, dy) with y up; `80 01 dx dy` colour change, `80 02 dx dy`
jump (`80 04` read as jump too), `80 10` end. The pyembroidery reader treats `80 10` as a
stitch when 2 more bytes follow, so the writer ends the file right after `80 10`. Chart index 0
("Unknown") is read but never written. No trim record: trims are dropped; stops become colour
changes to the same thread. Max delta ±127.

pyembroidery 1.5.1 has no SEW writer either: the fixture is built the same way as PCS. What
PE-DESIGN puts in the 0x1D78-byte header besides the colour list is unknown.

## HUS and VIP stream compression (`lzh`)

HUS and VIP store their records as three separately compressed streams (commands, x deltas,
y deltas). The coding is LHA's `-lh5-` block format: LZ77 matches of 3–256 bytes and
static Huffman trees sent per block, read most significant bit first.

| Field | Bits | Content |
|---|---|---|
| block size | 16 | tokens in the block (big-endian bit order) |
| length tree | 5 + … | count `n` (0 = one symbol, 5-bit value follows); `n` code lengths, each 3 bits, `7` continued in unary; after the third, 2 bits skip up to 3 zero lengths |
| character tree | 9 + … | count (0 = one symbol, 9-bit value); lengths coded with the length tree: `0` one zero, `1` 3 + 4-bit zeros, `2` 20 + 9-bit zeros, `n ≥ 3` length `n − 2` |
| distance tree | 5 + … | as the length tree, without the skip |
| tokens | … | character code; for 256–509 a match of length `c − 253`, then a distance code `p` and `p − 1` extra bits: distance `(1 << (p−1)) + extra + 1` (1 when `p = 0`) |

Codes are canonical (shorter first, ties by symbol), as pyembroidery's `Huffman.build_table`
builds them. OpenStitch's writer does greedy hash-chain LZ77 with an 8 KiB window (the `-lh5-`
dictionary) and limits code lengths to 16 bits (ITU T.81 Annex K.3 adjustment). The reader
caps output at the header's record count (≤ 4,000,000), rejects over-subscribed codes and
code lengths over 16, and accepts incomplete codes until an unassigned code is hit, so it also
reads the literal-only streams pyembroidery's `EmbCompress.compress` and libembroidery's
`hus_compress` emit (`size(16le) 02 A0 01 FE` + raw bytes).

Sources: pyembroidery 1.5.1 `EmbCompress.py` (decoder, MIT); the public LHA `-lh5-` format
notes and Haruhiko Okumura's public-domain `ar002`, which this layout matches. Nothing was
copied; the encoder is OpenStitch's own.

**Unverified:** whether Husqvarna Viking / Pfaff machines and PE-DESIGN accept an 8 KiB window
and multi-block streams. If a machine rejects OpenStitch files, the fallback is literal-only
streams.

## HUS (Husqvarna Viking)

| Offset | Content |
|---|---|
| 0x00 | u32 magic `0x00C8AF5B` |
| 0x04 | u32 record count (END included) |
| 0x08 | u32 colour count |
| 0x0C | i16 +x, +y, −x, −y extents (y up): max x, −min y, min x, −max y in OpenStitch's y-down units |
| 0x14 | u32 offsets of the command, x and y streams (y runs to end of file) |
| 0x20 | 8 bytes string (zero), u16 (zero) |
| 0x2A | u16 HUS chart index per colour (29 entries, `HUS_CHART`) |

Records `(command, dx, dy)`, signed bytes, y up: `80` stitch, `81` jump, `84` colour change,
`88` trim (a non-zero delta adds a move first), `90` end. Moves are split at 127; a colour
change further away is reached by jumps. Stops are written as colour changes to the same
thread. The meaning PE-DESIGN gives the string and the u16 is unknown, and the extents' sign
convention is unverified (libembroidery and pyembroidery disagree on names, not values).

Evidence: `oracle.hus` is assembled by `tools/genfixtures.py` from the layout pyembroidery's
`HusReader` expects (literal-only streams); its read is the oracle. `openstitch.hus` is the
same design written by OpenStitch with real compression; pyembroidery decodes it to exactly
the same stitches and threads (`openstitch_hus_read.json`).

## VIP (Pfaff)

The HUS header with magic `0x0190FC5D`, then:

| Offset | Content |
|---|---|
| 0x2A | u32 colour-table length (4 × colours) |
| 0x2E | colour table, 4 bytes per colour (R, G, B, `01`), obfuscated: `out[i] = in[i] ^ KEY[i] ^ out[i−1]` with libembroidery's 400-byte key (so ≤ 100 colours) |
| … | `colours + 1` × u32 `1`, u32 `0`, u16 `0` (libembroidery's layout; meaning unknown) |
| 0x38 + 8 × colours | command, x and y streams as HUS |

Records as HUS. libembroidery's reader maps `81` to a trim; OpenStitch keeps the HUS meaning
(jump) and writes trims as `88`. Evidence: libembroidery's `sew` CLI (built locally from
commit `1138e04`, not shipped) reads `openstitch.vip` with the right record count and the exact
RGB colours; its decompressor is a stub (`character = 0 /* fix this */`), so it cannot check
the stitches. Self round trip in `vip_round_trips_rgb_colours_and_stitches`.

## SHV (Husqvarna Viking, read only)

Big-endian. Layout as pyembroidery 1.5.1's `ShvReader` reads it:

| Offset | Content |
|---|---|
| 0x00 | 0x56 bytes of header text |
| 0x56 | u8 name length `n`, then `n` bytes of UTF-8 name |
| … | u8 preview width `w`, u8 height `h`, then `4 + ⌈h/2⌉ × w` bytes (preview, skipped) |
| … | u8 colour count, 18 unknown bytes |
| … | per colour 14 bytes: u32 BE number of 2-byte record words in the block, u8 SHV chart index (43 entries, `SHV_CHART`), 9 unknown bytes |
| end − 2 | records start 2 bytes before the end of the colour table |

Records, y down: `dx dy` signed bytes (a jump while in jump mode); `80 01 dx(i16 BE) dy(i16 BE)`
moves and enters jump mode (3 words); `80 02` leaves jump mode; `80 03` is skipped (1 word).
A colour block ends after its word count: the colour change is implicit. No trim record.

Evidence: `oracle.shv` is assembled by `tools/genfixtures.py` from that layout (trims as
`80 03`); pyembroidery's read is the oracle (`shv_reads_oracle_file_like_reference`). Neither
pyembroidery nor libembroidery (`writeShv` is a stub) writes SHV, and nothing public explains
the unknown header bytes, so OpenStitch does not export SHV; `design.export` refuses it.

## PHC (Brother, read only)

| Offset | Content |
|---|---|
| 0x00 | `#PHC` and an ASCII version (`#PHC0001` in the fixture) |
| 0x2B | u8 size of the block in front of the stitch header |
| 0x30 | u16 offset of that block |
| 0x4A / 0x4C | icon height / icon byte stride |
| 0x4D | u16 colour count, one PEC chart index per colour, then 1-bit icons |
| offset + size | u16 `n` + `n` bytes, u32 `m` + `m` bytes (bounds), 10 bytes, u8 colour count `c`, `c + 0x1D` bytes, then a PEC stitch stream |

Consecutive blocks with the same thread are read as stops (pyembroidery's
`interpolate_duplicate_color_as_stop`). Evidence: `oracle.phc` is assembled by
`tools/genfixtures.py` around the colour list and stitch stream of `oracle.pec`; pyembroidery's
`PhcReader` read is the oracle. libembroidery's `readPhc` agrees on the layout. PE-DESIGN
cannot convert to PHC and no public writer exists, so OpenStitch does not write it.

## PHX and PEN: Blocked

- **PHX:** only its `#PHX0` signature is mentioned publicly (file-database sites). Neither
  pyembroidery (1.5.1 or master), libembroidery nor any other open project reads it. Blocked:
  the only route is studying PE-DESIGN output, which needs the legal opinion and reference
  files.
- **PEN:** Brother's PED-BASIC documentation describes PEN files as encrypted designs bought
  from iBroidery.com. No public description exists and decrypting them would circumvent a
  technical protection measure (EU 2001/29 Art. 6, US 17 USC 1201). Out of scope.

## CSD (Singer, read only)

An 8-byte identifier, then obfuscated bytes. Masks: the C `rand` LCG
(`seed × 0x41C64E6D + 0x3039`, byte = bits 16–23) fills 479 "subtract" bytes, then 501 "XOR"
bytes. Byte at offset `o` decodes as `(b ^ xor[k % 501]) − sub[k % 479]`:

- type 0 (identifier byte 0 is `7C` or byte 2 is `C3`): seed 12, `k = o − 1`;
- type 1: seed = identifier byte 0, `k`'s low byte remapped through a 256-entry table
  (libembroidery's `csd_decryptArray`, copied), with the high part lowered by 0x100 for low
  byte 0 at `k mod 0x300 = 0x200` and low bytes 0–1 at 0x100.

Decoded: 16 × RGB, 2 unknown bytes, 14 colour-order bytes (palette index per colour block),
then 3-byte records `flags dy dx` (0.1 mm): bit 0x20 / 0x40 negate x / y; low five bits 0 =
stitch, any of 0x0C = colour stop, others = trim with a move; `F8`, `87`, `91` end.

Source: libembroidery `src/formats/singer.c` (zlib; the same code was published under MIT in
Embroidermodder in 2013). pyembroidery has no CSD module. Evidence is spec-derived only:
synthetic files built by the inverse transform in `csd::tests` decode as intended for both
mask types. **Unverified** (no reference files): y up, the colour-order mapping (libembroidery
indexes it off by one), a leading stop selecting the first colour, and the trim move. Wilcom
documentation mentions "Type I" and "Type II" CSD (Type II with RGB and an icon); which one
this is, is unknown. CSD export: no public writer, the unknown bytes are undocumented, so
OpenStitch does not write CSD (Documented, needs reference files).

## FCM (ScanNCut): Blocked

Brother publishes no FCM specification. Public descriptions (researched 2026-10-19):

- **fcmlib** (Rust, github.com/justjanne/fcmlib, commit `c0f6e5e`, MPL-2.0): a full reader and
  writer, tested against ~1,100 `.fcm` files that are Brother's own downloadable project
  designs.
- **open-fcm** (TypeScript, github.com/markuryy/open-fcm, commit `e80a974`, labelled MIT, but
  "inspired by" fcmlib): its `docs/format.md` describes the structure in prose. Header
  (`#FCM`/`#VCM`, version `0100`, names, an 88×88 1-bit BMP thumbnail, generator), cut data (cut
  or print-and-cut, mat, page size in 0.01 mm, registration marks), and a piece table. Pieces
  carry an affine transform, restriction flags and paths. Paths carry tool flags (cut, draw,
  emboss, …) and a start point followed by line or cubic Bézier outlines. Little-endian,
  coordinates are i32 in 0.01 mm. Not encrypted.

Why it is not implemented:

1. The byte-level layout comes only from the MPL-2.0 library and from an MIT port of unclear
   provenance. Neither is a specification, and our rule is public specifications or
   compatibly licensed implementations.
2. The only test files are Brother's copyrighted designs. We cannot ship them, and studying
   Brother output is the open legal question in LEGAL_AND_LICENSING.md.
3. open-fcm reports that an invalid thumbnail can make the machine reboot. Exporting without
   a validated fixture would put the owner's machine at risk.

Unblocks it: the owner (a) accepts implementing from the fcmlib/open-fcm descriptions (clean
room, no code copied; MPL-2.0 permits reading), and (b) provides cut files the owner made in
Canvas Workspace (with the legal opinion covering their use), or a sacrificial ScanNCut test.
With that, a reader (FCM → Bézier shapes, which maps directly onto `Contour`) is about a day's
work; a writer should wait for a machine test.

## Format limits and split export

Each writer refuses (`ValueOutOfRange`) a design it cannot represent:

| Format | Limit | Where it comes from |
|---|---|---|
| PES, PEC | 255 colour blocks; stitch block ≤ 16 MB | u8 colour count − 1; u24 block length |
| DST | 999 colour changes; 9,999,999 records | 3- and 7-digit `CO:` / `ST:` header fields |
| PCS | 65,535 records | u16 record count |
| SEW | 3,771 colours | colour table must fit the 0x1D78-byte header |
| VIP | 100 colours | the 400-byte colour key |

`design.export` with `split: true` (always on in the editor) writes `NAME-1.ext`,
`NAME-2.ext`, … instead (`openstitch_formats::split`). Files hold whole colour blocks where
possible. A block too large for one file is cut between stitches. Later files open with a jump
to their first stitch, and coordinates stay absolute, so the parts line up in the same hoop.
Without `split`, the error says so and nothing is written. Machine-specific limits (for example
a model's maximum stitch count) are not published and are not applied.

## Vector and image imports

**SVG** (`import.svg`, engine module `imports`): parsed by `usvg` 0.48 (linebender/resvg, MIT
OR Apache-2.0) with default features off (no text, fonts or `.svgz`). External and embedded
`<image>` references are disabled, so an SVG cannot make the engine read files outside the
sandbox. usvg applies CSS, `viewBox`, units (96 px per inch) and transforms and converts basic
shapes to paths. Each visible filled or stroked path becomes one shape object:

- subpaths → contours; lines stay straight, cubic Béziers stay exact, quadratics are raised
  to cubics; a filled path closes every subpath (as SVG filling does);
- solid fill → fill region in that colour; solid stroke → running line in that colour;
  gradient or pattern paint → black; an open path's fill becomes its line colour;
- several paths → one group; the artwork keeps its physical size (or `width`) and is centred
  on the Design Page unless `x`/`y` are given; one Undo.

Limits: 16 MB file, 5,000 paths, 500,000 vertices. Not imported: `<text>`, `<image>`,
clipping and masks (ignored), stroke widths. SVG's `nonzero` fill rule is approximated by
OpenStitch's even-odd rule (overlapping same-direction subpaths read as holes).

**WMF / EMF:** not implemented. The only pure-Rust parsers found are `wmf-core` 0.1.2 and
`emf-core` 0.1.1 (MIT, first published 2026-08, single maintainer, MSRV 1.88 above the
workspace's 1.85). Not yet vetted enough to parse untrusted files; revisit when they mature, or
convert WMF/EMF to SVG with a trusted tool first.

**Raster background** (`background.set`, `background.show`, `background.clear`): PNG, JPEG or
BMP through `openstitch-digitize`'s decoder (32 MB file, 8192 px side, 16 Mpx). The design
stores the absolute path, pixel size, placement (0.1 mm), opacity and visibility
(`Design.background`, omitted from `.osd` when absent); the image itself is not embedded and
is never sewn. The editor draws it behind the stitches.

**Clipboard** (editor File ▸ Background Image ▸ Paste from Clipboard): the image is read with
`arboard` 3.7 (MIT OR Apache-2.0, the clipboard crate eframe already uses), checked against the
same size limits, and kept as `pasted-<hash>.png` in the per-user data folder, because the
project only references files:

| OS | Clipboard images | Pasted images kept in |
|---|---|---|
| macOS | ✅ (NSPasteboard) | `~/Library/Application Support/OpenStitch/backgrounds` |
| Windows | ✅ (CF_DIB) | `%APPDATA%\OpenStitch\backgrounds` |
| Linux | ✅ on X11 and XWayland; native Wayland clipboard not enabled (`arboard`'s `wayland-data-control` feature is off, as in eframe) | `$XDG_DATA_HOME/openstitch/backgrounds` (default `~/.local/share/...`) |

**Scanner (PE-DESIGN's TWAIN import): Blocked per OS.** Every route is a native API. The
workspace forbids `unsafe` code, and no vetted safe Rust binding exists:

| OS | API | Status | Meanwhile |
|---|---|---|---|
| Windows | TWAIN / WIA (COM) | Blocked: needs `unsafe` FFI | Scan to a file with Windows Fax and Scan, then Background Image ▸ Choose |
| macOS | ImageCaptureCore | Blocked: needs Objective-C FFI | Scan with Image Capture or Preview, then Choose, or paste |
| Linux | SANE (`libsane`) | Blocked: needs C FFI | Scan with Simple Scan (`simple-scan`), then Choose |
## OpenStitch stitch patterns (`openstitch.pattern`)

OpenStitch's own, documented format for programmable-fill tiles, motifs and decorative-fill
rows, used by the bundled library (`crates/openstitch-design/patterns/`, generated by
`tools/gen_patterns.py`), the Programmable Stitch Creator and `pattern.import` /
`pattern.export`. One UTF-8 JSON document per pattern, at most 1 MiB:

```json
{"format": "openstitch.pattern", "version": 1, "kind": "fill", "id": "tulip",
 "name": "Tulip", "rowShift": 0.5, "paths": [[[0.5, 0.9], [0.5, 0.45]]]}
```

| Field | Rule |
|---|---|
| `kind` | `motif`, `fill` or `decorative` |
| `id` | 1–64 characters `a-z`, `0-9`, `-`; user patterns may not reuse a bundled id |
| `name` | 1–128 bytes |
| `rowShift` | decorative only, 0–1 tile widths per row (optional) |
| `paths` | 1–64 polylines, at least 2 points each, at most 4000 points in all |
| `regions` | fill only, optional: at most 256 `{"effect": "emboss" \| "engrave", "at": [x, y]}`, each the closed area of the tile around `at` |

Coordinates are in the pattern's unit box: motif x −0.5…1.5 (along the line), y −1…1
(across, scaled by the motif height), one path from (0, 0) to (1, 0); fill x, y 0…1 (the
tile; programmable fill puts needle points where its rows cross the lines); decorative x
−0.5…1.5, y 0…1, every path from (0, y) to (1, y) so tiles chain into rows. Unknown fields
are refused. A region's area is found on a 64 × 64 grid over the tile that wraps at its sides
(the tile repeats); a point on a line or in an area the lines do not close names nothing, and a
later region of the same area replaces an earlier one. Programmable fill sews engraved areas
with 1 mm stitches and drops needle points inside embossed areas (stitches up to 7 mm), like
the stamp effects. Designs carry the user patterns they define in `design.patterns` (same document
shape), so a project sews the same everywhere.

Brother's `.pas`, `.pmf` and `.plf` pattern files are undocumented and cannot be read or
written (Blocked: no public description, and reverse engineering is ruled out).

## OpenStitch sewing stitches (`openstitch.sewing`)

The Stitch Design Factory's own documented format for sewing-machine stitch patterns
(`openstitch_design::sewing`). One UTF-8 JSON document, at most 1 MiB; mm, x along the feed
(0–200), y across (±width/2):

```json
{"format": "openstitch.sewing", "version": 1, "kind": "satin", "id": "tapered-leaf",
 "name": "Tapered leaf", "width": 7, "density": 5,
 "profile": [[0, -0.3, 0.3], [6, -3.5, 3.5], [12, -0.3, 0.3]]}
```

| Kind | Fields |
|---|---|
| `decorative` | `points`: 2–2000 needle points; no move longer than 7 mm along the feed |
| `satin` | `profile`: 2–200 stations `[x, left, right]`, x increasing, left ≤ right; `density` 1–10 /mm |
| `decorativeSatin` | `outline`: 3–500 points enclosing an area; `density` 1–10 /mm |
| `cross` | `cellSize` 0.5–5 mm, `rows` (rows × cell ≤ width), `cells`: 1–500 `[column, row]`; `triple` |

Common: `id` (1–64 `a-z 0-9 -`), `name` (1–128 bytes), `width` 1–9 mm, optional
`swapped: true` (Swap: start and end exchanged, the needle points sewn in reverse order;
written only when on). Exports add a
`needles` array (the generated needle points, rounded to 0.001 mm) that readers ignore and
regenerate. Unknown fields are refused. Projects keep their patterns in `design.sewing`.

**PMV** (Brother sewing-machine stitch file) is written exactly as the independent
pyembroidery project's `PmvWriter` writes it (byte-for-byte fixtures in
`crates/openstitch-design/tests/fixtures/pmv/`, from `tools/gen_pmv_fixtures.py`): header
`#PMV0001`, 6-bit relative x / 5-bit absolute y pairs in 0.25 mm, widths over 7 mm scaled to
fit, at most 100 needle points, then pyembroidery's fixed length table (which it notes is not
understood) and a width table. Experimental: no machine has been tested. **PMP** (large
patterns) and **PSF** (PE-DESIGN's editable sewing-stitch file) are Blocked: the guide names
them, nothing describes them.
## OpenStitch font (`.osfont`)

OpenStitch's own embroidery font format, written by `fonts.save` and read by `fonts.open`
(user-mapped text; Font Creator). It is JSON, UTF-8:

```json
{
  "format": "openstitch.font",
  "version": 1,
  "font": {
    "id": "ornaments",
    "family": "Ornaments",
    "style": "",
    "data": {
      "embroidery": {
        "capHeight": 100.0,
        "ascender": 120.0,
        "descender": 30.0,
        "lineGap": 0.0,
        "glyphs": [
          { "char": "A", "advance": 70.0, "contours": [ … ] },
          { "char": "h", "advance": 120.0,
            "stitches": { "stitches": [ {"x": 0.0, "y": -3.0, "command": "stitch"}, … ],
                          "threads": [ … ] } }
        ]
      }
    }
  }
}
```

- Lengths are 0.1 mm at the font's own size, +y down like the Design Page. A glyph's origin
  is the baseline at its left set line, so capitals reach up to `y = -capHeight`. Text set at
  size *s* mm scales a glyph by `s × 10 / capHeight`.
- `capHeight` is the standard character height (baseline to capital line), 1–500 mm.
  `ascender` + `descender` + `lineGap` is the distance from one baseline to the next. All
  three are 0–2000 mm.
- `glyphs` is sorted by character, with one entry per character. A glyph has `contours`
  (`.osd` contour objects; sewn with the text object's region and line sewing), `stitches`
  (a stitch block like a `.osd` stitch pattern object; sewn as registered, colours included),
  or both. `advance` is the set-line-to-set-line width.
- Readers refuse files over 64 MiB, other `format` values, newer versions and TrueType data.
  They also refuse fonts that break the limits: 4096 glyphs, 1 000 000 outline points,
  100 000 needle points per glyph and 1 000 000 per font, coordinates within ±5 m, 1–256
  threads per stitch glyph.
- `.osd` files carry the same font objects under `design.fonts`, next to TrueType fonts, which
  are stored as `{"trueType": {"face": 0, "bytes": "<base64>"}}`.

## OpenStitch template (`.ostemplate`)

Written by `template.save` and read by `template.list` / `template.open`. It is JSON:
`{"format": "openstitch.template", "version": 1, "name", "category", "description"?, "design"}`.
`design` is the same object as in an `.osd` file and is checked the same way. The slots are
its text objects with the Name Drop attribute (`text.nameDrop`, whose optional `label` names
the slot). A template without such a text is refused. Name and category are 1–64 characters.
The built-in templates are in `crates/openstitch-engine/templates/`, generated by
`examples/templates/*.json` (`stitchtool exec`).

## Known limitations

- PES editable shapes and text (PE-DESIGN object classes) are not read or written: an
  OpenStitch PES opens in PE-DESIGN as a stitch group (v1/v6 with objects) or stitch-only
  design (truncated). Version 6 keeps RGB threads, names and metadata.
- SHV is read only (see above).
