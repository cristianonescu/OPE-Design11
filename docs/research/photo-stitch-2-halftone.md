# Photo Stitch 2 as PE-DESIGN 11.50 sews it: a line screen (#129)

Status (2026-10-30): **measured; generator on hold.** The owner decides whether to build it
after a read of Brother's US 7,996,103 (see [Patent facts](#patent-facts)). This note and
`tools/screencmp.py` are the harness and the acceptance criteria for when it is built.

## The capture

pe-refs-4's round-4 `photostitch2-color-owner-shapes.pes`, made 2026-10-29. The input is the
owner-made `owner-shapes.bmp`: 240 × 240 px, a red disc, a green triangle, a yellow rectangle
and a blue square on white, placed at 84.7 mm.

Photo Stitch 2 Color ran at its defaults:

| Setting | Value |
|---|---|
| Line interval | 2.0 mm |
| Max. Density | 4.5 lines/mm |
| Direction | 0° |
| Add mask outline | off |
| Thread Chart | BROTHER EMBROIDERY |
| Color Set | Auto Select (the list also offers CMYK, RGBK, CRVK, BMYK) |

Brightness and Contrast were centred. The result has four threads and 14 133 stitches, 4 jumps
and no trims, 84 × 84 mm. The capture is not in the repository yet; pe-refs-4's reference PR
adds it.

## What PE-DESIGN sews

Measured with `tools/screencmp.py`, angles counter-clockwise on screen. The densities are
needle points per cm² in each colour area of the source image.

| Thread (PEC) | Line angle | Spacing | White | Red | Blue | Yellow | Green |
|---|---|---|---|---|---|---|---|
| 405 blue (2) | 0° | 2.0 mm | 21 | 19 | **297** | 19 | **312** |
| 800 red (5) | 45° | 2.0 mm | 23 | **298** | 20 | 20 | 19 |
| 206 yellow (58) | 90° (88° peak) | 2.0 mm | 23 | 20 | 20 | **296** | 75 |
| 614 purple (55) | 135° | 2.0 mm | 21 | 20 | 52 | 20 | 20 |

**Screen geometry:**
- Each thread sews one set of parallel lines across the whole mask, the white page included.
- The lines are 2.0 mm apart (the Line interval).
- Each thread has its own screen angle, 45° apart: 0°, 45°, 90°, 135°, in sewing order.

**Baseline:** where the thread's colour is not needed, its line is a straight running stitch of
about 2.4–2.5 mm. That gives about 20 needle points per cm² per thread everywhere, so about 88 on
white for four threads.

**Colour:** where the thread's colour is needed, the line becomes a zigzag across it. The zigzag
is 1.8 mm deep, 90 % of the interval, and advances 0.2 mm per stitch, about 5 stitches per mm or
one zigzag line every 0.4 mm. Its own area reaches about 300 needle points per cm²; blue, red and
yellow match to within 1 %.

**Mixing:** a colour without its own thread is mixed. The green triangle is blue's full zigzag
(312) plus a partial yellow one (75) on the baselines. The purple thread zigzags partly in the
blue area (52).

**Path:** one jump per thread and no trims. Every line is sewn end to end, serpentine.

## What OpenStitch sews today (Lines style, for comparison)

`digitize.photoStitch` with `style: lines` at its defaults, on the same image and placement:
7173 stitches, 9 threads, 173 trims.

- Every thread swings along horizontal rows 1.0–1.2 mm apart (angle 0°, or 171° where the
  zigzag dominates the histogram).
- Its own area gets about 105 needle points per cm².
- There is no baseline on white: 2–8 per cm² there.

The structure differs throughout, so no single parameter brings it closer. A change to the Lines
defaults alone would move the count further from PE's: 2.0 mm rows give 3290 stitches.

## Proposed model

1. **Threads:** keep the colour reduction to N threads. Auto Select gave 4 here, PE's Color Set
   of four; whether Auto Select is always four needs another capture. Assign the screen angles
   0°, 45°, 90°, 135° in sewing order, offset by the Direction setting.
2. **Lines:** for each thread, lines at its angle, Line interval apart (default 2.0 mm), over
   the whole mask. Each is sewn end to end, serpentine, with one jump per thread.
3. **Coverage:** along each line, sample the thread's share of the local colour. Where the share
   is near zero, sew a straight run at 2.5 mm. Where it is above zero, sew a zigzag across the
   line:
   - depth up to 90 % of the interval;
   - advance per stitch from 0.2 mm at full share, the Max. Density;
   - depth and advance following the share.
   The capture pins only the two ends, full share and none. The green area (one thread full,
   one partial) is the one mixed point.
4. **Mono:** one thread, presumably one line set, with the same coverage law on darkness. It
   has not been captured.

## Acceptance criteria (for the generator, on this capture)

Each criterion is measured with `tools/screencmp.py` on OpenStitch's export of the same image
and placement.

- **Line angle:** within 3° of 0°, 45°, 90° and 135° for the four threads.
- **Line spacing:** 2.0 ± 0.1 mm.
- **Own-area density:** within 10 % of PE's (297 / 298 / 296).
- **Baseline density:** within 25 % of PE's 19–23 per cm². The baseline count depends on the run
  stitch length and line ends.
- **Mixed green area:** the two main contributors are blue and yellow.
- **Total stitches:** within 10 % of 14 133.
- **Jumps:** at most one per thread.

## Captures still needed (owner-made, at defaults unless named)

- **Colour sets:** the same image with each Color Set (CMYK, RGBK, CRVK, BMYK), to pin the
  angle per colour and whether Auto Select is always four threads.
- **Direction:** Direction 30°, to confirm the whole screen turns.
- **Interval:** Line interval 3.0 mm and Max. Density 2.0 lines/mm, to confirm what each controls.
- **Mono:** Photo Stitch 2 Mono on `grey-tonal.bmp`, for the coverage law on a continuous tone.

## Patent facts

Not legal advice. These are the facts for the owner's lawyer, from
[beyond/patents.md](beyond/patents.md).

Brother's US 7,996,103 B2 is active until 2030-03-27. Claim 1 combines three elements:
1. A per-pixel angle feature computed from the image.
2. Line segments and colours generated from those features.
3. A decision for each connecting move whether to sew or jump it, by coverage, colour difference
   and crossing-angle tests.

The model above differs on each element:
- It uses a fixed angle per thread, the same everywhere: no per-pixel angle field.
- Its lines are straight screen lines, not image-derived segments.
- Its connector rule is serpentine line ends, with no coverage or colour test.

Whether that difference is enough is the lawyer's question, not this note's.

## Harness

```bash
python tools/screencmp.py owner-shapes.bmp 84.7 --fit photostitch2-color-owner-shapes.pes
python tools/screencmp.py owner-shapes.bmp 84.7 --at 0 0 ours.pes
```

`--fit` places the image over a file's stitch bounds; PE's file is drawn edge to edge here.
`--at X Y` places it by its top-left corner in mm, for OpenStitch's export.
