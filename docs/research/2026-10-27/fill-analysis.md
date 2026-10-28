# Fill track: PE-DESIGN 11.50 fill and under sewing, measured (2026-10-27)

Black-box measurements of the round-1 reference PES files (PR #34), read from their PEC stitch
data with a small independent reader. Coordinates are in mm, y down, absolute page positions
unless noted. "Rel" values are PEC-relative (origin at the first needle point).

## Under sewing (default on, "Medium"): two layers before the top fill

Seen in every fill reference (fill-frequency-*, region-travel-u-*, hole-sewing, programmable-fill).

1. **Edge run.** A running stitch on the outline inset by **1.0 mm**, holes included: the star
   in hole-sewing gets its own inset run. Each straight side is split into equal stitches of at
   most **2.5 mm**: a 38 mm side gives 16 × 2.375, a 19 mm half side 8 × 2.375, and the pentagon
   sides 2.36–2.5. It starts at the shape's start point, inset: bottom centre (30, 49) for
   the Rectangle tool, the first vertex (11, 11) for the U drawn with Closed Straight Line. It
   goes once round, then on along the outline to where the rows begin.
2. **Rows across.** Tatami rows at **fill direction + 90°**, **2.0 mm apart**, stitch **4.0 mm**
   (= step pitch; not separable here because every file uses 4.0), needle shift **1.2 mm per
   row = 30 %**, rows **1.0 mm inside** the outline. The first and last rows lie on the inset
   (x = 11 … 49 for the 40 mm square). Row needle phase: row i (in sweep order) has its grid at
   (i+1) × 1.2 mm from the rotated bounding-box minimum (checked on rows 0–5 and 18–19 of the
   square).
   - Ordering ends at the start point: the square sews the left half outside-in (x = 11 → 29),
     runs along the bottom inset, sews the right half outside-in (49 → 31), and ends at (30, 49).
     The U sews x = 39 → 11 and ends at its start, the top-left (11, 11).
   - Row directions alternate so that the last row ends at the start point. When the needle
     arrives at the wrong end of the first row, PE sews that row twice (out and back): the U's
     x = 39 row.
3. Travel to the top layer: a straight running stitch of ≤ 2.0 mm (running pitch), e.g. 23 ×
   1.94 mm from (30, 50) to (10, 10) in the square, over the under sewing.

Programmable fill: same edge run, with rows at 45° in file (y-down) coordinates, i.e. across
the top layer's 135°.

## Top fill

- **Density 4.5 lines/mm.** Rows 0.2222 mm apart, rounded to the file's 0.1 mm grid (gaps
  0.2/0.2/0.3). The first row is **on** the edge (y = 10) and so is the last (y = 50): 181 rows
  for 40 mm.
- **Frequency.** Shift per row = Frequency × step pitch: 0 → 0, 25 % → 1.0 mm, 50 % → 2.0 mm.
  Row i (0-based from the first row) has its grid at (i+1) × shift from the rotated bbox
  minimum (x = 10): freq 0 points at 10, 14, …, 50; freq 25 row 0 at 11, 15, …; row 1 at 12, …;
  freq 50 row 0 at 12, row 1 at 10. So the Frequency semantics match the specs.
- Row ends: rows sewn in one direction keep both edge points. Rows sewn in the other direction
  have none: the stitch goes from the previous row's edge point to the next grid point. So the
  edge alternates, brick-like. The last row keeps both.
- The first row is sewn out and back when the needle arrives at its wrong end (row-direction
  parity is set so the last row ends nearest the exit). In the square, PE arrives at (10, 10),
  sews row 0 L→R, then R→L as the real row 0.
- Exit: after the last row (ending at (10, 50)) a 2.0 mm running stitch along the bottom edge
  goes back to the start point (30, 50). A 0.3 mm lock stitch sits at each end.
- Multi-part regions (U): PE sews right leg bottom → top, travels along the notch edge, left
  leg bottom → top, then the top bar up to the first vertex. Travel between parts is straight
  running at ≤ 2.0 mm. The U's first row (top) is also sewn out and back right after the under
  sewing, before the travel to the right leg.
- Pull compensation 0: rows end exactly on the outline.

## Direction convention (programmable-fill-45)

PE's default 45° programmable fill measures 135° as the dominant direction in the file (y
down), and its under sewing runs at 45° there. PE's angle turns counter-clockwise on screen (y
up). OpenStitch's angle turns clockwise on screen. See Spec questions in TRACKS.md.

## Gradation (region-travel-u-gradation)

With the default Gradation Pattern (Max 53 %, Min 5 %), PE's rows are about 0.45 mm apart at
the top (≈ 50 % of 4.5 lines/mm) and 1.4–2.0 mm at the bottom (≈ 11–16 %). Under sewing is
also present. 619 stitches against OpenStitch's 1088 with `gradation: 50`, which means 100 % →
50 % in OpenStitch. Matching needs PE's max/min gradation model (audit: "Gradation: the options
differ"). Not attempted in the fill PR.

## Measurement issues (TRACKS.md Spec questions)

- `rowShift` counts each under-sewing or travel needle point with a new y as a row. PE's
  arrays start [1.3, 0 ×15, 2.0, …]. The fill rows alone would give constant 0/1.0/2.0.
- `stitchedArea`: PE pentagon − star measures 669.4 mm². Horizontal stitches total 3355 mm
  (3254 mm fill only), with the median gap 0.2 instead of 0.2222. PE's slanted row-end stitches
  (dy 0.22, dx < 4.4) are excluded by the dy < 0.05·dx filter, about 500 mm of row length.
  OpenStitch at 4.5 lines/mm measures 767 for the same reason (quantised gap).

## What the fill PR changed in OpenStitch

- `RegionSewing::new` (fill fields only): Fill density 2.5 → **4.5**; Fill and Programmable fill
  `underlay` **on** with the new style `EdgeFill`. Programmable fill density is left at 2.5
  (not measured). The Frequency default stays unset (⅓ stagger), because `fill-frequency-default`
  pins it. PE's UI default is 30 %.
- `UnderlayStyle::EdgeFill` (region.rs `fill_underlay`): an edge run 1.0 mm inside the outline at
  ≤ 2.5 mm, then tatami rows at angle + 90°, `underlay_spacing` (2.0 mm) apart, with the top
  layer's stitch length, a 30 % shift and a 1.0 mm inset. New constant `FILL_UNDERLAY_INSET`;
  satin's `UNDERLAY_INSET` (0.5 mm) is unchanged.
- compile.rs: with `EdgeFill`, the under sewing and the top layer are joined by running stitches
  inside the region (no jump or trim between them).
- generate.rs `fill_edge_to_edge`: plain fill rows run from edge to edge (181 rows for 40 mm at
  4.5 rows/mm, as PE).
- Measures (openstitch-compat), approved by the coordinator, with synthetic tests:
  - `rowShift` counts only fill rows: runs with two consecutive interior points one stitch length
    apart.
  - `stitchedArea` = Σ row needle extent (including the stitches into and out of the row from
    ≤ 0.5 mm away) × the mean row gap. Re-measured: PE 839.9 mm², OpenStitch 839.9 mm²,
    spec 833.5 mm².
- Result: fill-frequency-0/25/50, region-travel-u-fill/along-outline/on-outline and
  hole-sewing-pentagon-star pass with their round-1 references.
- Still out: programmable-fill-45 (angle convention, a separate PR) and region-travel-u-gradation
  (PE's max/min gradation model).
- Not replicated: PE's exact row ordering (outside-in halves, ending at the start point), the
  out-and-back first row, edge points on alternate rows, and bbox-anchored needle phases.
