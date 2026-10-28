# Implementation readiness of the "beyond PE-DESIGN" backlog

Research date: 2026-10-28. Which backlog items of [competitive-features.md](../competitive-features.md)
the existing crates can take first, with the concrete entry points, what each adds, and a
grouping into development tracks that do not touch the same files. Each item has an
algorithm sketch in this folder (B1–B10) or a note in the main document (B11–B15). Line
numbers are at `main` on 2026-10-28 and will drift; the function names are the anchor.

## 1. Which crate does the work

The coordinator asked about `openstitch-digitize`. That crate converts images (Auto Punch,
Cross Stitch, Photo Stitch, Design Center) and takes **none** of B1–B15 directly: every
backlog item is stitch generation on editable objects, which lives in `openstitch-design`,
with commands in `openstitch-engine` and panes in `openstitch-app`. `openstitch-digitize`
contributes two things: `coverage::trace` (`crates/openstitch-digitize/src/coverage.rs:65`)
is the coverage measurement several new specs need, and the deferred F16 (photo
thread-painting with density-modulated meanders) would be a new conversion in
`digitize/photo.rs` built on B9's meander generator. `openstitch-formats` is untouched by
B1–B15 (every item compiles to plain stitches); only W6 (VP3 and other formats) would land
there.

## 2. Readiness ranking

Readiness = how much of the item is existing code (reuse), how small the model change is,
whether it needs editor work before it is usable, and whether another item must land
first. "Reuse" names the functions the item builds on; "New" is what has to be written.

