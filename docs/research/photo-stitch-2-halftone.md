# Photo Stitch 2 as PE-DESIGN 11.50 sews it: a line screen (#129)

Status (2026-10-30): **built.** The owner approved the generator on 2026-10-30 ("Go ahead",
asked whether the halftone model could go ahead despite Brother's US 7,996,103; see
[Patent facts](#patent-facts)). It is `crates/openstitch-digitize/src/screen.rs`, behind
`digitize.photoStitch` with `style: lines`. `tools/screencmp.py` and the test
`line_screen_meets_pe_design_capture` check it against the capture. Results are under
[Acceptance results](#acceptance-results).

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

## What OpenStitch sewed before the generator (Lines style)

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

## Acceptance results

Measured on 2026-10-30 with `tools/screencmp.py`. OpenStitch ran `digitize.photoStitch`
`style: lines` at its defaults: 4 colours, Line interval 2.0 mm, Max. Density 4.5, Direction 0.
Same image and placement as the capture.

| Criterion | PE-DESIGN | OpenStitch | Met |
|---|---|---|---|
| Line angles | 0, 45, 88, 135 | 0, 45, 88, 135 (the lines run at exactly 90°; 88 is the histogram peak with the zigzag) | yes |
| Line spacing | 2.0 mm | 2.0 mm | yes |
| Own-area density | 297 / 298 / 296 | 280 / 281 / 292 / 290 | yes, within 6 % |
| Baseline density (white) | 21–23 | 21–24 | yes |
| Total stitches | 14 133 | 13 701 (−3.1 %) | yes |
| Jumps | 4, one per thread; no trims | 4, one per thread; no trims | yes |
| Mixed green area | blue and yellow | green has its own thread | **no** |

The last criterion fails because of PE's colour-set choice, not the screen. PE's Auto Select
chose blue, red, yellow and purple for an image whose colours are red, green, blue and yellow.
Green is then a mix, and purple adds a fifth line set. OpenStitch reduces the image to its own
four colours, so green has its own thread. How Auto Select chooses its four, and what CMYK,
RGBK, CRVK and BMYK hold, needs the colour-set captures below.

The same checks run in the test suite on the owner's image rebuilt procedurally:
`line_screen_meets_pe_design_capture`. It asserts:
- angles within 3° (on the straight baseline stitches);
- lines 2.0 ± 0.1 mm apart;
- own-area density within 10 % of 297;
- white within 25 % of 21;
- one jump per thread;
- total within 10 % of 14 133.

Implementation choices beyond the capture:
- **Shares.** Each grid cell (0.5 mm) is matched as the fabric plus a non-negative mix of the
  threads' reduced colours, summing to at most 1. In mono the share is the darkness after the
  full-range contrast stretch.
- **Zigzag.** A thread zigzags where its share is at least 0.08. Each stitch advances
  `1 / (Max. Density × 4/3) / share` mm: 0.17 mm at full share, PE's ≈ 6 needle points per mm.
  The depth stays at 90 % of the interval.
- **More than four threads.** A fifth thread and later ones take the four angles shifted by
  22.5°.
- **Moves between lines.** Line ends are joined by a stitch when they are within three
  intervals.

## Second round of captures (2026-10-30)

These are pe-refs-4's round-4 `photostitch2-cmyk`, `-rgbk`, `-crvk` (labelled CRYK in PE),
`-bmyk`, `-dir30`, `-int3-den2`, `-mono-grey-tonal` and `-specks-auto`. Each was made from a
fresh wizard at its defaults with one setting changed.

PE's input ranges: Line interval 1.2–5 mm; Max. Density 2.5–5 lines/mm (2.0 is refused, so
that capture used 2.5).

| Capture | PE-DESIGN | OpenStitch | |
|---|---|---|---|
| CMYK (PEC 32, 51, 13, 20 in order) | angles 0/45/90/135, 13 954 st | same threads, order and angles, 12 757 st | mixing differs |
| RGBK (5, 2, 14, 20) | 14 420 st | 12 729 st | mixing differs |
| CRYK (32, 5, 13, 20) | 14 926 st | 12 757 st | mixing differs |
| BMYK (2, 51, 13, 20) | 13 963 st | 12 897 st | mixing differs |
| Direction 30° | angles 30/75/120/165, 13 434 st | angles 30/75/120/165 | as PE |
| Interval 3.0, density 2.5 | spacing 3.0 mm, own areas 100–124/cm², 6842 st | spacing 3.0 mm, 90–106/cm², 6666 st | as PE |
| Mono, grey-tonal | 4665 st; bands 305 / 286 / 212 / 153 / 107 / 72 / 21 / 26 | 4671 st; 306 / 275 / 221 / 162 / 107 / 59 / 23 / 23 | as PE (ramp below) |
| Auto Select on specks.bmp | four threads (red, blue, ultramarine, lime green) for a red-and-blue image | two threads | open |

Findings:
- **Angles follow the sewing position, never the colour.** Every capture has 0°, 45°, 90° and
  135° in sewing order (30° more with Direction 30°). A named Color Set is four fixed chart
  threads sewn in its listed order. `ColorSet` and `colorSet` implement them.
- **Mono tone curve.** PE sews no zigzag below about 17 % darkness and full density from about
  86 % (after the contrast stretch), linear between. `screen::mono_share` implements that ramp.
  The old linear share from 0 sewed the light tones twice as densely (49 against 21 needle
  points per cm² at grey 180–209).
- **Mixing within a named set (open).** PE gives an area's main thread nearly full coverage,
  about 300 needle points per cm². Least squares spreads the colour over several threads, so
  OpenStitch's main threads reach 231–276. Neither nearest-thread nor least squares explains
  PE's choices:
  - in RGBK, image yellow becomes red (181) plus lime green (173), though lime is the nearest
    thread;
  - in CRYK, yellow becomes red (279) plus yellow (231);
  - in CMYK, green becomes sky blue (full) plus black (94).
  A colour ramp per set would pin the law.
- **Auto Select (open).** It always gives four threads; on a red-and-blue image it adds
  ultramarine and lime green. Our Auto Select reduces to the image's own colours.

## Colour ramp (2026-10-30)

pe-refs-4's `photostitch2-ramp-cmyk.pes` and `-ramp-rgbk.pes`: `ramp.bmp`, 80 × 20 mm, HSV hue
0–300° across, S = V = 1. The band shares are in its `ramp-bands.txt`.

Findings:
- **Sewing order.** PE sews the most-used thread first. On the ramp CMYK sews cyan, yellow,
  magenta, black, where on the shapes it sews cyan, magenta, yellow, black. Every capture, Auto
  Select included, fits that rule, and the screen angles follow it. OpenStitch's Photo Stitch 2
  now orders threads by their total share; Photo Stitch 1 still sews light threads first.
- **Mixing.** Each hue goes to the one or two set colours nearest it, and a saturated colour's
  chromatic threads sum to about full coverage.

Five candidate laws were scored by mean share error on the 32 ramp bands and on the 8
owner-shapes areas:

| Law | Ramp | Shapes |
|---|---|---|
| Least squares (as before) | 0.069 | 0.107 |
| Two nearest threads by RGB distance | 0.118 | 0.105 |
| Hue interpolation between the set's thread hues, with plateaus | 0.073–0.090 | 0.155–0.192 |
| **Least squares, chromatic shares raised to `min(1, saturation / 0.8)`** | **0.064** | **0.093** |

The last law is now `screen::saturate`. It only raises shares that fall short, so pale colours
stay light.

| Set (owner shapes) | PE | before | now |
|---|---|---|---|
| CMYK | 13 954 | 12 757 (−8.6 %) | 13 704 (−1.8 %) |
| RGBK | 14 420 | 12 729 (−11.7 %) | 13 501 (−6.4 %) |
| CRYK | 14 926 | 12 757 (−14.5 %) | 14 049 (−5.9 %) |
| BMYK | 13 963 | 12 897 (−7.6 %) | 13 510 (−3.2 %) |

On the ramp, RGBK sews 5743 stitches against PE's 5892. CMYK sews 5793 against 6522, because PE
gives red half to yellow and half to magenta where least squares gives magenta two thirds.

Still open:
- the exact transitions between neighbouring set colours;
- why CMYK collapses green and blue onto cyan alone;
- Auto Select's choice of four threads: round 5 narrows it to a CMYK-like base with
  substitutions (section above); round 6's swatches are requested.

## Auto Select on the test images (round 5, 2026-10-31)

Eight Higgsfield test images went through Photo Stitch 2 Auto Select at defaults, with
PE-DESIGN's default chart, Ver4 Option Color. The threads were read from the PES files and
named by Ver4 number:

| Image | Auto Select picks |
|---|---|
| g09 rainbow gradient | 70 sky blue, 89 yellow2 (lime), 76 purple, 90 dark fuchsia (pink-red) |
| g13 twelve-colour mosaic | 89, 90, 66 prussian blue (navy), 70 |
| g14 orange circle on navy | 66, 90, 89, 75 dark brown (maroon) |
| p01 portrait | 75, 66, 89, 90 |
| p04 ginger cat | 70, 75, 90, 89 |
| p06 sunset | 75, 66, 90, 89 |
| p07 fruit bowl | 75, 70, 89, 90 |
| p09 facade (blue sky, grey and white) | 77 cornflower, 69 moss green, 66, 85 magenta |

What they show:
- **The palette.** Auto Select always takes four threads from the 29 Ver4 Option colours.
  OpenStitch reduces the image to N colours and matches each to the nearest PEC thread. Even
  the two-colour poster g14 gets four threads.
- **The base.** Seven of the eight are PE's own CMYK set (70, 90, 89, 75) with at most two
  threads swapped:
  - navy 66 for sky blue 70 (p01, p06, g14), or for maroon 75 (g13);
  - purple 76 for maroon 75 on the gradient, which has no dark.

  p04 and p07 are exactly CMYK. Only the facade, almost without warm colours, leaves the base.
- **Not a mixing optimum.** For every 4-subset of the 29 colours, the best feasible mix of
  each image colour (non-negative coverage shares over white fabric, least squares in RGB)
  was computed and the subsets ranked. PE's set ranks 62nd to 6530th, and on the mosaic and
  the facade it mixes worse than plain CMYK.
- **Not a colour mapping either.** Mapping the image's 4–8 median-cut colours to the nearest
  Ver4 colour, plainly or after full saturation, matches at best 13 of the 32 picks. "Always
  CMYK with navy for sky blue" matches 24.

### Round 6: single colours (2026-10-31)

The reference session ran 20 flat swatches and 3 two-colour pairs through Auto Select at
defaults:
- 12 hues every 30°;
- navy, maroon, brown, tan, grey, black, teal and olive;
- the pairs red + blue, yellow + magenta, and tan + navy.

Each swatch was placed at 50 mm on white; the threads were read from the dialog (Ver4
numbers, in its order).

| Image | Picks | Image | Picks |
|---|---|---|---|
| hue 0° (255,0,0) | 84 90 81 83 | navy | 66 75 69 86 |
| hue 30° | 89 90 84 83 | maroon | 75 81 83 91 |
| hue 60° (yellow) | 89 84 83 91 | brown | 75 84 80 67 |
| hue 90° | 80 83 84 92 | tan | 89 78 90 85 |
| hue 120° (green) | 71 84 83 91 | grey 128 | 84 81 83 91 |
| hue 150° | 73 85 84 89 | black | 75 69 85 83 |
| hue 180° (cyan) | 74 84 81 91 | teal | 66 72 68 87 |
| hue 210° | 68 82 84 91 | olive | 89 84 75 87 |
| hue 240° (blue) | 67 82 84 91 | red + blue | 67 90 84 82 |
| hue 270° | 76 92 84 82 | yellow + magenta | 89 92 84 83 |
| hue 300° (magenta) | 92 84 83 81 | tan + navy | 66 89 75 90 |
| hue 330° | 90 84 83 81 | | |

What they show:
- **The image's own colour.** In 14 of 20 swatches, the Ver4 colour nearest the swatch by
  RGB is in the set, usually in the first slot. The exceptions sit between two grid colours
  (hue 90°, 210°, 270°, 330°, teal) or are neutral (grey).
- **Fillers.** A flat colour still gets four threads. The rest come from a short list:
  - red 84 (13 of 20), light blue 83 (10), deep rose 91 (7), leaf green 81 (6);
  - a filler close to the image's colour is replaced by a neighbour: 82 for 83 next to blue,
    92 for 91 next to violet, 81 for 91 next to rose.
- **Pairs.** Each colour of a pair gets its own nearest thread, and fillers complete the four.

Still not a rule. "Nearest colours of the image's main clusters, then fillers from 84, 83,
81, 91 skipping any close to a chosen colour" reproduces 68 of the 124 picks over rounds 5
and 6. Floyd–Steinberg dithering onto the 29 colours, keeping the four most used, reproduces
51. OpenStitch's Auto Select stays as it is. The captures and their notes are in the owner's
folder (`round6/autoselect`).

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

Whether that difference is enough is the lawyer's question, not this note's. The owner
decided on 2026-10-30 to go ahead with this fixed-angle design. The generator keeps to it:
- the angles come from `screen::ANGLES` and the Direction setting only, never from the image;
- the lines are straight;
- a move between lines is sewn when it is short and jumped otherwise, with no coverage, colour
  or angle test.

## Harness

```bash
python tools/screencmp.py owner-shapes.bmp 84.7 --fit photostitch2-color-owner-shapes.pes
python tools/screencmp.py owner-shapes.bmp 84.7 --at 0 0 ours.pes
```

`--fit` places the image over a file's stitch bounds; PE's file is drawn edge to edge here.
`--at X Y` places it by its top-left corner in mm, for OpenStitch's export.
