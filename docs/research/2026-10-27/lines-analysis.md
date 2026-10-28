# lines track — analysis (phase 2)

Branch `lines/calibrate-pe11` in worktree `.claude/worktrees/logo-splash-17a75d` (session local_a72d845c... the brand session).

## Status (2026-10-27)
- Specs written (references pending): compat/specs/stitches/line-{running,triple,zigzag,e,v,motif,stem,candlewick,chain}-default.json,
  line-zigzag-density-{min,max}.json. Shape: open line (10,20)-(50,20)-(50,40) mm, no attribute set.
  Measures: stitch count (±10 %), median and max stitch length, the four bounds (width/side/corner/ends).
- Reference requests posted for pe-gui (TRACKS.md "[lines → pe-gui"): defaults (11 files) and Start/End Feed vs Run (8 files).

## OpenStitch's current line defaults (LineSewing::new, compile.rs)
pitch 2.5 mm (all run pitches), run_times 1, width 2.0 mm, density 3.0 /mm (zigzag stitches per mm;
candlewick spokes = density x 2), motif height 3.0, spacing: motif 0, stem 1.5, candlewick 5.0, E/V 3.0,
others 1.0; stem angle 45, stroke_times 1, arrange Normal, chain Triangle.
Known PE UI defaults (audit): running 2.0 mm; zigzag 2.0 mm at 4.5 lines/mm.
line-running-default fails today on its internal check (median 2.5 vs 2.0); zigzag passes (width 2.0).

## Start/End Type (guide 16_1293444)
Applies when a line's entry/exit point is changed or optimised; motif, stem, candlewicking, chain only.
Feed: jump stitches at the start and end of the pattern stitching. Run: running stitches along the path at the
start and between pattern stitching. OpenStitch today restarts the pattern at the moved entry (c.starting_at(p)).

## Measured (2026-10-27, pe-gui's round2/lines files; decoder: PEC records, mm relative to the first needle point)