| Rank | Item | Crate(s) | Reuse (entry points) | New | Model change | Editor need | Depends on | Effort |
|---|---|---|---|---|---|---|---|---|
| 1 | **B6 density profiles, blend, hidden travel** | design, engine | `region::row_positions` (`region.rs:1379`) already integrates spacing over `Gradation::fraction` (`model.rs:786`); `RunningStitchPath` travel (`region::reroute_with`, `region.rs:427`); `generate::fill_edge_to_edge` (`generate.rs:178`) | profile shapes, the complement copy in a `shape.blend` command, the open-fill travel rule | `Gradation.shape` + custom points (serde default = today) | one dropdown and a button | none | S |
| 2 | **B1 contour fill** | design, engine | `boolean::offset` (`boolean.rs:107`), `geometry::contains`, `regions::concentric` / `flexible_spiral` (`regions.rs:768`, `843`) for the ring plumbing, `region::tie_and_lock` (`region.rs:981`), `route::nearest` | ring tree, the arc-length blend, ring ordering, echo rings | `RegionType::Contour` + 4 optional fields | inspector rows only | none | S–M |
| 3 | **B8 crosshatch and lace** | design, engine | `regions::net_families` / `tile_rows` (`regions.rs:1088`, `547`), `generate::zigzag` (`generate.rs:103`), `Region::sequence_raw`, the Background Fill crosshatch (`wizards/background.rs:26`) | the family builder with free angles, `double_zigzag`, the "Lace" preset | `RegionType::Crosshatch` + style fields | inspector rows | B2 for the curved variant (optional) | S |
| 4 | **B7 3D puff mode** | design, engine | `column::satin` (`column.rs:157`), `skeleton::satin` (`skeleton.rs:204`), `lines::zigzag_rails` (`lines.rs:767`), `Command::Stop` (as the cutwork wizard inserts it, `wizards/cutwork.rs:33`), `order::optimized` (`order.rs:174`) | the cap generator, the puff-last rule, the foam stop | `puff` + 4 optional fields on region and line sewing | a toggle and a pane | B3 corner style and B5 push are nice-to-have, not required | S–M |
| 5 | **B12 carving lines, knockdown fill** | design, engine | `stamp.rs` masks needle points for stamps; `wizards/background.rs` for the knockdown as a first-sewn background fill; `boolean::offset` for the inflation | needle points forced onto drawn lines; the knockdown command | a `carving: Vec<Contour>` field; none for knockdown | a draw tool reuse (pen) | none | S |
| 6 | **B10 variable-width satin line** | design, engine, app | `Skeleton` and `skeleton::satin` (`skeleton.rs:22`, `204`), `shape.skeletonSatin` (`punch.rs:269`), `freehand::simplify` for pressure trails, `import.svg` (`imports.rs:219`) | the key model, asymmetric rungs, tapers, conversions, canvas handles | `LineType::Satin` + keys | handles on the canvas (the main cost) | B3 for corners (optional) | S–M |
| 7 | **B5 compensation, auto underlay, fabric table** | design, engine, app | the under-sewing match in `region::layers_inner` (`region.rs:98`, the `UnderlayStyle` arms around line 263), `column::satin` / `fill`, `skeleton::rungs` (`skeleton.rs:181`), `fabric::FABRICS` and `fabric.apply` (`fabric.rs:35`, `160`) | `Compensation` with serde alias, `UnderlayStyle::Auto` with the width table, centre-run generator, editable fabric table | the largest: every generator signature takes two compensation values | a full pane | none, but B3/B7/B10 read its fields | M |
| 8 | **B3 corners and short stitches** | design | `lines::zigzag_rails` mitre (`lines.rs:767`), `region::fan_corners` (`region.rs:1136`), `column::rungs_at` (`column.rs:123`), `half_stitch` | corner detection on rung chains, cap and lap constructions, the inset rule, the regression guard | `Corners` and `ShortStitch` optional structs | a dropdown and a disclosure | none | M |
| 9 | **B11 closest join, auto start/end, continue digitizing, overlap removal** | engine, design | `order::link_entry_exit` (`order.rs:349`), `route::route_region` / `nearest` (`route.rs:217`), `shape.combine` / `boolean::combine` (`boolean.rs:71`) | per-object re-apply, leading/trailing jumps, pen-tool append, automatic clip option | none (commands) | pen-tool mode | none | S |
| 10 | **B13 density map, troubleshoot, thread conversion, auto hoop** | engine, app, core | the compiled `StitchPattern`, the charts in `openstitch-core::charts`, `BROTHER_HOOPS` (`machine.rs`) | a grid counter overlay, rule set, ΔE2000 nearest colour, smallest-hoop pick | none | an overlay and a dialog | none | S each |
| 11 | **B9 maze, meander tiles, fur, randomisation** | design, engine | `regions::stipple` (`regions.rs:917`, the spanning-tree walk), `tile_rows`, `paths::Rng`, `freehand::simplify` (`freehand.rs:57`), `PatternKind` (`patterns.rs:29`) and `tools/gen_patterns.py` | tree bias and edge clipping, the `meander` tile kind and five tiles, the fur pass, the randomise post-pass | two `RegionType`s, a pattern kind, `randomise` struct | inspector rows, Stitch Creator kind | none | M |
| 12 | **B2 two-guide fills and angle maps** | design, engine, app | `direction::Warp` (`direction.rs:42`), `object.setDirectionCurve` (`sewing.rs:53`), the direction-line column branch in `region::layers_inner`, `column::rungs_at`, `boolean::combine`, `region::spans_at` | the sweep warp with Newton inverse, strip decomposition and ordering, inner-side thinning | `direction_curve_2`, `angle_map` | second-guide drawing, strip display | B5/B6 touch `region.rs` first; B3 for thinning | M–L |
| 13 | **B4 auto-route, Redwork, branching** | design, engine, app | `route::travel` (`route.rs`), `order::optimized`, `generate::triple` for bean passes, `skeleton::derive` (`skeleton.rs:489`) for centrelines | a `graph.rs` (intersections, matching, Hierholzer), the branch walk, a group flag in `compile.rs` (`compile.rs:183`) | group `branch` flag | Sewing Order pane entries, Branch command | none for routing; B3 for junction caps optional | M + L |
| 14 | **B15 lettering depth** | design, engine, app | `text.rs` layout and `envelope_edges`, `DesignFont`, Name Drop (`text.rs:311`), Connection Points work | baseline model, saved kerning map, CSV team names, closest-point letter linking | `Baseline` enum, kerning map on `DesignFont` | Text Attributes pane | B11's closest join | M |
| 15 | **B14 stitch-file resize with density** | design, engine | `stitch_edit.rs`, the stitch→shape approximation, `column::satin` / `generate::fill` for regeneration | run segmentation into satin / fill / run classes, regeneration | none | a dialog | none | M; patent check |

