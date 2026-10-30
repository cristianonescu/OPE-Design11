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
  so non-ASCII names never misalign the 512-byte header. `design.export` labels the section
  with the output file's name (metadata `pec.label`), as PE-DESIGN 11 does whatever the Design
  Property name says; a pattern without that key is labelled with its name.
- JEF date: a supplied stamp must be 14 ASCII digits; the default is the current UTC time.

| Format | Read | Write | Evidence level | Notes |
|---|---|---|---|---|
| PES | ✅ all versions, via the PEC section; v4+ metadata, v5+ RGB thread table, `CEmbOne`/`CSewSeg` objects | ✅ v1 and v6, truncated or with one `CEmbOne`/`CSewSeg` pair | Oracle-verified (pyembroidery), byte-identical for 1t, 1, 6, 6t | Shape, text and other PE-DESIGN object classes: no public description (Blocked). |
| PEC | ✅ | ✅ | Oracle-verified | Byte-identical round trip of the oracle file. |
| DST | ✅ | ✅ | Oracle-verified read; PE-DESIGN 11.50 reference write (records and header identical) | See "PE-DESIGN's record layout" below. |
| EXP | ✅ | ✅ | Oracle-verified read; PE-DESIGN 11.50 reference write (byte-identical) | |
| JEF | ✅ | ✅ | Oracle-verified read; PE-DESIGN 11.50 reference write (records and header identical except the date and one hoop block) | |
| XXX | ✅ | ✅ | Oracle-verified read; PE-DESIGN 11.50 reference write (records and counts identical) | Two header tables and the unused colour slots PE-DESIGN fills are left zero. |
| PCS | ✅ | ✅ | Oracle-verified read; PE-DESIGN 11.50 reference write (byte-identical) | Fixed 16-colour table, 1/6 mm grid with a fraction byte. |
| SEW | ✅ | ✅ | Oracle-verified read; PE-DESIGN 11.50 reference write (records identical, from PE-DESIGN's start point) | PE-DESIGN's 340 header bytes unknown. |
| HUS | ✅ | ✅ | Oracle-verified read; PE-DESIGN 11.50 reference write (streams and header identical) | Own LZ77 + Huffman compressor (`lzh`); 29-colour HUS chart. |
| VIP | ✅ | ✅ | PE-DESIGN 11.50 reference write (streams and header identical); colour table confirmed by libembroidery's reader | No independent stitch decoder exists (pyembroidery has no VIP module; libembroidery's decompressor is a stub). |
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

