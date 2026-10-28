# B9. Maze and tiled meander fills, fur variant, hand-stitch randomisation: algorithm sketch

Backlog item B9 of [competitive-features.md](../competitive-features.md) (features F8, F9).
Research date: 2026-10-28. A design note for `openstitch-design` (`regions.rs`,
`compile.rs`, `model.rs`, `patterns.rs`). No other product's code was read.

## 1. What the feature is

- **Maze fill.** One continuous running path that never crosses itself fills the region at a
  set spacing, with long straight corridors and square turns (a labyrinth), rather than the
  rounded wandering of stippling. Low stitch count, strong texture.
- **Tiled meander fill.** The path is a repeating tile (hexagon, vermicelli, a space-filling
  curve, a wave) scaled and rotated, clipped to the region and joined along the outline.
- **Fur variant.** Any meander or maze path is sewn as a zigzag of a given width instead of a
  running stitch, giving a fur or grass texture.
- **Hand-stitch randomisation.** Stitch length, satin width and row angle are jittered by a
  percentage with a fixed seed per object, so output looks hand-sewn but is reproducible.

Who has it: Wilcom Maze Fill (never crosses itself) and Hand Stitch effect, Hatch 3
[[ES-hand]](https://wilcom.com/resources/blog/natural-organic-hand-stitch-effect-for-machine-embroidery),
[[Hatch-3]](https://hatchembroidery.com/resources/blog/whats-new-in-hatch-embroidery-3);
Ink/Stitch Meander Fill (pattern tile, scale, angle, clip, smoothness, zig-zag spacing and
width, random stitch length with a seed defaulting to the element id)
[[IS-meander]](https://inkstitch.org/docs/stitches/meander-fill/),
[[IS-zigzag-meander]](https://inkstitch.org/tutorials/zigzag-meander-fill/); Pulse Fractal
Fill (random runs, low count) and random density for satin
[[DG16]](https://fonts.pulsemicro.com/Home/Embroidery); Embird Random Shift and per-edge
Max. Random Broadening [[Embird-fill]](https://www.embird.net/machine-embroidery-tutorials/studio-next/par_fill.htm);
mySewnet QuiltStipple **(single source)**
[[MSW-area]](https://softwarehelp.mysewnet.com/MSW/140/Digitizing/Topics/Area_Types.htm).

PE-DESIGN 11: Stippling Stitch (run pitch for turn smoothness, spacing, optional motif) and
Decorative Fill's Random Shift 0–50 %. OpenStitch today: `regions::stipple` lays a grid of
cells of twice the spacing over the region, keeps the cells wholly inside, grows a **random
spanning tree** with a fixed seed, walks around the tree at half a cell (a Hamiltonian cycle
of the doubled grid, so the path never crosses itself) and rounds the corners by three
corner-cutting passes; one loop per connected part. `paths::Rng` is a seeded generator;
`random_shift` jitters decorative-fill tiles; the feathered edge is a seeded jitter of row
ends. So the maze is the stipple walk without the rounding and with a different tree bias,
and randomisation is a post-pass with the same generator.

## 2. Maze fill

### 2.1 Tree bias

The stipple's spanning tree picks the next cell uniformly among unvisited neighbours, which
makes short, twisty branches (right for stippling). A maze wants corridors: bias the choice
towards continuing in the current direction with probability `straightness` (default 0.7)
and otherwise choose uniformly. The walk around the tree is unchanged; the cycle visits
every quarter cell, so the path spacing is exactly `spacing` everywhere and the path never
crosses itself, whatever the bias.

### 2.2 Corners and clipping

- Rounding: off (square turns), or `rounding` 0–3 corner-cutting passes (0 = maze, 3 =
  stippling); a single generator with the parameter, so `RegionType::Stippling` keeps its
  output (rounding 3, straightness 0) and `RegionType::Maze` is (rounding 0, straightness
  0.7).
- Boundary: the stipple keeps only cells wholly inside the region, which leaves a ragged
  margin of up to one cell. The maze (and, as an option, stippling: `fillToEdge`) also uses
  cells partly inside: the path is generated on all cells whose centre is inside, then
  clipped to the region inset by `spacing / 2` with `clip` (as the net families are), and
  the clipped pieces are rejoined along the outline. The result reaches the edge at the
  cost of a few outline travels where corridors are cut.
- Subdivision to the run pitch and the run stitch type (`running` / `triple`, bean repeats
  as `runTimes`) as for stippling.

### 2.3 Entry and exit

The cycle is closed, so it is opened at the point nearest the entry; with an exit point,
the cycle is opened at the entry and the walk direction chosen so the end is nearer the
exit (the exit is then reached by travel along the outline, since a Hamiltonian cycle cannot
end elsewhere).

## 3. Tiled meander fill

### 3.1 Tiles

A tile is an `openstitch.pattern` document of kind `meander`: one open path from the tile's
left edge to its right edge inside the unit square, drawn so that tiles chain horizontally
and rows stack (the same contract as the `decorative` kind, with `rowShift`), plus the rule
that the path does not cross itself or its neighbours when chained. Tiles ship in
OpenStitch's own library (generated by `tools/gen_patterns.py`, as the motif and decorative
tiles are): `hexagon` (a honeycomb walk), `vermicelli` (the quilting squiggle), `hilbert`
(a second-order Hilbert curve), `wave`, `brick`. None is copied from any product.

### 3.2 Generation

Reuse the decorative-fill machinery (`tile_rows` with the tile's path, scale, angle and the
reference point for phase), then:

1. clip the chained path to the region (`clip`), giving pieces;
2. join pieces along the outline into as few runs as possible (`join` / `order`, as the
   decorative fill does with `Minimize feed`);
3. smooth with `smoothness` (Douglas–Peucker tolerance in mm, default 0.2, the figure the
   surveyed products suggest) and subdivide to the run pitch.

The difference from Decorative Fill is only the tile contract (open, non-crossing path
instead of closed decorative shapes) and the smoothing; the `Minimize feed`, `Random Shift`
and direction options apply as they do there.

## 4. Fur variant

For any `Maze`, `Stippling` or `Meander` path: `zigzagWidth > 0` sews each run as a zigzag
(`generate::zigzag(path, width, step)` with `step = 10 / density`) instead of a running
stitch, so every path becomes a narrow satin "ribbon" and the texture reads as fur or grass.
The path spacing should then be about the zigzag width (the inspector shows a hint). With
`randomise` (section 5) on, the zigzag width and angle jitter give the fur its unevenness.

## 5. Hand-stitch randomisation

A post-pass in `compile.rs` (or at the end of `region::layers_with`) over the generated
runs of one object, driven by `Randomise { length_pct, width_pct, angle_deg, seed }`:

- **Stitch length** (`length_pct`, default 0): each needle point along a run is moved along
  the run by `U(−p, +p) × stitch length`, keeping the first and last points and the minimum
  stitch length (points that would come closer than the minimum are dropped). Applies to
  running, triple, fills (along the row) and meanders.
- **Width** (`width_pct`): for satin and zigzag, each rung end is moved along the rung by
  `U(−p, +p) × w/2` (Embird's per-edge broadening; both edges by default, `widthSide`
  selects one). Applies to satin regions, columns, zigzag lines and fur.
- **Angle** (`angle_deg`): for fills, each row's direction is rotated by `U(−a, +a)` about
  the row's midpoint before clipping (the row generator takes a per-row angle), which breaks
  the perfectly parallel look.
- **Seed**: `seed` is the object's id by default (reproducible across saves and machines),
  and a "Reseed" button picks another; the generator is `paths::Rng`, which is OpenStitch's
  own and deterministic across platforms.

Randomisation never moves the first or last point of a run (entry and exit stay), never
produces a stitch shorter than the minimum or longer than the maximum, and is applied
after tie-in and lock stitches are placed (so they stay exact). PE-DESIGN's own Random Shift
on Decorative Fill stays as it is (it jitters tile placement, not stitches).

## 6. Parameters and defaults

| Attribute | Type | Default | Range |
|---|---|---|---|
| `type` | `maze` \| `meander` | — | new `RegionType`s; `stippling` unchanged |
| `spacing` | mm | 2.0 (maze), tile size (meander) | 1–10 |
| `straightness` | 0–1 | 0.7 (maze), 0 (stippling) | — |
| `rounding` | 0–3 | 0 (maze), 3 (stippling) | — |
| `fillToEdge` | bool | true (maze), false (stippling) | — |
| `pattern` | tile id | `vermicelli` | library and user tiles of kind `meander` |
| `patternWidth`/`Height`, `angle`, `reference` | existing | | |
| `smoothness` | mm | 0.2 | 0–1 (meander) |
| `runPitch`, `runTimes` | existing | 2.0, 1 | |
| `zigzagWidth`, `density` | mm, lines/mm | 0 (off), 4.5 | fur |
| `randomise.lengthPct` | % | 0 | 0–50 |
| `randomise.widthPct` | % | 0 | 0–50 |
| `randomise.angleDeg` | degrees | 0 | 0–15 |
| `randomise.seed` | int | object id | — |

## 7. Model and code changes

- `model.rs`: `RegionType::Maze`, `RegionType::Meander`; `straightness`, `rounding`,
  `fill_to_edge`, `smoothness`, `zigzag_width`; `randomise: Option<Randomise>` on
  `RegionSewing` and `LineSewing`.
- `patterns.rs`: `PatternKind::Meander` with validation (open path from `x = 0` to `x = 1`,
  no self-crossing when chained once); the Programmable Stitch Creator gets the kind.
- `regions.rs`: `stipple` takes `(straightness, rounding, fill_to_edge)`; `meander` built
  on `tile_rows` + `clip` + `join` + a Douglas–Peucker pass (`freehand.rs` has one);
  `fur(runs, width, step)`.
- `region.rs` / `compile.rs`: `randomise(runs, rule, seed)` after tie/lock.
- Engine: ids and ranges; `pattern.*` commands accept the `meander` kind.
- App: inspector rows; the Stitch Creator's kind list; a "Reseed" button.

Effort: M for maze and tiles (the generators exist; the new parts are the bias, edge
clipping, the tile contract and five tiles), S for fur and randomisation.

## 8. Test cases (internal regression tier, plus a reference guard)

| Spec | Script | Expected |
|---|---|---|
| `stipple-is-today` | every stippling and Background Fill stippling script | identical output (the parameters default to today's values) |
| `maze-square` | 40 × 40 mm square, `maze`, spacing 2 | one run; no self-intersection; every point inside the region; nearest-approach distance between non-adjacent path segments ≥ `0.9 × spacing`; mean straight-run length ≥ 3 cells (the bias works) |
| `maze-fill-to-edge` | same | coverage (digitize `coverage` at 2 mm radius) ≥ 97 % of the region, versus ≤ 90 % with `fillToEdge` off |
| `maze-hole` | with a hole | no point inside the hole; one run per connected part |
| `maze-seeded` | two runs of the same script | identical output; with another `seed`, different output with the same statistics |
| `meander-vermicelli` | 40 × 40 mm, `meander`, tile `vermicelli` at 8 mm | every stitch within 0.3 mm of the chained tile path; no self-intersection; pieces joined along the outline only |
| `meander-hilbert` | tile `hilbert` at 10 mm | path visits every 2.5 mm sub-cell of each tile (the curve's property); one run |
| `meander-smoothness` | `smoothness` 0 versus 0.5 | the 0.5 run has fewer points and a maximum deviation ≤ 0.5 mm from the 0 run |
| `fur-zigzag` | maze with `zigzagWidth` 1.5, density 5 | every run is a zigzag of width 1.5 ± 0.1 mm about the maze path; stitch count ≈ path length × 5 |
| `random-length` | 40 mm running line, `lengthPct` 30, pitch 2 | stitch lengths in `[1.4, 2.6]` mm, standard deviation > 0.2 mm; first and last points unchanged; identical on re-run |
| `random-width` | 6 mm column, `widthPct` 20 | rung ends within 20 % of `w/2` of the rails; none inverted |
| `random-angle` | 40 × 40 mm fill, `angleDeg` 5 | per-row direction by fit in `[−5°, +5°]`, not all equal; rows do not cross |
| `random-locks-kept` | satin region with tie-in and lock, all three jitters | the tie-in and lock stitches are identical to the un-randomised run |

Measurement helpers needed: "self-intersection count of a run" (shared with B1),
"nearest-approach distance between non-adjacent segments", "coverage" (exists), "straight
run statistic" (new, small), "stitch-length histogram" (exists for the sweeps).

## 9. Compatibility and licensing

Stippling is unchanged (guard). The maze is OpenStitch's existing spanning-tree walk with
two parameters; tiles are the pattern library's own drawings; randomisation is a seeded
post-pass. Ink/Stitch's documentation gave the parameter names (pattern, scale, angle, clip,
smoothness, zig-zag spacing and width, random seed defaulting to the element id); its
implementation and its tile library (GPL) were not read and are not used. "Maze Fill", "Hand
Stitch", "Fractal Fill" and "QuiltStipple" are vendor names; OpenStitch labels the types
"Maze", "Meander" and the effect "Randomise".

## 10. Open questions

- Whether `Meander` should simply be Decorative Fill with an open-tile kind (one type, one
  inspector) rather than a new `RegionType`; proposal: a new type, because the attributes
  differ (smoothness, fur) and users look for "meander" by name.
- Whether randomisation belongs in the Fabric Selector (a "hand-made" fabric preset) or only
  per object; proposal: per object, with a saved attribute set for the look.
