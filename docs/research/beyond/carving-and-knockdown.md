# B12. Carving lines on fills and knockdown (laydown) fill: algorithm sketch

Backlog item B12 of [competitive-features.md](../competitive-features.md) (features F11,
F12). Research date: 2026-10-28. A design note for `openstitch-design` (`stamp.rs`,
`region.rs`) and `openstitch-engine` (`wizards/background.rs`). No other product's code was
read.

## 1. What the feature is

- **Carving lines.** The user draws open or closed lines over a fill (or a satin) and the
  needle penetrations are forced onto those lines, so the thread shows a crease along them:
  veins on a leaf, folds in cloth, lettering embossed into a fill. A **region carve** gives
  a drawn area inside the fill a different pattern or density without splitting the object.
- **Knockdown fill.** A light, open fill sewn **first**, a few millimetres larger than the
  design, in the fabric's colour, to flatten the pile of towelling, fleece or fur so the
  design on top is not swallowed; it replaces water-soluble topping.

Who has it: Pulse Line Carving and Region Carving [[DG15]](https://embroidery.pulsemicro.com/dg15-levels.html);
Embird Carving object (follows a fill or column; width in mm, above 0.2 mm short stitch
lines along the fill angle) [[Embird-objects]](https://www.embird.net/studio/manual/0075objects.htm);
Wilcom Carving Stamp and Flexi Splits, Hatch Carving Stamps
[[ES-designing]](https://wilcom.com/embroiderystudio/designing),
[[Hatch-dig]](https://hatchembroidery.com/features/digitizing); Embrilliance Fill Embossing
(50+ patterns) and Carving Lines (L3) [[EM-SA]](https://www.embrilliance.com/stitchartist);
Ink/Stitch patterns on fills (strokes add penetrations, filled shapes remove them)
[[IS-patterns]](https://inkstitch.org/docs/stitches/patterns/). Knockdown: Wilcom Laydown
Stitch (ES 2025, Hatch 3: a light open grid, sized to or larger than the design, sewn first,
no underlay) [[ES-2025]](https://wilcom.com/resources/blog/top-10-new-features-of-wilcom-embroiderystudio-2025),
[[Hatch-3]](https://hatchembroidery.com/resources/blog/whats-new-in-hatch-embroidery-3);
Ink/Stitch Selection to Knockdown Fill (keep holes, offset, join method, emboss shape)
[[IS-filltools]](https://inkstitch.org/docs/fill-tools/); Embrilliance Knockdown Stitches
(bi-directional, about 4 mm stitches, inflation) [[EM-ent]](https://www.embrilliance.com/enthusiast);
SewWhat-Pro nap tack [[SWP]](https://sandscomputing.com/products-shop/sewwhatpro-embroidery-editor/).

PE-DESIGN 11: Stamps (emboss and engrave areas from a pattern library, or from a shape)
on fill regions; OpenStitch has them (`stamp::Stamp { effect, contours }`, `stamp::apply`
masks needle points of engraved areas with short stitches and drops them inside embossed
ones, `shape.emboss` converts a shape into a stamp). No free-drawn carving **line**
(stamps are areas) and no knockdown.

## 2. Carving lines

### 2.1 Geometry

A carving is an open or closed polyline (`Contour`) stored on the region (`carvings:
Vec<Carving>`, each `{ contour, width_mm }`), in model coordinates, transformed with the
object as stamps are. It applies to the region's **top** layer only (not under sewing).

For each fill row (after the row generator, before tie/lock, where `stamp::apply` runs
today): intersect the row's segments with every carving contour. At each intersection
point `P`:

- insert `P` as a needle point in the row;
- for `width_mm > 0`, also insert needle points at `P ± û·width/2` along the row (so a wide
  carving reads as a groove, Embird's "above 0.2 mm" rule), and remove the row's own needle
  points that fall between them;
- then, as `stamp::apply` does, enforce the minimum stitch length by dropping neighbouring
  points closer than `min_stitch` to the inserted ones (never dropping an inserted point).

The row's stitch grid (Frequency) is otherwise unchanged, so the carving shows as a line of
aligned penetrations across rows whose other penetrations are staggered, which is what
makes it visible. For satin regions and columns, the same intersection is applied to each
rung: the rung is split at `P` (the stitch becomes two), which is a "split stitch" carving.

Carvings do not cross holes (a carving segment inside a hole has no rows to intersect) and
are clipped to the region by construction.

### 2.2 Region carve

`Carving { contour (closed), region: Some(RegionOverride) }` where the override is a subset
of region attributes (pattern, density, angle, type among the row-based ones). Generation:
subtract the closed carving from the region's outline for the main fill
(`boolean::combine` Difference), generate the carved area as its own region with the
override attributes, and sew it immediately after the main fill of the same object (same
colour, travel through the object). It is the "Complex Fill Modifier" idea in Pulse's terms
and a convenience over drawing a second object: the two stay one object with one outline,
one sewing-order entry, one set of entry and exit points.

### 2.3 Editing

The Pen and Pencil tools get a "Carve" mode when a fill is selected: the drawn path becomes
a carving on that region (one undo step). Carvings are drawn on the canvas as thin lines over
the region and are selectable for point editing with the existing Point tool (the region's
`stamps` already have this treatment for their outlines). `shape.carve` takes a line object
and makes it a carving of the region under it (consuming the line), the mirror of
`shape.emboss`.

## 3. Knockdown fill

### 3.1 Construction

`wizard.knockdown` on a selection (or the whole design):

1. Union the selected objects' outlines (`boolean::union_all`), including text (its glyph
   outlines) and stitch blocks (their convex hull or a 1 mm offset of their outline trace
   from `digitize::coverage::trace`).
2. Offset outward by `inflate` (default 2 mm; the sources say "a few mm"), rounded joins;
   keep holes larger than `keepHoles` (default 10 mm across) so a ring design does not get
   a filled centre.
3. Fill the result with a `Fill` region: density `knockdownDensity` (default 1.0 line/mm, that
   is rows 1 mm apart, versus 4.5 for a normal fill), stitch length 4 mm, two passes at 0°
   and 90° (`UnderlayStyle::Double`-style, the "bi-directional" grid the products describe),
   no under sewing, thread = the Design Page's fabric colour (a new Design Page field,
   default white) so the grid disappears into the fabric.
4. Insert the region **first** in the sewing order, named "Knockdown", and mark it
   `knockdown: true` so `order.optimize` keeps it first (as B7 keeps puff objects last).

Emboss variant (Ink/Stitch's "emboss border"): `shape: rectangle | circle | none` around
the union with `shapeOffset` so the knockdown region is a plain shape rather than the
design's silhouette, which some users prefer on towels (the design appears "pressed" into
the pile inside a frame).

### 3.2 Updating

The knockdown region is a normal object afterwards (editable, deletable). "Update knockdown"
re-runs the construction from the current design, replacing the previous region. Objects
added later are covered on the next update; the inspector shows a notice when the design's
bounds exceed the knockdown's.

## 4. Parameters and defaults

| Attribute / command | Type | Default | Range |
|---|---|---|---|
| region `carvings[].width` | mm | 0 | 0–2 |
| region `carvings[].region` | override | none | pattern, density, angle, type |
| `shape.carve` | command | — | line id, region id |
| `wizard.knockdown` | command | — | |
| `inflate` | mm | 2.0 | 0–10 |
| `keepHoles` | mm | 10 | 0–50 (0 = keep every hole) |
| `knockdownDensity` | lines/mm | 1.0 | 0.5–2 |
| `stitchLength` | mm | 4.0 | 2–6 |
| `shape` | `silhouette` \| `rectangle` \| `circle` | `silhouette` | |
| `shapeOffset` | mm | 5 | 0–20 (rectangle, circle) |
| design `fabricColour` | thread | white | |

## 5. Model and code changes

- `model.rs`: `RegionSewing::carvings: Vec<Carving>` (serde default empty), `DesignObject::
  knockdown: bool`, `DesignPage::fabric_colour`.
- `stamp.rs`: `carve(runs, carvings, min_stitch)` next to `apply`; the rung split for satin
  in `column.rs` / `skeleton.rs`.
- `region.rs::layers_inner`: the region-carve subtraction and the carved sub-region.
- `wizards/background.rs` (or a new `wizards/knockdown.rs`): the union, inflation and
  double-pass fill; `order.rs::optimized`: knockdown first.
- Engine: `shape.carve`, `object.setCarving` / `removeCarving`, `wizard.knockdown`,
  `design.setPage` field.
- App: Carve mode on the Pen and Pencil tools, carving outlines on the canvas, a Knockdown
  entry in the wizards menu with its options.

Effort: S for carving lines (the stamp mask code is the model), S for region carve, S for
knockdown.

## 6. Test cases (internal regression tier, plus a reference guard)

| Spec | Script | Expected |
|---|---|---|
| `carve-stamps-unchanged` | every stamp / emboss spec and script | identical output (guard) |
| `carve-line-on-fill` | 40 × 40 mm fill, a diagonal carving, width 0 | every row has a needle point within 0.05 mm of the carving line; no row has two needle points closer than the minimum stitch; needle points elsewhere unchanged versus the uncarved run except where dropped next to the line |
| `carve-width` | same, width 1.0 | each row has points at `P ± 0.5 mm` along the row and none between them |
| `carve-closed-shape` | a circle carving | penetrations on the circle in every row it crosses; rows inside and outside otherwise unchanged |
| `carve-satin-split` | 8 mm Manual Punch column with a carving across it | every rung crossing the line is split there (two stitches); the rails unchanged |
| `carve-region-override` | a square carving with density 2.0 and angle 90° | inside the carving: rows at 90° and 0.5 mm apart; outside: the region's rows; one object, one colour, no jump |
| `carve-hole` | carving crossing a hole | no needle point inside the hole |
| `knockdown-silhouette` | a star and text, `wizard.knockdown` | a new first object named Knockdown; its outline = union of the two, offset 2 mm (area within 3 % of the expected); rows 1 mm apart in two passes at 0° and 90°; stitch length ≤ 4 mm; no under sewing; thread = the fabric colour |
| `knockdown-keep-hole` | a ring (outer 40, inner 20 mm) | the hole is kept (no needle point inside the 16 mm circle) |
| `knockdown-rectangle` | `shape: rectangle`, offset 5 | a rectangle 5 mm outside the design bounds |
| `knockdown-order` | `order.optimize` after adding objects | Knockdown stays first |
| `knockdown-update` | add an object outside the bounds, "update" | the region is rebuilt to cover it; the previous one removed |

Measurement helpers needed: "needle points near a polyline" (point-to-polyline distance,
exists), "area of a run's outline" (coverage trace, exists), "layer identification" (shared).

## 7. Compatibility and licensing

Stamps and the Background Fill wizard are unchanged (guard). Forcing penetrations onto a
drawn line and a light pre-fill are published practice; "Carving Stamp", "Flexi Splits",
"Laydown Stitch" and "Knockdown Stitches" are product names; OpenStitch uses "Carving" and
"Knockdown". Ink/Stitch's documentation gave the knockdown option names; its code was not
read.

## 8. Open questions

- Whether the knockdown's thread should be a special "fabric" thread that machine formats
  write as the first colour (so the operator loads the matching thread) or a real chart
  colour chosen by the user; proposal: a real chart colour, picked from the fabric colour
  field, because machines need a real thread anyway.
- Carvings on text: text compiles glyph by glyph through the region code, so carvings on a
  text object apply per glyph; defer the editor affordance.
