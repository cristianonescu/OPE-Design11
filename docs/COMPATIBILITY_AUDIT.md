# Compatibility audit — 2026-10-19

The 20 highest-impact differences between OpenStitch and PE-DESIGN 11. They cover stitch
generation, editable PES and embroidery-font behaviour. Each entry gives the Rust module, the
evidence, a reproducible test and the proposed fix. The audit was made at `7b02267`.

## What "reference behaviour" means here

Since 2026-10-27 the items are checked against files saved by the owner's PE-DESIGN 11.50
(black box). [COMPATIBILITY_STATUS.md](COMPATIBILITY_STATUS.md) lists, per item, the specs
at the reference tier; the "Next 11" list below records what is still open.

The audit uses the reference evidence that is available and permitted under
[LEGAL_AND_LICENSING.md](LEGAL_AND_LICENSING.md):

- **Guide:** Brother's public PE-DESIGN 11 online guide. Page links are given per item; the full
  index is in [research/pedesign11-manual-toc.md](research/pedesign11-manual-toc.md).
- **Spec:** public format descriptions:
  - pyembroidery's [PesWriter](https://github.com/EmbroidePy/pyembroidery/blob/master/pyembroidery/PesWriter.py)
    and [PesReader](https://github.com/EmbroidePy/pyembroidery/blob/master/pyembroidery/PesReader.py)
  - the [EduTechWiki PES page](https://edutechwiki.unige.ch/en/Embroidery_format_PES)
  - the [libpes wiki](https://github.com/frno7/libpes/wiki/PES-format)
- **Measured:** OpenStitch output, produced with `stitchtool exec` at `7b02267`.

Each item has an evidence grade:

| Grade | Meaning |
|---|---|
| **Documented difference** | The code contradicts the guide or a public spec. It can be fixed now. |
| **Documented, unverified** | The guide or a spec describes the behaviour, but no test proves we match it. |
| **Needs reference** | The behaviour is undocumented. The exact values need a black-box reference (see below). |
| **Defect** | Wrong regardless of PE-DESIGN, for example a needle path no embroidery program produces. |

### REA evidence

No item has REA evidence, for two reasons:

- REA analyses executables, packages and bundles. It rejected a PES file (`oracle.pes`) as an
  unsupported format, so it cannot inspect embroidery data.
- Using REA on PE-DESIGN's own binaries is restricted by the technique register in
  LEGAL_AND_LICENSING.md:
  - decompiling stitch-generation or font code is **prohibited**;
  - decompiling file-format code is **prohibited pending legal review**.

Every item below therefore says "REA: none". The next section explains how undocumented
behaviour can still be settled without decompilation.

### Legitimate ways to settle undocumented behaviour

"Undocumented" is not treated as a permanent blocker. The permitted methods, from cheapest to
dearest:

1. **Black-box runs of a licensed PE-DESIGN 11** (Art. 5(3), allowed):
   - Open OpenStitch outputs in PE-DESIGN, and create the same test shapes there.
   - Save the results as PES and DST, then compare them with the harness below.
   - This settles every "Needs reference" value: densities, underlay geometry, connection
     points, trims and envelope shapes.
   - It uses the owner's licensed PE-DESIGN 11.50 on their own machine; the procedure is in
     [compat/reference/README.md](../compat/reference/README.md).
2. **PES files the owner saves from their own PE-DESIGN** (allowed). Studying fields that no
   public source describes, from those files alone, is allowed as project policy (owner's
   decision, 2026-10-27; see [LEGAL_AND_LICENSING.md](LEGAL_AND_LICENSING.md)). The law is
   unsettled there, and a lawyer's review stays on the owner's list.
3. **Sew-outs on the owner's Brother machine** (allowed). This is the only route to
   Machine Verified status.
4. **Art. 6 decompilation of file-format code, limited to interoperability.** This could
   apply to the editable PES object classes, and REA could do it. It is possible only after the
   legal review confirms the information is not otherwise available. It is not used for
   stitch-generation or font code.

## Summary

| # | Difference | Area | Severity | Grade | Needs reference to finish? |
|---|---|---|---|---|---|
| 1 | Satin sewn as a square wave, not a zigzag (**fixed**, internal regression) | Stitches | 5 | Defect | No |
| 2 | Default PES is truncated v1 with no object section (**fixed**, internal regression; open test pending) | PES | 5 | Documented, unverified | Black-box open test |
| 3 | Trims and jumps inside one fill or satin region; no Running Stitch Path (**fixed**, internal regression) | Stitches | 5 | Documented difference | No |
| 4 | Trims between underlay, top stitching and outline of one object | Stitches | 4 | Documented, unverified | Single-needle policy |
| 5 | Every design is written as one stitch group, nothing editable (**partly fixed**, 2026-10-29: `pesVersion` 11 writes shapes as `CEmbLine`, which PE-DESIGN 11.50 opens editable, and Lato Bold text as `CEmbNText`, which it opens editable and retypable (both reference); stitch patterns as `CEmbOne` stitch groups (reference pending, `pes-0110-stitches`)) | PES | 5 | Reference | Yes (the Hoop Size page check; Text Transform and text on an outline) |
| 6 | One sewing direction per text instead of per stroke | Fonts | 5 | Documented difference | Angles |
| 7 | Auto direction is one fixed angle (**fixed**, internal regression) | Stitches | 4 | Documented difference | No |
| 8 | Underlay model differs (**partly fixed**: PE's two region types, Density Dense / Medium / Light, the Manual Punch column's edge / piping / zigzag); zigzag lines have no underlay | Stitches | 4 | Documented difference | Geometry |
| 9 | PE-DESIGN-made PES files lose objects, properties and colours on re-save | PES | 4 | Documented difference | Pass-through only |
| 10 | Hoop and Design Page fields hard-coded in PES | PES | 4 | Documented, unverified | Field encoding |
| 11 | No Connection Points (Default / Closest / Farther) for text | Fonts | 4 | Documented difference | Geometry |
| 12 | No minimum font sizes or small-text fonts | Fonts | 4 | Documented difference | No |
| 13 | No per-character colour or multi-character selection | Fonts | 4 | Documented difference | Colour grouping |
| 14 | Programmable fill defaults to 0° instead of 45° (**fixed**, internal regression; type change rule pending) | Stitches | 3 | Documented difference | No |
| 15 | Missing fill, satin and zigzag attributes; wrong ranges (**partly fixed**: Frequency and ranges; Stitch Type, Half Stitch, Miter limit pending) | Stitches | 3 | Documented difference | Geometry |
| 16 | Optimize Sewing Order does not move entry and exit points (**fixed**, reference) | Stitches | 3 | Documented difference | No |
| 17 | Hole sewing not validated or paired (**fixed**, internal regression) | Stitches | 3 | Documented difference | No |
| 18 | PES thread table drops chart names and colour type (**fixed**: round trip; chart strings pending) | PES | 3 | Documented difference | Chart strings |
| 19 | Design Property metadata cannot be set; lost on import (**fixed**, internal regression) | PES | 3 | Documented difference | No |
| 20 | Character kerning moves one glyph without reflowing the line | Fonts | 3 | Documented, unverified | Reflow rule |

Line references are relative to `crates/`.

---

## 1. Satin is sewn as a square wave, not a zigzag

- **Minimum density (2026-10-28, `sweep-satin-density-0p2/0p3/0p5.pes`, reference tier):** between
  PE-DESIGN's minimum 0.1 lines/mm (parked: PE sews 5 crossings 10 mm apart) and 0.5, the crossings
  are joined by diagonals whose longest stitch OpenStitch matches exactly (11.66, 8.99, 7.21 mm) with
  the counts within 10 %. PE's density field has one decimal, so 0.15 cannot be set.
- **Wide columns (2026-10-28, `machine-satin-width-16/18/20/22/25/30.pes`, reference tier):**
  PE-DESIGN sends satin crossings of 16 to 30 mm to the machine whole (every stitch the full
  width, 209 stitches on each 40 mm column at density 4.5) and OpenStitch matches it: no split
  below 30 mm.

- **Impact:** severity 5.
  - Every satin row goes straight across, then takes a short stitch along the edge to the next
    row.
  - That doubles the stitch count and puts a ~0.33 mm stitch on both edges of every column at
    density 3, which perforates the fabric and ragged-edges the column.
  - It affects region satin, Manual Punch columns and skeleton satin.
- **Module:**
  - `openstitch-design/src/generate.rs:161-169`: `satin` emits `[start, end]`, then
    `[end, start]`.
  - `openstitch-design/src/column.rs:147` and `openstitch-design/src/skeleton.rs:212` have the
    same `k % 2` flip. The comment at `skeleton.rs:201` describes "left, right, left…", which
    the code does not do.
- **Evidence:**
  - Measured: a 5×2 mm rectangle, satin at 90° and density 3, gives 32 points with segment
    lengths 2.0, 0.33, 2.0, 0.33, ….
  - The guide describes satin only in pictures
    ([region sewing attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293518.html)).
- **REA:** none.
- **Grade:** Defect.
- **Measured after the fix (`c5f3bad`):** the same 5×2 mm rectangle sews 15 stitches of
  2.0–2.03 mm, alternating edges, with no edge stitches.
- **PE-DESIGN 11.50 behaviour** (black-box, 2026-10-27; references
  `compat/reference/stitches/satin-zigzag-rect.pes` and `satin-zigzag-column.pes`, under
  sewing off): no square wave, but not a symmetric zigzag either. Fixed-direction and Manual
  Punch satin is a sawtooth: each row (rung) is sewn across along the sewing direction and the
  stitch to the next row slants back, rows 2/density apart (2.0 mm crossings and 2.09–2.15 mm
  slants on the 5×2 mm column). Every satin object starts with a tie-in (0.3 mm out along the
  first stitch and back; three steps out and two back after a first stitch over 3 mm) and ends
  with a lock (0.3 mm back along the last stitch and return; two steps after a last stitch over
  4.25 mm), per `satin-lock-column-3p5.pes` and the other satin references.
  A rectangle starts and ends in the middle of its bottom edge: PE runs along that edge to one
  end, sews back to the middle (the last row 0.2 mm short, reached along the far edge), then
  does the same for the other half: 27 stitches, 7 of them short edge stitches at the start
  point. The Manual Punch column sews 18: tie-in, 15 crossings over 8 rungs, lock. Auto satin
  stays a symmetric zigzag.
- **Status:** fixed to PE-DESIGN's structure, reference tier. Region satin with a fixed
  direction and Manual Punch columns sew the sawtooth (`region::sawtooth_satin`,
  `column::sawtooth`), split at a start point inside the column (`region::split_satin`), with
  PE's tie-in and lock (`region::tie_and_lock`); skeleton and Auto satin keep the symmetric
  zigzag. satin-zigzag-column and -skeleton measure the body without the tie-in and lock
  (`exclude`); satin-zigzag-rect expects PE's observed values. Pull compensation still
  lengthens every row. Regions of several sections (the U, the H) are sewn in PE's order
  (`region::sections_in_order`, satin-sections-h): an opening row along the start point's
  edge, the sections from the far end of the rows back, legs from the tip corner that comes
  first in the outline, the start point's section last; a feathered edge keeps the sawtooth
  and moves the rows' starting ends by PE's fixed pattern (satin-feather). Tests: `crates/openstitch-engine/tests/satin_compat.rs`
  (`region_satin_sews_a_zigzag_across_the_column`, `manual_punch_column_sews_a_zigzag`,
  `skeleton_satin_sews_a_zigzag`, `pull_compensation_widens_the_zigzag`).
- **Test:**
  - Inputs: `design.new` → `shape.rectangle {x:10,y:10,width:5,height:2}` →
    `object.setRegion {id:1,type:"satin",angle:90,density:3}` → `object.toStitches {ids:[1]}`
    → `stitches.points {id:1}`.
  - Assert: consecutive needle points alternate between the two edges, no stitch shorter than
    0.5 mm lies along an edge, and the stitch count is 15 ±1.
  - Repeat with `shape.column` and `shape.skeletonSatin`.
- **Fix:**
  - One needle point per row: left on even rows, right on odd rows (rungs in `column`/`skeleton`).
  - Update `satin_alternates_rails_and_follows_the_turn` and the skeleton tests to assert side
    alternation.
  - Golden fixtures that contain satin change and must be regenerated in the same PR.

## 2. Default PES is truncated v1 with no object section

- **Status (2026-10-19): fixed, internal regression.** `WriteOptions::default` is v6 with the
  stitch group; `FileType::write`, `design.export`, `nameDrop.export`, `transfer.send` and
  `library.convert` follow; `1t` stays as the explicit machine-only choice. Tests:
  `default_pes_is_version_6_with_objects_and_thread_table`, `pes_compat::*_writes_full_v6`,
  `transfer::tests::sent_pes_is_version_6_with_objects_and_threads`. Not verified: the
  black-box open test in PE-DESIGN below is still needed before it can be called Reference
  Verified.

- **Impact:** severity 5.
  - Most paths write `#PES0001` with no object section and no thread table: `design.export`
    without `pesVersion`, lettering output, USB transfer and Design Database convert.
  - A public write-up says PE-DESIGN opens such a file as a blank document.
  - Even if it opens, colours fall back to the PEC chart and names are lost.
  - Machines are unaffected, since they read the PEC section.
- **Module:**
  - `openstitch-formats/src/pes.rs:321-328`: `WriteOptions::default` is V1, truncated.
  - `openstitch-formats/src/lib.rs:159`: `FileType::Pes`.
  - `openstitch-engine/src/commands.rs:2096-2099`: the `"1t"` default.
  - `openstitch-engine/src/lettering.rs:831`.
  - `openstitch-engine/src/transfer.rs:375` and `openstitch-library/src/convert.rs:172` use
    the defaults.
- **Evidence:**
  - Spec: EduTechWiki says a minimal PES "will load normally and display a blank document" in
    PE-DESIGN.
  - pyembroidery's default v1 is not truncated.
- **REA:** none.
- **Grade:** Documented, unverified.
- **Test:**
  - Inputs: `design.new {name:"t1",width:100,height:100}` → `shape.rectangle {fill:"Red",outline:"Black"}`
    → `design.export {path:"a.pes"}`. Do the same through `library.convert` from `oracle.dst`
    and through `transfer.send`.
  - Assert: every user-facing PES has version ≥ 6, at least one `CEmbOne` and a thread table
    (`pes::read_objects`).
  - Black-box: open `1t`, `1`, `6t` and `6` outputs in PE-DESIGN and record what appears.
- **Fix:**
  - `WriteOptions::default` → `{V6, truncated:false}`, and route `FileType::write` for PES
    through it.
  - Change the `"1t"` defaults in `commands.rs` and `lettering.rs`.
  - Keep `1t` as an explicit "machine only, smallest" choice.
  - v6 is already byte-identical to pyembroidery, so the risk is low.

## 3. Trims and jumps inside one fill or satin region; no Running Stitch Path

- **Impact:** severity 5. Concave or holed regions (letters, U shapes) get trims and jumps
  inside the region, where PE-DESIGN runs hidden travel stitches. The result is extra trims,
  loose tails and longer sew time.
- **Module:**
  - `openstitch-design/src/model.rs:760`: `travel = false` for Fill and Satin.
  - `openstitch-design/src/region.rs:165`: rerouting only when `travel` is on.
  - `openstitch-design/src/compile.rs:27-28,74-81`: gaps over 3 mm become TRIM+JUMP.
- **Evidence:**
  - Guide ([region sewing attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293518.html)):
    for Satin and Fill, Running Stitch Path is Inside of region, Along outline or On outline.
    All three travel with running stitches; there is no jump option.
  - Measured: a U polygon (10,10)-(40,10)-(40,40)-(30,40)-(30,20)-(20,20)-(20,40)-(10,40)
    filled at 0° gives 2 trims and 2 jumps inside the object.
- **REA:** none.
- **Grade:** Documented difference.
- **Status:** fixed, internal regression tier only (not reference verified).
  - `RegionSewing::running_stitch_path` (`RunningStitchPath {Inside, AlongOutline, OnOutline}`)
    replaces `travel`; fill and satin default to Inside. Old `.osd` files load: `travel: true`
    is Inside, `travel: false` or no value keeps the jumps they sewed before.
  - `region::reroute_with` and `paths::TravelGraph::outline`: Inside is the shortest path inside
    the region; On outline walks the outline the shorter way round; Along outline walks it
    0.5 mm inside (a placeholder until measured). Both outline variants fall back to Inside
    when the two ends are not on one contour.
  - Exclusions: `object.setRegion {runningStitchPath}` is refused with Auto direction, a
    feathered edge, on text, for other sew types, and Inside with gradation. Where the
    attribute is unavailable the region travels inside (what PE-DESIGN sews there needs a
    reference); gradation turns a stored Inside into Along outline.
  - Exposed in `object.setRegion` (`runningStitchPath`; `travel` still accepted) and the
    Sewing Attributes pane (Running stitch path, hidden for text).
  - Tests: `crates/openstitch-engine/tests/region_compat.rs`
    (`fill_and_satin_travel_between_parts_without_trims` for the U polygon with every value,
    satin and gradation; `the_outline_paths_differ_from_inside_travel`,
    `running_stitch_path_follows_the_guide_exclusions`,
    `running_stitch_path_defaults_and_old_files`, `old_fill_without_travel_still_jumps`) and
    `outline_travel_walks_on_or_just_inside_the_outline`. The golden fixtures did not change:
    their regions have one part each, or come from `.osd` files without `travel`.
- **Test:**
  - Inputs: the U polygon → `object.setRegion {type:"fill",angle:0}` → `object.toStitches` →
    `stitches.points`.
  - Assert: 0 trims inside the object, and every travel segment's midpoint inside the region
    (0.05 mm tolerance).
  - Repeat for satin, and for each Running Stitch Path value.
- **Fix:**
  - Replace `travel: bool` with `RunningStitchPath {Inside, AlongOutline, OnOutline}`,
    defaulting to Inside for Fill and Satin.
  - Implement the outline variants in `region::reroute` / `paths::TravelGraph`.
  - Follow the guide's exclusions: Inside is not available with gradation, and the attribute is
    not available with Auto direction, text or a feathered edge.

## 4. Trims between underlay, top stitching and outline of one object

- **Settled 2026-10-28** (references `machine-jumps.pes`, `machine-trim-bound.pes`,
  `machine-trim-fine.pes`, specs of the same names at the reference tier): between same-colour
  parts PE-DESIGN 11.50 sews a needle-to-needle move of up to 2.24 mm across as a plain stitch
  (no jump at all) and trims a move of 2.33 mm and more (gaps of 0.5, 0.8 and 1.0 mm sewn, 1.1 mm
  and up trimmed, with its exit 0.5 mm inside the right edge and 1 mm up and its entry 0.5 mm
  inside the next left edge: moves of sqrt((gap + 1)^2 + 1)). OpenStitch's compiler now sews a
  move of up to 2.3 mm between objects across (`Compiler::sew_across_distance`) and trims every
  longer one, so no plain interior jump is written. Open: where PE-DESIGN puts the optimized
  exit and entry (0.5 mm inside the edges; OpenStitch's lie on the edges, so the specs draw the
  squares at PE's needle-to-needle moves), and the value PE's multi-needle "minimum jump stitch
  length for trimming" field shows (read the Design Settings dialog; the recordings were made
  with the multi-needle machine type at its default). Also settled (`machine-rbr.pes`, reference
  tier): objects drawn red, blue, red sew as both reds (one block, a trim between them) then blue,
  one colour change, as `order.optimize` does. Also (`machine-rbr-far.pes`, `machine-rbgb.pes`, reference tier): the reds merge even
  70 mm apart, and red, blue, green, blue sews red, both blues, green: same colours merge in
  first-occurrence order across other colours.

- **Impact:** severity 4. One same-coloured rectangle with underlay and an outline sews with
  3 trims and 3 jumps. That means more cuts, more tie-offs and longer sew time.
- **Module:**
  - `openstitch-design/src/compile.rs:114`: the first run of each element calls
    `travel(.., false)`.
  - `compile.rs:277-286`: underlay, top stitching and outline are separate elements.
  - `compile.rs` `Writer::travel`: every frame move between elements is TRIM+JUMP on
    single-needle machines (PE-DESIGN 11.50 trims every interior move: compat spec
    `machine-formats`, evidence); multi-needle machines trim moves over the design's minimum
    jump length (3 mm when unset).
  - `openstitch-design/src/machine.rs:305-310`.
- **Evidence:**
  - Guide, [entry and exit points](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/04_1858418.html):
    stitching continues from the region exit (R2) to the line entry (L1).
  - Guide, [jump trimming](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/04_1908282.html):
    per-object on/off with a minimum length, for multi-needle machines only.
  - Measured: a 30×30 rectangle with fill, zigzag underlay and a same-colour outline gives
    trims at stitch indexes 61, 738 and 789 on the default single-needle machine.
- **REA:** none.
- **Grade:** Documented, unverified for the routing. The single-needle trim policy is now
  measured: in 177 PE-DESIGN 11.50 PES files (2026-10-27) every interior frame move carries
  the PEC trim flag and no plain jump occurs inside the design, so PE-DESIGN does write trims
  for single-needle machines, for every move it does not sew across (shortest observed trim
  3.0 mm). OpenStitch's compiler trims every interior move since the `machine-formats` spec.
- **Test:**
  - Inputs: `shape.rectangle {x:50,y:10,width:30,height:30,fill:"Blue",outline:"Blue"}` →
    `object.setRegion {id:1,type:"fill",angle:0,underlay:true,underlayStyle:"zigzag"}` →
    `stitches.points`.
  - Assert: 0 trims inside the object. On multi-needle, trims only for jumps ≥ `minJumpTrim`.
- **Fix:**
  - Route an object's layers as one sequence: underlay end → top start → region exit → line
    entry, with inside travel.
  - Gate trims on machine type, the per-object `trim_jumps` setting and the minimum length.
  - Keep the forced cut before a colour change.

## 5. Every design is written as one stitch group; nothing stays editable in PE-DESIGN

- **Status (2026-10-29):** `design.export` with `pesVersion` 11 (editor: File ▸ Export ▸ PES ▸
  PE-DESIGN 11) writes `#PES0110` with one `CEmbLine` per shape in PE-DESIGN 11.50's own layout:
  fill and satin regions, running, zigzag, triple, E and V lines, colours, under sewing, density,
  direction, Frequency, Running Stitch Path, the gradation flag, Bézier outlines, groups, the page
  size and the sewing-order list (`openstitch-formats/src/pes11.rs`, `openstitch-engine/src/pes11_export.rs`;
  FILE_FORMATS.md "PES 0110 object list", "Writer"). The 37 owner files the reader parses
  completely are rewritten from what it read and read back equal, and their object sections are
  byte-identical to PE-DESIGN's outside the documented differences (the pattern block, the
  geometry marks' parameters, the hoop fields). Text, stitch patterns, several outlines per
  object and the other region and line types are refused with the object named; version 6 stays
  the default. **PE-DESIGN's check (2026-10-29, reference tier):** PE-DESIGN 11.50 opens the
  written rectangle with no warning and the shape editable, and re-saves the object section
  unchanged except its own pattern block (spec `pes-0110-editable-shapes`, MANIFEST entry). It
  showed the region direction 0°: the file's value, because OpenStitch's default fill direction
  is 0° where PE-DESIGN's tools start at 45° (r01). The thread records now carry PE-DESIGN's
  BROTHER EMBROIDERY numbers for OpenStitch's chart colours (it showed `16 / Brother` before).
- **Text (2026-10-29):** Lato Bold text is written as `CEmbNText` with one `CTTFLetter` per
  character: size, Satin or Fill with density and colour, a running, triple or zigzag line,
  line breaks and spacing, and the click point PE-DESIGN stores (the first line's start).
  PE-DESIGN keeps no outlines for text and lays it out again from the font. The 18 owner text
  files are rewritten byte-identical outside two PE-side quirks (FILE_FORMATS "Writer").
  Spaces are letters (r27). Centred text is written as PE-DESIGN writes it (alignment field
  and click point), and Text Transform text as `CEmbDText` with PE-DESIGN's preset code and
  default strength (FILE_FORMATS "`CEmbDText`"; spec `pes-0110-editable-text-transform`,
  reference tier: PE-DESIGN opens it editable with the preset at its default level, retypable;
  **open:** OpenStitch's arch sews HHHHHHH 79.9 mm wide where PE-DESIGN sews the same object
  79.2 mm (its own text-transform-1: 79.3) round the same 40.8 mm circle, so the spec's width
  check stays at the internal tier until the letters' placement round the circle matches at
  0.3 mm), and text fitted to an outline as `CEmbAText` with its Fit Text to Outline settings
  and the outline it follows (spec `pes-0110-editable-text-fit`, reference: PE-DESIGN 11.50 opens it
  with the text still fitted to the circle and re-fits it when the circle is reshaped; its re-save
  rewrites only the circle's pattern block and the text's box, which it recomputes); Manual
  Punch columns as PE-DESIGN's Straight and Curve Block objects (specs `pes-0110-manual-punch`,
  `pes-0110-curve-block`, reference pending; owner files m01–m11 read and rewritten byte for
  byte outside the pattern block; a Curve Block written from a design has handles within one
  file unit of PE-DESIGN's, which way PE-DESIGN rounds them not mapped); the Appliqué wizard's stages as PE-DESIGN's appliqué lines and colours (spec
  `pes-0110-applique`, reference pending; owner files a01–a05 read and rewritten byte for byte
  outside the documented differences);
  Shapes-gallery shapes (`CEmbSpecialShape`, `CEmbLine` records under another class name)
  round-trip (o-gallery-heart, shape-start-points). Refused for
  now: right-aligned and vertical text, stretched fit, rotation, letter spacing, kerning,
  per-character edits, other fonts.
  **PE-DESIGN's check (reference tier, `pes-0110-editable-text`):** it opens the file as
  editable text with every attribute as written and retyping works. Two differences came with
  it: a Hoop Settings tab with no stitch count in the status bar, from the Custom Size page
  the file was on (a 100 × 100 mm design is now written on PE-DESIGN's default Hoop Size page,
  with the PEC header form PE-DESIGN writes for it; confirmation pending, probe p3), and a
  plain click that seemed not to select the text: it does, on a stroke, as PE-DESIGN's own
  text.
- **Object-class samples (2026-10-28):** `crates/openstitch-formats/tests/fixtures/pe11/o-*.pes`
  (PROVENANCE.md "Round 3") hold one owner-drawn example of every object kind PE-DESIGN writes:
  the Outline, Rectangle/Circle/Arc and Manual Punch tools and every region type are `CEmbLine`,
  gallery shapes `CEmbSpecialShape`, plain and vertical text `CEmbNText`, Text Transform
  `CEmbDText`, text fitted to an outline `CEmbAText` (the compat `text-fit-*` files), plus a group,
  the Applique Wizard's output, page 200 × 200, Hoop-Size pages (7.7 KB files: a shorter header)
  and the single-needle machine type. Small Text, Monogram, Motif and Decorative Fill use Brother
  content and have no sample.

- **Impact:** severity 5. In PE-DESIGN, a design saved by OpenStitch arrives as a single
  stitch pattern:
  - only needle points can be edited;
  - shapes and text lose their sew types and attributes;
  - text cannot be retyped;
  - the per-object sewing order is gone.
- **Module:**
  - `openstitch-formats/src/pes.rs:380-392,476-533`: one `CEmbOne`+`CSewSeg` per file.
  - `openstitch-engine/src/commands.rs:2106`: exports the compiled stream.
  - `pes.rs:235-238`: the reader stops at an unknown class.
- **Evidence:**
  - Guide, [types of data](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/03_1219818.html):
    stitch patterns only allow point edits.
  - Spec (EduTechWiki, libpes): `CEmbRect`, `CEmbCirc` and `CEmbLine` have partial field
    lists; `CEmbPunch`, `CSewFigSeg`, `CLetter` and `CEmbNText` are unknown.
- **REA:** none. Decompiling PE-DESIGN's PES writer is prohibited pending legal review (method 4).
- **Grade:**
  - Rectangle, circle and line: documented, unverified.
  - Text and filled outlines: needs reference.
- **Test:**
  - Now: write one `CEmbOne` per OpenStitch object and assert the object count equals the
    design's object count. PEC stays byte-identical to today's output.
  - With a reference: a 40×20 mm rectangle exported as v6 must contain `CEmbRect`, with
    l/t/r/b within ±1 unit. In PE-DESIGN, the file must open with Sew type editable.
- **Fix:**
  - Short term: `pes::write_objects(&[StitchPattern])`, fed by `Compiler::compile_object`,
    giving one stitch group per object in sewing order. This needs one black-box check that
    PE-DESIGN accepts several `CEmbOne`.
  - Long term: shape classes from the public field lists, checked against owner files. Text
    only after method 2 or 4.

## 6. One sewing direction per text instead of per stroke

- **Impact:** severity 5. PE-DESIGN's built-in fonts give each stroke its own sewing direction,
  so satin follows each stroke. OpenStitch gives every glyph in a text the same angle, so
  diagonal and curved strokes are sewn at the wrong angle. This is the largest lettering
  quality gap.
- **Module:**
  - `openstitch-design/src/compile.rs:263-284`: each glyph reuses the object's `RegionSewing`.
  - `openstitch-design/src/region.rs:86-99,162`: the direction is defined in object
    coordinates, not per glyph.
- **Evidence:**
  - Guide, [converting text](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/05_1915274.html):
    Convert to Blocks keeps the multiple sewing-direction lines, while Convert to Outline makes
    the direction uniform.
  - Guide, [region sewing attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293518.html):
    for satin text, Direction cannot be set.
- **Measured (2026-10-30, [research/stroke-lettering.md](research/stroke-lettering.md)):** the
  owner's Lato Bold references show PE-DESIGN cutting each TrueType letter into strokes, for
  Fill as for Satin: the satin H is sewn as the left stem's top part, its bottom part (sewn
  upwards), the crossbar with vertical crossings and the right stem (285 needle points against
  OpenStitch's 243, all horizontal); in B the rows turn with each bowl in steps of 12-20°; x and
  g are sewn in two runs with a trim between (11 trims in "OpenStitch" against 9). The note lists
  the open rules (cutting at junctions and corners, order, under sewing, size) and the owner
  captures requested to settle them.
- **REA:** none. Font internals are in the prohibited category.
- **Grade:** Documented difference. The per-stroke angles are measured on the H, A, B, x and g;
  the cutting rules need the requested captures.
- **Test:**
  - Inputs: `text.add {"text":"AVOW","size":30,"x":50,"y":50}` →
    `object.setRegion {"id":1,"type":"satin"}` → `design.generate`.
  - Assert: each diagonal letter has at least 2 dominant stitch-angle clusters, each within ±15°
    of the stroke perpendicular.
  - `object.setRegion` with `angle` on satin text is refused.
- **Fix:**
  - Store direction lines per glyph, in glyph space. Derive them for TrueType glyphs from
    `skeleton.rs`/`column.rs`, and store them in `.osfont`.
  - Transform them with each glyph's placement in `text::layout_in`.
  - Refuse `angle` on satin text.
  - Make Convert to Blocks emit one block per stroke.

## 7. Auto direction is one fixed angle

- **Impact:** severity 4. In PE-DESIGN, Auto rows turn with the shape on curved or bent
  regions. In OpenStitch they stay at one angle.
- **Module:** `openstitch-design/src/region.rs:105-107,222-260`: `auto_angle` returns one
  principal-axis angle.
- **Evidence:** Guide, [region sewing attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293518.html):
  Auto varies the direction to follow the shape. Half Stitch, Stitch Type and Running Stitch
  Path are unavailable with Auto.
- **REA:** none.
- **Grade:** Documented difference.
- **PE-DESIGN 11.50 behaviour** (black-box, 2026-10-27; references `satin-auto-bend-90-nohalf.pes`,
  `satin-auto-bend-60.pes`, `auto-satin-bent-band.pes`): Auto satin starts at the start point, runs
  0.5 mm inside the outline *forward along it* (over a bent band's outer edge, not the shorter way
  round) to the far end, sews back from there with one needle point per rung on alternate edges and
  ends on the start point itself. The rungs lie at whole spacings from the far edge (the 90° band's
  outer edge: 30.0, 29.3, 28.7 … mm), the remainder landing at the start (the 60° band's start leg
  sits 0.05 mm off the lattice). On straight legs the rungs are square to
  the leg; at a bend the outer ends keep an even spacing along the outer edge and the rung
  direction turns linearly with the distance along it, over three band widths for a 90° turn (two
  for 60°), centred on the outer corner; each rung reaches across to the inner edge, so the inner
  ends crowd towards the inner corner. The satin reaches the band's square ends.
- **Bend sweep (2026-10-28, references `satin-auto-bend-30/45/75/90.pes` and their `-nohalf` twins):**
  PE-DESIGN's count is the same at 30°, 45°, 60° and 75° (171–177 stitches on the 5 mm band) and the
  same with Half Stitch on or off; OpenStitch is within 10 % and the longest crossing within 0.5 mm
  at all three (reference tier). **120° (landed 2026-10-30, `satin-auto-bend-120` and `-nohalf`,
  reference tier):** PE-DESIGN sews 161 stitches: square rungs on both legs, a fan of crossings up
  to 9.6 mm (two band widths) round the bend whose inner ends crowd within 2 mm of the inner
  corner, and its needle points 1/3 mm apart along the whole 47.3 mm outer edge. The derived
  skeleton's rungs swap sides round a bend this sharp, so past 100° (`SHARP_BEND`, between the
  90° references the skeleton lattice matches and this one) Auto satin rebuilds its rungs on the
  outer edge (`region::even_outer`: stations every spacing from the far end along the stretch of
  outline that does not pass the reflex corner, each rung square to that edge), and the corner
  fan fans only corners that turn as the inner corner does (not the end cap's, which lie within
  two widths on this band), keeps the rung end nearer the corner, and ends a crossing longer
  than two widths at the inner corner. pescmp: 273 → 171 stitches to PE's 161, mean needle
  distance 0.36 → 0.23 mm, coverage 0.614 → 0.699; the longest crossing 9.7 mm to PE's 9.6.
  Bends up to 90° and the bent band sew exactly as before.
- **Status:** fixed; the corner fan, end reach, forward start travel, the far-end needle lattice
  and the end on the start point follow PE-DESIGN (reference tier: satin-auto-bend-90-nohalf,
  satin-auto-bend-60, satin-auto-bend-30/45/75 (±nohalf), auto-satin-bent-band;
  `region::fan_corners`, `region::reach_ends`, `region::auto_satin`; 2026-10-28: mean needle
  distance 0.20 → 0.08 mm on the 60° band and 0.16 → 0.07 mm on the 90° band, overlap 0.71 → 0.77
  and 0.70 → 0.87; on the 30°/45°/75° bands PE's start-leg lattice sits 0.07-0.27 mm off ours,
  a phase the far end's fan sets that two bends per angle cannot pin down; a density-3 satin's
  overlap turns on such 0.1 mm phases). Earlier: Auto satin
  derives a skeleton and sews along it, each row reaching the outline
  (`skeleton::rungs_within`). Running Stitch Path is hidden with Auto satin, which travels
  inside (item 3); OpenStitch has no Half Stitch or Stitch Type for regions. Fill, regions with holes or
  several parts, gradation, feathered edges, split stitches and shapes the skeleton does not
  cover (85 %) keep the principal-axis angle. Test:
  `auto_direction_satin_follows_a_bent_band`.
- **Test:**
  - Inputs: a 90° bent band 5 mm wide → `object.setRegion {type:"satin",autoDirection:true}`.
  - Assert: the stitch angle is within ±10° of perpendicular to the local band axis at 5
    stations along it. The current code fails at the bend.
- **Fix:** For Auto, derive a skeleton (`skeleton::derive`) and sew along it with
  `skeleton::satin`, once item 1 is fixed. Apply the guide's exclusions.

## 8. Underlay model differs; zigzag lines have no underlay

- **Impact:** severity 4. PE-DESIGN's under sewing (Edge only / Single / Double, at Dense /
  Medium / Light) cannot be reproduced:
  - OpenStitch's Double is two crossing ±45° layers.
  - Zigzag outlines have no underlay at all.
- **Settled 2026-10-27/28 from the under sewing sweep (`sweep-satin-us-*`, `sweep-fill-us-*`):**
  PE-DESIGN 11.50's Region sew pane offers two type buttons (the second's icon a double layer
  of crossing rows) and a Density of Dense / Medium / Light; no spacing or inset fields. The
  first type is modelled (`EdgeFill` for fill, `EdgeZigzag` for satin: the edge run 1 mm
  inside, rows 2 mm apart). The second type is modelled since 2026-10-28 as `Cross`
  (`underlayStyle: cross`): the same edge run, then two layers of lines 1 mm inside the
  outline at the sewing direction minus and plus 45°, 2 mm apart (2.83 mm along the edges),
  in 4 mm stitches (a line of exactly 4 mm in two), each layer sewn in two halves from the
  region's far corners towards the start point (the half whose corner comes first along the
  outline from the start point first), straight travel to the corner at 2.5 mm, walks along
  the inset outline at 2 mm, back to the start point at the end; specs sweep-satin-us-type-2
  (395 stitches to PE's 404) and sweep-fill-us-type-2 (2650 to 2651) at the reference tier.
  **Density (2026-10-29, #124):** Dense / Medium / Light are 1.4 / 2.0 / 3.5 mm nominal; PE's
  rows lie on both inset edges with the whole number of gaps nearest to the inset width over the
  nominal (38 mm: 27 gaps of 1.407, 19 of 2.0, 11 of 3.455), modelled as `underlayDensity`
  (`region::inset_rows`; specs sweep-fill-us-density-dense/-light at the reference tier). PE's
  type buttons are exclusive radio buttons (pe-refs-4, round 4: a Satin region enables all five,
  Fill the first row only), so the PE-compatible attribute stays the exclusive `underlayStyle`
  (plus `underlayDensity`); underneath, each style is a set of parts (`underlayParts`: edge,
  zigzag = rows across the direction, lattice, center = piping), and setting the parts
  directly, in any combination, is OpenStitch's own extension with no PE reference.
  OpenStitch's styles from before PE's types were measured (`edge`, `double`, and `zigzag` /
  `edgeZigzag` on fill) keep their original geometry unless parts are set, so saved designs
  sew as before (the badge golden is unchanged). **Manual Punch columns (round-4 captures `satin-col-us-off/-type-1..5`, a
  39.5 × 6 mm straight satin column at 4.5 lines/mm, Medium):** PE sews the column's under
  sewing 0.5 mm inside the rails and the end rungs: the edge run in equal stitches ≤ 2.5 mm
  (39 stitches with the travel from the start point), the piping down the middle (16 × 2.41 mm)
  and a zigzag between the inset rails from the far end back, its stations 38.5 / 19 = 2.03 mm
  apart and every 5.4 mm diagonal split into stitches ≤ 4 mm (42 stitches); the captures show
  edge + piping + zigzag for the first row's two buttons (type 1, the default, and type 2:
  286 stitches, the parts in either order), the edge alone for the second row's outline and
  lines buttons (types 3 and 5: 225) and piping + zigzag for its zigzag button (type 4: 249).
  Modelled in `column::underlay` for a column with several direction lines, the styles mapped
  by `UnderlayStyle::column_parts` (edgeZigzag / double → types 1-2, edge → 3 and 5, zigzag →
  4): 275 / 219 / 239 stitches, IoU ≥ 0.999 against the captures. Specs `satin-col-us-type-1..5`
  and `satin-col-us-off` draw the column with `shape.column` (lines (10,10)-(10,16) and
  (49.5,10)-(49.5,16)), as PE's files are Manual Punch blocks (until 2026-10-29 they drew a
  Rectangle-tool shape, a region with a split satin PE does not sew here); type 5 and the
  plain column, parked by pe-refs-4 at 292 and 207 stitches, landed at 219 / 225 and 186 / 186.
  Still open: which of the two types PE applies to the other region kinds; the zigzag's exact split
  points (PE's follow a 12-diagonal cycle, OpenStitch splits each diagonal evenly).
- **Module:**
  - `openstitch-design/src/model.rs:613-625,822`: the styles and a 2 mm spacing.
  - `openstitch-design/src/region.rs:15,120-156`: satin underlay runs parallel to the top
    stitches, with a 0.5 mm inset.
  - `LineSewing` (`model.rs:928+`) has no underlay field.
- **Evidence:**
  - Guide, [region sewing attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293518.html):
    the types and the context-dependent layers (edge run, zigzag-shaped, piping), with three
    densities.
  - Guide, [line sewing attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293378.html):
    zigzag has under sewing On/Off.
- **REA:** none.
- **Grade:**
  - The option set: documented difference.
  - Spacing and inset: needs reference.
- **Test:**
  - Inputs: a 40×20 mm rectangle in satin and in fill, for each underlay type × density; and a
    3 mm zigzag line with under sewing on.
  - Assert: the pass count and layer type match the guide for the context.
  - Record the spacing and inset against reference files to ±0.1 mm.
- **Fix:**
  - `underlay: {kind: EdgeOnly|Single|Double, density: Dense|Medium|Light}`, mapped per
    context to layers in `region.rs`.
  - `LineSewing.underlay` adds a centre run plus a narrow zigzag before `generate::zigzag`.
  - Calibrate the spacings with method 1.

## 9. PE-DESIGN-made PES files lose objects, properties and colours on re-save

- **Impact:** severity 4. Opening a user's own PE-DESIGN file and saving it again, even with
  no change, removes:
  - every editable shape and text;
  - Design Property data;
  - hoop and page settings;
  - thread chart identity.
  Files with programmable fills, motifs or feathers also lose their exact RGB colours.
- **Module:**
  - `openstitch-engine/src/commands.rs:2062-2070` and `openstitch-design/src/model.rs:510-521`:
    import keeps only stitches and threads.
  - `openstitch-formats/src/pes.rs:176,181,186,187-191,197,200`: skipped fields, and the thread
    table abandoned on fills, motifs or feathers.
- **Evidence:**
  - Spec: pyembroidery documents and round-trips these fields, and OpenStitch's reader already
    parses most of them.
- **REA:** none.
- **Grade:**
  - Lost metadata: documented difference.
  - Pass-through: needs no field knowledge.
- **Test:**
  - (a) `oracle_v6.pes`, with author and keywords set by `tools/genfixtures.py` → `stitches.import`
    → `design.export pesVersion:6` → `pes::read`. Metadata and threads equal the input, exact.
  - (b) With an owner file: re-export unchanged. The object section is byte-identical.
- **Fix:**
  - Carry metadata and threads on the imported object.
  - Keep the original object-section bytes plus a hash of the stitch stream. If both are
    unchanged at export, write the bytes back and recompute the PEC offset only. Otherwise
    warn and fall back.
  - Since 2026-10-29 the `#PES0110` writer (item 5) writes back everything the reader parses
    (`CEmbLine` shapes, texts, `CEmbOne` stitch groups, thread table with its names, groups, the
    header's page and hoop fields verbatim), so once the import turns the reader's shapes
    into design objects (ROADMAP item 6) a PE-DESIGN file of shapes re-saves with its objects.

## 10. Hoop and Design Page fields are hard-coded in PES

- **Partly fixed (2026-10-29, #125):** `design.export` writes the selected hoop's width and
  height (rotation applied; the custom page when no hoop is chosen) in mm as the v6 header's
  hoop fields, where pyembroidery's constant 100 × 100 stood. PE-DESIGN 11.50 stores the hoop
  the same way in its own `#PES0110` files (u16 100, u16 100, then the hoop name "nv880e-S"
  for a 100 × 100 mm page, in all 163 owner-drawn reference files). The page, grid and flag
  fields keep pyembroidery's values, and v1 still writes its fixed hoop. Over-size designs
  (round-4 capture `hoop-oversize-100.dst`): PE-DESIGN neither rejects a 129 × 105 mm design
  on a 100 × 100 mm page nor promotes it to a larger hoop; Save As warns and saves on Yes,
  Export writes silently. OpenStitch writes it and returns a warning, `fitsPage: false` and
  the smallest Brother hoop it fits (`smallestHoop`); the editor shows the warning. Not
  implemented from #125, against the reference: per-machine colour-change and data-size caps
  that reject an export, and automatic hoop promotion. Also against the issue: the two ASCII
  characters after the header's u16 are "10" in PE's own 0110 files and "01" in files it
  re-saved from OpenStitch exports, all on 100 × 100 mm pages, so they are not the
  "01"/"02"/"10" size code the issue describes; OpenStitch keeps pyembroidery's "02".

- **Impact:** severity 4. Every v6 file claims a 100×100 hoop and a fixed page, whatever hoop
  the user chose. Every v1 file claims 130×180. Imported files never set the hoop.
  - The guide ([Design Page settings](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/03_1245955.html))
    says the page choice changes the needle start point and what the machine's layout screen
    allows.
- **Module:**
  - `openstitch-formats/src/pes.rs:370-371,430,435-443,490-493`: constants.
  - `openstitch-design/src/machine.rs:241-250`: the hoop is known but never reaches the writer.
  - `pes.rs:176`: the reader skips these fields.
- **Evidence:** Spec: EduTechWiki names the v6 header fields in order. Their units and
  indicator codes are not given.
- **REA:** none.
- **Grade:** Documented, unverified. The value encodings need owner files (method 2).
- **Test:**
  - Inputs: `machine.set` with a 130×180 hoop, then a custom 300×200 hoop, then `rotate90`;
    50 mm shape → `design.export pesVersion:6`.
  - Assert: the header fields at their EduTechWiki positions hold the chosen hoop and the flags,
    and the reader round-trips them.
- **Fix:**
  - Add `PesPage {hoop, rotated, custom, page, sewing_area, offset}` to `WriteOptions`, filled
    from `Design.machine`/`Design.page`. Parse the same fields on read.
  - Keep pyembroidery's constants as the fallback until owner files confirm the encodings.

## 11. No Connection Points (Default / Closest / Farther) for text

- **Impact:** severity 4. Users can't choose short joins between letters (fewer trims) or long
  ones (easy to trim by hand). Each letter's start and end is left to the fill generator.
- **Module:**
  - `openstitch-design/src/compile.rs:64-84,263-289`: glyph regions first, then all outlines;
    travel by distance only.
  - `openstitch-design/src/route.rs:217-246`: entry and exit are object-wide.
- **Evidence:** Guide, [connection points](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/05_1817518.html):
  three settings. They are not available for TrueType, user-defined or small-text fonts,
  multi-colour text, or text whose characters are not sewn in a continuous order.
- **REA:** none.
- **Grade:** Documented difference. The exact join geometry needs a reference.
- **Test:**
  - Inputs: an embroidery font (`fonts.convertTrueType`) with "AVA" at 20 mm, for each
    `connection` value.
  - Assert: total join length closest ≤ default ≤ farther, and trims(closest) ≤ trims(default).
  - The setting is refused for TrueType and multi-colour text.
- **Fix:**
  - Add `connection: Default|Closest|Farther` on text. In `compile::elements`, pick each
    glyph's entry and exit on its outline nearest to, or farthest from, its neighbours.
  - Sew each glyph's region and then its outline, unless a reference shows otherwise.

## 12. No minimum font sizes or small-text fonts

- **Impact:** severity 4.
  - Text of any size from 1 to 300 mm is accepted without a warning.
  - 3–6 mm text is sewn as an ordinary fill, which puckers and fills in.
  - PE-DESIGN gives each font a minimum size and has a separate small-text class, with no trims
    between characters and restricted attributes.
- **Module:**
  - `openstitch-design/src/text.rs:23-52`: the font list.
  - `openstitch-engine/src/commands.rs:1564,1610,1658`: the 1–300 mm clamp.
- **Evidence:** Guide:
  - [font list](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1169793.html):
    a minimum size per font;
  - [small-text list](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1171127.html):
    3–6 mm recommended;
  - [Small Text](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/05_1811349.html):
    the disabled attributes.
- **REA:** none. Brother's fonts and size tables must not be copied; take minimum sizes from
  testing our own OFL fonts.
- **Grade:** Documented difference.
- **Test:**
  - `fonts.list` reports `minSize` per font.
  - `text.add {"size":4}` below the minimum returns a warning.
  - With a small-text font, `object.setRegion` and `text.setNameDrop` are refused, and there are
    no trims between characters.
- **Fix:**
  - Add `min_size`/`recommended_size` and a `small_text` font class to `fonts`/`.osfont`.
  - Enforce them in the engine, and turn jump trimming off between small-text glyphs.
  - Add an OFL or OpenStitch-made small-text run-stitch font.

## 13. No per-character colour or multi-character selection

- **Impact:** severity 4.
  - Multi-colour names and initials are a core workflow, and letters can't be coloured or sewn
    differently.
  - Point Edit selects one character at a time.
- **Module:**
  - `openstitch-design/src/text.rs:141-158`: `CharEdit` holds x, y, rotation and size only.
  - `openstitch-design/src/compile.rs:278-281`: one thread per object.
  - `openstitch-engine/src/commands.rs:1641-1661`: a contiguous `index..index+count` range.
  - `openstitch-app/src/editor/tools/characters.rs:216`: single selection.
- **Evidence:** Guide, [editing individual characters](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/05_1904502.html):
  box, Ctrl-click or Shift+arrow multi-select; thread colour and sewing attributes per character.
- **REA:** none.
- **Grade:** Documented difference. Whether PE-DESIGN groups same-colour letters in the sewing
  order needs a reference.
- **Test:**
  - Inputs: `text.add {"text":"ABC","size":20,"fill":"Black"}` →
    `text.setCharacter {"id":1,"indices":[0,2],"fill":"Red"}` → `design.generate`.
  - Assert: thread order Red, Black, Red with 2 colour changes; `rotation:20` with indices
    `[0,2]` turns only A and C.
  - App test: dragging a box over A and B selects both.
- **Fix:**
  - Add per-character `region_thread`/`line_thread` to `CharEdit` and per-glyph threads in
    `compile::elements`.
  - Accept `indices` in `text.setCharacter`.
  - Make the selection in `characters.rs` a set, with box, Ctrl-click and arrow keys.

## 14. Programmable fill defaults to 0° instead of 45°

- **Impact:** severity 3. New programmable fills tile horizontally, while PE-DESIGN's default
  is 45°.
- **Module:**
  - `openstitch-design/src/model.rs:764`: `angle: 0.0` for every type.
  - `openstitch-engine/src/commands.rs:1048-1050`: a type change keeps the old angle.
- **Evidence:** Guide, [region sewing attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293518.html):
  the programmable fill example is captioned "45° (default)".
- **REA:** none.
- **Grade:** Documented difference.
- **Status:** fixed for new regions, internal regression tier only (not reference verified).
  `RegionSewing::new` gives programmable fill 45° and every other type 0°; `object.setRegion`
  on an object without a region starts from the requested type's defaults. Not changed: a
  type change keeps the old angle (a default-filled rectangle switched to programmable fill
  stays at 0°); whether PE-DESIGN does the same needs a reference. Test:
  `programmable_fill_defaults_to_45_degrees` (an outline-only 40×40 mm rectangle).
  Reference verified (2026-10-27): PE-DESIGN 11.50's default programmable fill
  (compat/reference/stitches/programmable-fill-45.pes) has its dominant direction at 135° in
  the file (y down), because PE-DESIGN's angles turn counter-clockwise on screen. Since `.osd`
  version 2 OpenStitch stores every direction angle that way too (version-1 files are
  migrated on load), so the stored 45° sews at 135° in the file, as PE's does.
- **Test:** `shape.rectangle 40×40` → `object.setRegion {type:"programmableFill"}`. Assert the
  stored angle is 45 and the dominant row direction in the file is 135° ±1° (compat
  programmable-fill-45, reference tier).
- **Fix:**
  - Give each sew type its own default angle in `RegionSewing::new`.
  - Whether a type change keeps the angle needs a reference; leave it as is until then.

## 15. Missing fill, satin and zigzag attributes; wrong ranges

- **Impact:** severity 3. These PE-DESIGN settings can't be set or reproduced:
  - Fill Frequency (fixed ⅓ stagger) — fixed; since 2026-10-29 (#123) the regular Frequency
    brick is one of two stagger strategies, the other OpenStitch's seeded random stagger
    (`stagger: random`, `staggerSeed`), which has no PE-DESIGN counterpart: PE's Fill pane
    offers the Frequency field only (every round-2 sweep's MANIFEST settings list its fields);
  - Stitch Type (turning ends);
  - Half Stitch;
  - zigzag Sharp Corners / Miter limit.
  Two ranges are also too wide: Frequency accepts 0–100 % and E/V run times accept up to 6.
- **Module:**
  - `openstitch-design/src/generate.rs:143` and `region.rs:279`: `phase = (row % 3)/3`.
  - `openstitch-engine/src/commands.rs:1140`: 0–100.
  - `commands.rs:1294-1306`: run times up to 6 for E/V.
- **Evidence:** Guide:
  - [region attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293518.html):
    Frequency 0–50 %, Stitch Type, Half Stitch.
  - [line attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293378.html):
    Miter limit; run times 1–5; E/V stroke times 1–3; Chain Time 1–6.
- **REA:** none.
- **Grade:**
  - Missing options and ranges: documented difference.
  - Turning-end and half-stitch geometry: needs reference.
- **Status:** partly fixed, internal regression tier only (not reference verified).
  - Fill `frequency` (0–50 %, the needle shift from row to row) feeds `generate::row_phase`
    in plain and shaped fill. New fills take PE-DESIGN's default, 30 % (sweep references: the
    Frequency field shows 30 and rows shift 1.2 mm per 4 mm stitch); a file without a Frequency
    (saved before) keeps the ⅓ stagger, so existing designs sew as before.
  - `object.setRegion` refuses Frequency outside 0–50 (all types that use it);
    `object.setLine` limits run times to 1–5 for E/V and every other type, 1–6 for chain.
  - Fill and satin Density take PE-DESIGN 11.50's 0.1–14.0 lines/mm (was 0.5–10), and Pull
    compensation stays 0–2.0 mm, as PE's fields clamp (sweep references, 2026-10-27). Issue
    #139 (2026-10-29) asked for a 0.2 floor; the references contradict it: the MANIFEST entries
    for `sweep-fill-density-min.pes` and `sweep-satin-density-min.pes` record "typing 0 shows
    0,1 (min 0.1)", both files sew at 0.1 lines/mm and pass at the reference tier, and the
    "0,15 rounds to 0,2" note of `sweep-satin-density-0p2` is the field's one-decimal rounding.
    The floor stays 0.1; `attribute-ranges` now checks 0.1 and 14.0 accepted, 0.09 and 14.1
    refused, for fill and satin (0.4 refused for the 0.5–10 types). OpenStitch accepts finer
    values than PE's one-decimal field (0.15 is sewn as 0.15, not rounded to 0.2). The
    `sweep-fill-density-*` and `sweep-fill-pull-*` specs check rows 1/density apart from edge to
    edge and each row end moved out by the pull compensation (the under sewing is not), against
    PE-DESIGN's files.
  - Fill row structure as PE-DESIGN sews it (sweep references, stitch-sweeps track): the rows
    on each side of the start point are sewn as two halves from the far edge inward, the first
    row out and back when a half has an odd number of rows, rows sewn backwards without edge
    points (diagonal joins), straight travel in stitches up to 2 mm, a 0.3 mm tie-in and lock,
    the spacing stretched so the last row lies on the far edge, and the needle grid counted from
    the first row's start. Convex fills match PE-DESIGN's files needle point for needle point
    (`sweep-fill-*` specs, `same-stitches`); regions with several spans per row (U, holes) keep
    the older nearest-row order with PE's row ends and grid.
  - Tests: `frequency_and_run_times_follow_the_guide_ranges`,
    `fill_frequency_sets_the_row_to_row_needle_shift` (0, 25 and 50 %, ±2 % of the stitch).
  - Not implemented, needs reference geometry (method 1): Stitch Type (turning ends), Half
    Stitch for regions, and zigzag Sharp Corners / Miter limit.
- **Test:**
  - `object.setRegion {type:"fill",frequency:60}` and `object.setLine {type:"eStitch",runTimes:6}`
    are refused.
  - Fill a 40×40 square at frequency 0, 25 and 50. The row-to-row needle shift equals the
    frequency ±2 % of the stitch length.
- **Fix:**
  - Add `frequency` to Fill, fed into `tatami_points`.
  - Clamp the ranges.
  - Add `stitch_type`, `half_stitch` and `miter_limit`, with geometry from method 1.

## 16. Optimize Sewing Order does not move entry and exit points

- **Impact:** severity 3. After optimising, travel between same-coloured objects is longer than
  in PE-DESIGN, giving more and longer jumps and trims.
- **Module:** `openstitch-engine/src/order.rs:168,175-224`: nearest object by bounding-box
  centre; `entry`/`exit` are never changed. (The nearest-object sort was itself a difference:
  see the PE-DESIGN behaviour below.)
- **Evidence:** Guide:
  - [optimise order](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/04_1827935.html):
    rearranges the order **and** entry/exit points so neighbours connect at their closest
    points;
  - [options](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/15_1162908.html):
    the "Optimize Entry/Exit Points" setting.
- **REA:** none.
- **Grade:** Documented difference.
- **Test:**
  - Inputs: three same-colour 10 mm squares at x = 0, 50, 25, in that drawing order, under
    sewing off → `order.optimize` → `design.generate`.
  - Assert (corrected 2026-10-27 after the PE-DESIGN reference): the order stays 0, 50, 25, and
    each jump equals the gap between outlines in that order (40 and 15 mm ±0.5). The first
    version of this item assumed the optimiser also sorts same-colour objects left to right
    (0, 25, 50); PE-DESIGN 11.50 does not.
- **Fix:**
  - After reordering, set each entry to the outline point nearest the previous exit and each
    exit to the point nearest the next entry.
  - Add the persistent option, applied when shapes are created.
- **PE-DESIGN 11.50 behaviour** (black-box, 2026-10-27; reference
  `compat/reference/stitches/optimize-entry-exit.pes` and its MANIFEST notes):
  - Optimize Sewing Order groups colours but keeps drawing order within a colour. It does not
    sort same-colour objects by distance. Red 0, blue 50, red 25 becomes red 0, red 25, blue 50.
  - It moves the exit of each object and the entry of the next to the facing points, but only
    between neighbours of the same colour. Across a colour change it leaves the default points.
  - Options ▸ Optimize Entry/Exit Points (on in the reference install) does the same linking
    as each shape is drawn; clicking Optimize Sewing Order with it off gives the same stitches.
  - With under sewing on (its default), each move is about 1.3 mm longer, because the under
    sewing starts 1 mm inside the outline. The spec turns under sewing off to keep that out.
- **Status:** fixed at the reference tier (spec `optimize-entry-exit`). `order.optimize` keeps
  drawing order within a colour and sets each exit and the next same-colour entry to the
  closest points of the two outlines; Options ▸ Optimize Entry/Exit Points (`options.set {optimizeEntryExit}`, user
  settings, default off — the PE-DESIGN default is unverified) links each new shape the same
  way. Tests in `openstitch-engine/tests/order_holes.rs`:
  `optimize_moves_entry_and_exit_to_the_closest_points`,
  `optimize_entry_exit_option_persists_and_applies_to_new_shapes`,
  `optimize_keeps_colour_groups_and_overlap_order`. The squares keep their order 0, 50, 25, and
  the jumps measure 40.0 and 15.0 mm, as in PE-DESIGN (50.0 and 25.0 without linking).
- **Issue #137 (2026-10-29):** the issue asked for a 2-opt minimum-jump order in place of the
  greedy chaining. PE-DESIGN 11.50 does neither: every capture (`optimize-entry-exit`,
  `machine-rbr`, `machine-rbr-far`, `machine-rbgb`) shows colours grouped in order of first
  use and the drawing order kept within a colour (0, 50, 25 stays 0, 50, 25 with 40 and
  15 mm jumps, where 0, 25, 50 would jump 15 and 15), so the default mode stays PE-DESIGN's
  and the specs stay green. The 2-opt optimiser is an explicit option beyond PE-DESIGN:
  `order.optimize {mode: minJump}` or the design's `optimizeOrder` setting
  (`design.setProperties`, saved in `.osd`; editor: the mode box beside Optimise). It keeps
  PE-DESIGN's colour blocks and overlap order, arranges each block by a greedy nearest-free
  walk from the previous block's last object and then 2-opt reversals of the open path until
  none shortens the outline-to-outline travel (never longer than the greedy order by
  construction), and links the entry and exit points of every neighbouring pair, colour
  changes included. Tests: `openstitch-engine/tests/order_min_jump.rs`. pe-refs-4's capture
  for the issue (round 4, `order/six-before.pes` and `six-after.pes`: six scattered squares in
  three colours before and after Optimize Sewing Order) has an identical stitch stream in both
  files, only four header bytes differ: PE-DESIGN reordered nothing and moved no points, as
  this item predicts.

## 17. Hole sewing is not validated or paired

- **Impact:** severity 3. OpenStitch accepts hole sewing that PE-DESIGN refuses, and cuts every
  later overlapping region out of the first object. That behaves like Remove Overlap, so
  coverage differs.
- **Module:**
  - `openstitch-engine/src/commands.rs:1341-1355`: only checks that a region exists.
  - `openstitch-design/src/compile.rs:249-261`: `holes_above` subtracts every later closed
    region.
- **Evidence:** Guide, [hole sewing](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/04_1836721.html):
  - applies only to a pair where one region fully encloses the other;
  - refused for self-crossing outlines or multiple direction lines;
  - feathered edges are unavailable with it
    ([attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293518.html)).
- **REA:** none.
- **Grade:** Documented difference.
- **Test:**
  - A 40 mm pentagon with a 20 mm star inside → `object.setHoleSewing {ids:[1,2],enabled:true}`.
    The stitched area equals pentagon minus star (±2 %).
  - A partial overlap, a self-crossing outline, or ≥2 direction lines are refused.
  - `featherWidth` on a hole-sewn object is refused.
- **Fix:**
  - Store the inner object's id on the outer object.
  - Validate full enclosure with `boolean::combine`, and check for self-crossing and the
    direction-line count.
  - Subtract only the paired shape.
- **Status:** fixed at the internal-regression tier (not verified against PE-DESIGN output).
  The outer object stores `holeInner`; `.osd` files saved before pair with the first enclosed
  region sewn after. Tests in `openstitch-engine/tests/order_holes.rs`:
  `hole_sewing_cuts_out_only_the_paired_shape` (measured 829.1 mm² vs pentagon − star
  833.5 mm²), `designs_saved_before_pairs_cut_out_the_enclosed_region_only`,
  `hole_sewing_is_refused_outside_the_guide_limits`.

## 18. PES thread table drops chart names and colour type

- **Status (2026-10-19): fixed for round trips, internal regression.** `Thread::chart` and
  `Thread::kind` are read and written back; OpenStitch's own colours still get type `0x0A` and
  an empty chart. `pes_v6_thread_chart_and_kind_round_trip_byte_identically` rewrites
  `oracle_v6_chart.pes` (pyembroidery) byte for byte. Still open: chart strings for
  OpenStitch's colours (owner files) and the meaning of the type codes.

- **Impact:** severity 3.
  - Every thread is written as custom colour (`0x0A`) with an empty chart.
  - Re-saving a PE-DESIGN file erases its brand charts.
  - Appliqué markers may not be recognised.
- **Module:**
  - `openstitch-formats/src/pes.rs:195-206` (read: type and chart discarded) and
    `pes.rs:451-458` (write: `0x0A`, empty chart).
  - `openstitch-core/src/lib.rs:51-61`: `Thread` has no chart or type fields.
- **Evidence:**
  - Spec: pyembroidery reads and writes `thread.chart`.
  - EduTechWiki lists type codes (1 list colour, 2 appliqué, A custom, B Brother), with one
    inconsistency.
- **REA:** none.
- **Grade:**
  - Chart string: documented difference.
  - Type codes: unverified.
  - Expected chart strings: needs owner files.
- **Test:** `tools/genfixtures.py` writes `oracle_v6_chart.pes` with chart, brand, catalogue
  and description set. `pes::read` → `write_with(V6)` must be byte-identical to the input.
  Today it fails at the chart byte.
- **Fix:**
  - Add `chart` and `kind` to `Thread` (serde-optional), and read and write them.
  - Default `kind` to `0x0A`.
  - Set chart strings from OpenStitch thread charts only after owner files confirm them.

## 19. Design Property metadata cannot be set; lost on import

- **Status (2026-10-19): fixed, internal regression.** `Design.properties`,
  `design.setProperties` (≤ 255 bytes per field, no control characters except line breaks and
  tabs), compiled into `StitchPattern.metadata`, kept in `.osd`, offered and adopted on
  `stitches.import` (`adoptProperties`), editable in Application menu ▸ Design Property.
  Tests: `pes_compat::design_properties_*`, app `design_property_edits_apply_as_one_undo_step`.

- **Impact:** severity 3.
  - PE-DESIGN users fill in category, author, keywords and comments, which Design Database
    searches.
  - OpenStitch writes only the name, and drops the other four on import.
- **Module:**
  - `openstitch-design/src/model.rs:15-40`: no properties on `Design`.
  - `openstitch-design/src/compile.rs:139`: only the name reaches the writer.
  - `openstitch-formats/src/pes.rs:212-219,432-434`: read and write already exist.
- **Evidence:**
  - Guide, [Design Property](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/04_1911437.html).
  - Spec: the field order in pyembroidery and EduTechWiki.
- **REA:** none.
- **Grade:** Documented difference.
- **Test:**
  - `design.new {name:"Rose"}` → `design.setProperties {category,author,keywords,comments}` →
    shape → `design.export pesVersion:6` → `pes::read_objects`. All five strings are present,
    byte-exact.
  - The properties survive an `.osd` round trip and are restored on import.
- **Fix:** Add `Design.properties` (serde-default), a `design.setProperties` command, a Design
  Property dialog, and copy the properties into `StitchPattern.metadata` in
  `Compiler::compile`.

## 20. Character kerning moves one glyph without reflowing the line

- **Impact:** severity 3.
  - In PE-DESIGN, dragging a character sideways sets its kerning in mm, a spacing change.
  - In OpenStitch it shifts only that glyph, so letters can overlap and centring doesn't
    update.
  - The text-wide `kerning` flag is a different setting: it turns the font's kerning table on
    or off.
- **Module:**
  - `openstitch-design/src/text.rs:836-883`: `Run::along` applies `e.x` after the pen advance.
  - `text.rs:1068-1072`: alignment.
  - `text.rs:102-105`: the flag.
- **Evidence:** Guide, [text attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/05_1810676.html)
  and [character size and attributes](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/05_1844109.html):
  per-character kerning in mm, default 0, set by a horizontal drag.
- **REA:** none.
- **Grade:** Documented, unverified. Whether later characters reflow needs a reference.
- **Test:**
  - Inputs: `text.add {"text":"ABC","size":20,"align":"left"}` →
    `text.setCharacter {"id":1,"index":0,"kerning":5}`.
  - Assert: B and C shift +5.0 mm and the width grows 5.0 mm (±0.05).
  - With `align:center`, everything shifts −2.5 mm.
- **Fix:**
  - Add `CharEdit.kerning` (mm), added to the pen advance so it counts in the run length.
  - The editor's horizontal drag writes `kerning`.
  - Rename the text flag in the UI to "Use font kerning".

---

## Next 11 (lower impact or reference-bound)

- **Uncalibrated numeric defaults:** cross-cutting, high impact, but needs method 1.
  - Values: satin 3.0 rows/mm, stitch length 4 mm, pull compensation 0, underlay spacing 2 mm,
    line pitch 2.5 mm (stem, candlewick, chain and motif lines; still open).
  - Lines calibrated from PE-DESIGN 11.50 reference files (2026-10-27, `line-*-default` specs):
    running and triple 2.0 mm, zigzag 2.0 mm wide at 4.5 lines/mm with under sewing and half
    stitch on, E/V strokes 2.5 mm every 5.0 mm at 2.0 mm run pitch. Switching the line type
    gives the new type's defaults, as PE-DESIGN does. Lines tie in and lock like a satin. PE's
    zigzag is sewn between mitred rails, matched along each leg. Its V puts a V at every
    multiple of the spacing, with arms to ±1/5 of it.
  - Then stem, candlewicking and chain (`line-stem-default`, `line-candlewick-default`,
    `line-chain-default`, each matching PE's needle points):
    - Stem: 2.0 mm wide, stations every 1.5 mm per leg, 45°, each sewn top, bottom, top,
      bottom 0.14 mm apart.
    - Candlewicking: 5.0 mm knots 1.0 mm apart per leg; a Medium knot is a {24/11} star.
    - Chain: 4.0 mm triangles sewn 5 times, every 0.75 × size per leg.
    - V, stem, candlewicking and chain tie in with three 0.3 mm steps.
  - Still open: the motif line's defaults. PE's default motif is PE-DESIGN's own artwork, so no
    stitch comparison is possible; candlewicking Low and High, and the chain diamond, are not
    measured.
  - Calibrated from the PE-DESIGN 11.50 reference files (2026-10-27): fill 4.5 rows/mm with
    under sewing on (`EdgeFill`: edge run 1 mm inside the outline at <= 2.5 mm, then tatami
    rows across the fill, 2 mm apart, 4 mm stitches, 30 % shift, 1 mm inset), also on for
    programmable fill. Fill Frequency default 30 % as PE (2026-10-27, sweep references). Still
    open: the programmable fill density.
  - The guide gives word scales only. Fit the mapping from reference files at slider minimum,
    middle and maximum.
- **Line Start/End type (Feed/Run) (calibrated 2026-10-27 for stem, candlewick and chain):**
  PE-DESIGN 11.50 keeps every element of a closed line where it is without the moved points;
  only where the sewing starts and ends changes. An element belongs to a point when it is the
  last to begin at or before it. The pattern starts with the exit point's (L2) element and
  goes round to the one before it, so it also ends there (a stem sews that station again, a
  candlewick sews that knot last). Feed starts with the pattern; Run starts at the entry
  point's (L1) element and sews only the leads (link points, stem station centres, candlewick
  running stitch) to L2's element, the shorter way round, backwards if need be. No jumps or
  trims. OpenStitch does the same (`object.setLine` `startEnd`, `object.setEntryExit`
  `lineExit`; ten specs line-{chain,stem,candlewick}-startend-*, all matching PE's needle
  points in order;
  [guide](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293444.html)).
  Still open:
  - Motif lines: PE's default motif is PE-DESIGN's own artwork, so no stitch comparison is
    possible; a reference with a simple motif drawn in PE would allow one.
  - Open lines and chain diamonds: not measured; they keep restarting at the entry point.
  - The editor: no L2 marker and no Feed/Run radio yet (the engine has both).
- **Line spacing and text size (calibrated 2026-10-27):** PE-DESIGN 11.50's line spacing is
  "% of char height", default 100; its TrueType size is 0.7 em (Lato Bold's H is 1.033 × size)
  and 100 % sets lines the font's Windows ascent apart. Measured in `compat/reference/text`
  (specs text-size-h, text-size-hg, text-line-spacing-50/100/150, all at the reference tier).
  OpenStitch now does the same; `.osd` files before version 3 keep the old capital-height
  basis.
- **Text sewing defaults (calibrated 2026-10-27):** PE-DESIGN 11.50's text tool sews Satin
  Stitch at density 5.0 with under sewing off and pull compensation 0; switched to Fill it is
  density 5.0, under sewing off, Frequency 30 % (pe-gui, specs text-defaults-h and
  text-defaults-fill). OpenStitch now gives new text the same; saved texts keep their stored
  attributes. Stitch counts still differ: Fill 544 vs PE's 483 (fill rows join with a short
  step along the outline where PE runs straight into the next row; the stitch-sweeps track),
  Satin 221 vs 285 (satin structure).
- **Fit Text to Outline (calibrated 2026-10-27):** PE-DESIGN 11.50's dialog has Horizontal
  Alignment left / centre (default) / right / stretched / spaced evenly, Vertical Alignment
  above (default) / centred / below (by the size, not the capital height), Text Orientation
  along / upright, and "the other side"; a circle's outline starts at its bottom, and only one
  text fits an outline. OpenStitch now does the same: specs text-fit-* (eight files) at the
  reference tier; stretched is implemented but has no comparable reference (PE's file is far
  off the page). Fitted texts take no Text Transform, as in PE.
  ([guide](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/05_1810870.html))
  Read stitch by stitch on 2026-10-28 (text-fit-* and the round-3 size/spacing/AHHHH sweeps):
  the circle's running stitch is PE's Bézier sampling (each quarter arc halved until the chord
  fits the 2 mm pitch: 4 × 32 chords; OpenStitch now matches it point for point, 0.04 mm mean,
  text-fit-default's mean needle distance 0.27 → 0.20 mm; #121, 2026-10-29: the halving stays
  where this reference pins it, a flatness-tolerance polyline resampled at the pitch would move
  every needle point; what changed is the 256-chord ceiling, now 2^16, so long or tight curves
  never keep a chord over the pitch; outlines are flattened adaptively to 0.05 mm as before); the letters sit where the font's
  advance box (no kerning) centred on the path's middle puts them (the A's ink starts at 75.04 mm
  from the circle's start in both), character spacing spreading symmetrically. Still different:
  PE sews every letter stroke by stroke with the rows across each stroke (see Text below), the
  letters after the first drift 0.05–0.07 mm/letter earlier in PE, and PE's round strokes end
  0.2–0.5 mm short of the outline's extreme points (B/C/D 5.91/6.42/6.90 mm wide against the
  font's 6.13/6.98/7.34; A and E match).
- **Text Transform (calibrated 2026-10-27):** PE-DESIGN 11.50 has 26 presets (two arches,
  then a 4 × 6 grid; ticking Transform applies preset 3; fitted text cannot be transformed,
  and fitting cancels it). OpenStitch lays out 24 of them, from envelopes fitted to PE's
  output by `tools/fit_envelopes.py` (specs text-transform-N, reference tier: bounds within
  0.5 mm, every H stem's top and bottom within 0.5 mm). 13 and 14 (fan out / in) since
  2026-10-28: each letter keeps its height and is sheared linearly between one edge of the
  straight box and the other edge scaled by 0.70 about the text's centre (13 shrinks the
  bottom, 14 the top; fitted from the letters' corners); specs text-transform-13/-14 at the
  reference tier by their box. Their stitch counts are 41-43 % above PE-DESIGN's (1653 vs
  1171, 1659 vs 1159) against +14 % on the straight-edged presets: the fill rows across
  sheared stems, for the text-fill item below.
- **Text Transform level (round 4, 2026-10-29):** PE-DESIGN 11.50 has no numeric
  "Transform level": ticking Transform puts one or two green diamond handles on the canvas
  and the strength is set by dragging one (the status bar shows nothing). The value is
  stored in the saved file (a float32 0x7E bytes after the last UTF-16 font name of the
  `CEmbDText` object, with the arch radius in 0.1 mm as the float32 4 bytes later): a
  percentage of the font size for the straight-edged presets (default 50; 20 for the bows
  and waves 5, 6, 15, 16, 19, 20, 23, 24), an angle in degrees for the slants (9, 10:
  default 10) and the fans (13, 14: default 30, the handle moving horizontally), and for
  the arches (1, 2) the half-sweep angle (default 30) with the radius that keeps the text
  length (38.4 mm for "OpenStitch", 30.2 mm for "Agjy 123"); the end-of-arc handle moves
  horizontally (41.2 deg / 27.9 mm to 1.15 deg / 1000 mm = straight), while the two arrows
  at the top scale the letters, not the transform. Clicking a preset resets its value to
  the default. Round 4 recorded every preset on the string "OpenStitch" (round letters,
  ascenders, descenders) and "Agjy 123" (descenders and digits) at the default level, and
  at four other handle positions per preset (OpenStitch only; values per file in the owner's
  notes and the MANIFEST). At the default level OpenStitch's envelopes keep the box within
  0.5 mm on 21 of the 26 presets for "OpenStitch" (specs text-transform-N-openstitch)
  and 18 of 26 for "Agjy 123" (text-transform-N-agjy; the arches 1 and 2 and 18 fail
  there as well, by 0.6-1.3 mm);
  parked until 2026-10-30 (owner's round-4 notes, numbers there): 3 (maxY 0.8 mm low), 9 (minX
  1.1 mm), 10 (maxX 1.4 mm), 14 (minX 1.7, maxX 2.1 mm) and 17 (maxY 0.7 mm): the envelopes were
  fitted on the HHHHHHH box and PE-DESIGN applies them to the cap-height box, so letters below
  the baseline (p, g, j, y) and round tops landed differently; all pass since the cap-height box
  (below). Since 2026-10-30 `text.add` / `object.setText` take
  `envelopeLevel` (the stored value; a new preset resets it, as in PE-DESIGN), which scales
  how far the preset's envelope bows from straight (the slants by the shear's tangent, the
  arches' angle with the radius keeping the text length), and the PES 0110 writer stores it.
  Since the same day the envelope warps the cap-height box (the capital line to the baseline,
  as PE-DESIGN does: the envelopes were fitted on HHHHHHH, whose box it is), so letters below
  the baseline go on below the bottom edge instead of squeezing the envelope: 105 of the 108
  level captures and 25 of the 26 default-level captures on "Agjy 123" (all 26 on
  "OpenStitch") keep the box within 0.5 mm (specs `text-transform-N-openstitch[-lL]`,
  `text-transform-N-agjy[-lL]`). Parked (round4\text, not in the repository): arch 2 on
  "Agjy 123" at the default level (minX 0.6 mm) and on "OpenStitch" at level 25 (minX
  0.6 mm), taper 4 at level 100 (maxX 1.1 mm) and slant 9 at level 100 (maxX 0.6 mm). Checked
  across the levels (2026-10-30), no single rule fixes them: arch 2 sits 0.3-0.6 mm left
  (minX) at every level while its right edge moves from -0.2 to +0.3 mm, so a shift would push
  level 100 out; taper 4 is within 0.1 mm at every level but 100 (the full taper); slant 9's
  right edge is already 0.4 mm short at the default and 0.6 mm at level 100 (the fitted envelope,
  not the level scaling). Left parked.
- **Sewing order inside a text (checked 2026-10-27):** not a gap. PE-DESIGN 11.50 also sews
  every letter's region first, then every letter's outline, one colour block each (spec
  text-sewing-order, reference tier). Still open: the guide disables "some editing of
  individual characters" on user-mapped text without naming them; checking which needs
  characters registered with Brother's tools, which the project does not use.
- **Machine format exports (calibrated 2026-10-28):** PE-DESIGN 11.50's DST, EXP, JEF, SEW,
  XXX, PCS, HUS and VIP exports of one OpenStitch design (`compat/reference/machine`, spec
  `machine-formats`, reference tier) showed the writers differed in how they laid out a frame
  move, a trim, a colour change, a stitch longer than a record and the origin. The writers now
  follow PE-DESIGN record for record (EXP and PCS byte for byte; the rules are tabulated in
  FILE_FORMATS.md "PE-DESIGN's record layout"): the design centred on the stream origin, a long
  stitch as jumps plus one stitch (one needle penetration), a frame move as a jump run with no
  trim record (DST padded to three records, XXX one `7E` record, VIP `88`, HUS a zero-length
  jump), the move's remainder folded into the next stitch in JEF, SEW, XXX and VIP, PCS's fixed
  16-colour table indexed by its colour records, HUS's `0x00C8FC5D` magic. The readers follow:
  a single jump no longer than a record is read as the long part of a stitch, not a trim (JEF,
  EXP), `80 02` is an EXP jump, PE-DESIGN's HUS trim marker and XXX `7E` moves read as trims,
  PCS's index byte and fraction byte are read. SEW's start point (settled 2026-10-28,
  `sew-a-moved10.*`, `sew-square-centre.*`): the stream is relative to the design (PE-DESIGN's
  export of the same design moved 10 mm on the page is byte-identical) and starts 15.9 mm left
  of and 9.7 mm above the design centre, the same for the 40 mm design and the 20 x 20 mm
  square, the way to the first needle point sewn as a stitch and split like one; the writer
  starts there and the reader keeps the move, so the SEW compare is record for record from the
  first one. The spec exports OpenStitch's saved a.pes itself (`stitches.import`), the file
  PE-DESIGN opened, so the writers are compared on exactly PE-DESIGN's input whatever the
  stitch generator does later; an imported stitch block's needle points are exported as
  recorded (a 15 mm stitch stays one penetration, as PE-DESIGN exports the opened file; the
  compiler used to split it at the machine's longest stitch). The square's export also shows
  PE-DESIGN exporting its own objects from their
  first needle point, the end of the PEC's first jump (its tie-in: there, 0.3 mm back, there
  again), where OpenStitch's PES reader starts at the first PEC stitch record: one record more
  in PE-DESIGN's stream, the rest identical (`sew_stream_is_design_relative_and_starts_where_pe_design_does`).
  Still open, each settled by one more export: how the folding formats write a frame move shorter
  than one record; VIP's exact piece length (7.5–7.9 mm); JEF's `-1` hoop block; XXX's header
  tables and palette filler; SEW's 340 header bytes; DST's `AX`/`AY`; PE-DESIGN's RGB for chart
  colour 800 RED (`E3 31 1F`) against the PEC chart table's `ED 17 1F`.
- **Concentric Circle and Spiral (calibrated 2026-10-29, #138):** issue #138 assumed PE-DESIGN
  has no concentric or spiral sewing; it has both as region sew types, with only Density
  (default 2.0), Run pitch (2.0) and two Start Point buttons. Fourteen references
  (`concentric-*`, `spiral-*`: a 40 mm circle at defaults, start 2, density 1.0 and 3.0, pitch
  3.0; a 40 × 30 mm rectangle; a 40 mm ring with a 15 mm hole) show:
  - Concentric Circle: circles about the bounding-box centre at d, 2d, … (d = 1 / density);
    the whole circles from the largest inwards, each from one radial seam, counter-clockwise;
    a trim; the cut circles from the smallest outwards as arcs, back and forth per piece, with
    running travel along the outline. The seam is where the first cut circle meets the
    outline (on the rectangle where the 15.5 mm circle leaves the bottom edge, 75.4°).
  - Spiral: not an Archimedean spiral about a centre but the outline offset a row inwards per
    turn (turns 0.5 mm apart on both axes of the rectangle), from R1 on the outline to the
    middle; per turn 4·2ⁿ needle points (64 a turn down to r ≈ 10 mm on the circle, then 32,
    16, 8), corners kept on the rectangle.
  - The second Start Point button sews the same stitches backwards; neither type travels from
    R1 or to R2.
  - A ring made as Set hole sewing with the inner circle's region Not Sewn keeps the hole
    (OpenStitch dropped a paired hole whose inner shape sewed no region; fixed).

  Status: reference tier, all 14 specs pass. Stitch counts within 3 %, bounds within 0.35 mm,
  mean stitch length within 0.15 mm. Open (count checks kept out of the reference tier, not
  loosened):
  - `spiral-circle40-pitch3p0`: PE 1354 stitches, OpenStitch 1284 (−5.2 %). The count is an
    internal-tier check pinning OpenStitch's 1284 at 3 %; bounds and stitch lengths stay at the
    reference tier. Investigated 2026-10-30, turn by turn: PE's outer turns have 64 needle
    points like OpenStitch's (the 4·2ⁿ rule), but below the radius where the rule halves the
    count PE's turns fall off gradually instead of in powers of two: 64 down to r = 16.2 mm,
    then 56, 44, 38, 39, 40, 40, 36, 34, 33, 32 … 28, 25, 22, 19, 18, 16 against OpenStitch's
    32 from r = 15.3 mm and 16 from 7.7 mm. Inside a turn PE splits some base pieces in half and
    not others (adaptive halving per piece), and the pieces next to the axis directions are
    narrower than the rest (about 8.8° against 11–12° at r = 12 mm, pitch 2), although the turns
    are true circles (radius within 0.04 mm all round): the parameterisation is not uniform in
    angle. Neither a cubic Bézier quarter (11.0–11.6° for 8 equal steps) nor a rational
    quadratic one (10.5–11.8°) reproduces it, so the rule is still open. The same pattern shows
    at pitch 2 (turn counts 60, 62, 55, 60 … in the 64 band; 42, 39, 36 … in the 32 band), where
    it cancels out in the total (within 2 %).
  - `spiral-ring40-hole15`: PE 1952 stitches, OpenStitch 1473 (−24.5 %). The spec compares
    bounds and stitch lengths only. Investigated 2026-10-30: PE sews the outline's spiral from
    19.85 mm in to 13.6 mm (908 needle points), then 31 separate pieces round the middle of the
    band (r 13.4–14.0 mm, 397 needle points, 64 trims), then the hole's spiral from 13.6 mm in
    to the hole (647 points). The pieces are wedges about 5.5° wide repeating every 11.25°;
    each has a tie-in and a lock (0.3 mm back-and-forth), 13 of them nothing else, the others
    up to 37 stitches zigzagging inside their 1.3 mm wedge. They are PE filling leftover slivers
    between its two spirals one by one. OpenStitch does not copy them: 64 trims in a 25 mm²
    band is a defect a user would not want, and the band is covered by the two spirals' last
    turns. Kept as a documented difference.

  Other known differences:
  - Concentric chord splits: inside each circle PE's stitches come as single 2.0 mm stitches,
    pairs of 1.0 mm and triples of 1.33 mm: key spans of about 2 and 4 mm split into
    ceil(length / pitch) equal stitches. Rounding the 4·2ⁿ points to 0.1 mm explains only part
    of it (OpenStitch's counts are within 3 %); where PE's key points come from is open.
  - Spiral winding and ending (settled 2026-10-30, round 5: `spiral-square40`,
    `spiral-rect30x40`, `spiral-rect40x30-drag-reverse`, `spiral-ellipse40x30`,
    `spiral-circle40-drag-reverse`, `spiral-triangle`): the drag direction does not matter
    (identical files); what decides is where the start point lies. On an outline vertex (a
    circle's or ellipse's bottom point, a triangle's bottom-left corner) the spiral turns
    counter-clockwise on screen and ends in the middle. Inside an edge (a rectangle's bottom
    middle) it runs clockwise to the next corner, spirals from there and, after a trim, returns to
    that corner and runs back along the edge to the start point, ending where it began. OpenStitch
    sews both cases that way now (`crate::spiral`, closing run as its own run); all six specs pass
    within 3 %. PE-DESIGN shows no R1/R2 markers for Spiral or Concentric Circle regions, so a
    moved start point cannot be captured. Open: the triangle's first turn reaches further into
    the base corners in PE (39.7 mm wide against 39.0 mm; width not compared).
  - The concentric seam on a Bézier circle stays open. Round 5 adds a 30 mm circle (seam at the
    left edge just above the middle, like the 40 mm one) and a 40 × 30 mm ellipse (seam at the
    top just right of the middle); no single walking rule fits those and the rectangle's 75.4°.
    The new specs `concentric-circle30` and `concentric-ellipse40x30` compare counts, bounds and
    stitch lengths, not the seam.
- **Image digitizing (Phase 6, issues #127, #128, #132, #129):** the Auto Punch, Cross Stitch
  and Photo Stitch pipelines were this project's own approximations; the Phase 6 gap analysis
  pins their arithmetic. Done 2026-10-29 (#128): mono Photo Stitch uses integer Rec. 601
  luminance `(30 R + 59 G + 11 B) / 100` and a full-range min/max contrast stretch
  `g' = (g − min) · 255 / (max − min)`, applied only when `max − min` is in (1, 255), instead of
  the 2nd–98th percentile stretch. On a linear grey ramp the two stretches agree (576 against
  573 needle points for the 50 mm Fill ramp, 464 against 456 for Lines); they differ on images
  with a few outlying dark or light pixels, which the percentile stretch ignored and the
  min/max stretch anchors on. Left: the colour mode's thread snap through the equal-weight
  metric (#119), the sepia re-tone, and the capture comparison (stitch density per tone band
  and total counts against pe-refs-4's Photo Stitch files at defaults) once those captures
  land. Done 2026-10-29 (#127): Auto Punch reduces colours from a census of the pixels' 3×3
  modal colours capped at 100 tracked colours (later colours count under their nearest
  tracked one), removes every 4-connected region of at most `max(4, round(param · 100 /
  mmPerPx²))` pixels (the parameter in mm², so `minArea / mmPerPx²`) into its neighbour, and
  decides satin by a centerline-thinness test (stroke width at the deepest point at most
  `satinMaxWidth`, default 6 mm) instead of the longest scan span across the principal axis:
  a bent 4 mm stroke is now satin, a 10 mm blob fill. Left for #127: the recorded defaults
  and the region, satin and stitch counts against pe-refs-4's Auto Punch captures; the
  optional graph-cut foreground step. Done 2026-10-29 (#132): Cross Stitch reduces colours
  through the same census prepass (one implementation, two callers), its cells vote with their
  modal colours, and its defaults are explicit: 2.5 mm cells (0.4 per mm), 8 colours, full
  crosses, each diagonal once (`times: triple` sews it three times), border colour left out.
  On the synthetic grids the cells are the same as before (8 × 4 cells, 6 + 1 coloured);
  what changes is the palette's weighting. Left for #132: PE-DESIGN's recorded defaults from
  the capture, which may move the cell size or the times.
  optional graph-cut foreground step. #132 follows in its own PR.
  land; #127 and #132 follow in their own PRs.
  Compared with PE-DESIGN 11.50 on 2026-10-29 (round-4 captures from the owner's
  four-shape image `owner-shapes.bmp`, 240 × 240 px placed at 84.7 mm: a red disc, a green
  triangle, a yellow rectangle and a blue square on white, every wizard at its defaults):
  - **Auto Punch** (`autopunch-default.pes`; Max. Number of Colors 5, white omitted): PE
    makes 4 fill regions (none satin; OpenStitch agrees) of 4482 stitches, each starting with
    the 1 mm edge run, at 45° except the triangle at 90°. OpenStitch had 4241 (−5.4 %) at 0°
    (the principal axis); with fill at 45° and 5 colours it has 4387 (−2.1 %): red 1481 / 1485,
    blue 1279 / 1329, yellow 851 / 881, green 776 / 787 (PE's at 90°). Open: PE's colour order
    (yellow, blue, red, green, the same in its Cross Stitch; not area, position or chart order)
    and the triangle's 90°, which one image cannot pin down.
  - **Cross Stitch** (`crossstitch-default.pes`; Cross size 2.5 mm, 5 colours, Times Single):
    2866 stitches. PE's grid matches OpenStitch's (cells red 161 / 162, blue 143 / 143, green
    75 / 74; yellow 94 / 88 at the rectangle's partial edge cells). PE sews six stitches per
    cell, all on the diagonals: each diagonal once whole and once in halves through the cell
    centre (median stitch 1.77 mm). OpenStitch sewed four diagonals and two cell-side steps
    (1907 stitches, −33 %); it now sews PE's six (2841, −0.9 %). Open: PE travels from run to
    run along the diagonals with one jump per colour, where OpenStitch jumps between row runs.
  - **Photo Stitch 1** (`photostitch1-default-plus-cross.pes`, items 5–14; Max. Number of
    Colors 10, Run Pitch 3.0 mm, Sew Page Color ticked): 12 255 stitches in 10 threads, the
    page colour (White) first over the white areas (7212), every tone band sewn at 160–210
    needle points per cm². PE's path is not row-based: contour-following meanders of ~3.2 mm
    runs, 1–5 jumps per colour. OpenStitch had 5235 stitches with the white bare (14 per cm²,
    stray dither tints) and 2 mm stitches; with Sew Page Color on by default, 3.0 mm stitches,
    10 colours and 0.25 mm rows it has 12 860 (+5 %, with #119's colour metric): white 157 / 162,
    yellow 181 / 170, green 234 / 211, red 173 / 175, blue 237 / 176 needle points per cm²
    (OpenStitch / PE). Open:
    the path (OpenStitch trims at every run end, about 1100 times here) and PE's thread choice
    (it mixes two or three threads per colour area); both are the stitch-path model of #129,
    which the patent note in docs/research/beyond/patents.md bears on.
  `openstitch.pattern` fill, motif and decorative patterns. Brother's bundled patterns are
  never read.
- **Programmable fill, motif fill and motif line from owner-made patterns (reference,
  2026-10-29, #130/#131):** PE-DESIGN 11.50's fills of six owner-made Programmable Stitch
  Creator files showed OpenStitch's own model was off: PE-DESIGN tiles the pattern on the page
  axes from the region's top-left corner and sews only the row ends and the crossings with the
  tile lines (rows sharing their turning points, 8 mm gaps unsplit); OpenStitch tiled in the
  row's frame and added a staggered grid (3392 and 3209 needle points against PE-DESIGN's 2355
  and 1638, mean distance to PE-DESIGN's nearest needle point 0.63 and 1.03 mm). Now 2329 and
  1636, within 0.2 mm (mean 0.10 and 0.11 mm); specs `prog-fill-square-4pt`,
  `prog-fill-line-2pt`. Motif fill rows now start half a motif below the top edge (bounds
  40 × 37.5 mm as PE-DESIGN, `motif-fill-2pt`); motif lines fit a whole number of stretched
  motifs (`line-motif-zigzag-3pt`). The import mapped a motif's cross offset at twice its size
  (fixed: 1024 cell units are one motif size). A reference point was ignored for programmable
  and motif fills, and a user pattern fell back to the default with a reference point or a
  curved direction (both fixed). The motif fill's travel is PE-DESIGN's since 2026-10-30: rows
  from the one nearest the start point, travel along the outline in even steps of at most
  2 mm, the fill's tie-in and lock (130 needle points against PE-DESIGN's 130, was 103; spec
  `motif-fill-2pt` now checks the count within 3 %). Left: the motif line's tie stitches (23
  against 16) and which way PE-DESIGN walks when both ends of the first row are as near.
  curved direction (both fixed). The motif fill's travel is PE-DESIGN's since 2026-10-30: rows
  from the one nearest the start point, travel along the outline in even steps of at most
  2 mm, the fill's tie-in and lock (130 needle points against PE-DESIGN's 130, was 103; spec
  `motif-fill-2pt` now checks the count within 3 %). The motif line ties in and locks like
  PE-DESIGN's other lines since 2026-10-30 (23 needle points against PE-DESIGN's 23, was 16;
  spec `line-motif-zigzag-3pt` checks the count within 3 %). Left: which way PE-DESIGN walks
  when both ends of the first motif-fill row are as near.
  Second round of captures, 2026-10-29 (pe-refs-4, inputs drawn by its `wizimg.py`):
  - **Auto Punch bars** (`autopunch-thin-strokes.pes`, nine black 40 mm bars 0.5–4 mm wide;
    `autopunch-thin-strokes-wide.pes`, seven 30 mm bars 5–14 mm; Max. Number of Colors filled
    in as 2): PE makes satin from 0.75 to 6.1 mm (Auto direction, no under sewing, 200–214
    stitches per 40 mm bar whatever the width) and fill for 0.5 mm and from 7.1 mm (under
    sewing on; 90° across the bar up to 12 mm, 45° at 14 mm), widest bar first. OpenStitch now
    satins 0.7–6.5 mm strokes with Auto direction and no under sewing, fills across the stroke
    up to 13 mm and at 45° beyond, and sews each colour's regions largest first: the same
    choices on all 16 bars, 2011 / 3100 stitches against 2018 / 3197. Before, its working copy
    averaged the bar edges into greys that became extra threads and thin grey regions (3446 /
    4639 stitches, 2 and 4 threads). The working copy now keeps each box's most common colour,
    and the colour count is automatic as PE's is (2 for the bars, 5 for the shapes).
    At other working resolutions the choices did not hold (checked 2026-10-30): the chamfer
    width read an even-width stroke one pixel short, so the 0.75 mm bar turned fill at 300,
    350, 500, 700, 800 and 960 px (the 1 mm bar too at 300) and the 7 mm bar satin at 250 px;
    only 400 and 600 px matched PE. `regions::thickness` now adds the pixel when the deepest
    pixels form a two-pixel ridge (a stroke centred between two pixel rows), and every
    resolution from 250 to 960 px gives PE's choice on all 16 bars; widths of 1 to 12 px
    measure exactly. Counts at the default resolution are unchanged (3094 / 4373 on main).
  - **Specks, fill direction and order (2026-10-30, pe-refs-4's `autopunch-specks-default.pes`,
    `-specks-noise+1.pes`, `autopunch-right-triangle.pes`):** on a red square, a blue disc and
    square specks of 1, 2, 3, 5 and 8 px (0.35–2.82 mm) in both colours, PE drops the 1, 2
    and 3 px specks and keeps the 5 and 8 px ones; one more tick of Noise reduction gives a
    byte-identical file. OpenStitch already did the same: its 3×3 modal census removes the 1
    and 2 px specks, and the floor `max(4, round(minArea / mmPerPx²))` = 8 px removes what
    the census leaves of the 3 px one. Stitches 2355 against 2365. The fill direction was the
    leftover: PE gave the same disc 45° on one image and 90° on two others, and the isosceles
    triangle 90°. A single rule gives all 16 fills of the round-4 captures: the direction of
    90°, 45°, 135° and 0° whose rows spread furthest across the region, a diagonal winning
    over 90° only with at least one more row at 4.5 lines/mm (the 45° disc's diagonal spread
    is 0.50 mm wider, the 90° disc's 0.21 mm). It replaces the 13 mm stroke-width rule
    (`autopunch::fill_angle`). Within a colour PE sews blue (first) from the bottom specks up
    to the disc and red (second) from the square down to the specks, as it sews the bars
    bottom first: regions in scan order, reversed for the 1st, 3rd, … colour; it replaces
    "largest first". Counts: shapes 4384 / 4482, moved 4358 / 4436, specks 2355 / 2365,
    right triangle 2502 / 2547, bars 3094 / 3197 and 2011 / 2018.
  - **Colour order** (`autopunch-owner-shapes-moved.pes`, the same four colours with every
    shape moved and recoloured): PE sews yellow, blue, red, green again, so the order is a
    property of the colours. It is the reduced colour's R + G + B, highest first (yellow 480,
    blue 290, red 280, green 250; thread R + G + B would give yellow, red, blue, green).
    OpenStitch now sews colours in that order in Auto Punch and Cross Stitch: 4361 stitches
    against 4436.
  - **Photo Stitch 1 Mono** (`photostitch1-mono-grey-tonal.pes`, a grey gradient with soft
    blobs, values 0–234): no Sew Page Color, light areas unsewn, 2917 stitches, density falling
    about linearly with lightness from 154 needle points per cm² in the darkest band to none
    above grey 210. OpenStitch's Sew Page Color now applies to Color mode only. Its mono gives
    4068 stitches: 143 per cm² in the darkest band, 37 at 180–209 and 2 above 210 as PE, but
    132–181 in the mid-tones where PE has 87–142, because the dithered rows break into short
    runs there (426 trims; PE 2). That is the stitch-path model again.
  - **Photo Stitch 1 path (2026-10-30):** a thread's runs are now sewn nearest first instead
    of row by row with a jump to every run. From the needle, the closest unsewn run end within
    2 cells across and 2 rows down is entered with a travel stitch; a hop to the next row
    leaves out the run's last needle point so the row turns, as a fill row does (those hops
    had been sub-millimetre travel stitches, each a needle point). Mono capture: 3225 stitches
    against PE's 2917 (was 4068), 99 trims against PE's 2 (was 426); needle points per cm²
    by grey band 143 / 146 / 123 / 118 / 97 / 69 / 40 / 3 against PE's 154 / 142 / 125 / 92 /
    87 / 66 / 20 / 1 (0–29 … 210+), was 143 / 181 / 162 / 178 / 132 / 81 / 37 / 2. Colour
    capture: 11 654 stitches against PE's 12 255 (was 12 860), 249 trims against 6 (was 1158);
    white 148 / 162, yellow 173 / 170, green 198 / 211, red 166 / 175, blue 192 / 176 (was blue
    237). Left: PE's paths run along contours, diagonally; OpenStitch's stay horizontal rows,
    and the remaining trims are runs more than 2 cells from any other.
  - **Photo Stitch 2** (`photostitch2-color-owner-shapes.pes`, Line interval 2.0 mm, Max.
    Density 4.5 lines/mm, Direction 0°, Color Set Auto Select): a halftone screen. Each of four
    threads (blue, red, yellow, purple) sews parallel lines 2 mm apart at its own screen angle
    (0°, 45°, 90°, 135°) over the whole mask, with zigzag excursions where its colour is
    needed (white areas 88 needle points per cm², coloured areas 355–426), 14 133 stitches.
    OpenStitch's Lines style sews every colour along horizontal rows (7035 stitches). This is
    #129's model; noted on #129. Measured in detail on 2026-10-30 with the new
    `tools/screencmp.py`: each line is a straight 2.4–2.5 mm run where its colour is not
    needed (about 20 needle points per cm² per thread) and a zigzag 1.8 mm deep advancing
    0.2 mm per stitch where it is (about 300 per cm² in the thread's own area); one jump per
    thread, no trims. The model, the acceptance criteria and the patent facts are in
    docs/research/photo-stitch-2-halftone.md. Built 2026-10-30 after the owner's "Go ahead"
    (`screen.rs`, a fixed angle set, no image-derived direction): 13 701 stitches against
    14 133, angles, spacing, own-area (280–292 / 296–298) and white (21–24 / 21–23) densities
    and one jump a thread as PE. Second round of captures (2026-10-30: the four named Color
    Sets, Direction 30°, Line interval 3 / Max. Density 2.5, Mono, Auto Select on a two-colour
    image):
    - Angles follow the sewing position in every file.
    - A named set is four fixed threads sewn in order; OpenStitch now offers `colorSet`.
    - Direction turns the whole screen.
    - Interval and density act as modelled: 6666 stitches against 6842.
    - Mono follows a tone ramp from 17 % to 86 % darkness, now OpenStitch's too: 4671 against
      4665 stitches, every grey band within 6 % but 150–179.

    Left (open):
    - The mixing law within a set. PE gives an area's main thread nearly full coverage, and its
      choices are neither nearest-thread nor least squares; OpenStitch's named sets sew 8–15 %
      fewer stitches.
    - Auto Select's thread choice. PE always picks four threads; OpenStitch reduces to the
      image's colours, so where PE mixes green from blue and yellow, OpenStitch gives green its
      own thread.
- **Sewing-machine stitch files (PMV, 2026-10-29, #130):** OpenStitch now reads the PMV layout
  it writes (pyembroidery's description: 8-byte magic, 36-byte banner, 56-byte preamble, needle
  count and its double, 6-bit relative x / 5-bit absolute y pairs in 0.25 mm, the length and
  width tables, an 18-byte trailer; docs/FILE_FORMATS.md → PMV) and imports it as a decorative
  stitch. Not yet compared with files PE-DESIGN's Stitch Design Factory writes: the owner-made
  samples (2-point and 4-point stitches, one re-saved) are requested from the reference round;
  the reader refuses any other preamble or table shape until they pin it. PMP and PSF stay
  Blocked.
- **Programmable Stitch Creator sources (2026-10-29, #131):** OpenStitch imports the user's
  own `.pas`, `.pmf` and `.plf` text sources (the `@DIRECTIVE` layout: `@SYSENV` arity gives the
  role, geometry in a 0…1024 cell; docs/FILE_FORMATS.md → Text pattern sources) into
  `openstitch.pattern` fill, motif and decorative patterns, which sew with the design's own
  tile and motif sizes. Not yet pinned: the physical size PE-DESIGN gives the `@SYSENV`
  numbers and how it tiles a programmable fill from such a pattern (owner-made Programmable
  Stitch Creator files and the fills sewn from them are requested from the reference round).
  Brother's bundled patterns are never read.
  what changes is the palette's weighting.
- **Decorative fill with the owner's free-line pattern (reference, 2026-10-30, #131):**
  PE-DESIGN 11.50's Decorative Fill of `psc-deco-1line.plf` at 100 mm on a 40 mm square
  (`deco-fill-1line-100mm`) centres the tile on the region and sews the line corner to corner in
  1.95 mm stitches, travelling along the outline at 2 mm. OpenStitch imported such a `.plf` as a
  fill tile, so Decorative Fill could not use it and sewed a bundled pattern instead; it now keeps
  the lines free (`freeLines`) and sews the same needle points on the line, the same travel pitch
  and tie-in. Left: PE-DESIGN sews the line three times and walks parts of the outline two or
  three times (292 needle points against 73). The off-centre rectangle (`deco-fill-1line-offcentre`,
  2026-10-30) settled the placement: the tile is centred on the shape's bounding box, which
  OpenStitch does (the same 28 needle points on the line). Still open: the repeat rule (the line
  three times, the outline walked whole: 357 against 83 there), and a two-line tile whose lines
  fall outside the shape (`deco-fill-2lines`, parked: PE-DESIGN walks the border, 245 needle
  points, OpenStitch sews nothing).
- **CSewSeg drops trims:** not a difference (reference, `pes-csewseg-trims`). `CSewSeg` has
  no trim type: PE-DESIGN 11's own Ver.6.0 file stores a trim as a plain jump segment, as
  OpenStitch does, and keeps the trim through the PEC section's `0x20` flag.
- **PES writer:** only v1 and v6.
- **Wide satin:** stitches over 12 mm are split along the same line.
- **Feathered edge:** the guide's exclusions apply since 2026-10-27: not with gradation, Auto
  direction, hole sewing or text (refused when set, not sewn when a file holds them anyway), and
  the default width is PE-DESIGN 11.50's 3.0 mm. Still different: PE-DESIGN's top/bottom row
  switches and patterns versus OpenStitch's start/end/both with one width, and programmable
  fill (PE offers it; OpenStitch refuses it until it sews it).
- **Gradation:** the pattern is PE-DESIGN's since 2026-10-27: Max/Min density with evenly
  spaced step values (default Max 53 %, Min 5 %, 9 steps), the density falling linearly across
  the region, and rows at equal steps of the cumulative line count: the region holds the
  nearest whole number of gaps to the line count the profile integrates to (a flat pattern
  rounds down like a plain fill), the first row on the start edge and the last on the far
  edge. Satin takes the same rows as its lines (satin-gradation, 2026-10-28, needle for
  needle: 37 stitches as PE-DESIGN's, mean needle distance 0.01 mm): a satin split at its start
  point puts the crossings of each half on every other line counted from that half's own end, a
  hop of one line and a crossing close the half, and the "7.6 mm travel" the parked file showed
  is the diagonal crossing of the widest gap. The same lattice rule gives the uniform split
  satin its 0.3 mm hop (satin-zigzag-rect, unchanged). At the minimum density (0.1 lines/mm,
  sweep-satin-density-min, 2026-10-28, needle for needle) PE-DESIGN puts a crossing on every
  line, 10 mm apart with 11.7 mm diagonals, where counting forward from the end by two lines
  would space them 20 mm with 20.9 mm diagonals. The round-3 column at 0.2, 0.3 and 0.5
  lines/mm (43, 47, 55 stitches, record for record) settled the rule: each half hops to the
  last lattice line at or before the start point (on the start point's own line the crossing
  lands on the start point) and puts its crossings on every other line counting back from the
  hop, plus the far end's line (lines 0, 1, 3, 5, ...); at 3 lines/mm (satin-zigzag-rect) and
  on the gradation rows the same rule gives the crossings OpenStitch already sewed. No period
  limit exists. Two edges of the rule: a gradation row within 0.05 mm of the start point
  (satin-gradation's row 12, 0.02 mm before it in OpenStitch's lattice, on it in PE-DESIGN's
  0.1 mm file) is the second half's hop only, the first half hops to the row before it, as
  PE-DESIGN's file shows; and on a feathered satin a single-line gap next to the far end
  shorter than 1 mm is not crossed, the crossings before the hop spread evenly instead
  (satin-feather at 3 lines/mm: 15 crossings 0.69 mm apart over 29 lines, then the 0.33 mm
  hop). **Settled 2026-10-29 (#122, pe-refs-4's `satin-length-sweep-d4p5/-d1p0/-d0p2`: twelve
  3 mm wide columns 0.5–15 mm long at 4.5, 1.0 and 0.2 lines/mm, read column by column):** plain
  satin has no minimum-crossing threshold, the single-line gap next to the far end is crossed
  at every size measured (0.22, 0.33, 1 and 5 mm), so the spread is a feathered-edge rule
  only. The lattice is stretched to `length × density` rounded half down equal gaps (15 mm at
  4.5 lines/mm: 67 gaps of 0.2239 mm, PE's 14.8 / 14.3 where a nominal lattice gives 14.7 / 14.2;
  1.5 mm: 7 gaps of 0.214 mm in pe-refs-4's `satin-length-sweep-noopt-d4p5`; every earlier satin
  reference has a whole `length × density`). A column whose start point is at one end is
  sewn from its far end back to the start point: the opening row across and back, a straight
  travel inside to the far end's other corner, then the far end's row and every other line
  counted back from the start point's line, every crossing towards the start point's side
  (a 3 mm column at 1 line/mm, needle for needle: rows 3, 2, 0 mm); regions without a Running
  Stitch Path (files saved before it existed) keep their old order, as their travel would be a
  jump. A first stitch of 3.0 mm already takes the three-step tie-in (bound 2.65 mm, between
  PE's 2.3 and 3.0 mm). pescmp on the three sweep files (stitches ours / PE, coverage IoU) before
  → after: 4.5 lines/mm 358 → 438 / 424, 0.919 → 0.965; 1.0: 150 → 232 / 216, 0.693 → 0.779;
  0.2: 98 → 190 / 184, 0.444 → 0.690; `satin-length-sweep-noopt-d4p5` 205 / 202, 0.952 (the
  residue: PE starts the 12 mm column 0.5 mm down its edge, a split satin, and sews columns under
  1 mm differently from one file to the next).
  region-travel-u-gradation passes at the reference tier; all 40 of its rows lie within
  0.12 mm of PE-DESIGN's (0.04 mm on average; the earlier midpoint-density placement drifted
  to 0.4 mm by the far edge), and the round-3 Max/Min sweeps (80/20, 95/50, 50/50, pattern 2)
  fit the same rule within 0.1 mm. Designs saved with the old single far-side percentage
  still sew a continuous fall from 100 %. The guide's exclusions apply: not with a feathered edge, Auto
  direction or text (refused when set, not sewn when a file holds them anyway). Settled by the
  2026-10-28 gradation sweep (references `u-gradation-*.pes`): Max/Min rescaling (80/20, 95/50, a
  flat 50/50) and the mirror pattern 2 (scripted as Max 5 / Min 53) and Direction 90 are at the
  reference tier within the 15 % count tolerance; the dialog's Direction is the region's own
  Direction field; the Max slider stops at 95 %, Min at 5 %; the eight presets are per-step shapes
  (pattern 3: 53/42/32/24/17/12/8/6/5 %, 4: 5/6/8/12/17/24/32/42/53, 5: 53/52/50/46/41/34/26/16/5,
  6: 5/16/26/34/41/46/50/52/53, 7: 5/26/41/50/53/50/41/26/5, 8: 53/32/17/8/5/8/17/32/53) and Blend
  sews two colour blocks (second colour first); their specs check only the routing until the model
  has per-step values and Blend. Raised: without under sewing PE-DESIGN sews 434 stitches on the U
  where OpenStitch sews 366 (`u-gradation-nous.pes`, parked; on a plain 40 × 40 mm square without
  under sewing the gap is within tolerance: `square-gradation-default.pes` 673 / 644 and
  `square-gradation-30-10.pes` 489 / 461 at the reference tier, so the U's legs carry most of it): the under-sewing layer hides a
  difference in the gradation rows themselves. Still missing: per-step editing, Blend, the
  gradation direction arrow, and programmable fill (PE offers it; OpenStitch refuses it until it
  sews it).
- **Over-long moves in PEC (2026-10-29, #126):** the PEC writer split a move over ±2047 units
  into equal pieces computed on unrounded coordinates, so a half-unit fraction could still
  encode a 2048 delta (masked to 12 bits). It now splits on the rounded coordinates with the
  rule PE-DESIGN 11.50 shows in its DST for the 211 × 278 mm move of `long-jump-300.dst`
  (pe-refs-4): the maximum along the longer axis, the proportional share on the other,
  remainder last, never halved (the issue proposed halving). PE-DESIGN's PEC bytes for that
  move (`9e 39 96 95` then 39 × `80 46 00`) do not decode to the DST's vector with the 12-bit
  rule, so its PEC encoding over 204.7 mm stays open and the DST rule is applied at the PEC
  limit; internal regression tier (`pec_splits_moves_over_2047_units_like_pe_design_dst`).
- **Thread-colour matching (2026-10-29, #119):** nearest-chart matching (PEC colour indexes,
  JEF/SEW/HUS/PCS charts, image digitizers, `threads.remap`) used a red-mean weighted RGB
  distance; it is now equal-weight squared Euclidean (ΔR² + ΔG² + ΔB²), and a second
  "approximate" matcher keeps a city-block minimum beside it and returns the city-block winner
  (`openstitch_core::approximate_index`), for the built-in sew palette. Checked against the
  owner's twelve-swatch capture (`compat/reference/pes/thread-match-swatches.pes`, PE-DESIGN
  11.50, 2026-10-29: twelve RGB fills imported from an SVG and the Brother chart entry PE
  picked for each): squared Euclidean against chart entries 1–61 reproduces all twelve picks
  (`pe_design_swatch_picks_are_reproduced`); red-mean missed blue (21 Ultramarine for 405) and
  cyan (53 Light Blue for 019); city block alone missed four. Two findings against the issue's
  text and pyembroidery's table: (1) PE-DESIGN's own thread table writes (253, 217, 222) for
  entry 57 (124 FLESH PINK) where pyembroidery has (255, 204, 204), so the chart table now
  carries PE's value (entry 42 keeps pyembroidery's identical RGB); PE picked 57 for
  rgb(240, 200, 220), so ties go to the later entry, not the earlier one the issue asked for.
  (2) rgb(255, 128, 0) went to 11 Deep Gold (2210 away) and not to 62 Orange (625 away), so
  entries 62–64 are not candidates: matching runs over the first 61 entries (#136's "basic
  only" match), and the three are kept only for files that name them. Still to do for the
  spec's thread-table compare: SVG-import and hex fills stay exact custom threads in OpenStitch
  while PE-DESIGN snaps them to the chart with Brother's catalogue code (800, 513, …); that
  needs the codes for all 61 entries, known since the round-4 chart reading (see below), and goes with
  the palette work (#136). The machine-format references (`machine-formats`, red and black)
  pick the same entries under both metrics.
- **Thread-chart CSV and the built-in palette (2026-10-29, #136):** PE-DESIGN 11.50 has no
  thread-chart CSV (round-4 check: Options ▸ Edit User Thread Chart only imports; user charts
  are binary `.btc2` files, 89 bytes of high-entropy data for one colour, in the program
  folder), so the issue's `#userchart` schema, 6-character code and 1,499-colour cap have no
  PE-DESIGN counterpart. OpenStitch documents its own CSV (FILE_FORMATS.md "Thread chart
  CSV": the six columns it already imported, plus a column row and RFC 4180 quoting) and adds
  `threads.exportCsv` for user and built-in charts and `threads.builtinCharts`, which marks
  Brother entries 1–61 as the ones PE matches to (#119). Charts cannot be exchanged with
  PE-DESIGN through files; `.btc2` is not decoded (obfuscated, kept in the program folder).
  The 94-entry sew palette of the issue is not implemented: no PE-DESIGN capture shows it.
- **Readers against PE-DESIGN's ten-format export (2026-10-29, #135):** pe-refs-4's
  `sample-basic` (a satin S and a fill, exported to every type) found four reader problems.
  (1) PES "drops PE's tie-in records" (471 needle points against PE's 475): not a reader
  bug. That sample's PEC section has none of the 4 zero-length stitches its DST has (2 jump
  landings, 2 repeats; the other 471 points equal), and the status bar counts 1–9 more than
  the PEC in 99 of 114 references: the points are not in the file. (Corrected 2026-10-30: PE's
  PEC writer does not drop zero-length stitches as a rule; 28 of 249 PE references carry them,
  so OpenStitch's PEC writer keeps its own and the "drop them" follow-up was not done.) (2) CSD read as
  2 stitches and 51 colour changes: the mask index was the byte offset minus one; with the
  offset itself PE's CSD equals its DST needle for needle, with PE's colours and colour order
  (fixture `sample-basic.csd`, reference-checked). (3) SHV read an extra colour change at the
  end: pyembroidery starts the records 2 bytes before the colour table's end, PE-DESIGN at the
  end; the reader now follows the word counts (fixture `sample-basic.shv`). (4) Importing PE's
  DST merged its two colour blocks: a DST has no colours, so the stitch block got none and the
  blocks were merged as one colour; each block now gets its own Brother chart colour on import.
  The ten-format sample is now spec `machine-sample-basic` (2026-10-30, reference tier): PE's
  PES is imported as stitches and the eight writable formats are compared with PE's exports
  needle for needle. Three writer fixes made them equal: the design centre floors on an odd
  extent; DST pads the way back to the origin to a trim run only when it is one record; and a
  PE-DESIGN `#PES0110` import gets back the stitch at each jump landing that PE's PEC leaves
  out, which the folding formats (JEF, SEW, VIP, XXX) fold onto. The compare drops zero-length
  stitches on both sides (`dropRepeats`) and the report lists how many per side (section
  "Zero-length stitches left out by `dropRepeats`" of the summary, `droppedRepeats` in
  report.json). For this spec, PE / OpenStitch: DST 4 / 2, EXP 4 / 2, PCS 5 / 3, HUS 6 / 4,
  VIP 0 / 0, JEF 0 / 0, SEW 1 / 1, XXX 2 / 2 (PCS and HUS were 5 / 1 and 6 / 2 before their
  writers wrote the landing twice, 2026-10-30). Every repeat OpenStitch writes is at a point where
  PE writes one too (the jump landings, SEW's and XXX's entry points); none is OpenStitch's
  alone. The ones only PE writes:
  - two repeats inside the S's satin, at (17.9, 37.9) and (25.9, 31.9) mm from the design's
    minimum corner, in DST, EXP, PCS and HUS: in PE's own stitch data, absent from its PES, so
    no OpenStitch import can have them;
  - fixed: PCS and HUS write each jump landing twice (the move's own zero-length record, then
    the design's landing stitch) where OpenStitch wrote it once; both writers now keep the
    design's landing stitch after their own (in `machine-formats`, a stitch file PE only opened,
    there is no landing stitch to keep, and nothing changes). What is left in every format is
    the two repeats inside the S.
  The "trim before the colour change" the round-4 notes saw does not occur in this sample.
- **Brother catalogue codes (2026-10-29, #119, #136):** pe-refs-4 read PE-DESIGN 11.50's BROTHER
  EMBROIDERY chart from Sewing Attributes ▸ Color: 61 rows, codes 001 WHITE to 900 BLACK, brand
  EMBROIDERY, no row for PEC 62–64 (so the 61-entry match set of #119 is PE's own chart).
  Matched to the PEC chart by name, every code lands on one PEC entry 1–61 and all 30 pairs PE
  writes in the owner's `#PES0110` thread tables agree. pyembroidery's table names PEC 42 and 57
  both "Flesh Pink"; PE's files put 124 FLESH PINK at 57, so 307 LINEN is 42, and the chart
  table now calls 42 "Linen", with PE's RGB (254, 227, 197) since 2026-10-30 (the reference
  session's 307 LINEN sample; every coded thread in PE's own files now matches the chart,
  `pe_design_thread_tables_match_the_pec_chart`). The dialog's own
  first column (1 BLACK, 2 WHITE, 3 RED, …) is not the PEC index. API:
  `openstitch_core::BROTHER_EMBROIDERY`, `brother_embroidery_code`, `pec_index_of_brother_code`.
- **Contour fill (2026-10-30, B1):** an OpenStitch extension, region type `contour`. It
  reuses PE-DESIGN's Spiral Stitch's offset rings but no reference spec sets it, so every
  PE-DESIGN spec is unchanged; its tests are internal regressions. A design using it opens in
  PE-DESIGN as plain stitches. Label it "Implemented (beyond PE-DESIGN)" in FEATURE_MATRIX.md.
- **Imported colours snap to the chart (2026-10-30, #119):** `import.svg` gives every fill and
  stroke the nearest Brother chart thread 1–61, as PE-DESIGN's vector import does, and
  `thread-match-swatches` now imports the very SVG PE-DESIGN imported and compares the whole
  thread table (RGB, name, code, brand, chart, type) with PE's file: equal, reference tier. The
  PES 0110 writer takes the codes of all 61 entries from core (its own table had 25) and picks
  the PEC index among 1–61. Hex colours given to commands (`fill: "#rrggbb"`) stay exact on
  purpose: PE-DESIGN has no free-RGB colour field to match, and exact RGB is what PES v6 keeps.
- **Skeleton satin and the Satin Skeleton tool:** OpenStitch extensions with no PE-DESIGN
  counterpart. Label them "Extension" in FEATURE_MATRIX.md.
- **Text fill rows (for the next text session):** since fill took PE-DESIGN's row structure,
  letters (rows with several spans, sewn in the older nearest-row order) have more stitches than
  PE-DESIGN's: text-size-h 639 against PE 483 (main before: 622), the Text Transform specs about
  +14 % (e.g. text-transform-1 1489 → 1701, PE 1300). For text-size-h the cause is that rows
  shorter than one stitch keep their edge points (`generate::chain`, `Chain::Brick`): PE-DESIGN
  drops them on a 20 mm letter's 3.5 mm strokes but keeps the narrow rows at
  hole-sewing-pentagon-star's vertices (with the cut at half a stitch or at 3 mm text-size-h is
  511, but the pentagon's stitchedArea leaves its tolerance). The Text Transform excess does not
  depend on that cut (1685 at 3 mm); likely PE's grid keeping points down to 0.3 mm from a row end
  in narrow strokes, or letters at 45° (the grid anchor of angled fills, sweep-fill-dir-30/45/135).

- **Round-4 references (2026-10-29, PE-DESIGN 11.50 driven black-box; owner's notes folder
  `round4/` holds every file, log and the parked ones):**
  - *Under sewing types (#124):* for a Satin region all five type buttons are enabled (Fill
    enables the first row only) and they are exclusive radio buttons: PE-DESIGN cannot
    combine under sewing types. On a 39.5 x 6 mm Manual Punch column: type 1 287 stitches,
    2 287, 3 226, 4 250, 5 226, off 186. OpenStitch's `edgeZigzag`, `double`, `edge` and
    `zigzag` match types 1-4 within 8 % (specs satin-col-us-type-1..4, reference tier);
    parked: type 5 (OpenStitch `edgeFill` 292 vs 226) and the column without under sewing
    (207 vs 186: the plain column is 11 % over, the under sewing itself is close).
  - *Running stitch on a circle (#121):* a 59.9 x 60.4 mm Ellipse-tool circle at run pitch
    2.0 sews 133 stitches; OpenStitch's Bezier halving matches (run-circle-60, reference
    tier, count, mean and longest stitch).
  - *E and V stitch defaults:* list "E Stitch" / "V Stitch", stroke spacing 5.0 mm, arm
    2.5 mm, width 2.0 mm, Run time(s) 1, Stroke time(s) 1 (greyed for V), Arrange two
    buttons; a closed 30 x 20 mm rectangle sews 105 (E) and 87 (V) stitches (specs
    line-e-rect-default, line-v-rect-default, reference tier). Tatami defaults as before:
    step pitch 4.0 mm, Frequency 30 %, density 4.5, direction 45.
  - *Over-long move (#126):* two satin blocks in opposite corners of a 300 x 300 mm page
    (Custom Size accepts 100-1000 mm). PE-DESIGN's DST writes the 211 x 278 mm move as 23
    jump records of (92, -121) units plus one of (90, -118): the vector divided evenly into
    records whose longer axis is the 121-unit maximum, never halved; the runs from and to
    the origin are 11 x (-121, 121) + (-119, 119). OpenStitch's DST agrees on the total
    jump length and the bounds (long-jump-300, reference tier). Its PES stays parked, settled
    2026-10-30: PE-DESIGN does not encode the move in the PEC at all. After the colour change
    its PEC has one jump of (-45.5, +168.5) mm to (0, 180.0) mm (`9e 39 96 95`) and then 46
    stitch records in a straight 280 mm row at y = 180 mm (39 of them the 7.0 mm long form
    `80 46 00`), where the design's second block is a 39.5 x 6 mm satin like the first. The
    block header says 290 x 190 mm; the PEC holds 233 needle points (186 + 47), the status bar
    372 (186 + 186). For a design whose moves leave the 12-bit range, PE's PEC section is a
    stand-in, not the stitches, so it cannot be a reference for a long move; OpenStitch's
    split at 2047 units (#126, as PE's DST splits) stays. Reading it gives 280 x 174.5 mm.
  - *Hoop limits (#125):* a design scaled to 129 x 105 mm on a 100 x 100 mm Hoop-Size page
    is neither rejected nor promoted to a larger hoop. Save As warns "Please keep all designs
    within the Design Page. Saved data cannot be sent to the original card or the embroidery
    machine. Do you want to save the data anyway?" and saves on Yes; Export to DST writes
    the file with no warning (hoop-oversize-export, reference tier on the DST bounds). Its
    PES (settled 2026-10-30, now a formats fixture) has an empty machine section: the PEC's
    "no colours" form and a 13-byte stitch block with only the design's size (1290 x 1050
    units) and the end byte, which is what "cannot be sent to the embroidery machine" means;
    the design is only in the object section (13 threads, `CGroupObject` and `CEmbLine`). The
    211 x 229 mm OpenStitch read was its reader decoding the preview icons as 169 stitches and
    the colour byte `FF` as 256 colours; it now reports that the PEC holds no stitches for this
    129 x 105 mm design (`pe_design_oversize_save_has_no_machine_stitches`). OpenStitch's own
    export of an oversize design still writes its stitches, with a warning (#125, #157); PE
    writes none to the PEC. An empty OpenStitch PES (same "no colours" form) now reads back
    without the 256 Sky Blue threads it used to give.
  - *Thread picks (#119, #136):* twelve 20 x 20 mm squares with chosen RGB fills imported
    from an owner SVG. PE-DESIGN mapped rgb(255,0,0) 800, (0,255,0) 513, (0,0,255) 405,
    (255,255,0) 205, (0,255,255) 019, (255,0,255) 086, (128,128,128) 817, (200,150,100)
    348, (50,100,150) 070, (240,200,220) 124, (20,80,40) 415, (255,128,0) 214 (Brother
    Embroidery codes; PEC indices 5, 14, 2, 13, 32, 25, 39, 35, 4, 57, 38, 11). OpenStitch
    keeps an `#rrggbb` fill as a custom thread with the exact RGB and no chart name, so the
    thread table is not compared yet (thread-match-swatches checks the twelve blocks at the
    reference tier and records the picks for the matcher). Thread list to CSV: Layout &
    Editing 11.50 has none (Options > Edit User Thread Chart only imports charts; Design
    Database asks for a software key).
  - *Machine formats (#135), parked:* a two-colour design (20 mm satin S in Red, 10 x 8 mm
    Prussian Blue fill) exported to all ten types PE-DESIGN offers (DST, EXP, PCS, HUS,
    VIP, SHV, JEF, SEW, CSD, XXX; no PHC, PHX, PEN or PEC; no options, no warnings). Every
    writer compare fails by the same needle points: OpenStitch's PES reader drops the
    tie-in records at a block's start (PE-DESIGN's stream starts with a plain 0.2 mm
    stitch out and back, 475 needle points against 471 read; 1 in the first block, 3 in the
    second), so the exports are 4 stitches short of PE-DESIGN's and a trim stands where
    its colour change is. Importing PE-DESIGN's DST instead keeps 475 points but merges the
    two colours into one block (`stitches.import` of a DST). PE-DESIGN's CSD read by
    OpenStitch's spec-derived reader gives 2 stitches and 51 colour changes (the reader is
    wrong against a real file); its SHV reads with one extra colour change at the end. All
    eleven files wait in the notes folder for the reader fixes.
  - *PES object kinds (#120, #131):* programmable fill, motif fill and motif line with an
    owner-drawn V pattern from Programmable Stitch Creator (`.pas` / `.pmf`: ASCII,
    `@VERSION 10000`, `@SYSENV 1`, `@TILE 1` + `@TILELINE 0 3 ...` or `@LINE 3 ...` in a
    0-1024 cell, Default Size 7.5 mm) read and written back with their stitches
    (pes-objects-owner-patterns, reference tier; the files are also `pe11` fixtures). With
    round 3's satin, fill, running, text and applique files every editable kind has a
    sample.
- **Round-4 batch 4 (2026-10-29, PE-DESIGN 11.50 black-box):**
  - *Tatami stagger (#123):* PE-DESIGN has no random stagger. Its only stagger controls are
    Frequency (0-50 %, default 30) and the two Stitch Type buttons; on a 40 mm square at the
    defaults they give 2068 and 2054 stitches (tatami-40-stitch-type-1/-2, reference tier).
  - *Density floor (#139):* the Density field accepts 0.1 lines/mm; 0.05 and 0 become 0.1,
    0.15 rounds to 0.2, 20 becomes 14.0. The floor is 0.1 lines/mm with one decimal.
  - *Satin length sweep (#122):* twelve 3 mm wide satin columns 0.5-15 mm long at 4.5, 1.0
    and 0.2 lines/mm. 4.5 and 1.0 pass at the reference tier (satin-length-sweep-d4p5/-d1p0);
    0.2 is parked (OpenStitch 145 stitches against PE-DESIGN's 184). PE-DESIGN's needle points
    per column are in the MANIFEST entries for the satin track.
  - *Optimize Sewing Order (#137):* on six scattered squares in three colours it changed
    nothing (identical stitch streams before and after); PE-DESIGN groups same-colour
    objects into one block in first-occurrence order (sew-order-six-objects).
  - *Concentric and spiral fills (#138):* the Region sew pane offers no Direction field for
    Concentric Circle or Spiral Stitch, only Density (default 2.0 lines/mm), Run pitch (2.0 mm)
    and a two-button Start Point choice (the winding).
  - *Pattern sources (#130, #131):* Programmable Stitch Creator writes one type per mode:
    `.pas` (Fill/Stamp, Default Size 7.5 mm), `.pmf` (Motif, 5.0 mm), `.plf` (Decorative Fill,
    100.0 mm, `@SYSENV 10000 10000 1`). A closed square is written with its closing point; two
    separate lines are two `@TILELINE` records with nothing marking the jump. Six minimal
    sources and four designs that use them read and write back (pes-objects-psc-patterns).
  - *Thread charts (#136):* a user chart is stored as `MyChart\<name>.btc2` in PE-DESIGN's
    program folder, 89 bytes of high-entropy binary for one colour: no CSV.
  - *Image wizards (#127, #128, #132):* Auto Punch, Cross Stitch and Photo Stitch 1 at their
    defaults on an owner-made image are fixtures in
    `crates/openstitch-digitize/tests/fixtures/pe11-wizards` with every default recorded.
  - *Not recorded:* Font Creator and Stitch Design Factory output (#134, `.pef`, `.pmv`); the
    owner declined screen-control access to both programs.

- **Platform-dependent arched text (found 2026-10-29, fixed 2026-10-30):** the same build measured the PES 0110 Text Transform spec's arch height as 26.0 mm on Windows and macOS and 25.8 mm on Linux CI (PE-DESIGN 11.50: 26.5 mm). Cause: stitch generation called the platform's C math library (`f64::sin`, `cos`, `atan2`, `hypot`… go to glibc, the MSVC runtime or Apple's libm, which differ in the last bit for some arguments), and on the arch a last-bit difference moved a fill row across its threshold (rows 0.2 mm apart at density 5). Every transcendental call in `openstitch-design` and the engine's stitch paths now goes through `openstitch_design::fmath::Det` (the `libm` crate, a pure-Rust port of musl's libm, already in the tree via rustybuzz), so a design sews the same bits everywhere; a source scan (`fmath::tests`) keeps the platform functions out, and `crates/openstitch-engine/tests/determinism.rs` pins the arched text and a rotated satin ellipse to bit-exact fingerprints on all three OSes. Windows output is unchanged (every compat spec, both goldens). PE-DESIGN's re-save (`pes-0110-editable-text-transform.pes`) is back in the repository with its MANIFEST entry: 26.0 mm against 26.5, within the spec's unchanged 0.5 mm.

## Order of work

1. **Fix without a reference:** items 1, 3, 7, 14, 15 (ranges), 16 and 17, plus the
   documented PES fields: 2, 18 and 19. Each comes with the test above and becomes an internal
   regression.
2. **Black-box checks of OpenStitch output in the owner's PE-DESIGN:** items 2 and 5 (short
   term), 4 and 9. These are the first Reference Verified rows.
3. **Reference capture (method 1):** calibrate the defaults and settle 6, 8, 11, 13 and 20.
4. **Output-file study (method 2):** editable shapes and text (5), PES v7–v11 and undocumented
   fields, from the owner's PE-DESIGN files only (track `pes-objects`). Decompiling (method 4)
   stays prohibited pending legal review.

The harness for steps 2 and 3 is built (2026-10-19). It is `stitchtool compat`, in the
crate `crates/openstitch-compat`, and is described in
[COMPATIBILITY_TESTS.md](COMPATIBILITY_TESTS.md). It has four parts:

- **Test specs:** `compat/specs/<area>/<id>.json`. Each holds the command script, the exports and
  the measurements listed in the items above, with tolerances and evidence. The tests for items
  1–3, 7 and 14–19 are converted; the Rust tests stay.
- **Runner:** it executes each script headlessly in a sandboxed session, as `stitchtool exec`
  does, and writes `target/compat/report.json` and `summary.md`. CI runs it on the ubuntu-latest
  job.
- **Semantic comparer:** it compares stitch coordinates, commands, colour changes, jumps, trims,
  order, per-block counts, threads, dimensions, metadata and the PES object summary, with
  normalised origins. Every difference is reported with its location.
- **Status tiers, kept apart:** internal regression, oracle, reference (PE-DESIGN files in
  `compat/reference/` with a provenance manifest) and machine verified (sew-out records in
  `compat/machine/`). The generated [COMPATIBILITY_STATUS.md](COMPATIBILITY_STATUS.md) shows the
  tier each item and FEATURE_MATRIX row has reached.

The audit items' reference checks are written and wait for the owner's files: the specs report
them as "pending reference". Step 2 starts when the owner adds the first file, following
[compat/reference/README.md](../compat/reference/README.md).
