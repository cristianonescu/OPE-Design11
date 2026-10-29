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
| 5 | Every design is written as one stitch group, nothing editable (**partly fixed**, 2026-10-29: `pesVersion` 11 writes shapes as `CEmbLine`, which PE-DESIGN 11.50 opens editable, and Lato Bold text as `CEmbNText`, which it opens editable and retypable (both reference)) | PES | 5 | Reference | Yes (the Hoop Size page check; Text Transform and text on an outline) |
| 6 | One sewing direction per text instead of per stroke | Fonts | 5 | Documented difference | Angles |
| 7 | Auto direction is one fixed angle (**fixed**, internal regression) | Stitches | 4 | Documented difference | No |
| 8 | Underlay model differs; zigzag lines have no underlay | Stitches | 4 | Documented difference | Geometry |
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
  Spaces are letters (r27). Refused for now: vertical text, text on an outline, Text
  Transforms, rotation, letter spacing, kerning, per-character edits, other fonts.
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
- **REA:** none. Font internals are in the prohibited category.
- **Grade:** Documented difference. The per-stroke angles need a reference.
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
  at all three (reference tier). At 120° PE-DESIGN sews 161 stitches with a 10 mm longest crossing
  where OpenStitch sews 267 with a 13 mm one: the fan rule breaks down on a sharp turn (file
  parked with the parked-references session; its spec is not in the repository).
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
  Still open: Density Dense / Light (PE: row spacing 1.41 / 3.45 mm against Medium's 2.0,
  `sweep-fill-us-density-*`), and which of the two types PE applies to the other region kinds.
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
    (`CEmbLine` shapes, thread table, groups, page), so once the import turns the reader's shapes
    into design objects (ROADMAP item 6) a PE-DESIGN file of shapes re-saves with its objects.

## 10. Hoop and Design Page fields are hard-coded in PES

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
  - Fill Frequency (fixed ⅓ stagger);
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
    compensation stays 0–2.0 mm, as PE's fields clamp (sweep references, 2026-10-27). The
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
  text-fit-default's mean needle distance 0.27 → 0.20 mm); the letters sit where the font's
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
  PE-DESIGN's file shows; and a single-line gap next to the far end shorter than 1 mm is not
  crossed, the crossings before the hop spread evenly instead (satin-feather at 3 lines/mm:
  15 crossings 0.69 mm apart over 29 lines, then the 0.33 mm hop; the round-3 column at 0.5
  lines/mm keeps its 2 mm single-line gap). Open: the merge bound lies between 0.33 and 2 mm
  (the 40 x 6 mm column at 1, 1.5 and 2 lines/mm, start point at the middle, would pin it).
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
