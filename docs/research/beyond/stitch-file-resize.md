# B14. Stitch-file resize with density recalculation and density tuning: algorithm sketch

Backlog item B14 of [competitive-features.md](../competitive-features.md) (feature D14).
Research date: 2026-10-28. A design note for `openstitch-design` (`stitch_edit.rs`, a new
`segment.rs`) and `openstitch-engine` (`convert.rs`, `stitch_edit.rs`). No other product's
code was read.

## 1. What the feature is

Purchased and machine-made designs arrive as stitches, not objects. Scaling stitches scales
the gaps between them, so a design shrunk to 70 % becomes too dense and one enlarged to
150 % becomes thin and gappy. The tools the products offer:

- **Resize with density recalculation.** Scale a stitch block while re-spacing its stitches
  so satin keeps its stitch spacing, fills keep their row spacing and stitch length, and
  runs keep their pitch, within a published range (Embrilliance 50–250 %).
- **Density tuning.** Change the stitch count of a block by a percentage (±25 %) at the same
  size: more or fewer rows and satin stitches.
- **Target stitch count.** Choose a density so the whole design has a given stitch count
  (Embird: "prescribed stitch count").
- **Clean-up.** Remove hidden stitches under later layers (B11's clip), de-clump tiny
  stitches, limit density by fabric.

Who has it: Embrilliance Essentials resize 50–250 % with recalculation, Enthusiast Advanced
Sizing, Density Repair Kit (Density Tuning ±25 % preserving pattern fills, Remove Hidden
Stitches, De-Clump, Sweep, Limit Density) [[EM-ess]](https://www.embrilliance.com/essentials),
[[EM-DRK]](https://www.embrilliance.com/density-repair-kit) **(single source)**; Embird
Editor resize with or without density adjustment, keep or re-fill the pattern, prescribed
stitch count [[Embird-resize]](https://www.embird.net/sw/embird/tutorial/resclc/resclc.htm);
Pulse Outline Processor ("resizing adjusts stitch count"), Master Density
[[DG16-IE]](https://tajimaeurope.com/en/pulse-software-solutions/embroidery-software-tajima-software-dg16-pulse/illustrator-extreme/),
[[Pulse-density]](https://community.pulsemicro.com/topic148-how-to-make-stitch-design-more-dense.aspx);
mySewnet Stitch Editor density change **(single source)**; SewWhat-Pro constant-density resize
[[SWP]](https://sandscomputing.com/products-shop/sewwhatpro-embroidery-editor/); Ink/Stitch
Outline (stitch file to vector, with live preview) [[IS-stroke]](https://inkstitch.org/docs/stroke-tools/).

PE-DESIGN 11: objects regenerate on resize; imported stitch data is scaled as is, and the
guide's "Enlarging/reducing" supplement warns about it **(from the guide's table of
contents)**; a forum user asking how to fix the density of a shrunk design got no answer
[[t-shirtforums]](https://www.t-shirtforums.com/threads/trouble-with-pe-design-next.192989/).
OpenStitch today: `object.scale` scales stitch blocks as PE-DESIGN does; `object.toShapes`
traces a block's coverage into outlines (one, per colour, or blocks with a sewing direction:
the "Convert to Outline / Blocks" approximation); `stitch_edit.rs` has point moves, splits,
filters and `split_area`; `digitize::coverage::trace` gives the covered area.

## 2. Segmentation of a stitch block

Everything here depends on recognising what the stitches were: the same classification
`object.toShapes`'s "blocks" mode needs. A **segment** is a maximal run of consecutive
stitches with one character:

| Class | Signature (consecutive stitches `s_i`, vectors `v_i`) | Recovered geometry |
|---|---|---|
| **satin** | alternating direction (`v_i · v_{i+1} < 0`), similar length (`|v_i|` within 30 % of a running median), length ≥ 1.0 mm, the midpoints of consecutive stitches advancing by a small, steady step | two rails = the odd and even needle points smoothed; rung spacing = the median midpoint step |
| **fill** | long runs of stitches of near-equal length along one direction (a row), rows alternating direction, consecutive rows a constant distance apart, needle points on a shifted grid | row direction, row spacing (median), stitch length (median), the covered area as the union of the row spans (no raster) |
| **run** | stitches of near-equal length with small turning angles, not alternating | the path = the needle points; pitch = median length |
| **tie / lock** | 3–6 stitches under 1 mm at a run's start or end | kept as is |
| **other** | anything else (motif, cross, manual) | kept as is (scaled geometrically) |

The classifier is a state machine over the stitch stream with a lookahead window of 12
stitches, deterministic, with the thresholds above as constants and a test suite of blocks
OpenStitch itself generates (so the expected classes are known exactly). Jumps, trims and
colour changes always end a segment. The output is `Vec<Segment { class, range, geometry }>`.

A satin whose rungs are split (long stitches sewn as two) classifies as satin with a split
length; a fill with Frequency shifts still has a constant row spacing; a feathered or
gradation fill classifies row by row with a varying spacing (recorded per row).

## 3. Resize with recalculation

`stitches.resize(block, sx, sy, mode)`:

- `mode: geometric` — today's `object.scale`.
- `mode: density` — segment the block; for each segment:
  - **satin**: scale both rails geometrically, then regenerate the zigzag between them at the
    **original** rung spacing (`column::satin` on rungs rebuilt from the scaled rails at the
    old spacing). Width grows with the scale, the stitch count grows with the length. If a
    scaled rung exceeds `maxStitch` (12.1 mm), split it (the split-length attribute's rule).
  - **fill**: scale the covered area, taken from the recovered row spans directly (the
    union of the rows' extents, not a traced bitmap: the patent note in
    [patents.md](patents.md) says why the raster route is avoided), then regenerate rows at the original row spacing and stitch
    length with `generate::fill_edge_to_edge` at the original angle; the Frequency shift is
    the original grid shift; travel between sections as a fill today. The original needle
    pattern is not preserved (it cannot be, the grid changes), but the density is.
  - **run**: scale the path, resample at the original pitch (`generate::running`).
  - **tie / lock, other**: scaled geometrically; ties keep their size (they are re-emitted
    at the segment's new ends by `region::tie_and_lock`-style rules).
  - Segment order, colours, trims and jumps are preserved; a trim's position moves with its
    neighbours.
- Range: 50–250 % with a warning outside (what Embrilliance publishes as safe); below 50 %
  fills lose rows faster than detail, above 250 % satin stitches exceed the limit and are
  split into something that no longer reads as satin.

The result is still a stitch block (not objects), so the user keeps the purchased design's
look; `object.toShapes` remains the route to editable objects, and it gains the same
segmentation (its "blocks" mode becomes exact where the classifier is confident).

## 4. Density tuning and target stitch count

`stitches.tuneDensity(block, percent)` (−25…+25 %): segment, then regenerate satin at
`spacing × 100 / (100 + percent)`, fills at the same factor on the row spacing, runs
unchanged (their density is their pitch, left alone unless `includeRuns`). The size does
not change. Pattern fills and "other" segments are left as they are, which is what the
products advertise as "preserving pattern fills".

`stitches.targetCount(block or design, count)`: binary search over the tuning percentage
(a monotone function of the count) until the regenerated count is within 1 % of the target
or the percentage leaves ±40 %; reports the achieved count and factor.

## 5. Clean-up

- `stitches.removeHidden(block)`: B11's `stitches.clip` applied with the design's later
  objects and blocks as the clipping set (what `Remove Hidden Stitches` does), with the
  option to convert a hidden area's remaining border into satin (Embrilliance's variant) left
  for later.
- `stitches.declump(block)`: remove stitches shorter than `minStitch` that are not ties
  (the classifier knows which are), merging their neighbours; report the count.
- `stitches.limitDensity(block, fabric)`: tune every segment whose density exceeds the
  fabric table's maximum (B5) down to it; this is the one-click "fix for this fabric".

## 6. Parameters and defaults

| Command | Parameters | Defaults |
|---|---|---|
| `stitches.resize` | `sx`, `sy`, `mode: geometric | density` | `density` in the dialog, `geometric` for `object.scale` (unchanged) |
| warning range | 50–250 % | |
| `stitches.tuneDensity` | `percent`, `includeRuns` | ±25 max, runs excluded |
| `stitches.targetCount` | `count` | ±40 % search range |
| `stitches.removeHidden`, `declump`, `limitDensity` | | `minStitch` 0.3 mm |
| `stitches.segments` (read-only, for scripts and the inspector) | | |

## 7. Model and code changes

- `openstitch-design/src/segment.rs` (new): the classifier and the geometry recovery
  (rails, row spans, paths); unit tests from generated blocks.
- `stitch_edit.rs`: `resize_density`, `tune`, `declump`, `limit`; the ties re-emission.
- `convert.rs`: `object.toShapes` blocks mode uses `segment.rs` (its output improves, its
  tests are re-baselined with the classifier's known answers).
- Engine commands as above; `object.scale` unchanged; the resize dialog in the app offers
  "Keep density" (default on for stitch blocks) and shows the predicted stitch count.
- App: Design Database's convert dialog can offer "resize to fit hoop, keep density" for
  batch conversion later.

Effort: M. The classifier is the work (and the risk); the regeneration reuses the fill,
satin and running generators.

## 8. Test cases (internal regression tier)

Every expected value comes from blocks OpenStitch generates itself, so the ground truth is
exact.

| Spec | Script | Expected |
|---|---|---|
| `segment-generated-satin` | a 6 mm satin region → `object.toStitches` → `stitches.segments` | one satin segment covering all but the ties; rails within 0.2 mm of the region's outline; spacing = the region's |
| `segment-generated-fill` | a 40 × 20 mm fill at 30° → stitches → segments | one fill segment (plus travel runs); angle 30 ± 1°; row spacing and stitch length within 2 % |
| `segment-mixed` | a running outline + fill + satin border in one block | three classes in order, boundaries at the jumps/colour changes or within 2 stitches of the true ones |
| `segment-imported-pes` | the oracle fixtures (`crates/openstitch-formats/tests/fixtures`) | no panic; every stitch assigned; class proportions logged (no fixed expectation: external data) |
| `resize-satin-density` | satin block scaled 150 % with `density` | rung spacing unchanged within 5 %; width 150 %; no stitch over 12.1 mm |
| `resize-fill-density` | fill block scaled 60 % | row spacing and stitch length unchanged within 5 %; covered area 36 % of the original within 5 % |
| `resize-run` | running block scaled 200 % | pitch unchanged; path within 0.3 mm of the scaled original |
| `resize-geometric-unchanged` | `object.scale` on a block | identical to today's output (guard) |
| `resize-order-preserved` | mixed block | segment order, colour changes, trims and jumps in the same sequence |
| `tune-plus-25` | satin + fill block, +25 % | stitch count within 3 % of 1.25 × (satin + fill counts) + run count; size unchanged |
| `target-count` | design of 10 000 stitches, target 8 000 | achieved within 1 %; reported factor |
| `declump` | a block with 20 injected 0.1 mm stitches | exactly 20 removed, ties kept |
| `limit-density` | a 7 lines/mm satin block, fabric "thin" (max 4.5) | regenerated at 4.5 |

Measurement helpers needed: "segment list" (the command's output), "rung spacing / row
spacing / pitch of a block" (shared with B3, B5, B6), "covered area" (coverage trace).

## 9. Compatibility and licensing

`object.scale` and `object.toShapes` outputs are unchanged except where the classifier
improves the "blocks" mode (re-baselined, internal tier). Segmentation by stitch
statistics is generic signal analysis; regeneration uses OpenStitch's own generators.
Embrilliance's "patented" claim concerns multi-position hoop splitting, not density
tools, but the patent-check note lists it. "Density Repair Kit", "De-Clump", "Sweep",
"Outline Processor" and "Master Density" are product names; OpenStitch uses "Resize
keeping density", "Tune density", "Target stitch count", "Remove hidden", "Remove tiny
stitches", "Limit density for fabric".

## 10. Open questions

- Whether to regenerate fills with their recovered angle only, or also with the recovered
  Frequency shift (visible as the needle pattern); proposal: angle, spacing, length and
  shift, since the shift is cheap to estimate from the grid.
- Motif, cross-stitch and decorative segments: scaled geometrically today; a later step can
  recognise repeated motifs and re-tile them.
