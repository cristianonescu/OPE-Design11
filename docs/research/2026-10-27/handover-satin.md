# Satin track handover (2026-10-27)

## Landed (cristianonescu/OpenStitch)
- **#40** – satin defaults 4.5 lines/mm, under sewing on (EdgeZigzag); PE's satin under sewing (1 mm inset outline + rows across the satin, 2 mm, cut from the inset region); under sewing linked to the top without a jump; Auto rungs spaced along the longer side. Specs: auto-satin-bent-band, region-travel-u-satin.
- **#43** – sawtooth for fixed-direction and Manual Punch satin; tie-in and end lock; split at an interior start point; `exclude {first,last}` source option in the compat harness. Specs: satin-zigzag-rect, satin-zigzag-column (reference tier), satin-zigzag-skeleton.
- **#47** – satin crossings stay whole (no 12 mm split); under sewing rows on inset edges take the longer side; first under sewing row there and back.
- **#56** – PE section order (opening row, far end first, legs from the first outline corner, start-point section last), 2 mm satin travel, feathered edge as sawtooth + PE's fixed 12-step pattern, tie-in/lock step counts. Specs: satin-sections-h, satin-feather, satin-lock-column-3p5.
- **Corner fan** – PR #72 (branch `claude/satin-corner-fan-2`, one commit on e3caf47), next in the merge queue after #61. Auto satin: corner fan (rung direction turns linearly along the outer edge over 3 widths per 90°, rungs to the inner edge), band ends reached, start travel 0.5 mm inside the outline. Specs: satin-auto-bend-90-nohalf, satin-auto-bend-60. pescmp before → after: bent band IoU 0.79 → 0.95, mean 0.30 → 0.10 mm; 90° 0.59 → 0.73, 0.29 → 0.16; 60° 0.68 → 0.70, 0.31 → 0.20.

## Left (satin-goal.md)
- **Half Stitch** (item 2b, second half) – not modelled. With the corner fan's crowded inner ends it can now act: PE pulls every other crowded inner point back ~1.3 mm (a quarter of the 5 mm width). Evidence: auto-satin-bent-band (on) and satin-auto-bend-90-nohalf (off). A first attempt (`half_stitch`, applied where one side's step is < half the other's over 5 rungs) is in this session's history, but it was never landed.
- **Item 7, text satin**: PE's 20 mm Lato 'H' at text defaults (Satin, 5.0, under sewing off) sews 285 stitches, OpenStitch 221. Reference: `<reference-notes>\round2\text\text-defaults-h.pes`; spec text-defaults-h in PR #64. Not started.
- **Gradation satin**: reference `satin-gradation.pes` (in this worktree, untracked; its MANIFEST entry is in the scratch copy `MANIFEST.all.json`). #50/#67 gave gradation PE's model; there's no satin spec yet.
- **Smaller differences**: PE's feather row pitch on 20 mm halves is 0.69 mm (ours 0.67) and its last rows sit on the start point (ours 0.2 mm short); one extra row before the H's top-right junction; the H's opening move is a jump in PE (trim+jump in the file); density 0.1 sweep (stitch-sweeps S3: PE 5 crossings 10 mm apart, ours 3 + diagonals).

## Next steps
1. Merge the corner-fan PR (coordinator queue; rebase if #61 lands first).
2. Half Stitch on top of the fan; measure with `pes-compare/pescmp.py` on auto-satin-bent-band.
3. Item 7: decompose text-defaults-h.pes (rows, crossings, locks) against OpenStitch's export.

## Files and references
- Code: `crates/openstitch-design/src/region.rs` (sawtooth_satin, split_satin, sections_in_order, tie_and_lock, satin under sewing, auto_satin, reach_ends, fan_corners), `skeleton.rs` (rungs_within), `column.rs` (sawtooth), `compile.rs` (tie-in/lock hook, whole satin stitches).
- Notes: satin-analysis.md, satin-goal.md, `satin-refs-underlay-off/`; tools: pes-compare/pescmp.py.
- References: `compat/reference/stitches/satin-*.pes`, auto-satin-bent-band, region-travel-u-satin.

## Decisions pending
- None for satin. The lock cut (4.25 mm) and tie-in cut (3 mm) are bracketed by references (lock ≤ 3.5 → 1 step, ≥ 5 → 2; tie-in ≤ 2.3 → 1, ≥ 3.5 → 3), not measured exactly.
