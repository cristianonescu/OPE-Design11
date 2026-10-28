# Handover: stitch-sweeps track (2026-10-27)

**Landed.** #59 (e3caf47): fill and satin density range 0.1–14.0 (`RegionType::density_range`), compat
measures `rowSpacing`/`rowWidth`, 9 sweep specs (density, pull). **PR 2** = #73 (branch `sweeps/fill-row-structure`,
https://github.com/cristianonescu/OpenStitch/pull/73; pescmp main-vs-PR tables in its body): PE-DESIGN's fill row structure, Frequency 30 % default, 11 more sweep specs.
References: 38 PE files from pe-gui in `<reference-notes>\round2\sweeps\` (log.txt, MANIFEST
entries in the stitch-sweeps worktree's `compat/reference/MANIFEST-sweeps.json`); 20 of them are in the repo.

**Rules (stitch-rules.md).** Implemented and pinned by specs: F1 density, F2 pull (rows only, not under
sewing), F4 pitch/Frequency grid, F6 halves from the start point, out-and-back first row, brick row ends,
straight ≤ 2 mm travel, 0.3 mm tie-in/lock, stretched spacing, F7 grid anchor at the first row's start.
Convex fills (rectangles at 0°/90°, every density, pitch 2/6/10, Frequency 10/40, pull 1.0/2.0) match PE's
files needle point for needle point (`same-stitches` compare checks). Only measured, not implemented:
U1 under sewing (Dense/Light row spacing 1.4/3.5 mm; type 2 = ±45° cross-hatch), S1/S2 satin (sent to satin).

**Open gaps, next steps.**
1. Programmable fill: OpenStitch 2221 stitches vs PE 4076 (programmable-fill-45). Not studied. Next: request
   sweeps (density, pattern size, direction 0/45) from pe-gui on the 40 mm square, measure as for fill.
2. Angled fills (30/45/135°): counts within 0.5 %, but pescmp mean needle distance 0.30–0.43 mm. Neither the
   first-row nor the bbox anchor fits; the corner rows near the start differ. Files: sweep-fill-dir-*.pes.
   Next: per-row phase analysis in rotated space (scratch scripts dirs.py/cmp.py in this session's scratchpad).
3. Text fills got slightly worse with PR 2 (pescmp vs main 64af53e: text-size-h 622 → 639 stitches, PE 483;
   text-line-spacing 408 → 485, PE 372; mean distance +0.01–0.02 mm). Cause found: the brick rule in the
   multi-span chain (generate.rs `chain`, `Chain::Brick`) keeps the ends of rows shorter than one stitch,
   and a 20 mm Lato H's strokes (~3.5 mm) are such rows; PE drops them there. With the threshold at half a
   stitch text-size-h is 511, but hole-sewing-pentagon-star's stitchedArea becomes 853.7 (limit 850.2; PE
   839.9) because narrow vertex rows lose their ends (a 3 mm cut: same). Text Transform is +14 % (1489 → 1701,
   PE 1300) whatever the cut. Next: compare the pentagon's vertex rows with PE's row by row; for Text
   Transform test the 0.3 mm grid end gap and the per-letter tie-in. Recorded in the audit's Next 11.
4. Multi-span regions (U, holes, letters) keep OpenStitch's nearest-row order (`chain`, `Chain::Brick`);
   PE's order there (fill-analysis.md: legs bottom → top) is not replicated.
5. Pull 0.2/0.5 and pitch 1/3: PE sometimes keeps a grid point at exactly 0.3 mm from a row end
   (`PE_END_GAP` = 0.3 mm); ±1 % counts. Under sewing: OpenStitch adds one outline → inset stitch at the
   start (region.rs `fill_underlay`, `first.insert(0, at)`); PE starts on the inset. Fill's code.
6. Row-count rounding when H·d has a fraction > 0.5 (floor vs round) is unobserved: one PE file with e.g.
   40 × 37 mm at density 4.4 (162.8 gaps) would settle it.
