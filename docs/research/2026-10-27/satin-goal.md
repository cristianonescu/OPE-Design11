# Satin goal (satin track, set 2026-10-27)

Goal: OpenStitch's satin matches PE-DESIGN 11.50's reference structure closely enough that every satin
spec passes with margin, not at the edge of its tolerance. Each item lands as its own green PR on
cristianonescu/OpenStitch (fresh branch from origin/main; compat all green, fmt, clippy, workspace
tests; merge only on green CI). Status: [ ] open, [x] done (PR), [~] blocked (on whom).

1. [x] (#47, 6cbede7) Keep satin crossings whole: PE keeps 30 mm satin stitches; OpenStitch's compiler splits at
   12 mm. Done when region-travel-u-satin is within 5 % of PE's 466 and nothing else regresses.
   Result: 473 (+1.5 %), was 530.
2. [x] (#47) Auto satin count: auto-satin-bent-band within 5 % of PE's 429. Result: 419 (-2.3 %), was 397.
2b. [ ] Auto satin corner fan (PR #72) and Half Stitch: references in hand (satin-auto-bend-60, -90-nohalf). PE's fan: every
   rung end on an edge, inner ends crowding 0.2-0.3 mm down to the inner corner, outer ends sweeping the outer
   corner, longest 7.0 mm; Half Stitch then thins the crowded inner ends. Needs a new corner construction. Tried: Half Stitch acts once, because OpenStitch's corner rungs stop short of
   the inner edge; letting them reach further gives 7.87 mm stitches (spec max 7.5). PE's corner needs
   rungs that pivot on the inner corner; one bend is too little evidence to design that.
3. [x] (#56, 036f07f) Feathered satin: (PE's sawtooth + fixed 12-step feather pattern,
   spec satin-feather passes). Gradation: PE uses a stepped preset (9 steps, 53 % -> 5 %), not OpenStitch's
   linear model, and fill shares it -> cross-track item, not satin's alone. Split stitches: no PE option seen.
4. [x] (#56) Satin sections in PE's order: (sections from the rows' far end, legs
   from the tip corner first in outline order, the start point's section last, an opening row along the
   start edge, partial last row reversed, 2 mm satin travel; spec satin-sections-h passes, 224 vs 235).
5. [x] (#51, another track) Default start point per shape tool (rectangle: middle of the bottom edge). Cross-track: needs the
   coordinator's owner decision (posted under Cross-track in TRACKS.md) because fill-frequency-*, optimize-entry-exit and
   pes-default-v6 move.
6. [x] (#56) Lock rule: (tie-in 3 steps after a first stitch > 3 mm, lock 2 steps
   after a last stitch > 4.25 mm; spec satin-lock-column-3p5 passes stitch for stitch).

7. [ ] Text satin (coordinator, 2026-10-27): PE's 20 mm Lato 'H' at text defaults (Satin, density 5.0, under sewing
   off) sews 285 stitches, OpenStitch 221 (reference <reference-notes>
ound2	ext	ext-defaults-h.pes,
   spec text-defaults-h in PR #64). Compare row spacing, crossings and locks with pescmp.py; land the
   structural difference with its spec (after #64 lands, so the spec exists on main).

(alpha.2 tagged; hold lifted.)
