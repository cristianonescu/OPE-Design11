# pes-objects: PES object-section map (working notes, pes-io session)

Method: black-box diffs of PE-DESIGN 11.50 output files the owner made (project policy, PR #60).
Never PE-DESIGN's program files. Tool: `pesdiff.py` in the pes-io scratchpad (difflib-aligned
pairwise diff of the bytes before the PEC section, with u16/i16/f32 readings).
Offsets below are relative to the start of the object's class-name length prefix (u16 n, then
n ASCII chars), because the header before it (thread table) varies in length.

## Header (#PES0110), confirmed so far
- `#PES0110`, u32 PEC offset, then `01 00`, two ASCII digits (`10` in 0110, `01` in PE's Ver.6.0
  save; OpenStitch writes `02`), five u8-length metadata strings (Windows-1252, CR LF; #39).
- The thread table's strings are MFC Unicode CStrings (`FF FEFF`, u8 length in UTF-16 units,
  UTF-16LE); OpenStitch reads them since #53.

## `CEmbLine` (a closed or open shape with line and region sewing)
One object of this class per shape in every native round-1 file (rectangle, U polygon, satin
band). The class name is 8 chars, so the record starts `08 00 'CEmbLine'`.
- +10: four i16 bounds `x0 y0 x1 y1`, then the same four again. Fill-frequency rectangle
  (10,10)–(50,50) mm gives 600,600,1000,1000: 0.1 mm with a +500 offset (page 100 x 100 mm;
  to confirm with r01/r03 whether it is "+ page centre" or fixed).
