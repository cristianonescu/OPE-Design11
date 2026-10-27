# Compatibility audit — 2026-10-19

The 20 highest-impact differences between OpenStitch and PE-DESIGN 11. They cover stitch
generation, editable PES and embroidery-font behaviour. Each entry gives the Rust module, the
evidence, a reproducible test and the proposed fix. The audit was made at `7b02267`.

## What "reference behaviour" means here

**No item in this audit is verified against PE-DESIGN 11 output yet:**
- [FEATURE_MATRIX.md](FEATURE_MATRIX.md) has no Verified Compatible rows.
- The reference tier in [COMPATIBILITY_TESTS.md](COMPATIBILITY_TESTS.md) is empty.

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
   - It needs a PE-DESIGN installation activated with the owner's licence, in the VM from
     [VM_PLAN.md](VM_PLAN.md).
2. **PES files the owner saves from their own PE-DESIGN** (allowed for documented fields).
   Studying fields that no public source describes needs the legal review that is already on
   the owner's list.
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
| 5 | Every design is written as one stitch group, nothing editable | PES | 5 | Documented, unverified | Yes (shapes, text) |
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
  first stitch and back) and ends with a lock (0.3 mm back along the last stitch and return).
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
  lengthens every row. Tests: `crates/openstitch-engine/tests/satin_compat.rs`
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

- **Impact:** severity 4. One same-coloured rectangle with underlay and an outline sews with
  3 trims and 3 jumps. That means more cuts, more tie-offs and longer sew time.
- **Module:**
  - `openstitch-design/src/compile.rs:114`: the first run of each element calls
    `travel(.., false)`.
  - `compile.rs:277-286`: underlay, top stitching and outline are separate elements.
  - `compile.rs:78`: any travel over 3 mm is TRIM+JUMP on every machine type.
  - `openstitch-design/src/machine.rs:305-310`.
- **Evidence:**
  - Guide, [entry and exit points](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/04_1858418.html):
    stitching continues from the region exit (R2) to the line entry (L1).
  - Guide, [jump trimming](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/04_1908282.html):
    per-object on/off with a minimum length, for multi-needle machines only.
  - Measured: a 30×30 rectangle with fill, zigzag underlay and a same-colour outline gives
    trims at stitch indexes 61, 738 and 789 on the default single-needle machine.
- **REA:** none.
- **Grade:** Documented, unverified. Whether PE-DESIGN writes trims for single-needle
  machines needs a reference.
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
- **Status:** fixed, internal regression tier only (not reference verified). Auto satin
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
    in plain and shaped fill; unset keeps the ⅓ stagger, so existing designs sew as before.
  - `object.setRegion` refuses Frequency outside 0–50 (all types that use it);
    `object.setLine` limits run times to 1–5 for E/V and every other type, 1–6 for chain.
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
    line pitch 2.5 mm.
  - Calibrated from the PE-DESIGN 11.50 reference files (2026-10-27): fill 4.5 rows/mm with
    under sewing on (`EdgeFill`: edge run 1 mm inside the outline at <= 2.5 mm, then tatami
    rows across the fill, 2 mm apart, 4 mm stitches, 30 % shift, 1 mm inset), also on for
    programmable fill. Still open: fill Frequency default (PE 30 %, OpenStitch 1/3) and the
    programmable fill density.
  - The guide gives word scales only. Fit the mapping from reference files at slider minimum,
    middle and maximum.
- **Line Start/End type (Feed/Run):** motif, stem, candlewick and chain lines restart at a
  moved entry point instead of keeping the pattern in place.
  ([guide](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293444.html))
- **Line spacing:** in mm, while the guide uses % with default 100. The basis that text size
  measures (OpenStitch uses cap height) needs a reference.
- **Fit Text to Outline:** the dialog's horizontal and vertical alignment, orientation and
  other-side options are missing, and one text per outline is not enforced.
  ([guide](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/05_1810870.html))
- **Text Transform (envelope presets):** missing.
- **Sewing order inside a text:** all fills first, then all outlines. Character edits are
  allowed on user-mapped text, which the guide disables.
- **CSewSeg drops trims:** PEC keeps them.
- **PES writer:** only v1 and v6.
- **Wide satin:** stitches over 12 mm are split along the same line.
- **Feathered edge:** the guide's exclusions apply since 2026-10-27: not with gradation, Auto
  direction, hole sewing or text (refused when set, not sewn when a file holds them anyway), and
  the default width is PE-DESIGN 11.50's 3.0 mm. Still different: PE-DESIGN's top/bottom row
  switches and patterns versus OpenStitch's start/end/both with one width, and programmable
  fill (PE offers it; OpenStitch refuses it until it sews it).
- **Gradation:** the pattern is PE-DESIGN's since 2026-10-27: Max/Min density over equal steps
  (default Max 53 %, Min 5 %, 9 steps), rows from edge to edge; region-travel-u-gradation passes
  at the reference tier. Designs saved with the old single far-side percentage still sew a
  continuous fall from 100 %. The guide's exclusions apply: not with a feathered edge, Auto
  direction or text (refused when set, not sewn when a file holds them anyway). Still missing:
  other preset patterns, per-step editing, Blend (two-colour blending), the gradation direction
  arrow, and programmable fill (PE offers it; OpenStitch refuses it until it sews it).
- **Skeleton satin and the Satin Skeleton tool:** OpenStitch extensions with no PE-DESIGN
  counterpart. Label them "Extension" in FEATURE_MATRIX.md.

## Order of work

1. **Fix without a reference:** items 1, 3, 7, 14, 15 (ranges), 16 and 17, plus the
   documented PES fields: 2, 18 and 19. Each comes with the test above and becomes an internal
   regression.
2. **Black-box checks of OpenStitch output in the owner's PE-DESIGN:** items 2 and 5 (short
   term), 4 and 9. These are the first Reference Verified rows.
3. **Reference capture (method 1):** calibrate the defaults and settle 6, 8, 11, 13 and 20.
4. **Legal review:** editable shapes and text (5), PES v7–v10 and undocumented fields (method 4).

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
