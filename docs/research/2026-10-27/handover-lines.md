# Handover: lines track (brand session local_a72d845c, 2026-10-27)

## Landed / queued
- **#55 (merged, 15852ea)**: running, triple, zigzag, E, V.
  - PE defaults; switching the type resets to them.
  - Tie-in and lock (`region::tie_and_lock`).
  - Zigzag: PE's rails (`lines::zigzag_rails`), under sewing (`zigzag_underlay`), density 0.1–14, `underSewing`.
  - PE's V; E prongs on the corner bisector.
  - pes-default-v6 and pes-csewseg-trims re-saved by pe-gui with the locks.
  - Specs: `line-{running,triple,zigzag,e,v}-default`, `line-zigzag-density-max`.
- **#63 (queued, branch lines/group2-pe11)**: stem, candlewicking, chain.
  - Matches PE's needle points (pescmp mean distance 0.045 / 0.010 / 0.000 mm).
  - PE's three-step tie-in for V, stem, candlewicking and chain (`compile.rs long_tie_in`).
  - Specs: `line-{stem,candlewick,chain}-default`.
  - Awaits the coordinator's rebase ping.
- **#70 (Feed/Run, by the icons session)**: Start/End Type for motif, stem, candlewicking and chain.

## Parked (recorded, not done)
- **Zigzag at 0.1 lines/mm:** PE sews one long diagonal split at ~10 mm, plus a double end lock. The spec is in `round2/lines/parked`.
- **V's longer start lock:** fixed in #63 (three-step tie-in), once #63 lands.
- **Motif:** PE's default `motif001` is PE's own artwork (12.3 mm, run pitch 5.0), so no comparison is possible. The spec and file are parked. OpenStitch's motif defaults are unchanged (run pitch 2.5).
- **Not measured:**
  - candlewicking Low/High (OpenStitch: density = points/2, Medium = 12 → 24 points)
  - the chain diamond (old generator kept)
  - the V arm offset rule (1/5 of the spacing; 1.0 mm could also be pitch/2 or 0.4 × height)

## Next steps
1. Rebase #63 when pinged.
2. Ask pe-gui for:
   - candlewick Low and High
   - chain diamond
   - V at another spacing or height
   - a motif file using a simple motif drawn in PE that can be described in words

   Then spec, fix and land each, as before.
3. Curved lines: `zigzag_rails` and the per-leg rules apply to straight legs only; curves keep the even zigzag. A reference with a curve (e.g. a circle with a zigzag line) would settle them.

## Files
- **Analysis:** `lines-analysis.md` (this folder); the Feed/Run first look is at its end.
- **Decoder:** `pes-compare/pec_dump.py`.
- **References:** `<reference-notes>\round2\lines\` (19 files plus `MANIFEST-lines*.json`, `log.txt`; `parked/`). The ones in use are in `compat/reference/stitches/line-*.pes`.
- **Code:**
  - `crates/openstitch-design/src/lines.rs` (generators)
  - `compile.rs` (`line_elements`, locks)
  - `model.rs` (`LineSewing::new`)
  - `crates/openstitch-engine/src/commands.rs` (`object.setLine`)
