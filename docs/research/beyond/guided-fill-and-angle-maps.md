# B2. Two-guide fills and stitch-angle maps: algorithm sketch

Backlog item B2 of [competitive-features.md](../competitive-features.md) (features F2–F4).
Research date: 2026-10-28. A design note for `openstitch-design`: geometry, parameters,
model change and tests. No other product's code was read.

## 1. What the feature is

Today a fill or satin region has one direction: a fixed angle, Auto, or a single direction
curve (PE-DESIGN's "manual curve", OpenStitch `direction_curve`). Two extensions:

- **Two-guide fill.** The user draws two curves across the region; rows interpolate from the
  first to the second, so the flow of the stitching changes across the shape (a petal that
  is straight at the base and curls at the tip). Wilcom calls the one-guide case Florentine
  and the two-guide case Liquid [[ES-curves]](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Decorative/curves/curves-10.htm),
  [[ES-florentine]](https://docs.wilcom.com/embroiderystudio/e4/en/MainHelp/Decorative/curves/Create_curved_fills_with_Florentine_Effect.htm);
  mySewnet's MultiWave Fill takes one or more wave lines **(single source)**
  [[MSW-area]](https://softwarehelp.mysewnet.com/MSW/140/Digitizing/Topics/Area_Types.htm);
  Ink/Stitch's Guided Fill documents three one-guide strategies (copy, parallel offset,
  buffer) [[IS-guided]](https://inkstitch.org/docs/stitches/guided-fill/).
- **Stitch-angle map.** The user draws several short direction lines across the region;
  between two consecutive lines the stitch direction turns progressively, so one object sews
  as a "turning fill" without being split. Wilcom stitch angles
  [[ES-angles]](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/quality/quality-3.htm),
  Pulse angle lines [[DG16]](https://fonts.pulsemicro.com/Home/Embroidery), Embird direction
  lines [[Embird-fill]](https://www.embird.net/machine-embroidery-tutorials/studio-next/par_fill.htm),
  Embrilliance inclines [[EM-SA]](https://www.embrilliance.com/stitchartist).

PE-DESIGN 11 has neither. What OpenStitch already has decides the shape of the work:

- `direction::Warp` straightens one guide by the shear `(x, y) → (x, y − f(x))` in the
  guide's chord frame, fills the straightened region with horizontal rows, and bends the
  stitches back. That is the "copy" strategy: every row is a translated copy of the guide,
  and rows never cross.
- `RegionSewing::direction_lines` (two or more segments) already makes a Fill or Satin
  region sew as a Manual Punch column whose rungs interpolate between consecutive lines
  (`column::rungs_at`, `column::fill` / `column::satin`), with the curved-sides variant
  `curved_lines`. That is a stitch-angle map restricted to the column the lines span.

So B2 is: a second guide for the warp, and the clipping of the direction-line column to the
region (plus the rows before the first line and after the last).

## 2. Two-guide fill

### 2.1 Geometry

Guides `A` and `B` are open polylines, both oriented the same way (the user draws them in
the same direction; the engine flips `B` if its chord points against `A`'s). Both must leave
the region on both ends: the engine extends each guide along its end tangents until it is
outside the region's bounding box, as the one-guide `Warp` does by holding `f` constant
beyond the graph.

Define the sweep surface

```
G(s, t) = (1 − t) · A(s) + t · B(s),   s ∈ [0, 1] by arc-length fraction, t ∈ ℝ
```

Row `t` is the curve `s ↦ G(s, t)`. For `t` in `[0, 1]` it lies between the guides; for
`t < 0` or `t > 1` the formula extrapolates, which is a translated copy of the blend
direction and keeps rows parallel at the far ends. Rows are spaced by the density
`d = 1 / density` measured along the local normal, so the `t` step is not constant: step
from row `t` to the next by `Δt = d / |B(s*) − A(s*)|⊥` evaluated at the point `s*` where the
row is nearest the region's centre (one measurement per row keeps rows equidistant where it
matters most; the exact per-point spacing varies with the guide distance, which is the look
of the feature).

### 2.2 Generation in warped space

As with `Warp`, the ordinary generators run in a space where rows are horizontal. The warp
`W: (s, t) ↦ G(s, t)` is invertible on the part of the plane where the Jacobian keeps its
sign, that is where the guides do not cross and the rows do not fold. Its inverse has no
closed form; compute it per point by Newton iteration on `(s, t)` from a seed found by
sampling `s` at the guide's vertices (the guides are polylines, so `G` is piecewise bilinear
and two or three Newton steps converge). Procedure:

1. Validate: both guides at least two points and 1 mm long; chords in the same half-plane;
   no intersection between `A` and `B` inside the region's bounding box; the sign of
   `det[∂G/∂s, ∂G/∂t]` constant over a grid of the region's bounding box. Otherwise the
   error `CurveError::GuidesCross` and the region falls back to its straight angle, as the
   one-guide warp does on `TurnsBack`.
2. Map the region's contours and holes into `(s, t)` space with `W⁻¹`, splitting each edge
   where it crosses a guide vertex's `s` so the mapped outline is exact between vertices
   (`forward_contours` does this for the one-guide case).
3. Set `flat.angle = 0`, `flat.direction_curve = None`, map the entry, exit and reference
   points, and call `layers(&c, &flat, &h)` so under sewing, Frequency, gradation, feather,
   split length, Running Stitch Path and tie/lock all apply unchanged.
4. Map every stitch back with `W`. Rows come out as the blended curves; stitch length is
   measured in `(s, t)` space, so re-subdivide each row in model space to the stitch length
   (`generate::subdivide`) and merge points closer than the minimum stitch.

Row spacing in `(s, t)` space is `1 / density` in `t` units scaled by the mean guide
distance (so that the mean model-space spacing equals `d`); the per-row `Δt` refinement of
section 2.1 replaces the uniform `row_positions` with a position list computed from the
guides, which `region::row_positions` already accepts as a slice.

### 2.3 Degenerate and open cases

- Guides that touch at one end (a fan): the Jacobian is zero at the apex; allow it when the
  apex lies outside the region, refuse otherwise.
- One guide much longer than the other: arc-length fractions still pair the ends; the user
  sees rows compress towards the short guide, which is the intended "liquid" look.
- Region larger than the guides' span in `s`: the extended tangents cover it with straight
  continuations, so rows become straight outside the guides' reach.
- Satin: the two-guide warp applies to `Fill`, `ProgrammableFill`, `Piping` and `MotifFill`
  (today's `CURVE_TYPES`); Satin keeps its own skeleton and Manual Punch paths, because a
  satin "row" is one stitch and the warp would bend it into a polyline.

## 3. Stitch-angle maps

### 3.1 Geometry

Direction lines `L₁ … Lₙ` are segments the user draws across the region in sewing order
(the existing `direction_lines`, drawn with the Manual Punch direction tool). Extend each
line at both ends along its own direction until it leaves the region's bounding box. The
lines must not cross one another inside the region (error, fall back to the current column
behaviour, which is what Manual Punch expects).

The **strip** `Sᵢ` between `Lᵢ` and `Lᵢ₊₁` is the region clipped to the band bounded by the
two extended lines: a polygon with the two lines as sides and the region boundary as caps
(boolean intersection of the region with the quadrilateral spanned by the two extended
lines; `boolean::combine` with `BoolOp::Intersection`). Strip `S₀` is the part of the region
before `L₁` (clipped by the half-plane behind `L₁`) and `Sₙ` the part after `Lₙ`.

Inside `Sᵢ`, rungs interpolate between `Lᵢ` and `Lᵢ₊₁` exactly as `column::rungs_at` does
(`[a.lerp(c, t), b.lerp(d, t)]`), but each rung is **clipped to the strip** by intersecting
the infinite line through the rung with the strip polygon (`region::spans_at` with the rung's
own angle, at the rung's position). So a region wider than the drawn lines is still covered,
and a rung that crosses a hole becomes two spans, as in a straight fill. Rows in `S₀` and
`Sₙ` are parallel to `L₁` and `Lₙ`.

Rung spacing: the rungs of a strip whose lines are not parallel converge towards the inner
side. Space the rungs by `d` on the **longer** side (as `rungs_at` does) and thin the inner
side with the short-stitch rule from B3 for satin; for fills, drop every second needle row
where the inner spacing falls below `0.6 d` (what PE-DESIGN's Auto satin corner fan does in
OpenStitch today for crowded bends).

### 3.2 Sewing order and travel

Strips are sewn in line order `S₀, S₁, …, Sₙ` (so the lines double as the sewing-order
drawing). Each strip's rows are sewn edge to edge as a tatami; the strip is entered at the
end of its first row nearest the previous strip's last stitch, and the row phase (Frequency)
continues across the strip boundary so the needle grid does not jump at the line. Travel
between strips is a running stitch along the shared line, hidden under the next strip's
first row. Under sewing is the ordinary straight under sewing of the whole region (what
PE-DESIGN does under Auto satin); gradation and feather apply per region as today.

Strips that the region splits into several pieces (a strip crossing a hole or a notch) are
sewn piece by piece, nearest first, with travel through the already-sewn strip.

### 3.3 Relation to the existing direction-line column

The current behaviour (direction lines make the region a column, the column's outline being
the lines' ends) is PE-DESIGN's Manual Punch and must stay. The map is a new mode: an
`angleMap: true` flag on the region, settable from the inspector and the Sewing Direction
ribbon group ("Angle lines"), off by default. With the flag off nothing changes.

## 4. Parameters and defaults

| Attribute | Type | Default | Notes |
|---|---|---|---|
| `directionCurve` | contour | — | existing one-guide curve |
| `directionCurve2` | contour | none | second guide; present ⇒ two-guide warp |
| `directionLines` | `[[p, q], …]` | — | existing |
| `angleMap` | bool | false | strips mode instead of the Manual Punch column |
| `stitchLength` | mm | existing | the guide docs advise ≤ 4 mm on curved rows; the inspector shows a hint, no clamp |

Everything else (density, Frequency, gradation, feather, under sewing, Running Stitch Path,
entry/exit) applies unchanged.

## 5. Model and code changes

- `model.rs`: `direction_curve_2: Option<Contour>`, `angle_map: bool` (serde default), both
  cleared when the region type leaves `CURVE_TYPES` / `Satin|Fill` as `direction_curve` is.
- `direction.rs`: `Sweep` (two-guide warp) with `new`, `forward`, `back`, `forward_contours`,
  `back_runs`, the Newton inverse and the validation; `CurveError::GuidesCross`.
- `region.rs::layers_with`: the two-guide branch before the one-guide branch; the
  `angle_map` branch before the direction-line column branch, calling a new
  `regions::angle_map(closed, holes, r)` that builds strips with `boolean::combine`, rungs
  with `rungs_at`, spans with `spans_at`, and orders rows as in section 3.2.
- Engine: `object.setDirectionCurve` takes an optional second curve (or a new
  `object.setDirectionCurves`); `object.setRegion` accepts `angleMap`; the direction-line
  commands are unchanged.
- App: Sewing Direction group gets "Second curve" and "Angle lines" toggles; the canvas
  draws both guides with their handles (the existing curve editing) and the strips'
  boundaries faintly when `angleMap` is on.
- `.osd`: new optional fields; old files load unchanged.

Effort: M for the two-guide warp (the Newton inverse and validation are the work), M–L for
angle maps (strip clipping, ordering, the thinning rule, and the editor state).

## 6. Test cases (internal regression tier)

| Spec | Script | Expected |
|---|---|---|
| `guided2-rect-straight-guides` | 60 × 30 mm rectangle; guides: two horizontal lines at y = 5 and y = 25 | identical stitches to the 0° straight fill within 0.05 mm (the warp is the identity up to translation) |
| `guided2-rect-fan` | same rectangle; guide A horizontal at y = 5, guide B tilted 20° at y = 25 | row directions measured by linear fit per row turn monotonically from 0° to 20° across the region; mean row spacing within 5 % of `1 / density`; no two rows intersect |
| `guided2-petal` | a leaf outline; A straight along the base, B an S-curve | every stitch inside the outline; stitch length ≤ stitch length attribute + 0.05 mm; row count within ±1 of `height / d` |
| `guided2-reject-crossing` | guides that cross inside the region | `object.setDirectionCurves` refuses with `GuidesCross`; the region keeps its straight angle |
| `guided2-attributes-apply` | fan case with Frequency 30 %, gradation on, feather on | the straightened-space measurements (needle shift per row, density fall, feather width) match the straight-fill specs `fill-frequency-*`, `region-travel-u-gradation`, `satin-feather` within their tolerances |
| `anglemap-three-lines` | 60 × 20 mm rectangle; lines at 90°, 60°, 30° across it, `angleMap` on | three strips; row angle within each strip turns monotonically between its lines; the whole rectangle is covered (coverage ≥ 99 % by the raster coverage check in `openstitch-digitize::coverage`); exactly 2 inter-strip travels, each along a line |
| `anglemap-wider-than-lines` | lines drawn shorter than the region | coverage ≥ 99 % (rungs are clipped to the strip, not to the lines) |
| `anglemap-hole` | region with a hole crossed by a strip | no needle point inside the hole; the strip's two pieces sewn nearest-first |
| `anglemap-off-is-column` | same lines with `angleMap` off | byte-identical output to today's direction-line column (regression guard for Manual Punch) |
| `anglemap-fan-thinning` | two lines meeting at 30° just outside the region | inner-side needle spacing never below `0.5 d`; outer side within `[0.9 d, 1.1 d]` |

Measurement helpers needed: "row direction by linear fit" (group stitches into rows by
the travel pattern), "row intersection count", "coverage" (exists in digitize), "needle
points inside a polygon" (shared with B1).

## 7. Compatibility and licensing

Off by default; no reference spec sets either new attribute. Compiled output is stitch data
in every machine format; an editable `#PES0110` save writes a stitch group.

The sweep surface is textbook ruled-surface geometry; the strip decomposition is boolean
clipping with i_overlay. WO2005061774 (turning complex fill) and US 6587745 (curved line
fill) have expired per their priority dates; confirm before shipping, as the main document
says. Ink/Stitch's documentation named three one-guide strategies; this note uses the sweep
(a two-guide generalisation of the copy strategy) and OpenStitch's own existing warp. No
Ink/Stitch code was read. "Florentine", "Liquid" and "MultiWave" are vendor names and do
not appear in the UI.

## 8. Open questions

- Whether the per-row `Δt` refinement is worth it or a uniform `t` step (rows denser where
  guides converge) is the better-looking default. Proposal: uniform step with a density
  warning when the local spacing leaves `[0.5 d, 2 d]`, then revisit after a sew-out.
- Whether angle maps should also drive Satin (one stitch per rung, which is exactly the
  Manual Punch column) or stay fill-only; proposal: both, since the rung code is shared.
- Editor affordance for drawing the second guide: a second pass of the existing direction
  curve tool, or a modifier key. UX decision for the editor session.
