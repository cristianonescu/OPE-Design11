# B3. Satin corners (mitre, cap, lap) and short stitches: algorithm sketch

Backlog item B3 of [competitive-features.md](../competitive-features.md) (features O3, O4).
Research date: 2026-10-28. A design note for `openstitch-design` (`column.rs`,
`skeleton.rs`, `lines.rs`): geometry, parameters, model change and tests. No other
product's code was read.

## 1. What the feature is

A satin column that turns a corner bunches its stitches on the inside and fans them on the
outside; a sharp corner also leaves a notch where the two legs meet. The professional
products give the digitizer a per-object choice of corner construction and thin the inner
side of bends automatically:

- **Mitre**: the column is split at the corner into two legs whose ends are cut along the
  bisector; each leg's last stitches taper into the point, and the two overlap slightly.
- **Cap**: both legs end square at the corner, and a short extra segment ("cap") whose
  stitches stay parallel to the incoming leg closes the outer corner.
- **Lap**: the first leg runs past the corner by a fraction of the width and the second leg
  starts on top of it (one leg laps the other).
- **Short stitches**: on the inside of a bend, where consecutive needle points converge, every
  other stitch (or a repeating pattern of stitches) is pulled in from the inner rail by a
  percentage of the width, so the thread does not pile up and perforate at one point.

