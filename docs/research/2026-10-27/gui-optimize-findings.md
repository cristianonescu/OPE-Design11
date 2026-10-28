# gui track: what PE-DESIGN 11.50's Optimize Sewing Order does (2026-10-27)

Black-box only: shapes drawn with the Rectangle tool, files saved with Save As, stitches read from
the PEC block. Files and SHA-256 are in <reference-notes>\round2.
Setup for all: Design Page 100 x 100 mm, grid 1 mm with snap, Rectangle tool, line Not Sewn,
Fill Stitch defaults (4.5 line/mm, 45 deg, under sewing on, Medium), 10 x 10 mm squares with
top-left corners at (0,0), (50,0), (25,0), drawn in that order. Colour 800 EMBROIDERY RED.

| File | Options > Optimize Entry/Exit Points | Optimize Sewing Order clicked | Sewn order (x) | Travel gaps mm |
|---|---|---|---|---|
| r1-optimize.pes (round-1 reference, PR #34) | on | yes | 0, 50, 25 | 41.0, 16.0 |
| exp1: round-1 file reopened, Optimize clicked again, Save As | on | yes (again) | 0, 50, 25 | 41.0, 16.0 (stitches identical) |
| exp2-drawn-no-optimize | on | no | 0, 50, 25 | 41.0, 16.0 |
| exp3-drawn-optimize | on | yes | 0, 50, 25 | 41.0, 16.0 (stitches identical to exp2) |
| exp4-entryexit-off-no-optimize | off | no | 0, 50, 25 | 50.0, 25.0 (bottom-centre default entry/exit) |
| exp5-entryexit-off-optimize | off | yes | 0, 50, 25 | 41.0, 16.0 (stitches identical to exp2) |
| exp6-red-blue-red-optimize (square at x 50 = 007 PRUSSIAN BLUE) | off | yes | red 0, red 25, blue 50 | 16.0 (red to red); 25.0 jump to blue, not linked |

Conclusions:
1. The round-1 reference was made correctly: Optimize Sewing Order did run (exp5 = round-1 stitches).
2. Optimize Sewing Order does NOT reorder objects of the same colour by distance. It groups
   colours, keeping drawing order within a colour (exp6: the two reds become consecutive, still 0 then 25).
3. It moves the exit/entry points between consecutive SAME-colour objects to facing points
   (exit on the outline corner, entry stitch about 1 mm inside the next outline, so 40 -> 41.0 and 15 -> 16.0).
   Between different colours it leaves the default points (exp6: 25.0).
4. Options > Optimize Entry/Exit Points ON does the same linking as soon as a shape is drawn.
   With it off, clicking Optimize Sewing Order produces the same result.
5. After Optimize, the Sewing Order pane lists the same order. The button stays highlighted
   (a toggle), and a reopened file shows it off.

OpenStitch today (order.rs `optimized`): greedy nearest-centre within the current colour, so it
gives [1,3,2] and gaps [15,15], matching the spec's expected values but not PE-DESIGN. `link_entry_exit`
links every consecutive pair, including pairs of different colours.

## OpenStitch with PE's ordering (gui branch gui/optimize-sewing-order-pe11, local WIP)

order.rs changed: same colour keeps drawing order (no nearest sort); entry/exit linked only between
same-colour neighbours. compat for optimize-entry-exit then gives:
- order [1,2,3] (spec expected [1,3,2]: fail)
- travel [40.0, 15.0] (spec expected [15,15]: fail; PE reference [41.31, 16.33]: fail by 1.3 mm)
- travel-total 55.0 vs PE 57.04 (tol 2: fail by 0.04)

The remaining 1.3 mm is under sewing, not ordering. PE ends the previous square's fill on the outline
corner (5,-9), then jumps to the next square's under sewing, which starts 1 mm inside the outline at
the corner nearest the entry ((46,-8) with outline at 45,-9) and runs an inset rectangle first. Every
PE square also starts 1 mm in from the default bottom-centre entry. OpenStitch's first stitch after the jump
is on the outline itself, with about 105 stitches per square against PE's 233 (structure differs: fill track).
So the travel reference checks probably follow once fill's under sewing starts inset at the entry.
