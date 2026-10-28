# Handover: fill track (2026-10-27)

Repo: cristianonescu/OpenStitch. Black-box evidence only: PE-DESIGN 11.50 reference PES files in
`compat/reference/stitches/`; measurements in `fill-analysis.md` (this folder).

## Landed
- **#38** Fill defaults: 4.5 lines/mm; under sewing on (`UnderlayStyle::EdgeFill`: edge run 1 mm
  inside at ≤ 2.5 mm, then tatami rows across, 2 mm apart, 4 mm stitches, 30 % shift, 1 mm inset),
  joined to the top fill by Running Stitch Path travel. Rows edge to edge (`generate::fill_edge_to_edge`,
  chains from the row end nearest the entry, skips spans < 0.3 mm). compat measures `rowShift` and
  `stitchedArea` fixed (fill rows only; needle extent × mean gap).
- **#44** Direction angles counter-clockwise on screen, as in PE (`region::geometry_angle`,
  `geometry = −stored`). `.osd` v2; v1 migrated on load (`project::migrate_v1_angles`).
- **#50** Gradation as a pattern `Gradation {max, min, steps}`, default {53, 5, 9}; old numeric
  values read as `Gradation::continuous`. setRegion `gradationPattern/Max/Min/Steps`.
- **#52** Guide exclusions: no gradation with feather, Auto direction, text, or programmable fill;
  no feather with gradation, Auto, hole sewing, text, or programmable fill. Refused when set; kept but
  not sewn when stored (`RegionSewing::sewn_gradation/sewn_feather`). Feather default width 3.0 mm.
- **#66** `generate::row_spans` drops spans an inset inverts (they became needle points outside
  narrow shapes): hole-sewing trims 2 → 0.
- **#67** Gradation model fitted to PE: density **linear** from max to min across the region
  (fit 52.1 → 5.7 %), each row gap at the density **halfway across the gap**; rows edge to edge, rest
  before the far edge split evenly. Row error vs PE 0.239 → 0.058 mm.

## Known gaps (fill)
1. Feathered edge: PE has top/bottom row switches (each on/off, 3.0 mm default) and feather
   patterns; OpenStitch has start/end/both and one width. Evidence file: `satin-feather.pes`
   (round 2, <reference-notes>\round2; top row on 3.0 mm).
2. Gradation: other preset patterns, per-step values, Blend (two colours), gradation direction
   arrow. `steps` is stored but has no effect until per-step values exist.
3. Gradation and feather on programmable fill (PE offers both; OpenStitch refuses).
4. U section order: PE sews right leg → left leg → top bar, ending at the first vertex; OpenStitch
   chains greedily (its travel diagonal is in the other leg). Limits region-travel-u-gradation's
   coverage IoU (0.564). Routing; stitch-sweeps owns fill row structure/travel.
5. Frequency default: PE 30 %, OpenStitch ⅓ (spec `fill-frequency-default` pins ⅓; coordinator
   decision pending under Spec questions).
6. Programmable fill density default (2.5) not measured; PE's prog. fill had 4076 stitches vs ~2200.

## Next steps
- (1) Request from pe-gui: fill rectangle 40×20 mm with feather bottom row only, both rows, and
  widths 1.0/5.0 mm; model per-side widths in `Feather`, map the UI to PE's rows.
- (2) Request gradation files with non-default patterns (e.g. Max 80 / Min 20, 5 steps; a preset
  other than the first) to learn per-step values; then make `steps` carry values.
- (4) Implement PE's section order for multi-part regions (split at notches, sew from the part
  farthest from the exit, end at the exit) in region.rs/route.rs; verify with pes-compare/pescmp.py on
  region-travel-u-*.

## Files
`crates/openstitch-design/src/{model.rs (RegionSewing::new fill fields, Gradation, sewn_*),
region.rs (layers_inner, fill_underlay, join_layers, row_positions, geometry_angle), generate.rs
(fill_edge_to_edge, row_spans), compile.rs (join layers, text/hole stripping), project.rs (v2
migration)}`; `crates/openstitch-compat/src/measure.rs` (rowShift, stitchedArea);
tests in `stitch_type_tests.rs`, `crates/openstitch-engine/tests/{stitch_types,region_compat,order_holes}.rs`.
Specs: fill-frequency-0/25/50, region-travel-u-{fill,along-outline,on-outline,gradation},
hole-sewing-pentagon-star, programmable-fill-45 (all at the reference tier).