PE-DESIGN 11.50 writes no zero-length stitch to the PEC section: its status bar and its DST
export count needle points that the PEC section leaves out (a zero-length stitch at each jump
landing and where a stitch repeats its predecessor; e.g. 475 in the DST of `sample-basic`,
471 in its PEC, the DST's other 4 all zero-length; 99 of 114 references show 1–9 more in the
status bar than in their PEC). So reading a PE-DESIGN PES gives fewer needle points than PE
reports, by design of the file, not of the reader (#135).

Stitch stream: each axis value is either one byte (7-bit two's complement, used when
−64 < d < 63) or two bytes `1 f f f v v v v  v v v v v v v v` (bit 7 set; bit 5 = trim,
bit 4 = jump; 12-bit two's complement). Colour change: `FE B0` then `02`/`01` alternating. A move over ±2047 units (204.7 mm) is
split before encoding as PE-DESIGN 11.50 splits an over-long DST move (`long-jump-300.dst`:
maximum along the longer axis, proportional share on the other, remainder last; never halved),
so no written delta exceeds the 12-bit form (`pec_splits_moves_over_2047_units_like_pe_design_dst`,
#126). PE-DESIGN's own PEC bytes for such a move are not yet understood (owner's parked PES:
`9e 39 96 95` then 39 × `80 46 00`, which a plain 12-bit decode does not sum to the DST's
vector); the issue's halving proposal matches neither format.

Colour mapping: PEC stores indexes into Brother's fixed 64-entry chart, not RGB. Writer
assigns each distinct design colour the nearest still-unused chart entry among 1–61
(equal-weight squared-Euclidean RGB distance, later chart entry on ties;
`openstitch_core::nearest_index` over `pec_basic_chart()`, #119), so distinct colours stay
distinct on the machine. Entries 62–64 are read but never chosen: PE-DESIGN 11.50 sent
rgb(255, 128, 0) to 11 Deep Gold rather than 62 Orange (`thread-match-swatches.pes`). The same
matcher picks the chart entry for JEF, SEW, HUS and PCS. `approximate_index` is the "approximate" palette matcher
(city-block winner, squared-Euclidean fallback) for the built-in sew palette.

Brother catalogue codes: PE-DESIGN 11.50's BROTHER EMBROIDERY chart has exactly 61 rows, one
per PEC entry 1–61, with a 3-digit code and brand `EMBROIDERY` (none for 62–64).
`openstitch_core::BROTHER_EMBROIDERY` lists `(PEC index, code, name)` for all 61, with
`brother_embroidery_code(pec_index)` and `pec_index_of_brother_code(code)` (800 RED = PEC 5,
405 BLUE = 2, 900 BLACK = 20). Sources: the chart's rows read from PE's Sewing Attributes ▸ Color
tab, matched to the PEC chart by name and checked against the 30 pairs PE writes in the owner's
own `#PES0110` thread tables (all agree); 307 LINEN is PEC 42 by elimination (pyembroidery names
42 and 57 both "Flesh Pink", and PE's files put 124 FLESH PINK at 57). The dialog's own first
column (1 BLACK, 2 WHITE, 3 RED, …) is not the PEC index.

## PES

`#PESvvvv` signature, u32 offset of the PEC section, version-dependent object section, PEC
section. Default output (`pes::WriteOptions::default`, `FileType::Pes.write`, and so
`design.export` without `pesVersion`, Name Drop, USB transfer and Design Database convert) is
`#PES0060` with one `CEmbOne`/`CSewSeg` stitch group and the RGB thread table, byte-identical to
pyembroidery's `6`. `pesVersion` = `1t`, `1`, `6t` selects the others, and `11` writes PE-DESIGN
11's `#PES0110` with the design's shapes as editable objects ("PES 0110 object list", "Writer"
below); `1t` (`#PES0001`, offset 0x16, 10 zero bytes, no objects) stays as the smallest,
machine-only file. **Unverified:**
whether every Brother machine and PE-DESIGN 11 open these files; needs a black-box open test on
the owner's machine (docs/COMPATIBILITY_AUDIT.md item 2).

### Save as a PES version: what each keeps

| Written as | Stitches + PEC chart colours | Exact RGB, thread names, brands | Name, author, keywords… (metadata) | Stitch-group object (opens in PE-DESIGN as a grouped stitch pattern) | Editable shapes / text |
|---|---|---|---|---|---|
| `1t` (machine only, smallest) | ✅ | ❌ (nearest of the 64 Brother colours) | ❌ (8-character PEC label only) | ❌ | ❌ |
| `1` | ✅ | ❌ | ❌ | ✅ (colours as PEC indexes) | ❌ |
| `6t` | ✅ | ✅ | ✅ | ❌ | ❌ |
| `6` (default) | ✅ | ✅ | ✅ | ✅ | ❌ |
| `11` (PE-DESIGN 11, `#PES0110`) | ✅ | ✅ (thread table in sewing order; each thread named with its PEC chart index, as PE-DESIGN names its own colours) | ✅ | ✅ (a stitch pattern is a `CEmbOne`) | ✅ shapes: fill and satin regions; running, zigzag, triple, E and V lines; groups; Lato Bold text, with a Text Transform or fitted to an outline. ❌ the other region and line types, other fonts: the export fails and names the object. PE-DESIGN 11.50 opens shapes and text editable (specs `pes-0110-editable-shapes`, `-text`, `-text-transform`); stitch patterns: spec `pes-0110-stitches`, reference pending |
| 4, 5 (5.5, 5.6), 7–10 | **Blocked** | | | | |

Editor: File ▸ Export ▸ PES offers the five. Engine: `design.export` `pesVersion`.
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
| v5–v10 | fixed fields (36 bytes in v6: hoop/page/grid settings; OpenStitch writes the selected hoop's width and height in mm as the third and fourth u16 (`WriteOptions::hoop_mm`, from `design.export`; 100 × 100 without one) and pyembroidery's values for the rest; PE-DESIGN 11.50's `#PES0110` files store the hoop the same way, u16 mm width and height followed by the hoop name, e.g. 100, 100, "nv880e-S"), u8-length image file name, six f32 (identity transform), u16 counts of programmable fills, motifs, feather patterns, u16 thread count, threads |
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
PE-DESIGN files, is in "PES 0110 object list" below. The reader reads `CEmbLine` shapes,
`CEmbNText` texts, the groups and the page size and stops at the first other class (`ObjectSection::unparsed`);
`pes11::write` writes them back (`pesVersion` 11). Versions 1 and 6 still write the shapes as
stitches (a `CEmbOne` stitch group). Unverified: whether PE-DESIGN accepts several `CEmbOne`
objects in one file, so those writers emit one for the whole design.

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
- **Header** (after the five metadata strings; `o-page-200x200`, `o-hoop-*`, `o-machine-single-100x100`,
  r27 against r01): u16 0; u16 **page mode**: 1 Custom Size page, 0 Hoop Size page (r27,
  o-hoop-*; PE-DESIGN's default page for a new design); u16 hoop: 255, 15 (100 × 100 mm
  `nv880e-S` in r27), 1 single-needle; the hoop
  width and height in mm (150, 150) and in 0.1 inch (59, 59; 100 mm gives 39); a u8-length
  hoop/machine string (`nv880e-M` for the 150 × 150 multi-needle hoop, `nv880e-S` for 100 × 100);
  u16 0; two optional u16 values, each a u16 presence flag then the value when the flag is 1
  (1800 once in the 200 × 200 page file, 1300 twice in the single-needle 100 × 100 file; meaning
  unknown); u32 0; the **Design Page width and height in mm**; then u16 7 (0 single-needle), 100, 7,
  19, 1, 0, 1, 10, 1, 0, 0, 0, 1, 10, a u8-length image file name (empty), u32 0, the identity
  transform (6 × f32), 16 bytes whose fifth u16 is 4, and the thread count. The two-character code
  after `01 00` is `10` in the multi-needle files and `01` in the single-needle ones.
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
- `CEmbSpecialShape` (a Shapes-gallery shape: o-gallery-heart, the five gallery shapes of
  shape-start-points, the appliqué's heart) is a `CEmbLine` record byte for byte under its own
  class name: the same 4432-byte body, geometry header and vertices, and no gallery id. The
  reader reads it as a shape (`Shape::gallery`) and the writer writes it back under its class.
- `CEmbOne` (a stitch group: PE-DESIGN's re-saves of OpenStitch's version 6 files,
  pes-default-v6, pes-csewseg-trims, pes-design-properties) is the version 6 record with
  `#PES0110`'s object head: bounds (x0 y0 x1 y1 of the needle points, file units), extent, the
  transform (translation = the bounds' bottom-left corner, where version 6 centres the group on
  (1000, 1000)), u16 1 0 0 width height, 8 zero bytes, the u16 run count and the runs, each a
  `CSewSeg` (u16 flag, 0 stitches or 1 a jump; u16 thread-table index; u16 `FFFF`; u32 point
  count; i16 x y points relative to the bottom-left corner), then the colour log (u16 count,
  (run, thread) pairs). In the sewing-order list a stitch group lists one part per colour block.
  The reader reads it as a `StitchGroup` and the writer writes it back.
- **Header fields and thread names:** the fields between the metadata strings and the thread
  table (page mode, hoop, machine string, page size …) are kept as read and written back
  verbatim, and so are the two characters after the signature's `01 00` (`10` in the
  multi-needle files, `01` in the single-needle ones and PE-DESIGN's re-saves of version 6
  files). The thread table's fifth string is the thread's name: PE-DESIGN writes its own
  colours' PEC chart index there, and keeps a name it read from another file (pes-default-v6:
  `Red`, `Black`).
- Classes seen: `CEmbLine` (Shape tools: rectangle, ellipse, open and closed lines), `CEmbNText`
  with one `CTTFLetter` per letter (text), `CEmbSpecialShape` (Shapes gallery presets),
  `CEmbOne` with `CSewSeg` runs (stitch groups).

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
| 8 (cont.) | the extent of a running or unsewn line equals the bounds; a zigzag adds half its width; the circle's is one unit wider still (589..711) and an open zigzag line's grows only across the line (r11: y ± 10, x unchanged) | r19, r07, r21, r10, r11 |
| 132 | region sewn 1, Not Sewn 0 (every Round 3 file whose region is Not Sewn has 0; r11's open line keeps 1 with its unused Fill) | o-arc, o-rounded-rect, o-group-two-rects, r11 |
| 134 | region sew type: 0 Satin, 1 Fill, 2 Programmable fill, 11 Not Sewn (open lines, outlines with the region off, the inner copy of a hole-sewing pair); the Round 3 region types have their own codes (not mapped: `RegionSewType::Other`) | r04, programmable-fill-45, o-group-two-rects, o-arc, hole-sewing-pentagon-star |
| 136 | region density, 0.1 line/mm | r09 (4.5 → 6.0) |
| 138 | gradation 0 off, 1 on | region-travel-u-gradation |
| 162 | a thread-table index for an attribute not identified yet: it points at 800 RED (PE's default colour) in every file, even when nothing sews red | r17, r18, round-1 references |
| 168 | region direction, degrees | r08 (45 → 90) |
| 170 | region colour, a thread-table index | shape-start-points (three shapes: 0, 2, 3 = 800, 007, 027), hole-sewing-pentagon-star |
| 174 | Fill Frequency, % (default 30) | fill-frequency-0/25/50 |
| 180 | region under sewing 1 on, 0 off | r06 |
| 186 | Running Stitch Path: 0 Inside, 1 Along outline, 2 On outline | region-travel-u-* |
| 204, 262, 286, 354, 378 | i32 x, y: the bounds' top-left corner, five times (reference points of attributes not identified) | r02, r03, o-closed-curve |
| 298, 310 | i32 x, y: the bounds' centre (x0 + x1 and y0 + y1 halved, truncated) | r02, r03, o-open-freehand |
| 386 | the pattern block: u16 10000, u16 53, then 53 polylines, each six zero bytes, u16 1, u16 n and n × (i16 x, i16 y) in a 1024-unit cell; byte-identical in every `CEmbLine` (the default pattern of PE-DESIGN's programmable fill, Brother content) | all |
| 4360 | u16 1000, 1000, then fields not mapped (u16 1, 0, 20, 0, 650, 0, 650, 0, 20, 20, 0, 1 …) | all |
| 4372, 4388 | i32 x, y: the bounds' centre again | r02, r03 |
| 4414, 4426 | u16: the index of the outline's last segment (vertices − 1 closed, vertices − 2 open), followed by f32 1.0 | r10 (3), r11 (0), o-open-freehand (16), o-rounded-rect (6) |
| 4432 | geometry: an 84-byte header with three marks, a u32 vertex count n, then n vertices | all |

The geometry header is `u32 1, u32 segment, f64 t`, `u32 1, u32 segment, f64 t`, `f32 x, y` (mark A),
`u32 flag, f64 t, u16 1`, `f32 x, y` (mark B), `u32 segment, f64 t`, `f32 x, y` (mark C), `u16 0`.
For closed shapes the coordinates sit at fixed fractions of the bounds: A a tenth of the width
left of the left edge at 60 % of the height, B at the centre and 25 % of the height, C a tenth
of the width right of the right edge at 60 % (r01, r02, r03, r16, o-rounded-rect, o-region-*);
the curved and open files place A and C on the outline at 60 % instead (o-open-curve) or round
the first vertex (r11). The segment and `t` fields vary between files of the same shape (r01
1.0/1.0, r02 0.5/0.75, r08 1.0/0.5, o-region-cross 0.0/0.5): their meaning is not mapped.

Each vertex is 28 bytes: x, y, the handle of the segment arriving at the vertex, the handle of
the segment leaving it (6 × f32), then a u32 kind: bit `0x10` the arriving segment is a curve,
bit `0x01` the leaving one is (0 a corner between straight segments, `0x11` a smooth curve point;
`0x08` is also set on the freehand tool's cusps). A straight segment's handle is `(0, 0)` (the
open line r11 repeats the vertex instead, and o-rounded-rect keeps a stale handle after a curve).
A rectangle starts in the middle of its bottom edge and runs round the corners; an ellipse is
four smooth vertices whose handles lie 0.5523 × the radius along the tangent. The record ends
after its last vertex.

After the list: 10 zero bytes; the **group list**, a u16 count then per group its MFC tag
(`FFFF 0000 0C00 CGroupObject` the first time), a u16 with the `CGroupObject` class's archive
number (the objects' count + 2: 1 is `CEmbLine`, 2.. the objects, then the class), the u16
member count and the members' object indexes as u16 (o-group-two-rects, machine-rbr …
machine-trim-bound); 12 zero bytes; then the **sewing-order list**: u32 1 and, for every object in
sewing order, (object index, part, 1) triples, part 2 the region, 1 the line and 0 the connection
from the previous object (every object but the first has one): r01 `(0,2,1) (0,1,1)`, r16 adds
`(1,0,1) (1,2,1) (1,1,1)`; parts that are not sewn are still listed (r07, machine-trim-fine lists
the unsewn lines after every region).

After the PEC section, per colour block: the PEC colour list padded to 128 bytes with spaces
(as v6), a 144-byte 48 × 24 1-bpp icon with the thread's catalogue number and name drawn in it,
the RGB as u24, then a u16 count of preview images (one per block, 8–15 KB each, not mapped; 0 in
`o-hoop-100x100-s`, so a file without them is one PE-DESIGN writes itself) and, per block, `0D`
and the catalogue number as u16 (`0D 05 00` 005 SILVER, `0D 20 03` 800 RED).

#### `CEmbNText` (text)

Text from the Text tool is a `CEmbNText` with one `CTTFLetter` per character. Evidence: r12 (`A`,
Lato, size 10, clicked at (10, 20) mm), r13 (size 20 set afterwards, then dragged 10 mm down), r14
(`AB`), r15 (a rectangle, then the text), r23 (region Fill), r24 (region colour blue), r25 (`B`),
r26 (clicked 10 mm further right), o-text-10-letters (size 8), o-text-vertical and the compat
`text-*` references (sizes 20, line spacing 50/100/150 %, a running line, `Hxg`). PE-DESIGN stores
**no outlines**: it lays the letters out again from the installed font when it opens the file.

| Part | Content | Evidence |
|---|---|---|
| text +0 | bounds `x0 y0 x1 y1` (i16): across the letters, from the font's ascent above the first baseline (1.59 × the size for Lato) down to the baseline or the lowest letter | r12, r14, text-defaults-h, text-size-hg |
| +8 | the same four again (the extent) | all |
| +16 | transform, 6 × f32: identity, or the moves applied after the text was placed (r13: tx 1.0, ty 100.0 after a 10 mm drag) | r13 |
| +40 | u16 letter count | r14 (2), o-text-10-letters (10) |
| then | per letter: the MFC tag (`CTTFLetter` named the first time), u16 character (UTF-16), 148 attribute bytes, the font name (MFC Unicode string, `Lato`), u16 0 | all |
| letter +2 | f32 the size × 10 when the text was made (100.0 at size 10, 80.0 at 8, 200.0 at 20) | r12, o-text-10-letters, text-defaults-h |
| letter +6 | f32 scale applied later (2.0 when size 20 was set on r12's text) | r13 |
| letter +14 | f32 −0.0 in r12–r26, 0.0 in the later files (meaning unknown) | r12, o-text-* |
| letter +22 | u16 region density, 0.1 line/mm (the Text tool's 5.0) | all |
| letter +24 | u16 region colour, a thread-table index | r15 (1: the rectangle's line red) |
| letter +26 | u16 region sew type: 0 Satin, 1 Fill | r23 |
| letter +60, +62, +64 | u16 line sewn, line sew type (9 not sewn, 1 running), line colour index | text-sewing-order, r15 |
| letter +144 | u16 700 in r12–r26, 900 in the later files: likely the font weight (the same Lato Bold) | r12, o-text-* |
| after the letters | u16 count of line breaks, then per break the u16 index of the letter that starts the new line | text-line-spacing-* (`HH`: 1, 1) |
| tail +4 | f32 line spacing, % | text-line-spacing-50/100/150 |
| tail +8 | u16 alignment: 0 left, 1 centre (right not sampled) | r12, o-text-transform-1 (0); the round-4 and text-transform-N centred texts (1) |
| tail +10 | u16 direction: 0 horizontal, 1 vertical | o-text-vertical |
| tail +78 | transform again, 6 × f32 | r13 |
| tail +104 | f32 x, y: the click, the first baseline's alignment point (its left end for left-aligned text, its middle for centred text; file units) | r12 (600, 700), r26 (700, 700), r15 (900, 1000), text-transform-N (1000, 1000) |

A space is a letter like any other, with the same attributes (r27, `A B`), and is listed in the
sewing order. The tail is 112 bytes; the fields not listed (Text Attributes such as kerning, character spacing,
alignment, rotation) are zero or constant in every file. The colour of a text is in the thread
table: r24 changes only the table. In the sewing-order list a text of n letters lists the
connections into its letters (parts 0, 2 … 2n − 2, the first object's first skipped), then the
letters (1, 3 … 2n − 1); a sewn line adds no parts (text-sewing-order).

#### `CEmbDText` (Text Transform)

A text with a Text Transform is a `CEmbDText`: the `CEmbNText` record unchanged (letters, line
breaks, 112-byte tail), then 22 bytes. Evidence: compat/reference/text/text-transform-1 … 26
(HHHHHHH, each preset at the default level), o-text-transform-1, and the owner's round-4 captures
of every preset at levels 0, 25, 50, 75 and 100 on `OpenStitch` and of every preset at the default
level on `Agjy 123`.

| Offset | Content |
|---|---|
| +0 | u16 PE-DESIGN's code of the preset. By palette place 1–26: 24, 25, 0, 1, 8, 9, 4, 5, 10, 11, 3, 2, 7, 6, 18, 19, 12, 14, 20, 21, 17, 16, 22, 23, 13, 15 (the same on all three texts) |
| +2 | f32 strength from the level slider. Default level: 30 (presets 1, 2, 13, 14), 50 (3, 4, 7, 8, 11, 12, 17, 18, 21, 22, 25, 26), 20 (5, 6, 15, 16, 19, 20, 23, 24), 10 (9, 10). Preset 3 reads 0 at level 0 and 100 at level 100; preset 1 falls from 41.2 at level 0 to 1.15 at 100 |
| +6 | 4 × f32. For the arches (presets 1 and 2): the circle's radius, the straight text's centre x, and the circle's centre y, a radius below the baseline for 1 and a radius above the size's top for 2. The radius is the straight text's advance width (its letters' advances, as PE-DESIGN measures its text box, not their ink) over 1.887 on all three texts, and grows with the level (10000 at level 100: flat). For the fans (13, 14): 1000 and zeros in the file made before any arch. For every other preset: the last arch's numbers, carried over from file to file in one session, so not read by PE-DESIGN |

#### `CEmbAText` (Fit Text to Outline)

A text fitted to an outline is a `CEmbAText`: the `CEmbNText` letters and line breaks, then a
122-byte tail that agrees with the plain text's up to +102 (+0 f32 character spacing in 0.1 mm,
−10 and 20 in text-fit-spacing-m1 and -p2; +4 line spacing; +8 the Text Attributes alignment,
left; +78 the transform) and then holds the fit instead of the click point. Evidence: the 14
compat/reference/text/text-fit-* files, ABCDE (or AHHHH) fitted to a 60 mm circle with one
Fit Text to Outline option changed per file.

| Tail offset | Content |
|---|---|
| +104 | u16 horizontal alignment: 0 left, 1 centre (the default), 2 right, 4 spaced evenly (+0 then holds the computed spacing: 300.4 in text-fit-halign-spaced) |
| +106 | u16 vertical alignment: 0 above the outline (default), 1 centred, 2 below |
| +108 | u16 orientation: 0 along the outline, 1 upright |
| +110 | u16 other side: 1 when ticked |
| +118 | u16 the outline's MFC archive number (2: the circle, the list's first object; the outline is written before the text), then u16 1 |

Not mapped yet: the fields that differ between E and V, the line's run times, the marks'
segment and parameter fields, the Round 3 region type codes, the text tail's Text Attributes
(kerning, rotation, right alignment; character spacing on plain text), the letter's line width
and pitch fields, the Text Transform levels other than the default (OpenStitch has none), the
fit's fifth alignment (stretched), Manual Punch blocks (a `CEmbLine` body whose geometry header
is a block list: a u32 count, 3 in o-manual-straight and o-manual-curve, then per-block entries;
owner files with 1, 2 and 3 blocks requested), the appliqué's fifth object and
the preview images.

#### Writer

`pes11::write` (`design.export` `pesVersion` 11; `openstitch_engine::pes11_export` maps the
design) writes the layout above: the header with the metadata strings and the Design Page size,
the thread table in sewing order (region thread, then line thread, per object; strings in the
MFC Unicode form, the PEC chart index of the nearest chart colour, `kind` 10; a colour from
OpenStitch's Brother PEC chart gets the BROTHER EMBROIDERY catalogue number and brand
`EMBROIDERY` PE-DESIGN writes for it, from the 25 pairs found in the owner's files
(`pes11::BROTHER_EMBROIDERY`: 1 → 007, 5 → 800, 16 → 005, 20 → 900 …), any other thread its own
catalogue number, brand and chart), one `CEmbLine` per visible shape with every mapped field set,
one `CEmbOne` per stitch pattern (its needle points in runs cut as version 6 cuts them, a jump
run where the pattern jumps or trims, each colour block's thread added to the table), the
groups, the sewing-order list, the PEC section and the addendum. Texts in Lato Bold are written
as `CEmbNText` (letters at the text's size, Satin or Fill, an optional running, triple or
zigzag line at PE's defaults, line breaks, the line spacing %, left or centre alignment and the
click point at the alignment point), and fitted to an outline as `CEmbAText` (the fit settings, the
character spacing and the outline's archive number; the outline must come before the text),
and with a Text Transform as `CEmbDText` (the preset's code,
the default level's strength and, for the arches, the circle measured on OpenStitch's straight
layout; zeros where PE-DESIGN leaves stale numbers). What it cannot hold fails with the object named: several
outlines in one object, programmable, motif and the other region types, motif, stem,
candlewicking, chain and cutting lines; text in another font, with white space other than
spaces and line breaks, right-aligned, vertical, on an outline, with a Text Transform on several
lines, rotated or scaled, with letter spacing, kerning, per-character
edits or under sewing. Checks: the 102 owner files whose list the reader parses completely (37
with shapes, 19 with plain text, 27 with a Text Transform, 14 with text fitted to an outline,
3 with a stitch group; two of them with gallery shapes) are rewritten from what it read and read back equal (`written_files_read_back_to_the_same_objects`), and their object sections are
byte-identical to PE-DESIGN's outside these documented differences
(`written_object_sections_match_pe_designs_bytes_outside_the_documented_differences`):

- **Pattern block** (body 386–4360): OpenStitch writes its own placeholder of the same layout
  and size (the baseline's 53 point counts, drawn as evenly spaced horizontal lines) instead of
  copying PE-DESIGN's default pattern, which is Brother content.
- **Geometry header:** the closed-shape rule for the marks' coordinates and r01's segment and
  parameter fields (open lines: r11's), so the curved and open owner files differ there.
- **Extent:** bounds ± half the zigzag width (the circle's 589..711 and the open line's
  one-axis growth are not reproduced).
- **+162:** PE-DESIGN points this thread index at 800 RED whatever is sewn; OpenStitch points it
  at the line colour.
- **Header:** a design on a 100 × 100 mm page is written on PE-DESIGN's default page, a Hoop
  Size page (r27's fields: page mode 0, hoop 15, 100 × 100 mm `nv880e-S`, and a u16 of 5 where
  custom pages have 10); any other page as a Custom Size page with r01's hoop fields (150 × 150
  mm, `nv880e-M`, multi-needle) and the page size. PE-DESIGN shows a Custom Size page with a
  Hoop Settings tab, 290 × 190 mm section outlines and no stitch count in the status bar until
  something is selected; its own Hoop Size page file (r27) shows none of them. The machine type
  and hoop of the design's machine settings are not mapped yet.
- **Header strings:** the metadata strings are written in Windows-1252 with CR LF, as
  PE-DESIGN shows them; pes-design-properties holds OpenStitch's former UTF-8 bytes, which
  PE-DESIGN kept when it re-saved the file, so its strings (and the offsets after them) differ.
  A file read from PE-DESIGN keeps its header fields; a design's are written from the templates
  above.
- **Text Transform:** from a design, the arch circle is the straight text's advance width
  over 1.887, measured on OpenStitch's layout as the shift between a left and a right aligned
  setting: HHHHHHH's radius 40.83 mm against PE-DESIGN's 40.80; the other presets carry zeros
  where PE-DESIGN leaves its last arch's numbers. The text box of an arch is PE-DESIGN's straight
  box (ascent line to baseline) bent round the arch's circle: for HHHHHHH OpenStitch writes
  541..1459 × 841..1169 where PE-DESIGN writes 552..1441 × 841..1170 (it recomputes the box when
  it opens the file). OpenStitch's own arch layout uses the same circle since 2026-10-29 (its
  text-transform-1 and -2 references still pass at 0.5 mm) but sews HHHHHHH 79.9 mm wide where
  PE-DESIGN sews this object 79.2 mm: an open difference in how the letters are placed round the
  circle (COMPATIBILITY_AUDIT item 5). Other presets: the letters' extent, the top at most
  the ascent line.
- **Fitted text box:** from a design, the `CEmbAText` bounds and extent come from OpenStitch's
  layout (ABCDE on the 60 mm circle: 805..1218 × 373..759 against PE-DESIGN's 757..1242 ×
  573..757); PE-DESIGN recomputes them when it opens the file (compat
  `pes-0110-editable-text-fit`).
- **Text:** the letter field at +14 (OpenStitch writes r12's −0.0) and the last sewing-order
  field of the line-spacing files (PE-DESIGN leaves it unset, `08 CE 53 00`; OpenStitch writes
  1). A text written from the letter defaults gives r12's object section byte for byte
  (`a_text_built_from_defaults_writes_r12s_object_section`). From a design, the text box's x
  extent comes from OpenStitch's layout (r14's `AB`: 59.9–78.7 mm against PE-DESIGN's 79.3) and
  the thread table lists only the colours sewn (PE-DESIGN's also lists the Text tool's unused
  line colour, 900).
- **PEC section:** the stitch stream is byte-identical for a file re-written from its own
  stitches. The PEC header fields follow the page mode, as in all of PE-DESIGN's own `#PES0110`
  files: 0x1F `FF`; a Custom Size page 0x24–0x2F `20 FF 00 00 64 00 00 00 00 FF 20 20` and
  2900 × 1900 at 0x208 (663 files), a Hoop Size page `20 20 20 20 64 20 00 20 00 FF 20 20` and
  the design's size at 0x208 (46 files; `the_pec_header_carries_pe_designs_fields`). The label,
  the thumbnails, the icons (blank) and the preview images (none) still differ.

**PE-DESIGN's check (2026-10-29, compat `pes-0110-editable-shapes`, reference tier):** PE-DESIGN
11.50 opens the written rectangle with no warning and the shape editable (handles, Shapes tab,
Sewing Attributes; the region type can be changed), shows the attributes the file holds, and
re-saves the object section unchanged except the pattern block, where it writes its own default
pattern over OpenStitch's placeholder. Before the catalogue mapping above, PE-DESIGN showed
OpenStitch's chart colours as `16 / Brother` with no thread chart. **The text file (2026-10-29,
`pes-0110-editable-text`, reference tier):** PE-DESIGN opens it with no warning as editable text
(Lato 10, Satin red, line not sewn, every Text Attribute at its default, the baseline at the
written click point) and retyping works; its re-save keeps the object section but for the text
box's right edge, which it takes from its own layout (787 → 793). It showed a Hoop Settings tab and no
stitch count in the status bar: the file was on a Custom Size page (with PE-DESIGN's own PEC
stitches too, probe p2), so a 100 × 100 mm design is now written on PE-DESIGN's default Hoop Size
page (probe p3 asks PE-DESIGN to confirm). A plain click on a stroke selects the text, as for
PE-DESIGN's own text (a click inside the A's counter selects neither). **Text Transform (2026-10-29,
`pes-0110-editable-text-transform`, reference tier):** PE-DESIGN opens HHHHHHH with preset 1 as
editable text with Transform ticked, preset 1 at its default level (re-selecting the preset gives
the same 79.20 × 26.50 mm and 1307 stitches), centred, Lato 10, and the text can be retyped on
the arc. Its re-save keeps the object section but for the text box. **Fit Text to Outline
(2026-10-29, `pes-0110-editable-text-fit`, reference tier):** PE-DESIGN opens the circle and
ABCDE with no warning as one fitted unit (Fit Text offers Release Text from Outline; stretching
the circle re-fits the text along the ellipse, re-centred), shows the fit settings as written,
and sews the pair, the text and the circle with the stitch counts of its own text-fit-default
(860, 727, 133). Its re-save keeps the object section's length and rewrites two places: the
circle's pattern block (its own default pattern, as for every shape) and the fitted text's
bounds and extent, which it recomputes from its layout: 757..1242 × 573..757, the same as its
own text-fit-default, where OpenStitch writes 805..1218 × 373..759 (an open difference, as for
the arched text box). Version 6 stays the default export until the coordinator decides
otherwise.

## PE-DESIGN's record layout (DST, EXP, JEF, SEW, XXX, PCS, HUS, VIP)

The writers reproduce PE-DESIGN 11.50's exports record for record. The evidence is one design
exported by PE-DESIGN to every format it offers (`compat/reference/machine/machine-formats.*`,
2026-10-27; spec `machine-formats`): two running lines with a trimmed frame move between them,
a colour change and a 40 × 15 mm satin of 15 mm stitches, which every relative format has to
split. `crates/openstitch-formats/src/layout.rs` turns a pattern into the events the writers
share; the per-format rules are in each writer's doc comment and summarised here. Unit tests
(`*_match_pe_design*` in `tests/formats.rs`) hold OpenStitch's export of the same PES
(`tests/fixtures/machine-formats.pes`) against PE-DESIGN's files.

Shared by every format:

- The design is centred: the stream starts at the middle of the needle points' bounds, (0, 0)
  in the relative formats (PE-DESIGN's DST header is `+X 200 -X 200` for a 40 mm design).
- A stitch longer than a record holds is one needle penetration: the full-size pieces are
  jumps and only the remainder is a stitch (HUS alone writes two stitches).
- A frame move is a run of jumps with no trim record of its own; the formats that have a trim
  record do not use it for it (XXX writes the move itself as a `7E` record, VIP an `88`, HUS a
  zero-length jump after the run). The first move of the design counts as trimmed.
- Several formats fold the move's remainder into the first stitch after it (the needle first
  goes down at that stitch, as after a DST jump); others put a zero-length stitch at the
  move's end.

| | Jump pieces | Long stitch | Frame move | Colour change | End |
|---|---|---|---|---|---|
| DST | 12.1 mm, longer axis full, other proportional | jumps + stitch | run padded to ≥ 3 jump records (zero-length jumps before the last); 3 zero-length jumps open the file | `C3`, no move | padded jump run back to (0, 0), `F3` |
| EXP | 12.7 mm, proportional | jumps + stitch | `80 02` run | `80 01 00 00` | `80 02` run back to (0, 0), no end record |
| JEF | 12.7 mm, only the axes over the limit move | jumps + stitch | folded into the next stitch; `80 02 00 00` opens the file | `80 01 00 00` + `80 02 00 00` | `80 10 00 00` |
| SEW | as JEF | as JEF | as JEF | zero-length stitch, then as JEF | `80 10` + 16 zero bytes |
| XXX | 12 mm, each axis clamped | `7F 01` jumps + stitch | trimmed: one `7E` with 16-bit deltas; plain: `7F 01` jumps; folded, then a zero-length stitch | `7F 08 00 00` after the move to the next block, before its zero-length stitch | `7F 01` run back to (0, 0), `7F 7F 03 14` |
| PCS | absolute, no split | one record | jump at the current position, jump to the move's end, stitch there (first move: the stitch alone) | colour record (palette index, `ctrl` 3) before every block, the first included | none |
| HUS | jumps 8 mm, stitches 12 mm, proportional | two stitches | `81` run, `81 (0,0)` when trimmed, then `80 (0,0)` | `84` | `90` |
| VIP | `ceil(longer axis / 7.5 mm)` equal pieces | jumps + stitch | `88 (0,0)` when trimmed, then the pieces folded into the next stitch | `84` | `90` |

SEW's start point is not the centre: PE-DESIGN starts the stream 15.9 mm left of and 9.7 mm
above the design centre (the same for a 40 mm design and a 20 x 20 mm square,
`sew-square-centre.sew`) and sews the way to the first needle point as a stitch, split like a
long one; the stream carries no page position (`sew-a-moved10.sew`, the same design moved
10 mm, is byte-identical). OpenStitch starts there (`sew::START_POINT`).

Open questions, each answered by one more PE-DESIGN export: how JEF, SEW, XXX and VIP write a frame move shorter than one record (folded into a
stitch, as the rule above implies, or a jump plus a zero-length stitch; a design with a 3 mm
move between two objects would show it); the exact VIP piece length (7.5–7.9 mm fit the
file); JEF's `-1` in the 50 × 50 mm hoop block of a 40 mm design whose hoop code is 50 × 50;
XXX's two header tables and the default palette PE-DESIGN fills the unused colour slots with;
the 340 non-zero bytes PE-DESIGN writes in the SEW header; DST's `AX`/`AY` meaning (the file
has `+200 +200`, the start point measured from the lower left corner of the extents, which is
what OpenStitch writes). PE-DESIGN's RGB for its chart colour 800 RED is `E3 31 1F` in XXX
and VIP where OpenStitch's PEC chart table (pyembroidery's) has `ED 17 1F`.

## DST

512-byte header of `KEY:value` CR-terminated fields (`LA ST CO +X -X +Y -Y AX AY MX MY PD`),
`0x1A`, space padding. Records are 3 bytes encoding dx, dy ∈ [−121, 121] in balanced ternary
(powers 1, 3, 9, 27, 81); byte 2 bits: 7 = jump, 6+7 = colour change, `F3` = end. DST y points
up. `LA` is the file name with its extension, cut to 16 characters (`a.DST`), `ST` counts every
record including the end record, and `AX`/`AY` repeat `-X`/`-Y` as PE-DESIGN fills them.

## DST trims

DST has no trim command: the machine trims on a run of jump records ("jumps for trim", three
by default). PE-DESIGN writes every frame move as such a run, padded with zero-length jumps
before its last record, and opens the file with three zero-length jumps; OpenStitch does the
same with `trimJumps` (default 3), which the reader also uses: a trim is marked once a jump
run reaches that count, zero-length records included, and zero-displacement runs are clipped.
A single jump shorter than a record is the long part of a split stitch, not a trim.

## EXP

No header. 2-byte signed (dx, dy) records with y up; `80 02 dx dy` jump (`80 04` is read as a
jump too), `80 80 07 00` trim (read, never written: PE-DESIGN writes a trimmed move as its jump
run alone), `80 01 00 00` colour change. No colour data. Max delta ±127. The reader infers a
trim from a jump run that moves more than one record (12.7 mm) on an axis.

## JEF

| Offset | Content |
|---|---|
| 0x00 | u32 offset of stitch data (`0x74 + 8 × colours`) |
| 0x04 | u32 `0x0A` (pyembroidery writes `0x14`) |
| 0x08 | 14-char date `yyyyMMddHHmmss`, 2 zero bytes |
| 0x18 | u32 colour count (stops included) |
| 0x1C | u32 point count: 1 + stitches + 2 × (jumps + colour changes) |
| 0x20 | u32 hoop code (0 = 110×110, 1 = 50×50, 2 = 140×200, 3 = 126×110, 4 = 200×200) |
| 0x24 | 2 × (half width, half height) |
| 0x34 | 4 × hoop-edge distances (left, top, right, bottom; −1 if it does not fit) |
| 0x74 | u32 Janome chart index per colour (0 = stop), then u32 `0x0D` per colour |

Records: 2-byte signed (dx, dy), y up; `80 01` colour change, `80 02` jump, `80 10` end (two
zero bytes after it). The stream opens with `80 02 00 00`; a frame move is folded into the
stitch after it (jumps that move only the axes over 12.7 mm, then the stitch); a colour change
is `80 01 00 00` then `80 02 00 00`. JEF has no trim command. The reader infers a trim when a
jump run moves more than one record (12.7 mm) on an axis; a single shorter jump is the long
part of a split stitch.

## XXX (Singer)

| Offset | Content |
|---|---|
| 0x17 | u32 record count (colour change and end markers excluded) |
| 0x27 | u32 colour count (readers use the low 16 bits), u16 0 |
| 0x2D | i16 width, height, last stitch x, −last stitch y, then two zero words (pyembroidery writes −min x, max y) |
| 0x39 | 61 bytes PE-DESIGN fills with `01 01 08 01 01 …` (meaning unknown; zero in OpenStitch's files) |
| 0xF6 | u16 0 (pyembroidery writes `0x20`) |
| 0xFC | u32 file offset of the end marker |
| 0x100 | records |

Records: 2-byte signed (dx, dy) with y up; `7E dx(i16) dy(i16)` trimmed frame move (read as a
trim and a move); `7D dx(i16) dy(i16)` long stitch (read as a move, as the reference reader
does; never written); `7F 01 dx dy` jump; `7F 03 00 00` trim (read; a non-zero delta adds a
move); `7F 08 dx dy` colour change (`0A`–`17` also read as colour change); `7F 7F 03 14` end
(`02 14` read too). Then `00 00`, `00 R G B` per colour, entries up to 21 colours (PE-DESIGN
fills the unused ones with a default palette and appends a second table; OpenStitch writes
zeros), `00 FF FF FF`, `00 01`. Jumps and split stitches are clamped to ±120 per axis. Stops are
written as colour changes.

## PCS (Pfaff)

| Offset | Content |
|---|---|
| 0x00 | version byte `'2'` (`0x32`), as PE-DESIGN and libembroidery write |
| 0x01 | hoop: 2 = 80×80 mm, 3 = large (115×120 per libembroidery); PE-DESIGN writes 3 whatever the design's size, and so does OpenStitch |
| 0x02 | u16 colour count, then `R G B 00` per colour: PE-DESIGN's fixed 16-colour VGA table (`pcs::PALETTE`) |
| … | u16 record count (stitches and jumps), then 9-byte records |

Record: `xf x(i24le) yf y(i24le) ctrl`, absolute, y up, in 1/6 mm (pyembroidery's 5/3 ratio
to 0.1 mm) with `xf`/`yf` the fractional parts in 1/256 (truncated; pyembroidery skips them).
`ctrl` 0x00 stitch, bit 0x01 colour change (the record's first byte is the palette index of
the block's colour and its coordinates are zero), bit 0x04 jump (libembroidery calls it a
trim; pyembroidery a move). The design's centre sits at (60, 57.5) mm, the middle of the large
hoop. There is no trim or end record: the reference reader stops at end of file. On write,
trims are dropped and a stop becomes a colour change to the same thread. The reader takes a
colour record before any stitch as the first block's colour and, when any colour record
carries an index, the threads from the table by index; otherwise the table itself (pyembroidery's
files).

## SEW (Janome)

| Offset | Content |
|---|---|
| 0x00 | u16 colour count, then u16 Janome SEW chart index per colour (`SewThreadChart`, 79 entries) |
| … | zero padding (PE-DESIGN writes 340 non-zero bytes here, meaning unknown) |
| 0x1D78 | records |

Records as JEF: 2-byte signed (dx, dy) with y up; `80 01 dx dy` colour change, `80 02 dx dy`
jump (`80 04` read as jump too), `80 10` end, then 16 zero bytes. PE-DESIGN writes a
zero-length stitch before each colour change. Chart index 0 ("Unknown") is read but never
written. No trim record: trims are the jump runs; stops become colour changes to the same
thread. Max delta ±127. The stream starts 15.9 mm left of and 9.7 mm above the design centre, where
PE-DESIGN starts it (see "PE-DESIGN's record layout").

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
| 0x00 | u32 magic `0x00C8FC5D` (PE-DESIGN 11.50); `0x00C8AF5B` (libembroidery, pyembroidery's fixtures) is read too |
| 0x04 | u32 record count (END included) |
| 0x08 | u32 colour count |
| 0x0C | i16 +x, +y, −x, −y extents (y up): max x, −min y, min x, −max y in OpenStitch's y-down units |
| 0x14 | u32 offsets of the command, x and y streams (y runs to end of file) |
| 0x20 | 8 bytes string (zero), u16 (zero) |
| 0x2A | u16 HUS chart index per colour (29 entries, `HUS_CHART`) |

Records `(command, dx, dy)`, signed bytes, y up: `80` stitch, `81` jump, `84` colour change,
`88` trim (read; a non-zero delta adds a move first), `90` end. PE-DESIGN's layout: a frame
move is `81` jumps of at most 8 mm (longer axis full, the other proportional), a zero-length
`81` after them when the move is trimmed (the reader turns that marker into a trim), then a
zero-length `80` at the move's end; a stitch over 12 mm is `80` stitches of at most 12 mm (the
one format that splits a stitch into stitches); no return to the origin. Stops are written as
colour changes to the same thread. The meaning PE-DESIGN gives the string and the u16 is
unknown (zero in its file too).

Evidence: `oracle.hus` is assembled by `tools/genfixtures.py` from the layout pyembroidery's
`HusReader` expects (literal-only streams); its read is the oracle. `openstitch.hus` is the
same design written by OpenStitch with real compression; an earlier version of it, with the
same compressor, was decoded by pyembroidery to exactly the oracle's stitches
(`openstitch_hus_read.json`); the record layout has changed since, the compression has not.
`hus_streams_and_header_match_pe_design` holds the decompressed streams against PE-DESIGN's.

## VIP (Pfaff)

The HUS header with magic `0x0190FC5D`, then:

| Offset | Content |
|---|---|
| 0x2A | u32 `0x2E + 8 × colours` (PE-DESIGN; libembroidery reads the field as the colour-table length, 4 × colours) |
| 0x2E | colour table, 4 bytes per colour (R, G, B, `01`), obfuscated: `out[i] = in[i] ^ KEY[i] ^ out[i−1]` with libembroidery's 400-byte key (so ≤ 100 colours) |
| … | `colours + 1` × u32 `1`, u32 `0`, u16 `0` (as PE-DESIGN writes them; meaning unknown) |
| 0x38 + 8 × colours | command, x and y streams as HUS |

Records as HUS, laid out PE-DESIGN's way: `88 (0,0)` before a trimmed move (not the first);
a frame move folded into the stitch after it and cut into `ceil(longer axis / 7.5 mm)` equal
pieces (truncated, the last takes the remainder), jumps then one stitch; a stitch over 7.5 mm
likewise. The piece length is between 7.5 and 7.9 mm in the one reference file. libembroidery's
reader maps `81` to a trim; OpenStitch keeps the HUS meaning (jump). Evidence: libembroidery's
`sew` CLI (built locally from commit `1138e04`, not shipped) reads `openstitch.vip` with the
right record count and the exact RGB colours; its decompressor is a stub (`character = 0 /* fix
this */`), so it cannot check the stitches. `vip_streams_and_header_match_pe_design` holds the
decompressed streams against PE-DESIGN's; self round trip in
`vip_round_trips_rgb_colours_and_stitches`.

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

Where the records start: PE-DESIGN 11.50's own SHV (`tests/fixtures/pe11/sample-basic.shv`)
starts them at the end of the colour table, followed by exactly the counted words.
pyembroidery's reader starts 2 bytes before the end, which on PE's file adds a stitch at the
origin and, one word short at the end, an extra colour change (#135). OpenStitch starts at
the table's end unless fewer words follow than the counts add up to (pyembroidery's layout,
as in `oracle.shv`), and then 2 bytes earlier (`shv_reads_pe_design_export_like_its_dst`).

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
then 3-byte records `flags dy dx` (0.1 mm, magnitudes 0–255): bit 0x20 / 0x40 negate x / y;
low five bits 0 = stitch, any of 0x0C = colour stop (its bytes do not move the needle), others
= trim with a move; `F8`, `87`, `91` end. The mask index `k` is the byte's own file offset
(libembroidery takes the stream position after the read, minus one); OpenStitch read
`o − 1` until 2026-10-29, which turned PE-DESIGN's CSD into noise (#135).

Source: libembroidery `src/formats/singer.c` (zlib; the same code was published under MIT in
Embroidermodder in 2013). pyembroidery has no CSD module. **Verified for type 0** against
PE-DESIGN 11.50's own CSD export (`tests/fixtures/pe11/sample-basic.csd`,
`csd_reads_pe_design_export_like_its_dst`): every needle point equals PE's DST of the same
design, y is up, a leading stop selects the first colour, the colour order is 0, 1, … into the
palette (no off-by-one), the palette holds PE's RGB (800 RED is E3 31 1F, as in its XXX and
VIP), and a trim's move ends at the next block's first needle point. Type 1 (other
identifiers) is checked only by synthetic files built by the inverse transform in
`csd::tests`. Wilcom
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
| `freeLines` | decorative only, optional (`true`; since 2026-10-30): the paths are free lines inside the unit tile (x, y 0…1), tiled as a grid centred on the region instead of chained into rows; no `rowShift` |

Coordinates are in the pattern's unit box: motif x −0.5…1.5 (along the line), y −1…1
(across, scaled by the motif height), one path from (0, 0) to (1, 0); fill x, y 0…1 (the
tile; programmable fill puts needle points where its rows cross the lines, see below); decorative x
−0.5…1.5, y 0…1, every path from (0, y) to (1, y) so tiles chain into rows. Unknown fields
are refused. A region's area is found on a 64 × 64 grid over the tile that wraps at its sides
(the tile repeats); a point on a line or in an area the lines do not close names nothing, and a
later region of the same area replaces an earlier one. Programmable fill sews engraved areas
with 1 mm stitches and drops needle points inside embossed areas, like the stamp effects.
Designs carry the user patterns they define in `design.patterns` (same document shape), so a
project sews the same everywhere.

How the tiles are sewn (pinned 2026-10-29 against PE-DESIGN 11.50's own fills of owner-made
patterns, `compat/reference/stitches/prog-fill-*.pes`, `motif-fill-2pt.pes`,
`line-motif-zigzag-3pt.pes`):

- **Programmable fill:** the tile lies on the page axes (pattern direction 0), its grid starting
  at the region's top-left corner, or at the reference point when one is set. Rows run at the
  region's direction and density. A row's needle points are its two ends and every point where
  it crosses a tile line, nothing else; consecutive rows share their turning point. Gaps
  between crossings stay one stitch up to 12 mm (PE-DESIGN leaves 8 mm gaps whole) and are
  split evenly beyond. Frequency staggers the base sewing only. On the 40 × 40 mm square at
  45° and a 7.5 mm tile, PE-DESIGN sews 2355 and 1638 needle points with the square and line
  tiles, OpenStitch 2329 and 1636; every OpenStitch needle point lies within 0.2 mm of one of
  PE-DESIGN's (mean 0.10 and 0.11 mm).
- **Motif fill:** the same grid origin; each motif's base line runs through the middle of its
  row band, so the first row lies half a motif below the region's top edge. The rows start at
  the one nearest the start point; travel between rows and to and from the start point walks
  the outline in even steps of at most 2 mm (5 mm in three steps of 1.67 mm), with the fill's
  0.3 mm tie-in and lock: 130 needle points, as PE-DESIGN (2026-10-30).
- **Motif line:** a whole number of motifs, stretched along the line to fill it from end to end,
  on open and closed lines alike (8 motifs of 4.94 mm on a 39.5 mm line at the 5 mm size).
- **Decorative fill with free lines** (`compat/reference/stitches/deco-fill-1line-100mm.pes`,
  2026-10-30): the tile is centred on the region, or starts at the reference point (here the
  region's centre is also the page's, so the reference cannot tell the two apart); every line of
  every tile copy is clipped to the region and split evenly at the run pitch (the 56.6 mm
  diagonal of the 40 mm square in 1.95 mm stitches, the same needle points as PE-DESIGN); pieces
  are joined by travel along the outline in 2 mm steps, from the piece nearest the start point,
  with the fill's tie-in and lock. Not yet the same: PE-DESIGN sews the line three times and walks
  parts of the outline two or three times (292 needle points against OpenStitch's 73); one
  reference does not show the rule behind it.

Not yet the same: when both ends of the first motif-fill row are as far from the start point,
PE-DESIGN walks right and comes back down the right edge where OpenStitch walks left (the same
needle points mirrored; one reference cannot tell PE-DESIGN's tie-break), and its motif line adds
tie-in and tie-off stitches (23 against 16).
The motif line ties in and locks as PE-DESIGN does (since 2026-10-30): three 0.3 mm steps out
along a first stitch over 2.65 mm and back, one lock step at the end; 23 needle points as
PE-DESIGN.

Not yet the same: when both ends of the first motif-fill row are as far from the start point,
PE-DESIGN walks right and comes back down the right edge where OpenStitch walks left (the same
needle points mirrored; one reference cannot tell PE-DESIGN's tie-break).

### Text pattern sources (`.pas`, `.pmf`, `.plf`)

PE-DESIGN's pattern families are text files in a `@DIRECTIVE` layout. OpenStitch imports a
source in that layout when the user made it in their own Programmable Stitch Creator
(`pattern.import`, since 2026-10-29, #131; `openstitch_design::pattern_text`). Brother's
bundled patterns are copyrighted content: they are never read, copied or shipped; the importer
cannot tell them apart from the user's own files, so importing only one's own files is the
user's responsibility.

Layout (ASCII, CRLF or LF, one directive per line, a trailing `\` continues the line, at most
1 MiB and 10 000 directives):

| Directive | Meaning |
|---|---|
| `@VERSION n` | file version, returned as read (PE-DESIGN 11.50 writes `10000`) |
| `@UNIT n`, `@COOD …` | header values, read and ignored (PE-DESIGN 11.50 writes neither) |
| `@SYSENV n` | one number: a fill/stamp tile (`.pas`) or, when the file holds exactly one `@LINE` and no tile geometry, a motif (`.pmf`); PE-DESIGN 11.50 writes `1` for both, returned as read |
| `@SYSENV w h r` | three numbers: a decorative-fill tile (`.plf`): width and height in 0.01 mm and a flag; PE-DESIGN 11.50 writes `10000 10000 1`, its 100 mm Default Size, returned as read |
| `@TILE n` | number of `@TILELINE`s that follow (checked when present) |
| `@TILELINE i npts x0 y0 …` | tile line `i` (0, 1, … in order), `npts` ≥ 2 points |
| `@TILEAREA npts x0 y0 …` | a stamp outline, ≥ 3 points, kept as a closed path of the tile |
| `@LINE npts x0 y0 …` | a motif stroke |

Every coordinate is an integer in the 0…1024 cell (anything outside is refused, as are unknown
directives, a `@TILE` count that disagrees, lines out of order, a missing `@SYSENV` or one given
twice, non-ASCII bytes and more than 64 paths or 4000 points). Mapping into
`openstitch.pattern`: cell x and y divided by 1024 (y down); a motif stroke is turned so its first
and last points sit at (0, 0) and (1, 0) and is scaled the same way across (PE-DESIGN's base
line spans the cell, so 1024 cell units are one motif size: a point 256 units off the line sews
1.25 mm off it at 5 mm). A decorative tile whose lines all run from x = 0 to x = 1024 at one
height becomes a decorative row; one with free lines (PE-DESIGN's editor draws anywhere on a
16 × 16 grid) becomes a decorative pattern with `freeLines` (since 2026-10-30; before, it was
kept as a fill tile). The pattern then obeys the
`openstitch.pattern` rules above and sews at the design's own tile and motif size (PE-DESIGN's
Default Sizes: 7.5 mm fill/stamp, 5.0 mm motif, 100 mm decorative fill).

What PE-DESIGN 11.50's Programmable Stitch Creator writes (owner-made files, 2026-10-29,
`crates/openstitch-design/tests/fixtures/patterns/` with `PROVENANCE.md`): each editor mode
offers exactly one Save type (`.pas`, `.pmf`, `.plf`), so these text files are PE-DESIGN's own
format for user patterns, not only Brother's shipped ones; CRLF; points snap to the grid
(8 × 8 cells of 128 units for fill/stamp); a closed shape is written with its closing point (a
square has 5 points); each separate line is its own `@TILELINE`, with nothing marking the move
between them; the motif editor starts from a fixed base line (0, 512)–(1024, 512). Writing these
files is not planned.

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

PMV is also read (`sewing::pmv_read`, `sewing.import` of a `.pmv` file, since 2026-10-29,
#130): the same layout, every byte checked, all little-endian:

| Offset | Bytes | Content |
|---|---|---|
| 0x00 | 8 | magic `#PMV0001` |
| 0x08 | 36 | banner (pyembroidery writes `.` × 36; any bytes are accepted and returned) |
| 0x2C | 56 | fixed preamble, `01` at 0x2C, 0x39, 0x4F, 0x63, zeros elsewhere; refused if different |
| 0x64 | 2 | needle count *n* (at most 4096 on read; the writer stops at 100) |
| 0x66 | 2 | 2 *n* |
| 0x68 | 2 *n* | one byte pair per needle point: x as a 6-bit two's-complement move from the previous point (0–31 forward, 32–63 = −32…−1), y as a 5-bit two's-complement position (16–31 = −16…−1), both in 0.25 mm; a byte ≥ 64 or ≥ 32 is refused |
| | 2 + 2 | `0000`, `0001` (256) |
| | 16 | `00 00 00 00 05 00 00 00 00 00 00 00 00 00 02 00` |
| | 2 + 2 | `0001` (256), `0000` |
| | 2 + 4 *k* | length table: `k − 1`, `k`, then *k* pairs of u16 (the writer's 18 fixed pairs 0/0 … 500/2000) |
| | 2 + 4 *m* | width table: `m − 1`, `m`, then *m* pairs of u16 (1 pair 8192/1000 when every point has the same y, else 15 pairs 50 i / (28000 ⁄ span) i) |
| | 2 + 16 | `0x12`, 16 zero bytes; nothing may follow |

The reader returns the needle points in 0.1 mm (x as the running sum of the moves, y as
stored and negated back, so re-encoding them with the writer gives the file back byte for byte:
`pmv_reads_back_what_it_writes` over the four fixtures). The y centre the writer subtracted and
the scale it applied to patterns wider than 7 mm are not in the file; an import centres the
points across the feed, starts them at x = 0 and takes the width from their span (1–9 mm). Any
truncation, a count that disagrees with its doubled copy, a byte pair out of range, a table
header whose two bytes are not *k* − 1 and *k*, a changed fixed block or trailing bytes are
refused without reading further. Files PE-DESIGN's Stitch Design Factory writes have not been
compared yet: the owner-made samples requested from the reference round (a 2-point and a 4-point
decorative stitch, one re-saved) will pin whether PE-DESIGN's banner, preamble and tables match
this layout; until then the reader accepts only the layout above.
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

## Thread chart CSV (OpenStitch)

OpenStitch's own format for user thread charts (`threads.importCsv`, `threads.exportCsv`;
editor: Edit ▸ User Thread Charts… ▸ Import CSV… / Export CSV…). PE-DESIGN 11.50 has no
thread-chart CSV: its user charts are binary `.btc2` files in its program folder (`MyChart\`),
made and imported only through Options ▸ Edit User Thread Chart, so charts cannot be exchanged
with PE-DESIGN through files (round-4 check, issue #136). The `.btc2` format is not decoded.

- UTF-8; a leading BOM is ignored. Lines end in CR LF when written; LF is read too.
- Lines starting with `#` are comments. The writer puts `# OpenStitch thread chart: <name>` first.
- An optional column row `code,brand,description,R,G,B` (any case) is skipped; the writer
  always writes it.
- One colour per line, six fields in this order:

| Field | Content |
|---|---|
| `code` | catalogue number, digits only, may be empty |
| `brand` | text, may be empty; at most 100 characters kept |
| `description` | colour name, may be empty; at most 100 characters kept |
| `R`, `G`, `B` | 0–255 |

- A field in double quotes may hold commas; `""` in it is one quote (RFC 4180). The writer
  quotes a field only when it holds a comma, a quote or leading or trailing spaces. A record
  stays on one line: control characters are dropped. Spaces around fields are ignored.
- At most 2,000 colours per chart and 100 charts; files over 4 MiB are refused.

Earlier OpenStitch versions read the same six columns without quoting or a column row, so
their files still import. The built-in charts (`threads.builtinCharts`: `brother` = the
64-entry PEC chart, entries 1–61 marked `basic` as the ones PE-DESIGN matches to;
`janomeJef`, `janomeSew`, `husqvarnaHus`, `husqvarnaShv`) export in the same schema
(`threads.exportCsv {builtin}`), so any of them can be copied into a user chart and edited.
Not adopted from #136, for want of any PE-DESIGN reference: the `#userchart` header with a
chart code, the `id,R,G,B,attr,code,description` column order, the 6-character code and the
1,499-colour cap, and the 94-entry "sew palette".

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