- +26: affine transform, 6 × f32 (identity in every file so far).
- +144: u16 region sew type: 1 = Fill, 0 = Satin (region-travel-u-fill vs -satin; that one byte
  is the whole object-section difference, so all sew types' attributes are stored side by side).
- +148: u16 Gradation: 0 off, 1 on (region-travel-u-fill vs -gradation; only the flag changes
  when the default Gradation Pattern is used).
- +184: u16 Fill Frequency % (fill-frequency-0/25/50: 0, 25, 50; the only differing bytes).
- +196: u16 Running Stitch Path: 0 Inside, 1 Along outline, 2 On outline
  (region-travel-u-fill / -along-outline / -on-outline).
- Near the end of the object: the outline's vertices as f32 pairs in the same page coordinates
  (800.0, 1000.0, 600.0 … for the rectangle), with per-vertex fields to map (r01 vs r02/r03).

## From the minimal designs (<reference-notes>\pes-objects, r01 baseline)
r01 = rectangle (10,10)–(20,20) mm, PE defaults; `CEmbLine` at 245. Each line = r01 vs that file.
- r03 moved to (30,30)–(40,40): +10 bounds 600,600,700,700 → 800,800,900,900, so
  x_file = 10·x_mm + 500 (page 100 mm; centre?). +18 a second box 590..710 → 790..910. Vertices
  repeat as u16 later in the object (+214…) and as f32 near the end.
- r02 width 20: +14 x1 700 → 800, +22 810; centre 650 → 700 at several later fields (rotation /
  direction centre?).
- r07 line Not Sewn: +18 box shrinks from ±1 mm to the bounds (so +18 = sewn extent incl. line
  width); +52 u16 1 → 0 (line sewn), +54 u16 0 → 9 (line sew type? 9 = none?). Needs line-type
  variants.
- r04 Satin: +144 1 → 0 (confirms round 1).
- r09 density: +146 u16 45 → 60 (0.1 line/mm; fill density).
- r08 direction 90: +178 u16 45 → 90 (degrees); plus two fields at ~+4456/+4472 (f32 or double
  halves) change: a direction vector or cos/sin, to decode.
- r06 under sewing off: +190 u16 1 → 0.
- r05 colour: header thread table entry changes (catalog, RGB, the "33"-like code) and the file
  shrinks by 2 bytes (shorter string); per-object colour index to locate.

## Structure (r01, r16, r12)
- Editable objects exist only in `#PES0110` here: PE's Ver.6.0 save of r01 is a `CEmbOne`/`CSewSeg`
  stitch group (PE warns "saved as stitch data"). OpenStitch must write 0110 for editable shapes.
- Object list = u16 object count, then MFC `CArchive` objects: a new class is `FFFF`, u16 schema
  (0), u16 name length, ASCII name, then the object's fields; a later object of an already-named
  class is the tag `0x8000 | class index` (r16's second rectangle starts `01 80` at 4915). The same
  prefix is on `CEmbOne`/`CSewSeg` (`FFFF 0000 0700 'CSewSeg'`). MFC's scheme is public.
- Baseline B (pe-gui): line Zigzag, 800 RED; region Fill, 005 SILVER; all PE defaults. So +54 = 0
  is Zigzag (not Running); 9 with "not sewn".
- r10 circle and r11 open line are also `CEmbLine` (shorter vertex lists); text r12–r14 is
  `CEmbNText` + `CTTFLetter` per letter (632 bytes for one `A`).

## Text (r12 `A`, r13 size 20, r14 `AB`; Lato Bold, size 10, region Satin 800 RED, line Not Sewn)
- `CEmbNText` (9 chars) at 245: +10 bounds and +18 box as in `CEmbLine` (r12: 599,541,700,700;
  `AB` widens x1 700 → 793), +26 transform 6 × f32, then +51 u16 letter count (1 → 2 for `AB`).
- Then one `CTTFLetter` (10 chars) per letter: u16 character code (`41 00`), …, f32 scale
  (1.0 at size 10, 2.0 at size 20), the same sewing-attribute block layout as `CEmbLine`
  (…`14 00 2d 00`…), the font name as an MFC Unicode string (`FF FEFF 04 'Lato'`), f32 100.0
  fields. `AB` inserts a second letter record of 166 bytes (class tag, not the name).
- No glyph outlines: the whole one-letter object section is 632 bytes. PE re-renders from the
  named font, so reading text editably needs that font (Lato is OFL).

## Status 2026-10-27 late
- PR #68: FILE_FORMATS chapter "PES 0110 object list" + reader `formats/src/pes11.rs` (CEmbLine
  shapes) + fixtures tests/fixtures/pe11 (r01–r16). PR #62: missing PEC = error.
- CEmbLine record = tag, 4432-byte body, 84-byte geometry header (3 marks), u32 n, n × 28-byte
  vertices (6 × f32 + u32 kind 0 / 0x11); the record ends after the last vertex; 52+ trailer
  bytes after the list.
- Line types (+44): 0 Zigzag, 1 Running, 3 E and V (same code), 4 Triple, 9 not sewn.
  Region types (+134): 0 Satin, 1 Fill, 2 Programmable, 11 hole-pair inner copy.
- Thread table (r01 at 131): u16 count, entries = MFC catalogue ("005"), RGB + pad, u32 kind
  (10), MFC chart code ("16" silver, "5" red; = PEC index?), MFC brand ("EMBROIDERY"), MFC chart
  (""). r01 lists region silver first, line red second; r05 changes only entry 0. The body's
  colour indexes are not located yet: round-2 request r17/r18 moves them.
- `shape-start-points` (compat ref): 3 CEmbLine then `CEmbSpecialShape`. Manual Punch block
  (satin-zigzag-column) is a CEmbLine with another body layout.
- Round 2 requested from pe-gui (r17–r26): colour indexes, line Running pitch, zigzag width,
  line under sewing, text Fill / colour / letter / position.

## No stitches in the object section
Fill vs Satin differs by one byte in the object section while the PEC stitches differ entirely:
the editable object holds attributes and geometry only, and PE-DESIGN regenerates stitches.

## Waiting for (Reference requests, r01–r16)
Geometry encoding (width, position, circle, open line), colour, under sewing, line Not Sewn,
direction, density, text class and its fields, two objects.
