# PE-DESIGN 11.50 fill and satin rules, from parameter sweeps (stitch-sweeps track)

Black-box only: PES files saved by PE-DESIGN through its UI (made by pe-gui), read from their PEC
stitch data. Files: `compat/reference/stitches/sweep-*.pes` in the stitch-sweeps worktree (copies and
log.txt in `<reference-notes>\round2\sweeps\`). mm, y down; "rel" = PEC-relative
(origin = the start point, the middle of the square's bottom edge).

Base for the fill sweeps: Rectangle tool, 40 × 40 mm square at (10, 10); line Not Sewn; Fill Stitch,
Direction Manual 0°, Under sewing OFF, Density 4.5, Step pitch 4.0, Frequency 30 %, Pull 0.0.

## F1 Density (S1: 0.1, 2.5, 4.5, 6.0, 14.0 lines/mm)

- Field: 0.1 – 14.0 lines/mm, step 0.1, default 4.5. (OpenStitch: 0.5 – 10 → fix the range.)
- Rows are exactly 1/density apart, the first row ON the top edge and the last ON the bottom edge:
  rows = H·d + 1 (40 mm: 5 / 101 / 181 / 241 / 561). Row lines are rounded to the file's 0.1 mm grid
  (4.5: gaps 0.2/0.2/0.3; 6.0: 0.2/0.1/0.2; 14.0: some rows share a line).
- Status-bar stitches 125 / 1152 / 2024 / 2678 / 6166 (file records 123 / 1150 / 2022 / 2676 / 6164).
- H·d not whole (sweep-fill-density-4p5-h33, 40 × 33 mm, 4.5 → 148.5): PE sews 149 rows, first on y = 0 and
  last ON the far edge (33.0), mean gap 0.2230 = 33/148: the spacing is stretched so both edges get a row;
  gaps = floor or round of H·d (148.5 cannot tell them apart; a file with fraction > 0.5 would). OpenStitch
  sews 149 rows at the nominal 0.2222, its last row 0.11 mm short of the edge → PR 2 (fill-row-structure).
  1671 records, status bar 1673.
- OpenStitch already matches the spacing and count (generate.rs `edge_row_spans`).

## F2 Pull compensation (S2: 0.2, 0.5, 1.0, 2.0)

- Field: 0.0 – 2.0 mm (max 2.0). Each row end moves out by p along the row: width 40 + 2p
  (40.4 / 41.0 / 42.0 / 44.0); height unchanged (rows are not lengthened across, the first and last
  row stay on the edges).
- The needle grid does not move: interior points are identical to p = 0 (anchored to the
  uncompensated outline; with p ≥ 0.4 a grid point beyond the old edge, e.g. x = −20.4, appears).
- Under sewing (0p5-us) gets NO compensation: its edge run stays 1.0 mm inside (rel x ±19) and its
  rows across span rel y −38 … 0 (1.0 mm inside), as with p = 0.
- OpenStitch already matches (row spans ± p; `FILL_UNDERLAY_INSET` fixed).

## F3 Top-layer structure (seen in every S1/S2 file; OpenStitch differs)

- Needle grid: Frequency 30 % → 1.2 mm shift per row, row i's grid at (i+1)·shift from the rotated
  bbox minimum (stitch lengths at row ends 0.4, 0.8 … 3.6). OpenStitch's default is the ⅓ stagger
  anchored at the page origin (ends 3.3 / 4.7).
- Brick row ends: rows sewn in one direction keep both edge points; rows sewn the other way have
  neither (the stitch goes from the previous row's edge point straight to the first grid point).
  OpenStitch keeps both ends on every row (180 short 0.2/0.3 mm edge stitches PE does not have).
- Entry: needle at the start point (rel 0, 0), tie-in, then ONE straight running line at ≤ 2.0 mm
  (1.94) diagonally inside to the first row's start corner (top-left, rel −20, −40). OpenStitch
  runs along the outline at 2.5 mm.
- Exit: after the last row (ending at the bottom-left, rel −20, 0) a 2.0 mm run along the bottom edge
  back to the start point, then a lock: (0,0) → (−0.3, 0) → (0, 0).

## F4 Step pitch (S6: 1.0 min, 2.0, 3.0, 6.0, 10.0 max) and Frequency (S7: 10, 30, 40 %)

- Step pitch field 1.0 – 10.0 mm (OpenStitch 1.0 – 10.0: same). Interior stitches are exactly the pitch.
- Needle grid of row i (i = 0 at the first row in sewing order, top): x = bbox_min + ((i+1)·f·L mod L) + k·L,
  with f = Frequency, L = pitch. Checked: pitch 10 / f 30 % → row 0 at −17 (rel), brick rows (−17 … 13);
  pitch 2 → (−18.2 … 17.8); f 10 % → 0.4 mm shift, f 40 % → 1.6 mm. Same as fill-analysis.md's 4.0 mm rule.
- Rows and their spacing do not change with pitch or Frequency (181 rows, gaps 0.2/0.3).

## S1 (satin) Satin density and pull compensation (S3: 40 × 6 mm, 90°, under sewing off)

- Satin Density field 0.1 – 14.0 (default 4.5), Pull 0 – 2.0 mm (default 0), as fill.
- One crossing every 1/density mm along the column: a stitch straight across, then a slanted
  stitch back to the other edge 2/density further on (pairs 0.44 mm apart at 4.5). 4.5 → 184
  crossings of 6.0 mm; 14 → 564; 0.1 → 5 crossings 10 mm apart (stitches 6.0 and 11.7 diagonal).
- Pull p: every crossing 6 + 2p (7.0 at 0.5, 10.0 at 2.0): both edges move out by p.
- Layout of a Rectangle-tool satin: start at the middle of the bottom edge, run 2.0 mm along the
  edge to the left end, satin back to the middle, run 2.0 mm along the edge to the right end, satin
  back to the middle, end lock 0.3 mm across (0, −0.3, −0.6, −0.3, 0). [→ satin track: sections]

## U1 Under sewing (S4, partly; fill square with Under sewing ticked)

- Options seen: type 1 (default) / type 2; Density Medium (default) / Dense / (Light?).
- Type 1 Medium: edge run 1.0 mm inside at ≤ 2.5 mm (2.375 here) once round from the start point
  (bottom middle, leftwards), on to the bottom-left inset corner, then rows across (perpendicular to
  the fill) 2.0 mm apart, x = −19 … 19 rel, 4.0 mm stitches. Type 1 Dense: rows 1.4 mm apart (38/27).
- Under sewing Density: Light 3.45 mm (38/11), Medium 2.0 (38/19), Dense 1.41 (38/27): consistent with nominal
  3.5 / 2.0 / 1.4 mm and the gap count round(inset width / nominal), first and last row on the 1 mm inset.
- Type 2 (2651 st vs 2329): the same edge run, then a CROSS-HATCH: rows at +45° and then −45° to the
  fill direction, 2.0 mm apart (2.8 mm along the edges), 4.0 mm stitches (2.8 + 2.8), joined by
  1.4 mm steps along the 1 mm inset edge. Each layer is sewn in two halves split by a straight
  2.5 mm-stitch run from the start point (bottom middle) to an inset top corner. OpenStitch has no
  equivalent style (its Double = edge + zigzag?) [→ fill].

## F5 Direction (S5: 30, 45, 90, 135° on the square; U at 45°)

- Rows run along the angle counter-clockwise on screen (y up), as OpenStitch since the angle-ccw PR.
- Row spacing is 1/density measured across the rows at every angle (mean gap 0.2221 at 30°, 0.2218
  at 45°, 0.2210 at 90°); the 0.1 mm file grid makes single gaps 0.14 – 0.24.
- Rows run from the rotated bounding box's one side to the other (rows near a 45° corner are a
  single short stitch). Status-bar counts: 30° 2043 st, 45° 2067, 90° 2034, 135° 2062.
- Entry for 0°/30°/45°: diagonal travel from the bottom-middle start point as at 0°; for 90°/135°
  the needle first runs 2.0 mm along the bottom edge (PE takes the nearer row end).

## S2 (satin) Satin width (S8: 2, 3, 10, 15 mm at density 4.5)

- 184 crossings at every width, each the full width: PE never splits a satin stitch (15 mm kept).

## F6 Row order, halves, out-and-back and edge points (convex fill; base + S5 files)

Let u be the row direction (PE angle, ccw on screen) and rows be indexed 0..N from the side where
the normal n = (sin a, cos a) (file coords, y down) is smallest (0°: top; 90°: left; 45°/135°: the
corner at the far end). s = the start point (= exit for a Rectangle-tool shape).
- HALVES: rows on each side of s are sewn as two halves, each from its far edge inward to s; the
  row through s belongs to both (90°: x = −20 → 0, then 20 → 0; 182 passes for 181 rows). The
  min-side half goes first. At 0° s lies on the last row, so there is one half.
- TRAVEL: one straight run, equal stitches ≤ 2.0 mm, from s to the first row's +u end (0°: 23 ×
  1.94 to the top-left corner; 90°: 10 × 2.0 along the bottom edge), from s to half B's first row
  (90°: 10 × 2.0), and from the last row's end back to s (0°: 10 × 2.0).
- PASSES: each half's first row is sewn +u, its last row −u (ending at the end nearer s). When the
  half has an odd number of rows the first row is sewn out and back (+u, then −u on the same grid).
- EDGE POINTS: +u passes keep both edge points; −u passes have none (the stitch goes from the
  previous +u row's end straight to the first grid point, and from the last grid point to the
  next +u row's edge point), except each half's last row, which keeps both. When rows are far
  apart (0.1 lines/mm: 10 mm) the needle walks the edge between rows at 2.0 mm and the −u row
  keeps the edge point it walks from. (Threshold between 0.4 and 10 mm gaps: not observed.)
- GRID: the needle grid of row k (geometric index from the min side, not sewing order) is at
  ((k+1)·f·L mod L) from the row's +u start side of the rotated bbox (90°, half B: x = 20 is row
  180 → 217.2 mod 4 = 1.2 from the bottom; x = 19.8 → 0.0).
- SPACING: gaps = floor(H·d); spacing = H / gaps so both edges get a row (h33).
- LOCKS: tie-in = the first needle point lies 0.3 mm along the first stitch, then s (PE's first
  PEC record); end lock = 0.3 mm back along the last stitch, then s.

## F7 Grid anchor and multi-span rows (round-1 U and pentagon references, 2026-10-27)

- The needle grid starts at the FIRST ROW's start (the outline's extreme point across the rows),
  not the bounding box: the pentagon's rows (stretched spacing H/floor(H·d) = 0.22346) all fit
  x = top-vertex x + (k+1)·1.2 mod 4 (152 of 152 rows; with the bbox minimum they are off by 3.1).
  For the square and the U the first row starts at the bbox minimum, so both rules agree there.
- PE stretches the row spacing on the pentagon too (non-convex, with a hole), not only rectangles.
- Multi-span rows (U, holes) also have brick ends and keep grid points down to 0.3 mm from a row end;
  OpenStitch's older ¼-stitch rule there made 4.4/4.8 mm join stitches PE never has.
- OPEN: angled squares (30/45/135°) fit neither anchor exactly (pescmp mean 0.30–0.43 mm with the
  first-row anchor; 135° 0.22 with the bbox anchor). Needs a per-row analysis of the rotated files.
