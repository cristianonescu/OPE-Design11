# Satin: PE-DESIGN 11.50 stitch structure (satin track, 2026-10-27)

Decoded from the round-1 reference PES files (PEC stitch block, 0.1 mm grid). Coordinates are mm
from the design's min corner. "Crossing" = one stitch from one edge to the other.
These are black-box observations of the saved output only.

## Common structure (every satin object)

1. **Start**: one 0.3 mm stitch at the start, no separate lock before the under sewing.
2. **Under sewing** (default on, "Medium"):
   - an **outline inset ~1.0 mm** from the region edge, stitch ≤ 2.5 mm (equal parts per side:
     2.30–2.40 mm on 28 mm sides, 2.5 mm on 20/25 mm sides); on a 2 mm wide column the inset is
     clamped to 0.9 mm, leaving a 0.2 mm thin loop along the centre line;
   - then **rows perpendicular to the satin direction, 2.0 mm apart**, inset 1 mm, needle pitch
     **4.0 mm**, serpentine across the whole region (the U's top bar and both legs in one pass).
     For Auto satin the rows run at 90° everywhere (vertical on the bent band, also in its
     horizontal leg).
3. **Travel to the satin start**: running stitches ~1.9–2.0 mm, inside the region (U: diagonal
   through the region; band: along the outline ~0.5 mm inside it). The band also has a 3-stitch
   lock (0.5 mm back and forth) before that travel.
4. **Top satin**, density = crossings per mm (4.5 → 0.222 mm between crossings, 0.444 mm between
   needle points on one edge). Pull compensation 0: the needle points are exactly on the edge.
   - **Fixed direction** (rect 90°, U 0°): one crossing *parallel* to the direction, the return
     crossing slanted (sawtooth). Rows longer than the machine limit are NOT split (U: 30 mm and
     10 mm stitches).
   - **Auto** and **Manual Punch**: symmetric zigzag (both crossings slanted); Auto adds "Half
     Stitch" short crossings on the inside of the bend (alternate needle points stop at 1.3 mm
     from the inner edge around the corner).
   - The U is split into sections at the direction: right leg (bottom→top), travel inside along
     the inner edge, left leg (bottom→top), then the top bar; each section's last row ends where
     the next travel begins.
   - The 5 × 2 mm rectangle starts at its centre: run along one edge to an end, satin back to the
     middle, run along the edge to the other end, satin back to the middle.
5. **End lock**: 3–4 stitches of 0.2–0.3 mm back and forth at the last needle point.

## Counts

| Spec | PE total | under sewing | travel + locks | top satin | OpenStitch (density 3.0, no under sewing) |
|---|---|---|---|---|---|
| satin-zigzag-rect (5×2, 90°, density 3) | 40 seg | 13 | ~11 (runs along edge, 0.2–0.3 mm moves, lock) | 16 crossings | 15 |
| satin-zigzag-column (Manual Punch, density 3) | 40 seg | 15 | 7 (start lock/run up the start end) + 2 end lock | 16 crossings | 16 |
| auto-satin-bent-band (Auto, 4.5) | 429 seg | 127 (49 outline + 78 rows) | 35 (lock + travel) + 4 end lock | 263 | 164 |
| region-travel-u-satin (0°, 4.5) | 466 seg | 188 (66 outline + 122 rows) | ~45 | ~230 | 232 |

The big gaps on the band and the U come from (a) density 4.5 against OpenStitch's default 3.0 and
(b) the under sewing (~30–40 % of PE's stitches), which OpenStitch has off by default.

## Consequences

- Rect / column: the specs' internal values (15 ±1, 0 edge stitches, 0 off-edge points,
  alternation 1.0, shortest ≥ 1.95 mm) describe top satin only and contradict PE's default output
  (see TRACKS.md "Spec questions"). They cannot pass both tiers.
- Band / U: pass on stitch count only with PE's defaults (satin density 4.5, under sewing on) —
  model.rs, fill track's file (TRACKS.md "Cross-track").
- Pull compensation default 0 in both; nothing to model.

## Update (satin PR): under sewing off references, and state

pe-gui remade rect and column with Under Sewing off; the files and MANIFEST entries are parked in
`satin-refs-underlay-off/` here (not in the satin PR, because those specs still fail).

- **Column (Manual Punch, density 3):** 18 segments = a 0.3 mm start stitch along the start end,
  15 symmetric crossings (needle points 0.7 mm apart on each edge, first at x=0, last at x=5.0), then a
  2-stitch end lock (0.3 mm along the end edge). OpenStitch: 16 crossings, no start stitch, no lock.
- **Rect (fixed 90°, density 3):** 27 segments. Starts mid-column on one edge, runs (1.2–1.3 mm) to
  one end, a sawtooth back to the middle (one crossing parallel to the direction, one slanted), a 0.3 mm
  hop plus one full crossing, a run along the edge to the other end, a sawtooth back to the middle, and a
  3-stitch end lock (0.2 mm). The slanted crossings are 2.12 mm (> the spec's 2.1 max), the hops are
  0.2–0.3 mm edge stitches.
- Both contradict the specs' internal values (15 ±1, edge 0, alternation 1.0, 1.95–2.1 mm). Under the
  coordinator's rule, those values may move to PE's observed ones with the reference cited (and audit
  item 1 corrected), and edge/alternation may exclude end travel/locks only via an explicit measurement
  option. That, plus OpenStitch's lock stitches, mid-start and sawtooth, is the follow-up.

Shipped in the satin PR: satin defaults 4.5 / under sewing on / EdgeZigzag; PE's satin under sewing
(1 mm inset outline + perpendicular rows 2 mm apart from the inset region, serpentine), linked to the
top satin without a jump; Auto satin rungs spaced along the longer side. auto-satin-bent-band
(394 vs PE 429) and region-travel-u-satin (524 vs 466) pass. Remaining differences there: no lock
stitches; OpenStitch splits satin stitches longer than 12 mm (compile.rs max_stitch), PE keeps
30 mm stitches; no Half Stitch on Auto bends; sawtooth for fixed direction not done.