PE-DESIGN 11.50 defaults recorded by pe-gui (Sewing Attributes > Line sew, shown on switching the type; switching
RESETS every attribute to the type's default):
- Running: Run pitch 2.0, Run times 1. Triple: Run pitch 2.0.
- Zigzag: Under sewing on, width 2.0, density 4.5 line/mm (range 0.1-14.0), Half Stitch on, Sharp Corners 1st, Miter limit 0.0.
- E/V: prong spacing 5.0, prong height 2.5, run pitch 2.0, Run times 1, Stroke times 1 (greyed for V), Arrange 1st.
- Stem: width 2.0, spacing 1.5, angle 45, Stitch Type 1st, Start/End Feed.
- Candlewicking: Density Medium (drop-down), size 5.0, spacing 1.0, Start/End Feed.
- Chain: triangle, size 4.0, Time 5, Arrange 1st, Start/End Feed.
- Motif: motif001 (PE's own four-petal flower; we can't ship it), 12.3 x 12.3 mm, offset 0, spacing 0, run pitch 5.0, Start/End Feed.

Running / triple: stitches exactly 2.0 mm (equal division per segment), corner kept. Triple = forward, back, forward per step
(OpenStitch already identical). Lock: tie-in 0.3 mm along the first stitch and back; tie-off one 0.3 mm step back along the
last stitch and forward = region::tie_and_lock (satin's rule). Applied to running, triple, zigzag, E, V.

E: a prong at every multiple of the spacing along the arc (start, corner, end included), height 2.5 on the RIGHT of the sewing
direction; at a corner the prong points along the bisector (38.2,1.8 from (40,0)). Baseline between prongs split evenly at
<= run pitch (5.0 -> 3 x 1.67). Lock as above (0.3 along the first prong).

V: a V at every multiple of the spacing (start, corner, end): line -> apex (2.5 to the right, bisector at corners) -> line,
arms meeting the line +-1.0 mm from the station (OpenStitch: interval/5 - NOT verified: 1.0 could also be pitch/2 or
height*0.4; a file with another spacing or height decides). Start: start point -> apex -> +1.0; end: -1.0 -> apex -> end.
Baseline between arms split at <= pitch (3.0 -> 2 x 1.5). PE's start lock on V is longer (0.3, 0.6, 0.9, back 0.5, 0);
OpenStitch uses the one-step lock; the spec measures the mean of stitches >= 1 mm (the median flips on that lock alone).

Zigzag (line-zigzag-default, density-max): under sewing = a run offset width/4 to the right (mitred at corners, starting
and ending width/4 in from the line's ends) at <= 2.0 mm, across, and back width/4 on the left. Then the zigzag:
rails = the line offset +-width/2, mitred at corners. Within each straight leg, a stitch joins the two rails at the SAME
fraction of the leg (outer leads inner by 1.0 mm at mid-leg of a 90-degree corner: measured 20.0/19.0), so stitches lean
more and more towards a mitre; spacing along the leg = density spacing x lean, i.e. the spacing measured across the
stitches stays 1/density (rail spacing grows 0.45 -> 0.6 mm towards the corner). Starts on the LEFT rail at the line's
start (after the under sewing that ended on the left), ends with a stitch exactly at the line's end. Half stitch shows no
effect at a 90-degree corner with this model (no crowding). Implemented as lines::zigzag_rails for straight-line paths;
curves keep the even zigzag.
Not matched: line-zigzag-density-min (0.1 line/mm): PE sews the start rung, then ONE diagonal from the start to 30 mm along
split into ~10 mm pieces, an odd corner, and a double end lock. Parked (spec + file in round2/lines/parked); nobody sews that.

pes-default-v6: its reference is PE's re-save of OpenStitch's own running lines, so the new locks (8 stitches) need a new
re-save (request in TRACKS.md). Blocks PR 1.

## Group 2 (branch lines/group2-pe11): stem, candlewick, chain; motif parked

Tie-in: V, stem, candlewicking and chain use a LONGER tie-in than running/E/zigzag: 0.3, 0.6, 0.9 mm out
along the first stitch, back to 0.5, then the start (compile.rs long_tie_in). End lock = tie_and_lock (two steps
after a last stitch > 3 mm, e.g. candlewick).

Stem (line-stem-default, now 169 = 169, mean needle distance 0.045 mm): per straight leg, stations leg/round(leg/1.5)
apart (40 mm -> 27 x 1.48), first at the start, last at the end; at a corner the corner point itself, then the next
leg's stations from j = 1. Each station: top = c + t*along - n*w/2, bottom = c - t*along + n*w/2 (along = w/2*cot(angle);
45 = top right -> bottom left), sewn top-e, bottom-e, top+e, bottom+e with e = 0.14 mm perpendicular to the stitch, then
the next station's top; ends at the end point. Stored angle is passed straight (the old 180 - a mirrored it).

Candlewicking (305 = 305, 0.01 mm): knots at both ends of every leg and evenly between, round(leg/(size+spacing))
intervals per leg (40 -> 7 x 5.71, 20 -> 3 x 6.67); a corner knot faces along the next leg. Knot = star {24/11} on a
circle of size/2 (Medium: 24 points 15 deg apart, each chord jumps 11): starts and ends at the point straight ahead
(t * r), angles step -15*11 deg (file coordinates). First knot: front point -> centre -> front point, then the star.
Between knots: running stitch along the line from the previous knot's front point to this one's, every vertex kept,
the remainder first, then 2.5 mm. OpenStitch density = points/2 (Medium = 12); Low/High not measured.

Chain, triangle (348 = 348, 0.0 mm, IoU 1.0): per leg, n = floor(leg/(0.75*size)) links leg/n apart (40 -> 13 x 3.08,
20 -> 6 x 3.33); link = point p on the line, base p + size*t, corners base -+ size/3 * n; sewn p, L, R, p x times (5),
then L, base middle, next link's point. Last link runs past the leg end. Diamond not measured (kept as before).

Motif: PE's default motif001 is PE's own artwork (12.3 x 12.3 mm, run pitch 5.0); OpenStitch can't ship it and its
default motif differs, so no stitch comparison is possible. Spec + file parked (round2/lines/parked). A PE file
with a simple, describable motif drawn by the user (or an OpenStitch motif imported into PE) would allow one.

## Start/End Type, Feed vs Run: first look (2026-10-27, not implemented yet)

Files: <reference-notes>\round2\lines\line-{motif,chain,stem,candlewick}-startend-{feed,run}.pes, MANIFEST-lines-startend.json.
Shape: closed Straight Line (10,10)-(50,10)-(50,30)-(10,30), sewn clockwise on screen (top edge +x first), so the bottom
edge is sewn towards -x. L1 (entry) ~(25.5, 30), L2 (exit) ~(29.8, 30) as read from the screen (+-0.3; PE keeps them
about one pattern unit apart; on the bottom edge L2 is BEHIND L1 in sewing direction). Default for all four is Feed.
Decoder: pes-compare/pec_dump.py (python pec_dump.py FILE [N]), coordinates relative to the first needle point = L1. Branch to build on: lines/startend-pe11
(stacked on lines/group2-pe11 = PR #63, which has PE's stem/candlewick/chain generators).

Records (Feed / Run): chain 690 / 691, stem 324 / 324, candlewick 549 / 552, motif 326 / 331. NO jumps or trims in any file.

What is common: the pattern stays IN PLACE. Element positions are those of the line without moved points (anchored at the
outline's own start vertex, per-leg spacing as in PR #63); only where sewing starts and ends changes. OpenStitch today
restarts the pattern at the entry point (compile.rs: c.starting_at(entry)), which is wrong for both modes.

- chain Feed: first link apex exactly at L1 (25.5; PE snapped L1 to a link: bottom-edge links at 50 - j*3.077, j=8 -> 25.4),
  links around the loop, last link the one at 28.5 (the link before L1), ending at its base middle (24.5), tie-off lock.
  chain Run: from L1 a 3.0 mm running stitch to the next link (22.4, j=9), links around, the LAST link is the one at L1
  (base at 21.5), lock. Long tie-in in both.
- stem Feed: first station at L1, stations around, last station just before L1 (+1.5), lock.
  stem Run: from L1 running stitches -1.4, -2.9, -4.4 (1.5 mm) to the station at -4.4, stations around, ends at the station
  at -4.4 again (?), lock. Needs checking: whether Run ends at the station before L2.
- candlewick Feed: from L1 running -2.5, -5.0, -5.7, -8.2, -10.7 to the front point of the knot centred at -8.2, ...,
  ends at -5.0 (lock 2 steps). Run: running to -16.5, first knot centred at -14.0, ..., ends at -10.8.
  So candlewick has a lead-in even with Feed; Run's first knot is one knot further on.
- motif Feed vs Run: identical stitches except the tie-in: Feed starts straight at (-2.9, 0) with NO tie-in (the jump/feed
  lands on the pattern), Run has the long tie-in at L1. Both end at (0,0) with a one-step lock.

Open questions for the implementer: the exact element order rule (first element at/after L1 vs strictly after; where L2
ends the sewing), whether Feed jumps become stitches only because the moves are short (< PE's jump threshold), and a
second file pair with L1 not on an element would make the rule unambiguous (ask pe-gui: chain, L1 midway between links).

## Start/End Type, Feed vs Run: rule (lines-startend, icons session, 2026-10-27; branch lines/startend-feed-run)

Method: OpenStitch's in-place output (no moved points) for the same closed line, PE's file aligned on it by bounding box
(the first look's absolute positions were off by one element: a median-based alignment slips by one chain link, 3.08 mm),
then compared point by point IN ORDER. Coordinates below in mm on the drawn outline; the bottom edge is sewn towards -x.

Elements and where each begins (its anchor), bottom edge:
- chain: links at 50 - j*40/13 (j = 6: 31.54, 7: 28.46, 8: 25.38); a link begins at its point on the line.
- stem: stations at 50 - j*40/27 (13: 30.74, 14: 29.26, 15: 27.78, 16: 26.30, 17: 24.81); begins at its centre.
- candlewick: knots centred at 50 - j*40/7 (3: 32.86, 4: 27.14, 5: 21.43); a knot begins one radius before its centre
  (4: 29.64), its lead is the running stitch from the previous knot's front point.

RULE. An element belongs to a point when it is the last one to begin at or before the point (in sewing order).
With PE's screen readings L1 (25.5, 30) and L2 (29.8, 30) all three types agree (allowed range: L1 in (25.38, 26.30],
L2 in (29.26, 30.74]); no offset needed.
- Elements never move: they are where they are without the moved points (all four types; motif per the first look).
- Feed: start at the L1 element's lead, sew it, then round the loop to the element before it.
  chain: starts at link 7 (28.46), ends with link 6 at its base middle (27.46). stem: starts at station 16's centre
  (26.30), ends with station 16 sewn again. candlewick: starts with knot 4's lead (29.64, 27.14 ... front 24.64)
  but sews knot 4 LAST (its lead again, then the star).
- Run: start at the L2 element and sew only the leads up to and including the L1 element's lead: chain one straight
  stitch 31.54 -> 28.46 (link points, not subdivided); stem the station centres 30.74, 29.26, 27.78, 26.30; candlewick
  the running stitch of knots 3 and 4 (32.9 ... 24.6). Then as Feed. So the L2..L1-1 elements come last.
- Tie-in: long (0.3/0.6/0.9/0.5) when the sewing starts with an element and for chain's straight lead; the one-step
  tie-in (0.3 out, back) when a stem or candlewick starts with running stitch (stem Run, candlewick both). Lock as before.
- Where the loop closes (the drawn first vertex (10,10)): stem sews the corner point and the last leg's end-station
  bottom point (no e offset), then the first leg's station 0; candlewick sews ONE knot there, facing along the last leg
  (front (10, 7.5)), and the next lead starts at the corner with the short first stitch shared: 1.6 + 1.6 mm, not 0.7 + 2.5.
- No jumps or trims in any file; L2 needs no exit move because the line is the last object.

Result (pes-compare/pescmp.py mean needle-to-nearest, OpenStitch -> PE; "in order" = needle points within 0.15 mm
of PE's at the same index):
| spec | PE | before (restart at L1) | after |
|---|---|---|---|
| chain feed | 690 | 690 st, 0.024, start 3.0 mm off, 0 in order | 690, 0.000, 690 in order |
| chain run | 691 | 690, 0.024, 6.0 mm off, 0 | 691, 0.000, 691 |
| stem feed | 324 | 316, 0.167, 0.8 mm off, 0 | 324, 0.048, 324 |
| stem run | 324 | 316, 0.171, 5.1 mm off, 0 | 324, 0.048, 324 |
| candlewick feed | 549 | 575, 0.190, 6.0 mm off, 0 | 549, 0.007, 549 |
| candlewick run | 552 | 575, 0.190, 11.8 mm off, 3 | 552, 0.007, 552 |
Open: motif (parked as before), open lines, chain diamonds, L1 between two elements (pe-gui request posted in TRACKS.md).

### CORRECTION (same day, after pe-gui's chain -mid / -l2side files): L1 and L2 the other way round
In the four new files L1 (23.8, 30) and L2 (corner (10,30) or (50,30)) are unambiguous, and there the PATTERN starts
at L2's element (mid-feed starts with the left-edge link at (10,30); l2side-feed with the bottom link at (50,30)),
while Run starts at L1's element (link 8, 25.38) and leads to L2's: forwards over 5 links (mid-run) or BACKWARDS over
8 links against the sewing direction (l2side-run), whichever is shorter. So in the first eight files the point at
25.5 is L2 and the one at 29.8 is L1; the log's labels were swapped (both markers sat close together).
Corrected rule: the pattern starts and ends at L2's element (that is why it is the exit); Feed starts with it, Run
starts at L1's element and sews only the leads to L2's element the shorter way round. Everything else above stands
(element ownership, candlewick's last knot, stem's repeated station, tie-ins, the closing corner).
All 10 files match in order (chain 0.00 mm, stem/candlewick <= 0.10). Before -> after (stitches, pescmp mean mm):
chain feed 672/0.21 -> 690/0.0, run 672/0.21 -> 691/0.0, stem feed 316/0.167 -> 324/0.048, run 316/0.162 -> 324/0.048,
candlewick feed 548/0.199 -> 549/0.007, run 548/0.199 -> 552/0.007, chain mid-feed 672/0.207 -> 690/0.0, mid-run -> 695/0.0,
l2side-feed -> 690/0.0, l2side-run -> 698/0.0 ("before" given L1 only, which is all the old code had).
(The table in the section above used the swapped labels for "before"; these numbers replace it.)