Public descriptions with parameters: Wilcom Smart Corners (Mitre: "Mitre below" angle and
Overlap; Cap: "Cap below" 20° default, Overlap, "Max length" above which the cap reverts to
a mitre; Lap: "Lap below" 110° default, Full overlap with "Max Extension" % or Split overlap;
Mitre and Cap combine, Lap excludes both)
[[ES-corners]](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/quality/quality-11.htm),
[[ES-corners-2]](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/quality/quality-9.htm);
Bernina [[Bernina-corners]](https://softwarehelp.bernina.com/en/MainHelp/PART_Digitizing/quality/Controlling_corner_stitching.htm);
Pulse Custom Cornering, mitred steil, penetrations forced onto sharp corners
[[DG16-notes]](https://cloud.pulsemicro.com/Download/DownloadReleaseNotes?name=DG16),
[[DG17-notes]](https://cloud.pulsemicro.com/Download/DownloadReleaseNotes?name=DG17);
Embird Auto Corners (roof / folded / split) and Auto Shortening
[[Embird-column]](https://www.embird.net/studio/manual/3125par_cmn.htm); Ink/Stitch "Short
stitch distance" and multi-level "Short stitch inset"
[[IS-satin]](https://inkstitch.org/docs/stitches/satin-column/); trade articles on corners
and capping [[Impressions]](https://impressionsmagazine.com/process-technique/embroidery-tips-for-corners-and-capping/16307/),
[[Ignition]](https://blog.ignitiondrawing.com/corners-and-capping/print/).

PE-DESIGN 11 has "Sharp Corners: Miter limit" on zigzag lines only
([line attributes](http://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293378.html)),
and Half Stitch (every other inner point pulled to the centre on crowded bends).
OpenStitch today: `lines::zigzag_rails` mitres the rails of a zigzag line and leans the
stitches towards the mitre (reference tier, `line-zigzag-default`); `half_stitch` implements
Half Stitch; Auto satin's `fan_corners` fans the rungs around inner corners of a bent band
(reference tier, `satin-auto-bend-*`). None of this is a user-chosen corner construction,
and nothing thins the inner side of a Manual Punch column, a skeleton satin or a region satin.

All of it stays as it is: the new behaviour lives behind a `cornerStyle` attribute whose
default `auto` reproduces today's output, so the reference specs do not move.

## 2. Where corners are

The constructions apply to every satin that has a centreline: zigzag lines (`lines.rs`),
Manual Punch and direction-line columns (`column.rs`), skeleton and Auto satin
(`skeleton.rs`, `region::auto_satin`), and satin text strokes (which compile through the
same column code). A corner is a vertex of the centreline where the turn angle
`φ = angle between the incoming and outgoing directions` exceeds a threshold; the interior
angle is `θ = 180° − φ`. For a column given as rungs rather than a centreline, the centreline
is the chain of rung midpoints and the local width `w` is the rung length.

Decision per corner, in this order (the first rule that matches wins):

```
θ < capBelow                       → Cap        (default 20°  … acute spikes)
θ < mitreBelow and w ≥ mitreMinWidth → Mitre    (default 75°, 1.5 mm)
θ > lapBelow is NOT a rule: Lap is only chosen explicitly (see below)
otherwise                           → Turn      (today's fan: rungs rotate around the inner corner)
```

With `cornerStyle = lap` the Lap construction replaces Mitre and Cap for every corner with
`θ < lapBelow` (default 110°); it is for scalloped and wide shapes where a lap reads better
than a mitre. The thresholds are attributes with those defaults; the public sources give
20° and 110° for cap and lap and OpenStitch picks 75° for mitre, in the middle of the band
that the trade articles describe ("roughly 75–135°" is where mitres are used).

## 3. Constructions

All lengths in 0.1 mm as in the crate. `w` is the column width at the corner, `d` the
stitch spacing (`10 / density`), `V` the corner vertex on the centreline, `û_in`, `û_out` the
unit directions of the legs, `n̂` the outer-side normal.

### 3.1 Turn (today)

Unchanged: `fan_corners` for Auto satin, the leaning rungs of `zigzag_rails`, the plain
interpolated rungs of `rungs_at`. This is the `auto` result for obtuse corners.

### 3.2 Mitre

1. Split the centreline at `V` into leg A (incoming) and leg B (outgoing).
2. The bisector line through `V` with direction `b̂ = unit(û_out − û_in)` (pointing from the
   inner to the outer corner) is the cut.
3. Leg A's rails are extended past `V` and clipped at the bisector: the last rung of A runs
   from the inner corner point `V − n̂_in·w/2` (where the two inner rails meet) to the outer
   point where A's outer rail meets the bisector. Between the last regular rung of A (the
   one at distance `≥ w/2·tan(φ/2)` before `V`) and that cut rung, rungs are interpolated
   (`rungs_at` between the two) so stitches lean progressively into the bisector: a wedge of
   stitches that converge on the inner corner.
4. Leg B mirrors it: from the cut rung (now its first) to its first regular rung.
5. Overlap: leg B's cut rung is moved back along B by `overlap` (default `0.3 mm`) so the two
   wedges share a thin strip and no fabric shows at the cut. Both wedges contain the inner
   corner point, so stitches converge there; the short-stitch rule (section 4) thins them.
6. Sewing order: A then B, no travel (the cut rung's end is B's start).

This is the construction `zigzag_rails` already uses for zigzag lines (rails mitred,
stitches leaning). The change is to factor it into `column.rs` so columns and skeleton
satin share it, and to add the explicit overlap and the inner-corner thinning.

### 3.3 Cap

1. Split at `V` as above. Leg A ends with a square rung perpendicular to `û_in` at the
   point where A's **inner** rail reaches the inner corner (`V` moved back along `û_in` by
   `w/2 · cot(θ/2)`); leg B starts with the square rung where its inner rail leaves the inner
   corner.
2. The uncovered outer triangle between the two square ends and the outer corner point
   `V + n̂·(w/2) / sin(θ/2)` (the rails' intersection) is the cap. It is sewn as a short
   column of its own: rungs parallel to A's square end rung, stepping outward by `d` towards
   the outer corner point, each clipped to the triangle (so they shorten to a point), plus a
   final needle point at the tip. The cap is sewn **before** leg B (A → cap → B), so B's
   first stitches cover the cap's edge.
3. Overlap: the cap's first rung is moved `overlap` (default `0.3 mm`) back into leg A.
4. `maxLength` (default `7 mm`): if the longest cap rung exceeds it (a very wide column with
   a very sharp corner), the corner falls back to Mitre, as the Wilcom documentation
   describes, because a cap that long is a satin stitch too long to sew well.

### 3.4 Lap

1. Split at `V`. Leg A continues straight past `V` for `extension = min(maxExtension · w,
   distance to the outer corner point)` with `maxExtension` default `100 %` of `w`; its end
   rung is square.
2. Leg B starts at its regular first rung (not shortened) and sews over the extended end of
   A. B's first `overlapRungs` rungs (default 2) are kept at full width so the lap is covered.
3. Split overlap (`lapSplit = true`): A stops at `V` with a square end, B is extended
   backward by the same amount instead; the choice decides which leg lies on top. The default
   is Full (A extended) because B's cover stitches hide A's end.

### 3.5 Corners on closed and skeleton satins

A skeleton satin or a region satin with Auto direction has a centreline from
`skeleton::derive`; its corners are the skeleton's vertices with turn ≥ the threshold, and
the constructions apply to the rung chain exactly as for a column. For a closed satin ring
(a satin border around a shape), every corner of the ring is treated and the ring starts at
the entry point; the first and last corners' legs join as usual.

## 4. Short stitches on inner curves

Independent of corners (and of `half_stitch`, which stays PE-DESIGN's rule): after the rungs
of a column are placed, walk them in order and measure, for each rung `k`, the distance
between its inner-rail point and rung `k−1`'s inner-rail point, `g_k`. Where `g_k <
shortStitchDistance` (default `0.25 mm`, Ink/Stitch's documented default is in that range),
shorten rung `k`'s inner end by `inset[j] · w`, cycling `j` through the inset levels
(default `[40 %]`, that is every crowded stitch pulled in by 40 % of the width; two levels
`[30 %, 60 %]` alternate a short and a shorter stitch, Ink/Stitch's "multiple inset values").
The outer rail point is never moved, so the outer edge stays clean; the inner side of the
bend receives one full-length stitch in every `levels + 1`, so perforation is limited while
coverage at the inner rail is kept by the full stitches. `half_stitch` (every other inner
point to the centre, 50 %) is the special case `inset = [50 %]` applied to every crowded
rung rather than by distance, and remains the PE-DESIGN-compatible option.

The rule runs after corner constructions, so the convergent wedges of a mitre are thinned
too. It does not run under `puff` mode (B7), which forbids short stitches.

## 5. Parameters and defaults

| Attribute | Type | Default | Range | Applies to |
|---|---|---|---|---|
| `cornerStyle` | `auto` \| `turn` \| `mitre` \| `cap` \| `lap` | `auto` | — | satin regions, columns, skeleton satin, zigzag lines |
| `capBelow` | degrees | 20 | 5–60 | auto |
| `mitreBelow` | degrees | 75 | 30–150 | auto, mitre |
| `mitreMinWidth` | mm | 1.5 | 0.5–5 | auto |
| `lapBelow` | degrees | 110 | 30–170 | lap |
| `cornerOverlap` | mm | 0.3 | 0–2 | mitre, cap |
| `capMaxLength` | mm | 7 | 2–12 | cap |
| `lapMaxExtension` | % of width | 100 | 0–200 | lap |
| `lapSplit` | bool | false | — | lap |
| `shortStitchDistance` | mm | 0 (off) | 0–1 | all satin; 0 keeps today's output |
| `shortStitchInsets` | list of % | [40] | 1–4 values, 10–80 % | all satin |

`auto` with every threshold at its default reproduces today's output for every reference
spec because today's corners are all "Turn" at obtuse angles and the mitred zigzag line; a
regression guard spec checks that (section 7). `shortStitchDistance = 0` is off by default
for the same reason; the Fabric Selector may set it to 0.25 mm for knits later.

## 6. Model and code changes

- `model.rs`: a `Corners` struct (style and the thresholds above, all optional with
  defaults) on `RegionSewing` and `LineSewing`; `short_stitch: Option<ShortStitch>`
  (distance, insets). Serde defaults keep old `.osd` files unchanged.
- `column.rs`: `corners(rungs, corners) -> Vec<[Vec2; 2]>` detecting corners on the midpoint
  chain and applying the constructions, returning the rung sequence with the cap/lap pieces
  in sewing order; `short_stitches(rungs, w, rule)`; both called from `satin` and `fill`.
- `lines.rs::zigzag_rails`: route the mitre through the shared code (bit-identical output
  for `auto` is the acceptance test).
- `skeleton.rs` and `region::auto_satin`: pass the rung chain through the same two
  functions after `fan_corners`.
- `text.rs`: text strokes use satin columns; the attributes come from the text's sewing
  attributes.
- Engine: `object.setRegion` / `object.setLine` accept the fields; `attribute-ranges` gains
  their ranges.
- App: an "Corners" row group in the satin attributes (style dropdown; thresholds under a
  disclosure), "Short stitches" distance and insets.

Effort: M (the constructions are each under 100 lines; the care is in corner detection on
rung chains, the overlap bookkeeping and the regression guard).

## 7. Test cases (internal regression tier, plus one reference guard)

| Spec | Script | Expected |
|---|---|---|
| `corners-auto-is-today` | every existing satin and zigzag reference script re-run with `cornerStyle: auto` explicit | output identical to the reference-tier runs (same specs, same tolerances) |
| `corners-mitre-L` | 8 mm wide Manual Punch column along an L (two 40 mm legs, 90°), `mitre` | rungs near the corner converge on the inner corner point (all pass within 0.3 mm of it); no needle point outside the rails' envelope offset by 0.1 mm; the outer corner point is covered (a stitch ends within 0.3 mm of it); one run, no jump |
| `corners-cap-spike` | same column, 15° corner, `auto` | a cap: a group of rungs parallel to leg A's end rung within ±2° whose lengths decrease monotonically to a point; the tip within 0.2 mm of the rails' intersection; sewn between leg A and leg B |
| `corners-auto-mitre-30` | same column, 30° corner, `auto` | a mitre, not a cap (θ is above `capBelow`): wedges converge on the inner corner point |
| `corners-cap-max-length` | 12 mm wide column, 10° corner | the cap rung would exceed 7 mm, so the corner is mitred (no parallel rung group) |
| `corners-lap-full` | 8 mm column, 60° corner, `lap` | leg A's last rung lies `w` past the vertex along `û_in` (±0.2 mm); leg B's first rungs cover it (at least two rungs of B intersect A's extension) |
| `corners-closed-border` | satin border (6 mm) around a 30 mm square, `mitre` | four mitred corners; the ring is one run starting at the entry point |
| `corners-skeleton-bend` | the `satin-auto-bend-90-nohalf` region with `cornerStyle: mitre` | the bend's inner corner is a mitre (wedges converge on it) while `auto` reproduces the reference-tier fan |
| `short-stitch-arc` | 6 mm column on a 90° arc of 5 mm inner radius, distance 0.25 mm, insets [40] | on the inner rail, no two consecutive full-length needle points closer than 0.25 mm; every shortened stitch ends at 40 % ± 2 % of the local width from the inner rail; the outer rail's needle points are unchanged from the run without the rule |
| `short-stitch-two-levels` | same, insets [30, 60] | shortened stitches alternate 30 % and 60 % insets |
| `short-stitch-off-is-today` | same, distance 0 | identical to today's output |
| `corners-text` | the word "VAN" in Lato Bold at 20 mm, satin, `mitre` | the V's and N's acute corners mitred (wedges present), A's apex capped; stitch count within 10 % of the `auto` run |

Measurement helpers needed: "rung extraction from a satin run" (pair consecutive needle
points into rungs; exists in spirit in the satin specs), "rung direction and length lists",
"distance from needle points to a target point", "needle points within a polygon" (shared
with B1, B2).

## 8. Compatibility and licensing

`auto` with default thresholds is today's output; the guard spec enforces it. Puff mode
(B7) forces `turn` corners with no short stitches and no cap, because foam needs long,
uniform stitches.

The constructions are the standard ones described in trade articles for twenty years;
Wilcom's parameter names (Smart Corners, Mitre below, Cap below, Lap below) identify them in
this note only; OpenStitch labels them "Corner style: mitre / cap / lap" with "below" angles.
Ink/Stitch's documentation gave the short-stitch parameter names; no code was read.

## 9. Open questions

- Whether `auto` should pick Cap at 20° or follow the trade practice of capping anything
  below about 45° on narrow columns (< 3 mm). Proposal: keep 20° (the published default)
  and let the Fabric Selector or a user preset raise it.
- Whether to show the corner decisions on the canvas (a small mark per corner with the
  chosen style) so the digitizer can override one corner; per-corner overrides need a
  per-vertex attribute and are deferred.