Ranks 1–6 reuse an existing generator almost entirely and change the model by optional
fields only; they are the candidates for the first tracks. Rank 7 (B5) is the one model
change that several later items read, so it should start early even though it is M. Ranks
12–13 are the high-value long items and want the earlier tracks' measurement helpers in
place.

## 3. Shared test infrastructure

The sketches name measurement helpers for `openstitch-compat`'s library (`measure.rs:95`,
`enum Measure`) that several items share; build them once, first:

| Helper | Needed by |
|---|---|
| run count, jump and trim counts, stitch-length histogram | exist |
| self-intersection count of a run; nearest-approach distance between non-adjacent segments | B1, B9 |
| needle points inside a polygon | B1, B2, B3, B4, B8 |
| rung extraction from a satin run; rail distance per rung end; rung angle to the tangent | B3, B5, B7, B10 |
| row grouping and row direction by linear fit; row gap list from the start edge | B2, B6, B9 |
| coverage of a region (wrap `digitize::coverage::trace`) | B2, B9 |
| edge coverage count (passes per input edge) | B4 |
| command sequence (stops, colour changes) | B7 (exists in part) |
| layer identification (underlay vs top) | B5, B6 |

One small PR adding these to `measure.rs` with unit tests unblocks every track's specs.

## 4. Proposed development tracks

Grouped so that no two tracks edit the same file at the same time, following the TRACKS.md
pattern (own worktree, own PR, rebase on main after each merge). Items in one track land
one PR each, in the order given.

| Track | Items | Owns (files) | Reads | First PR |
|---|---|---|---|---|
| **measures** | the helpers of section 3 | `openstitch-compat/src/measure.rs`, `tolerance.rs` | — | all nine helpers with tests; one week |
| **fills** | B6 → B1 → B8 → B9 | `regions.rs`, `patterns.rs`, the `Gradation` part of `model.rs`, `tools/gen_patterns.py` | `region.rs` (B6's `row_positions` change is small and lands first, before the satin track touches `region.rs`) | B6: profiles and `shape.blend` |
| **satin** | B5 → B3 → B7 → B10 | `column.rs`, `skeleton.rs`, `lines.rs`, the compensation and underlay parts of `model.rs` and `region.rs`, `fabric.rs` | `punch.rs` for skeleton commands | B5: `Compensation` with the alias and the serde round-trip test, then `UnderlayStyle::Auto` |
| **routing** | B11 → B4 (running, Redwork) → B4 (branching) | `route.rs`, new `graph.rs`, `order.rs`, the group flag in `compile.rs` | `skeleton.rs` for centrelines (after the satin track's B3) | B11: closest join per object and auto start/end |
| **tools** | B13 → B12 → B14 | app overlays and dialogs, `stamp.rs`, `wizards/background.rs`, `stitch_edit.rs`, `charts` | compiled output only | B13: density map overlay and thread conversion |
| **turning** (after fills and satin) | B2 → B15 | `direction.rs`, the direction branches of `region.rs`, `text.rs` | everything above | B2: the two-guide `Sweep` warp with its validation |

Every PR keeps the PE-DESIGN defaults by construction (new attribute or command, off
unless chosen) and carries its guard spec that re-runs the affected reference specs, so the
release gate (every reference spec green on three OSes) is unchanged. The sew-out tier
matters most for the satin track: B5 and B3 change what the thread does on fabric, and a
sew-out of the corner and compensation specs on the owner's machine should come before
they become fabric defaults.

## 5. What is deliberately not ready

- **F16 photo thread-painting** (digitize): waits for B9's meander and a patent check
  (US 7996103).
- **D12 TrueType turning-satin rework**: waits for the exact medial axis already on the
  roadmap and the US 8532810 review, which also concerns today's skeleton satin.
- **W1 intelligent multi-hoop split with editable lines**: patent check (Embrilliance).
- **W6 VP3 and other formats** (formats crate): independent of all of the above; one
  track of its own when a Husqvarna/Pfaff user asks, with pyembroidery as the oracle.
- **F19 sequins, chenille, beads, cording**: no Brother home machine supports them.
