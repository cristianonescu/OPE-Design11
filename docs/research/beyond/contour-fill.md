# B1. Contour (offset) fill: algorithm sketch

Backlog item B1 of [competitive-features.md](../competitive-features.md) (feature F1).
Research date: 2026-10-28. This is a design note, not an implementation: it fixes the
geometry, the parameters, the model change and the tests so the feature can be built in
`openstitch-design` without reading any other product's code.

## 1. What the feature is

A region is filled by nested copies of its own outline, each inset by the row spacing, sewn
as running stitch. Three strategies, named after what the needle does:

- **Rings** (inner-to-outer or outer-to-inner): each inset ring is a closed running-stitch
  loop; a short connector joins one ring to the next. Reads as "echo quilting".
- **Single spiral**: consecutive rings are blended into one continuous path that starts on
  the outline and ends at the centre (or the reverse), so there is no connector at all.
- **Double spiral**: two interleaved spirals, one inward and one outward, so the path starts
  and ends on the outline. Useful when the next object continues from the border.

Two options extend it: **echo** rings outside the shape (a fixed count, as in the Background
Fill wizard, but as an attribute of the object), and **repeats** (bean-style passes over
every ring for a bolder line).

Who has it and how it is parameterised (sources in the main document): Ink/Stitch Contour
Fill (Inner to outer / Single spiral / Double spiral; join style round, mitred, bevelled;
avoid self-crossing; clockwise; smoothness; repeats)
[[IS-contour]](https://inkstitch.org/docs/stitches/contour-fill/); mySewnet Contour Fill
and Echo Fill (gap 2–20 mm, 1–30 lines, corner style Rounded / Square / Diagonal)
[[MSW-area]](https://softwarehelp.mysewnet.com/MSW/140/Digitizing/Topics/Area_Types.htm);
Wilcom/Hatch Contour Fill and Spiral (ripple) [[Hatch]](https://hatchembroidery.com/features/digitizing);
Pulse Cascade Fill [[DG15]](https://embroidery.pulsemicro.com/dg15-levels.html); Embird Fill
effect Contour and the Square/Rounded spirals from a focus point
[[Embird]](https://www.embird.net/machine-embroidery-tutorials/studio-next/par_fill.htm).

PE-DESIGN 11 has Concentric Circle (circles about a centre point, clipped to the shape),
Spiral, Flexible Spiral (shrinking copies of the outline towards a centre point, which
OpenStitch implements in `regions::flexible_spiral`) and the Background Fill wizard's echo
lines. Correction (2026-10-29, #138, references `spiral-*`): PE-DESIGN 11.50's Spiral Stitch
*is* a constant-distance offset spiral, a single spiral in the terms below, turns 0.5 mm apart
on both axes of a 40 × 30 mm rectangle; OpenStitch sews it that way (`crate::spiral`, the ring
tree and blend of §2.1–2.3). What PE-DESIGN lacks, and this feature still adds, is offset
rings as separate loops, the double spiral, echo rings outside the shape, join styles and
repeats.

## 2. Geometry

Notation: the region `R` is a set of closed contours (outer boundaries and holes, even-odd),
in 0.1 mm units as everywhere in `openstitch-design`. `d` is the row spacing.

### 2.1 Rings

Ring `k` (k = 1, 2, …) is the boundary of the inward offset of `R` by `k·d`:

```
R_0 = R
R_k = offset(R, −k·d)        // boolean::offset(contours, −k·d, rounded)
```

`boolean::offset` (i_overlay) already returns the offset as a list of regions, each a list of
contours with holes, and returns nothing once the offset swallows the shape. Two facts
decide the algorithm:

1. **An inset can split.** A U shape or a dumbbell offset inward becomes two regions after a
   few rings. So the rings form a **tree**, not a list: every region of `R_k` lies inside
   exactly one region of `R_{k−1}` (its parent). Build the tree by containment tests on one
   interior point of each child (`geometry::contains` on the parent's outer contour).
2. **Holes shrink the other way.** A hole in `R` grows by `d` per ring. i_overlay's even-odd
   offset handles this: the inward offset of a region with a hole is the region between the
   outer boundary moved in and the hole moved out. When the grown hole meets the shrunken
   outer boundary the ring splits, which the tree handles.

Join style is the `rounded` flag of `boolean::offset` today (round or mitre with a limit).
Add bevel by clipping the mitre at distance `d` (i_overlay `LineJoin::Bevel`). Rounded is the
default: it is what quilting echo looks like and it never produces a spike.

Stop conditions: no regions left; ring area below `d²` (a sliver); or the point budget
(`MAX_POINTS`, as for every generator) is reached.

### 2.2 Sewing order of rings (strategy "rings")

Each tree node is a closed loop. **Inner-to-outer** (default): depth-first, children before
parent, so the needle goes to the deepest ring of a branch, sews it, steps out to its parent,
and so on; sibling branches are sewn in nearest-first order from the current needle position.
Each step between rings is a connector of length ≈ `d` along the local normal, hidden by the
next ring only partially; it is the same short travel PE-DESIGN's concentric fills have
between rings. **Outer-to-inner** is the reverse order and is offered because it lets the
entry point stay on the outline (PE-DESIGN's R1 marker); its connectors are visible on
the last ring of each branch, so it is not the default.

Where a ring starts: at the ring point nearest the previous ring's end (`route::nearest`),
which makes every connector the shortest possible. The entry (R1) and exit (R2) points of the
object are honoured as today: the outermost ring starts at R1 for outer-to-inner, ends at R2
for inner-to-outer, and the sequence between is as above.

### 2.3 Single spiral

Blend ring `k` into ring `k+1` on the same tree branch. Parameterise both rings by arc-length
fraction `s ∈ [0, 1)` starting at the point of each nearest to a common start point `P`
(the entry point projected onto each ring, so the "seam" runs along one radial line). The
spiral turn between the two rings is:

```
S_k(s) = (1 − s) · ring_k(s) + s · ring_{k+1}(s),   s from 0 to 1
```

and the full path is `S_0, S_1, …` concatenated: at `s = 1` the point is on ring `k+1`,
exactly where `S_{k+1}` starts. The curve is continuous, crosses nothing (each `S_k` lies
in the band between its two rings, and the bands are disjoint), and ends at the deepest ring,
which it finishes as a plain loop (or, for a convex shape, at a point).

Rings must be sampled at the same count `n` before blending: resample both to `n = max(len_k,
len_{k+1}) / pitch` points. Direction (clockwise flag) is enforced on every ring before
resampling so the blend never twists.

**Branches.** A spiral cannot continue into two children. Rule: the spiral follows the child
with the largest area; every other child subtree is sewn first as its own spiral (recursively,
nearest-first), with a connector back to the branch point. So a U shape gets two spirals, one
per arm, joined by one connector, which is what the hand-quilting look expects. With
"avoid self-crossing" off (not offered; always on), nothing changes: there is no
configuration in which a blended band crosses another.

### 2.4 Double spiral

Use the rings of even index for the inward spiral and the odd ones for the outward spiral:

```
inward:  S over rings 0, 2, 4, …   (blend ring 2j into ring 2j+2 with s from 0 to 1)
outward: S over rings …, 3, 1      (blend ring 2j+1 into ring 2j−1)
```

The inward spiral ends at the deepest even ring; a connector of length ≤ `d` joins it to the
deepest odd ring, and the outward spiral returns to ring 1, ending next to the start point on
ring 0 (within `d`). Rows are effectively `2d` apart per spiral and interleave to `d`
overall, so density is unchanged. Branching uses the same largest-child rule, applied to
both spirals together.

### 2.5 Echo outside the shape

With echo count `e > 0`, rings `−1 … −e` are outward offsets `offset(R, +k·d)` clipped to the
Design Page; they are sewn before the inner rings (outermost first for inner-to-outer, so the
needle works inward through the shape), with the corner style mapped to the join style
(Rounded = round, Square = mitre, Diagonal = bevel, following mySewnet's three names). Echo
rings of neighbouring objects are not merged; that stays the Background Fill wizard's job.

### 2.6 Stitches

Each ring or spiral path is subdivided to the run pitch with `generate::subdivide`, then
smoothed. Smoothing: the offset polygons of i_overlay are exact, so the only smoothing needed
is removing points closer than `0.3 × pitch` and collinear points within a tolerance of
`0.05 mm` (the "smoothness" of other products is a deviation bound; `0.2 mm` is their
suggested value, and OpenStitch's `freehand.rs` Douglas–Peucker simplifier does the same
job). Repeats `r`: ring strategy sews each loop `r` times (odd counts return to the start
anyway for a closed loop); spiral strategies apply repeats as a triple-stitch-style
forward-back-forward over each segment (`generate::triple` logic with `r` passes).

Tie-in and lock stitches come from `region::tie_and_lock` as for every region, and the
connectors are plain running stitch, never jumps (no trims inside the object).

## 3. Parameters and defaults

| Attribute | Type | Default | Range | Notes |
|---|---|---|---|---|
| `type` | `contour` | — | — | new `RegionType::Contour` |
| `spacing` | mm | 2.0 | 0.5–20 | row spacing `d`; mySewnet's 2–20 mm range, PE-DESIGN's concentric "spacing" semantics |
| `contourStrategy` | `rings` \| `singleSpiral` \| `doubleSpiral` | `rings` | — | |
| `contourOrder` | `innerToOuter` \| `outerToInner` | `innerToOuter` | — | rings only |
| `join` | `round` \| `miter` \| `bevel` | `round` | — | maps to the echo corner style too |
| `counterclockwise` | bool | false | — | existing attribute (spiral direction on screen) |
| `runPitch` | mm | 2.0 | 1.0–5.0 | existing attribute `run_pitch`; lines default 2.0 in PE-DESIGN |
| `runTimes` | 1–3 | 1 | | repeats; reuse the `triple` flag semantics or a small int |
| `echoLines` | int | 0 | 0–30 | rings outside the shape |
| `startInside` | bool | — | — | existing attribute; alias for `contourOrder` when set |

Everything else on `RegionSewing` (thread, entry/exit, hole sewing) applies unchanged.
Underlay is not offered (as for PE-DESIGN's running-stitch fills); density is not a
parameter, spacing is.

## 4. Model and code changes

- `model.rs`: `RegionType::Contour` (id `contour`, label "Contour"), density range as the
  running-stitch fills, new fields `contour_strategy`, `contour_order`, `join`, `echo_lines`
  (all `Option`/default so existing `.osd` files load unchanged); `attribute_ranges` entries.
- `regions.rs`: `fn contour(closed, holes, p) -> Vec<Run>` built on `boolean::offset`, a
  `RingTree` type (node = region contours, children), the three strategies, the echo rings,
  then `region::reroute`-compatible output (one `Run` per connected path).
- `region.rs::layers_with`: dispatch for `RegionType::Contour`; travel between the output
  runs uses the existing travel graph.
- Engine: `object.setRegion` accepts the new ids; capabilities unchanged.
- App: inspector rows (strategy, order, join, spacing, run pitch, echo lines) in the region
  sewing attributes pane; the Background Fill wizard can later offer "contour" as a fill.
- PES export: a contour fill compiles to plain stitches like every OpenStitch fill; in a
  `#PES0110` editable save it is written as a stitch group (no PE-DESIGN sew type matches).

Effort: S–M. The offset, containment, walker, subdivide, tie/lock and travel pieces exist;
the new code is the ring tree, the blend and the ordering, roughly 400 lines plus tests.

## 5. Test cases (internal regression tier)

Written as `compat/specs/stitches/contour-*.json` once the attribute exists; the expected
values are geometric, not PE-DESIGN references (there is nothing to compare against), so the
tier is "internal regression".

| Spec | Script | Measurements and expected values |
|---|---|---|
| `contour-square-rings` | 40 × 40 mm square, spacing 2 mm, rings, inner-to-outer | 10 rings (inset 2…20 mm; the 20 mm inset is a point and is dropped, so 9 loops + centre reached); every stitch length ≤ run pitch + 0.05 mm; number of runs = 1 (connectors are stitches, not jumps); no jump inside the object; the last stitch is within 0.5 mm of the outline (exit on the border) |
| `contour-square-single-spiral` | same, `singleSpiral` | exactly 1 run; path never self-intersects (segment-pair check); consecutive-point distance between the path and its own earlier turn is within `[0.8d, 1.2d]` for 95 % of samples (even spacing) |
| `contour-circle-double-spiral` | circle Ø 40 mm, spacing 2 mm, `doubleSpiral` | 1 run; first and last stitch both within `d` of the outline and within `2d` of each other; even spacing as above |
| `contour-u-shape-splits` | a U (60 × 40 mm outer, 20 × 30 mm notch), spacing 2 mm, rings | ring count after the split equals the sum over the two arms; sewing order sews one whole arm before the other (the needle never crosses the notch except once); exactly 1 connector longer than `2d` (the arm-to-arm step) |
| `contour-hole` | 40 mm square with a 10 mm square hole (hole sewing pair), spacing 2 mm, rings | rings stop when the grown hole meets the shrunk outer boundary; no stitch inside the hole (point-in-polygon on every needle point); all rings closed |
| `contour-echo-3` | 20 mm circle, `echoLines` 3, spacing 3 mm | 3 outer loops at radii 13, 16, 19 mm (± 0.2 mm, measured as mean distance to centre) sewn before the inner rings |
| `contour-join-styles` | 40 × 40 mm square, spacing 2 mm, each join | round: ring 5 has no vertex with interior angle < 100°; miter: ring 5 has four 90° corners at the expected inset; bevel: ring 5 has eight corners |
| `contour-repeats` | 20 mm circle, rings, `runTimes` 3 | stitch count = 3 × the single-pass count ± 2 per ring |
| `contour-limits` | 300 × 300 mm square, spacing 0.5 mm | generator returns within the point budget and refuses or truncates deterministically; `attribute-ranges`-style refusal for spacing 0.2 mm and 25 mm |

Measurement helpers needed in `openstitch-compat`'s measurement library: "run count",
"max stitch length", "self-intersection count of a run", "spacing histogram between a path
and its previous turn" (nearest-distance from each needle point to the set of needle points
more than one ring earlier), "needle points inside a polygon". The first two exist; the rest
are small.

## 6. Interaction with PE-DESIGN compatibility

Nothing changes for existing types. The reference specs never set `type: contour`, and the
default region type stays Fill. A design using a contour fill saved as v6/v1 PES or opened
in PE-DESIGN is stitch data; the `.osd` keeps it editable. The FEATURE_MATRIX row stays
"Planned (beyond PE-DESIGN)" until the specs above pass, then becomes "Implemented
(internal tier)".

## 7. Licensing

Polygon offsetting and the blend are textbook geometry; i_overlay is MIT/Apache. US 5541847
(medial-axis placement of concentric stitches, 1996) has expired and is not used anyway.
Ink/Stitch's implementation is GPL and was not read; its documentation page gave the names
of the three strategies and the join styles, which are generic terms. "Cascade" and
"Contour Fill" as product names are not used in the UI ("Contour" is the generic word
PE-DESIGN's own guide uses for outlines, so it is fine as a sew-type label).

## 8. Open questions

- Whether the outer-to-inner order should also be offered for spirals (it is only a
  reversal of the path; cheap, but one more control).
- Whether to expose "smoothness" at all or keep the fixed 0.05 mm collinearity tolerance;
  proposal: keep it fixed until a user asks.
- Echo rings on text: text objects compile through `text.rs` and would need the region
  attributes exposed per glyph; defer.
