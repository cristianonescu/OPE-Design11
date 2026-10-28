# B13. Quality tools: density map, troubleshoot, thread-brand conversion, auto hoop

Backlog item B13 of [competitive-features.md](../competitive-features.md) (features D15,
W2, W3). Research date: 2026-10-28. A design note for `openstitch-engine`, `openstitch-core`
and the editor. No other product's code was read.

## 1. What the feature is

Four small tools that show the user what will go wrong before the machine does:

- **Density map.** A heat overlay on the canvas: green where stitch density is normal, yellow
  where it is high, red where it will break needles or pucker (layered fills, satin over
  fill, tiny stitches bunched at corners).
- **Troubleshoot.** A list of findings over the design with a marker on the canvas for each:
  stitches shorter than the minimum or longer than the maximum, satin wider than the limit,
  rails that cross, regions whose fill produced no rows, objects outside the hoop, colour
  changes that could be merged, long jumps without a trim.
- **Thread-brand conversion.** One command maps every thread of the design to the nearest
  colour of a chosen brand chart (or of the user's owned threads), with a report of the
  colour distance per thread.
- **Auto hoop.** Pick the smallest hoop of the machine's list that contains the design, on
  demand or after import.

Who has it: Ink/Stitch Troubleshoot Objects, Density Map (red/yellow/green dots by stitch
count within a radius, configurable thresholds), Apply Palette
[[IS-trouble]](https://inkstitch.org/docs/troubleshoot/), [[IS-visualize]](https://inkstitch.org/docs/visualize/),
[[IS-thread]](https://inkstitch.org/docs/thread-color/); Embird Density Map and X-ray views,
thread catalogue sorted by similarity with owned threads
[[Embird-tools]](https://www.embird.net/digitizing-tools/); Embrilliance WireLine map,
one-click brand conversion, owned threads [[EM-DRK]](https://www.embrilliance.com/density-repair-kit),
[[EM-ess]](https://www.embrilliance.com/essentials); mySewnet My Threads; Janome Colour Manager
[[AD]](https://janome.com.au/product/janome-artistic-digitizer); SewWhat-Pro's thread library
[[SWP]](https://sandscomputing.com/products-shop/sewwhatpro-embroidery-editor/); Wilcom Auto
Hoop (the smallest hoop that fits) [[ES-hoops]](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Production/hoops/hoops-3.htm).

PE-DESIGN 11: user thread charts and colour-chart import, the Design Page hoop list, the
Stitch Simulator and Realistic View; no density view, no conversion command, no automatic
hoop **(inferred from the guide)**. OpenStitch today: the compiled `StitchPattern`
(`compile.rs`), the PEC, JEF, SEW, HUS and SHV charts as colour facts in
`openstitch-core::charts`, `nearest_index` (`core/lib.rs:141`, RGB nearest colour), user
thread charts (`user.rs`), `BROTHER_HOOPS` and user hoops (`machine.rs`), `design.info`
with counts and sizes, and `object.setRegion`'s validation errors.

## 2. Density map

### 2.1 Measure

On the compiled stitches of the design (or of the selection), count needle points per cell
of a grid of `cell` mm (default 1.0) and then blur with a 3 × 3 kernel so a single row of a
dense satin does not light up alone. The value is needle points per mm². Thresholds, from
what sews cleanly in practice (a flat fill at 4.5 lines/mm with 4 mm stitches is about
1.1 points/mm²; satin at 4.5 lines/mm on a 6 mm column about 1.5; two layers roughly double):

| Colour | points/mm² | Meaning |
|---|---|---|
| none | < 1.5 | normal single layer |
| yellow | 1.5–3.0 | two layers or dense satin: check underlay and fabric |
| red | > 3.0 | three layers or bunching: needle breaks, puckering, holes |

The thresholds and cell size are preferences; the fabric table (B5) can lower them for thin
fabrics. The map is an overlay drawn as coloured squares with alpha over the canvas (the
renderer already draws the Design Page grid and stitches; the overlay is one more layer) and
is recomputed on every compile the editor does anyway, so it is live while editing.

### 2.2 Engine

`design.densityMap` returns the grid (`cell`, origin, width, height, values as a flat array)
so scripts and the MCP server can use it; the overlay is the app's rendering of the same
result. A per-object variant (`object.densityMap`) counts only one object's stitches, which
shows a fill's own density apart from what lies under it.

## 3. Troubleshoot

### 3.1 Rules

Each rule yields findings `{ severity, object id, position, message, fix }` where `fix` is
an optional command the user can run from the list:

| Rule | Source | Severity | Fix offered |
|---|---|---|---|
| stitch shorter than `minStitch` (default 0.3 mm, excluding tie-in and lock) | compiled stitches | warning | — (shows the object; B3's short stitches or a density change) |
| stitch longer than `maxStitch` (default 12.1 mm, PE-DESIGN's satin limit) | compiled | error | split length attribute |
| satin width over `maxSatinWidth` (default 12 mm) | region geometry (rung length) | warning | convert to fill |
| satin rails cross (a rung of negative length or a skeleton that folds) | geometry | error | reverse direction line |
| region generated no stitches (too small, too thin, offset swallowed) | layers empty | warning | — |
| object outside the Design Page or hoop | bounds | error | centre / auto hoop |
| consecutive colour blocks with the same thread | compiled | info | merge colours (`order.byColor`) |
| jump longer than the trim length without a trim (single-needle: colour change) | compiled | info | enable jump trimming |
| text with glyphs the font lacks | layout | error | choose font |
| hole sewing pair where the inner shape is not inside the outer | model | error | — |
| stamps or carvings that touch no row | geometry | info | — |
| density over the red threshold anywhere | density map | warning | — |

The rules run over the design on demand (`design.troubleshoot`) and, in the editor, after
every compile for the cheap ones (bounds, colours, jumps) with the expensive ones (density,
geometry checks) on the Troubleshoot pane's refresh. Findings are markers on the canvas
(a small icon at `position`) and rows in a pane; clicking a row selects the object.

### 3.2 Engine

`design.troubleshoot` returns the findings as JSON; `object.setRegion` and friends already
return validation errors for ranges, and those stay synchronous. The MCP server exposes
`troubleshoot` so an agent driving OpenStitch can check its own output.

## 4. Thread-brand conversion

### 4.1 Nearest colour

`nearest_index` compares in RGB, which is fine for mapping into the PEC 64-colour chart
(coarse) but wrong for a 400-colour brand chart where perceptual distance matters. Add a
CIE Lab conversion and ΔE2000 in `openstitch-core` (about 60 lines, no dependency) and
use it for charts above 64 colours; keep RGB for the PEC mapping so the machine-format
tests do not move.

### 4.2 Command

`design.convertThreads` with `chart` (a built-in chart id, or a user chart name) and
`ownedOnly: bool` (restrict to threads marked owned in the user chart): for every distinct
thread in the design, the nearest chart colour by ΔE2000, applied to every object and
stitch block using it (`stitches.setThread` for blocks, the region/line thread for
objects), with a report `[{ from, to, deltaE }]` so the user sees which matches are poor
(ΔE > 5 is visibly different; the pane highlights those). One undo step. Brand charts
beyond the five bundled ones are user data (CSV import exists for user charts; the list of
brands with public colour tables is long, and colour facts are not copyrightable, as
LEGAL_AND_LICENSING.md records).

"Owned threads": a per-colour flag on user charts (`user.rs`), with a filter in every
thread picker.

## 5. Auto hoop

`design.autoHoop`: among the machine type's hoops (`BROTHER_HOOPS` filtered by the Design
Page's machine type, plus user hoops), the smallest by area whose sewing area contains the
design's bounds with a `margin` (default 0, PE-DESIGN's page is the hoop) in either
orientation (rotating 90° where the hoop allows it, as the Design Page's Rotate 90 does);
sets the Design Page hoop (and rotation) and returns it. Offered after "Open machine file
as a new design" (today the page is sized to fit) and as a button next to the hoop list.
No hoop fits → the multi-position and split wizards are suggested.

## 6. Parameters and defaults

| Setting / command | Type | Default |
|---|---|---|
| `design.densityMap` (`cell`, `blur`, `ids`) | command | cell 1.0 mm, blur on |
| density thresholds (yellow, red) | points/mm² | 1.5, 3.0 |
| `design.troubleshoot` | command | all rules |
| `minStitch`, `maxStitch`, `maxSatinWidth` | mm | 0.3, 12.1, 12 |
| `design.convertThreads` (`chart`, `ownedOnly`) | command | — |
| chart colour `owned` | bool | false |
| `design.autoHoop` (`margin`) | command | 0 mm |

## 7. Model and code changes

- `openstitch-core`: `lab(thread) -> (L, a, b)`, `delta_e_2000`, `nearest_lab`.
- `openstitch-engine`: `quality.rs` (new) with `density_map`, `troubleshoot` rules,
  `convert_threads`, `auto_hoop`; `user.rs`: the owned flag; capabilities read-only for the
  first two, edit for the last two.
- `openstitch-mcp`: the four commands appear automatically (one tool per command).
- App: a density overlay toggle in View, a Troubleshoot pane (dockable like the Sewing
  Order pane), a Convert Threads dialog with the report, an Auto hoop button.

Effort: S each (the heaviest is the Troubleshoot pane).

## 8. Test cases (internal regression tier)

| Spec | Script | Expected |
|---|---|---|
| `density-single-fill` | 40 × 40 mm fill at 4.5 lines/mm, 4 mm stitches | every cell inside the fill between 0.8 and 1.5 points/mm² (none yellow) |
| `density-two-layers` | two identical fills overlapped | cells in the overlap between 1.6 and 3.0 (yellow); a third layer makes them red |
| `density-per-object` | same, `object.densityMap` on one | single-layer values |
| `trouble-long-stitch` | a satin region 15 mm wide | one "stitch longer than 12.1 mm" error per long stitch group, the satin-width warning, and the offered fix converts to fill |
| `trouble-outside-hoop` | an object at (120, 120) on a 100 × 100 page | one error with the object id; `design.autoHoop` fix picks a hoop that contains it |
| `trouble-same-colour-blocks` | two objects of one colour separated by another colour | one info finding; running the fix groups them |
| `trouble-missing-glyph` | text with a character the bundled font lacks | one error naming the character |
| `trouble-clean` | the `badge.json` example | no errors (warnings allowed, listed) |
| `threads-convert-nearest` | three threads, convert to the JEF chart | each mapped to the ΔE2000-nearest JEF colour; report lists ΔE per thread; one undo restores |
| `threads-owned-only` | mark two JEF colours owned, `ownedOnly` | every thread maps to one of the two |
| `threads-pec-unchanged` | PEC export of a design | identical to today (RGB nearest kept for the 64-colour chart) |
| `autohoop-smallest` | a 90 × 50 mm design on a multi-needle page | 100 × 100 chosen over 130 × 180; a 120 × 50 design picks 130 × 180 rotated when allowed |
| `autohoop-none` | a 400 × 400 mm design | no hoop; the response names the split wizard |

Measurement helpers needed: "density grid" (the command's own output), "findings list"
(JSON), "thread table of the compiled pattern" (exists).

## 9. Compatibility and licensing

Read-only tools and two explicit commands; nothing changes unless run. ΔE2000 is a
published formula (CIE 2000). Thread charts remain colour facts; brand names identify the
charts. "WireLine", "Density Repair Kit", "Colour Manager" and "Auto Hoop" are product
names; OpenStitch uses "Density map", "Troubleshoot", "Convert threads" and "Fit hoop".

## 10. Open questions

- Whether the density thresholds should be in points/mm² (what the map measures) or in
  "layers" (what users understand); proposal: show "1 layer / 2 layers / 3+" in the legend,
  computed from the thresholds.
- Whether troubleshoot should block export on errors (a dialog "export anyway?"); proposal:
  a non-blocking notice in the export dialog listing the error count.
