# B8. Crosshatch, curved crosshatch and double-zigzag lace: algorithm sketch

Backlog item B8 of [competitive-features.md](../competitive-features.md) (feature F7).
Research date: 2026-10-28. A design note for `openstitch-design` (`regions.rs`, `model.rs`).
No other product's code was read.

## 1. What the feature is

An open fill made of one or more families of parallel lines at chosen angles and spacing,
each line a running, triple or zigzag stitch, with no cover: square (two families 90°
apart), diamond (two families at ±angle), parallel (one family), or custom angles; a
**double zigzag** line type that retraces each line as a zigzag so the mesh is strong enough
for free-standing lace; and a **curved** variant whose families bend along a guide.

Who has it: mySewnet Crosshatch Fill (Diamond / Square / Parallel, gap, angle, stitch type
and length, Double Zigzag recommended for lace) and Curved Crosshatch **(single source)**
[[MSW-area]](https://softwarehelp.mysewnet.com/MSW/140/Digitizing/Topics/Area_Types.htm),
[[MSW-cross]](https://softwarehelp.mysewnet.com/MSW/140/Digitizing/Topics/Crosshatch_Fill.htm),
[[MSW-curved]](https://softwarehelp.mysewnet.com/MSW/100/Digitizing/Topics/Curved_Crosshatch_Fill.htm);
Embrilliance Freestanding Background (lace grids) [[EM-SA]](https://www.embrilliance.com/stitchartist);
Janome Net Fill [[AD]](https://janome.com.au/product/janome-artistic-digitizer); Ink/Stitch
patterns on fills [[IS-patterns]](https://inkstitch.org/docs/stitches/patterns/).

PE-DESIGN 11: Net Fill (6 fixed patterns, spacing, direction) and Zigzag Net Fill (4
patterns, zigzag width and density, under sewing on/off). OpenStitch has both at the
guide-specified level: `regions::net_families` builds the line families of a pattern with
`tile_rows` at fixed angles relative to the region angle (`diamond` = ±45°, `triangle`,
`honeycomb`, …), Net Fill sews them as running stitch and Zigzag Net Fill as a zigzag along
each line with an optional running pass under it. The Background Fill wizard has a
"crosshatch" option built on the same code. So crosshatch is mostly a matter of exposing
free angles, a second spacing and the lace line type on a region type of its own, plus the
curved variant through B2's warp.

## 2. Geometry

### 2.1 Families

A `Crosshatch` region has `families: Vec<Family>`, each `{ angle_offset, spacing }`, built by
the preset:

| Style | Families |
|---|---|
| `parallel` | one at `angle` |
| `square` | `angle`, `angle + 90°` |
| `diamond` | `angle + θ`, `angle − θ` with `θ` = half the diamond angle (default 45° → the net "diamond") |
| `triple` | `angle`, `angle + 60°`, `angle + 120°` (the net "triangle") |
| `custom` | the list as given, 1–4 families |

Each family is `tile_rows(closed, holes, angle + offset, unit, spacing, spacing, phase)` as
`net_families` does today, giving the clipped line pieces per family. A `phase` per family
(0–1 of the spacing) shifts the family so a user can centre the lattice on a point: the
region's reference point (`RegionSewing::reference`, already used by programmable fills)
sets the phase of every family so that a line of each passes through it.

### 2.2 Line stitch types

Each family's pieces are sewn as one of:

- `running` (pitch), `triple` (pitch; forward-back-forward, `generate::triple`),
- `zigzag` (width, density; `generate::zigzag` along the piece, today's Zigzag Net Fill),
- `doubleZigzag`: the piece sewn as a zigzag, then back along it as the same zigzag shifted
  by half a period, so the two passes cross each other and every crossing is a locked
  mesh node; the needle returns to the start of the piece, so pieces chain with no travel.
  Width `w` (default = spacing × 0.8, the zigzag net default) and density as for zigzag.

### 2.3 Order and travel

Families are sewn one after another (family A complete, then B, …), which is how lace is
built (the second family locks the first). Within a family, pieces are ordered as
`region.sequence_raw` orders the net pieces today: nearest-next with travel along the
region outline (`RunningStitchPath::OnOutline` behaviour) or through already-sewn lines
when `travel` is inside. For free-standing lace the travel must stay on sewn thread, so
`doubleZigzag` forces outline travel (there is no fabric to hide a travel run in).

Under sewing: none for running and triple; for zigzag types the existing Zigzag Net Fill
option (a running pass along each line before the zigzag) is kept as `underlay`.

### 2.4 Curved crosshatch

With a direction curve (one guide, today's `direction_curve`) or two guides (B2), the
families are generated in the straightened space and bent back, exactly as a curved fill
is: `Crosshatch` joins `direction::CURVE_TYPES`. The square style then gives a curved
lattice whose first family follows the guide and whose second stays perpendicular to it in
warped space, which is the quilting "curved crosshatch" look. Stitch length is re-subdivided
in model space after bending.

### 2.5 Lace borders and stabilisation

A lace mesh needs an edge: the Embroidered Patch wizard's satin border, or a `doubleZigzag`
outline, sewn **after** the mesh so the border locks the line ends. The region's existing
"region → line" conversion and the patch wizard already produce the border; this note only
requires that the border is sewn last, which the sewing order gives when the user adds it
after the fill. A "Lace" preset in the inspector sets `doubleZigzag`, square, spacing 3 mm,
outline travel and adds the border object in one click.

## 3. Parameters and defaults

| Attribute | Type | Default | Range |
|---|---|---|---|
| `type` | `crosshatch` | — | new `RegionType::Crosshatch` |
| `hatchStyle` | `parallel` \| `square` \| `diamond` \| `triple` \| `custom` | `square` | — |
| `angle` | degrees | 45 | existing |
| `diamondAngle` | degrees | 90 | 20–160 (diamond only) |
| `spacing` | mm | 5.0 | 1–20 (mySewnet's 5–20; 1 mm allowed for dense lace) |
| `families` | list | from the style | custom: 1–4 of `{angle, spacing, phase}` |
| `lineType` | `running` \| `triple` \| `zigzag` \| `doubleZigzag` | `running` | — |
| `runPitch` | mm | 2.0 | existing |
| `width`, `density` | mm, lines/mm | spacing × 0.8, 4.5 | zigzag types, existing fields |
| `underlay` | bool | false | zigzag types: running pass under each line |
| `directionCurve` | contour | none | curved variant |

## 4. Model and code changes

- `model.rs`: `RegionType::Crosshatch`; `hatch_style`, `diamond_angle`, `families`,
  `line_type` (reusing `LineType` ids `running` / `triple` / `zigzag` plus `doubleZigzag`);
  `Crosshatch` added to `CURVE_TYPES` and `REFERENCE_TYPES`.
- `regions.rs`: `crosshatch_families(closed, holes, angle, style, families, reference)` as a
  generalisation of `net_families` (the net patterns become presets of it, so Net Fill and
  Zigzag Net Fill stay byte-identical); `double_zigzag(piece, width, step)`; dispatch in
  `generate`.
- `region.rs`: the curved branch already covers `CURVE_TYPES`.
- Engine: `object.setRegion` ids and ranges; the Background Fill wizard's crosshatch option
  maps to the new type (unchanged output).
- App: inspector rows; a "Lace" preset button.

Effort: S. The line families, zigzag and travel exist; the new code is the family builder,
the double zigzag and the plumbing.

## 5. Test cases (internal regression tier, plus reference guards)

| Spec | Script | Expected |
|---|---|---|
| `hatch-net-is-today` | every Net Fill and Zigzag Net Fill script (and the Background Fill crosshatch) re-run | identical output (guard) |
| `hatch-square-45` | 40 × 40 mm square, `square`, 45°, spacing 5 | two families at 45° and 135° (line directions by fit, ±1°); line spacing 5 ± 0.1 mm within each family; family A complete before family B |
| `hatch-diamond-60` | `diamond`, `diamondAngle` 60 | families at angle ± 30° |
| `hatch-parallel` | `parallel`, 0° | one family; no second angle present |
| `hatch-custom-three` | custom `[0°/4 mm, 60°/4 mm, 120°/6 mm]` | three families with those spacings |
| `hatch-reference-phase` | reference point at (20, 20) | a line of every family passes within 0.1 mm of it |
| `hatch-double-zigzag` | square, spacing 3, `doubleZigzag`, width 2.4 | every piece sewn twice with the second pass shifted half a period (crossing count per piece = pass length / period ± 1); each piece ends at its start; travel between pieces only on the outline (no stitch inside the region that is not on a line) |
| `hatch-hole` | region with a hole | no needle point inside the hole; lines clipped at its edge |
| `hatch-curved` | square style with a direction curve (an arch) | family A's lines are translated copies of the arch (every point within 0.3 mm of a shifted arch); family B's lines cross them |
| `hatch-limits` | spacing 0.5 mm on 300 × 300 mm | refused by `attribute-ranges`; spacing 1 mm stays within the point budget |

Measurement helpers needed: "line direction by fit per piece" (shared with B2), "family
grouping by direction", "crossing count" (segment intersection count; new, small).

## 6. Compatibility and licensing

Net Fill and Zigzag Net Fill keep their outputs (guard spec). The new type is off until
chosen. Line families clipped to a polygon are elementary; the double zigzag is a sewing
construction every lace tutorial describes. "Crosshatch Fill" and "Freestanding Background"
are product names; OpenStitch uses "Crosshatch" and "Lace".

## 7. Open questions

- Whether `doubleZigzag` should also be offered as a line sew type on its own (for lace
  outlines), which would be a two-line change in `lines.rs`; proposal: yes, in the same PR.
- Whether the lattice phase should follow the reference point (as programmable fill does)
  or the entry point; proposal: reference point, consistent with the pattern fills.
