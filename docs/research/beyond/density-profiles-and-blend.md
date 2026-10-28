# B6. Density profiles, two-colour blending and hidden travel: algorithm sketch

Backlog item B6 of [competitive-features.md](../competitive-features.md) (features F5, F6,
F17). Research date: 2026-10-28. A design note for `openstitch-design` (`model.rs`,
`region.rs`, `column.rs`) and one engine command. No other product's code was read.

## 1. What the feature is

- **Density profile.** The row spacing of a fill, or the stitch spacing of a satin column,
  follows a curve along the object (dense at one end, open at the other; dense in the
  middle; a wave), from presets or a user-drawn polyline. PE-DESIGN's Gradation is the
  linear case with a fixed Max/Min pattern (53 % to 5 % by default, measured black-box).
- **Two-colour blend.** The same region is sewn twice in two threads with complementary
  profiles: the lower colour is dense where the upper is open and open where the upper is
  dense, so one colour fades into the other without a hard edge.
- **Hidden travel for open fills.** When rows are far apart the travel runs between rows and
  sections would show; they are moved to the outline and the duplicated half-rows at
  section joins are removed.

Who has it: Wilcom Accordion Spacing (8 presets, ES 2026 custom profiles), Colour Blending,
Trapunto; Hatch 4 Multi Blend profiles
[[ES-designing]](https://wilcom.com/embroiderystudio/designing),
[[ES-blend]](https://wilcom.com/resources/blog/color-blending),
[[ES-trapunto]](https://docs.wilcom.com/embroiderystudio/27/en/OnlineHelp/Decorative/specialty/specialty-14.htm);
mySewnet gradient and multicolour gradient density on Pattern Fill, Spiral Fill and Satin
Column, underlay suppressed under gradients
[[MSW-pattern]](https://softwarehelp.mysewnet.com/MSW/100/Digitizing/Topics/Pattern_Fill_—_Options.htm);
Pulse Density Profile and Auto Colour Blend [[DG15]](https://embroidery.pulsemicro.com/dg15-levels.html);
Embird linear and central gradient added to the base spacing
[[Embird-fill]](https://www.embird.net/machine-embroidery-tutorials/studio-next/par_fill.htm);
Ink/Stitch End row spacing and Convert to Gradient Blocks
[[IS-fill]](https://inkstitch.org/docs/stitches/fill-stitch/),
[[IS-filltools]](https://inkstitch.org/docs/fill-tools/).

OpenStitch today: `Gradation { max, min, steps }` maps the fraction of the fill extent to a
density factor (`Gradation::fraction`), `row_positions` places rows at `spacing /
fraction(y)` with PE-DESIGN's stepped pattern placement (reference tier,
`region-travel-u-gradation`); `RunningStitchPath` puts travel inside, along or on the
outline (reference tier, `region-travel-u-*`). Satin columns have no gradation.

## 2. Density profile

### 2.1 Definition

A profile is a function `f: [0, 1] → (0, 1]` of the position along the gradient axis; the
local row spacing is `d / f(t)`, so `f = 1` is the full density and `f = 0.05` the most
open allowed (PE-DESIGN's floor). `Gradation` today is the linear profile from `max/100`
to `min/100`; keep it and add:

```
Profile = Linear { max, min }             // today's Gradation
        | Preset(kind, max, min)          // kind below, scaled between max and min
        | Custom(Vec<(t, f)>)             // monotone t in [0,1], piecewise linear
```

Presets, with `g(t)` scaled to `[min, max]`: `linear` (`g = 1 − t`), `convex` (`(1 − t)²`),
`concave` (`1 − t²`), `centre` (`1 − |2t − 1|`, dense in the middle), `edges`
(`|2t − 1|`), `wave` (`½ + ½ cos 2πt`, two bands), `step` (PE-DESIGN's 9-step pattern,
today's `steps`). Eight shapes match what the published presets cover.

The axis: for fills, the row direction's normal (today's gradient axis, from the start
edge); for satin columns, the arc length along the column from its entry end; for
concentric and spiral fills, the ring index from the outside. Profiles never apply to
motif, net, cross and decorative fills (PE-DESIGN's exclusions, which the guide gives for
Gradation and which stay).

### 2.2 Rows and stitches

Fills: `row_positions` already integrates `spacing / fraction`; it takes a `Profile` instead
of a `Gradation`, and the pattern placement rule (first row on the start edge, last on the
far edge, rows between evenly by the integrated density) is kept for `Linear` and `step` so
the reference spec is unchanged, and used for every other shape too (it is the placement
that avoids a half row at the far edge).

Satin columns and skeleton satin: `rungs_at` spaces rungs by `spacing`; with a profile the
step between rung `k` and `k+1` is `spacing / f(s_k / L)`, with `s_k` the arc length at rung
`k` and `L` the column length. The rung geometry is unchanged, only its placement. Zigzag
lines use the same rule along the line.

Underlay under a profile: the professional products suppress underlay under gradient
areas because an open fill shows it. Rule: with a profile whose minimum is below 0.5, the
zigzag/tatami under-sewing rows are generated only where `f(t) ≥ 0.5` (the dense part), and
the edge run stays. Off by default for `Linear` (PE-DESIGN sews under sewing under its
Gradation, reference tier) and on for the other shapes; an attribute `underlayUnderOpen`
overrides.

## 3. Two-colour blend

### 3.1 Construction

`shape.blend` on a region object with a profile:

1. Duplicate the object; the copy gets the second thread and the **complementary** profile
   `f'(t) = min + max − f(t)` (dense where the original is open), the same angle, the same
   Frequency, no under sewing, and the same outline.
2. Sewing order: the copy (the "bottom" colour) first, the original on top. The order is the
   user's choice through the Sewing Order pane; the command sets the default: the colour that
   is dense at the start edge goes first, so the top layer's open rows lie over a dense bed
   where they are sparse and vice versa.
3. Both objects are grouped (`object.group`) so they move and scale together; the group
   carries `blend: true` so the inspector shows one "Blend" pane with both threads and the
   profile, and editing the profile on one member updates the other's complement.
4. Rows of the two layers interleave by construction: row `k` of the top layer and row `k`
   of the bottom layer are offset by half the local spacing (the bottom layer's rows start
   at `d/2` from the edge), so needle penetrations do not share holes.

Three-colour blends (Hatch's Multi Blend) are the same with three profiles whose sum is
constant; the command accepts a list of threads and gives each an equal band of the axis
(`centre`-shaped profiles centred at `t = 0.5·k/(n−1)`); deferred until two colours are
sewn out.

### 3.2 Hidden travel for open fills

For any fill whose local spacing exceeds `openSpacing` (default 1.0 mm, that is `f < d /
1.0`): force `RunningStitchPath::OnOutline` for the travel between sections and rows (the
reference-tier implementation of travel on the outline is reused), start the fill at the
section nearest the entry so the outline travel is one pass, and remove the duplicated
half-rows that `fill_edge_to_edge` emits at section joins (they exist to keep coverage in
dense fills and show as doubled lines in open ones). This is the Trapunto behaviour; it is
automatic because an open fill with visible travel is never wanted, and `travelInOpen: true`
restores today's output.

## 4. Parameters and defaults

| Attribute | Type | Default | Notes |
|---|---|---|---|
| `gradation` | existing | — | kept; reads and writes the old form |
| `profile` | `linear` \| `convex` \| `concave` \| `centre` \| `edges` \| `wave` \| `step` \| custom points | `linear` when gradation on | `max`/`min` as today (53/5) |
| `profileAxis` | `rows` \| `column` \| `rings` | by type | |
| `underlayUnderOpen` | bool | true for linear/step, false otherwise | |
| `openSpacing` | mm | 1.0 | threshold for hidden travel |
| `travelInOpen` | bool | false | keeps today's travel in open fills |
| `shape.blend` | command | — | `thread2`, optional `threads: [...]` |

## 5. Model and code changes

- `model.rs`: `Gradation` grows a `shape` field (serde default `step` when `steps` is set,
  `linear` otherwise) and `custom: Option<Vec<[f64; 2]>>`; `RegionSewing::underlay_under_open`,
  `travel_in_open`; `Design` groups get a `blend` flag (as B4's `branch`).
- `region.rs::row_positions`: takes the profile; the integration is the same code with a
  different `fraction`. `satin_underlay_rows` / `rows`: clip under-sewing rows to the dense
  part when `underlay_under_open` is false.
- `column.rs::rungs_at` and `skeleton::rungs`: optional profile.
- `generate.rs::fill_edge_to_edge`: drop section-join half rows when the spacing is open.
- Engine: `shape.blend`, group flag handling in `object.setRegion` (propagating the
  complement), `attribute-ranges` entries.
- App: the Gradation pane gets a shape dropdown and a small curve preview; a "Blend…" button
  in the region attributes; the Sewing Order pane shows a blend group as one item.

Effort: S for profiles (the integration exists), S for the blend command, S for hidden
travel, S for the editor pane.

## 6. Test cases (internal regression tier, plus reference guards)

| Spec | Script | Expected |
|---|---|---|
| `profile-linear-is-gradation` | `region-travel-u-gradation` re-run with `profile: step` explicit | identical output (reference guard) |
| `profile-convex-fill` | 40 × 40 mm fill, `convex`, max 100, min 10 | row gaps measured from the start edge: monotone increasing; gap at `t` within 5 % of `d / g(t)`; first row on the start edge, last on the far edge |
| `profile-centre-fill` | same, `centre` | gaps decrease to the middle and increase again; symmetric within 5 % |
| `profile-custom` | custom points `[(0, 1), (0.3, 1), (0.7, 0.2), (1, 0.2)]` | gaps constant to 30 %, ramp to 70 %, constant after |
| `profile-column` | 60 mm Manual Punch column, `linear` along the column | rung spacing grows linearly from `d` to `d / 0.05` capped; rung geometry unchanged (rails identical to the run without profile) |
| `profile-underlay-clip` | `convex` with under sewing, `underlayUnderOpen` false | zigzag under-sewing rows only where `f ≥ 0.5`; the edge run present over the whole outline |
| `blend-two-colours` | 40 × 40 mm fill, `shape.blend` with a second thread | two objects in a group; the bottom layer's profile is `min + max − f`; bottom sewn first; summed local density of both layers constant within 10 % across the region; no shared needle holes (nearest-point distance between layers ≥ `d/4`) |
| `blend-edit-propagates` | change the profile on the top object | the bottom object's profile is the new complement |
| `trapunto-open-fill` | 40 × 40 mm fill, `convex` to min 5 | in the open half: no travel stitch inside the region (every stitch not on a row lies within 0.2 mm of the outline); no doubled rows at section joins (nearest-row distance never below `0.5 × local gap`) |
| `trapunto-off-is-today` | same with `travelInOpen` true | identical to today's output |

Measurement helpers needed: "row gap list from the start edge" (exists for the gradation
spec), "layer density map" (shared with B13's density map), "stitches not on rows near the
outline" (point-to-polyline distance, exists).

## 7. Compatibility and licensing

The reference gradation spec keeps passing through the `step` shape; new shapes are off
until chosen; hidden travel changes output only where the spacing is over 1 mm, which no
reference spec has (the reference fills are at 4.5 lines/mm), and `travelInOpen` restores
it. The constructions are published practice; "Accordion Spacing", "Colour Blending",
"Trapunto" and "Multi Blend" are vendor names, and OpenStitch labels them "Density
profile", "Blend" and "Hidden travel".

## 8. Open questions

- Whether the blend's bottom layer should get a light edge run under sewing for stability
  on knits (the guides say no underlay under gradients); proposal: none, Fabric Selector
  can add it.
- The preset list: eight shapes are what the surveyed products ship; a user profile editor
  on the canvas (drag the curve) is an editor task for later.
