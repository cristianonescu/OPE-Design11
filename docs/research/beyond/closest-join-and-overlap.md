# B11. Closest join, auto start/end, continue digitizing, overlap removal: algorithm sketch

Backlog item B11 of [competitive-features.md](../competitive-features.md) (features D3, D7).
Research date: 2026-10-28. A design note for `openstitch-engine` (`order.rs`, `drawing.rs`,
`shape_ops.rs`) and `openstitch-design` (`route.rs`, `boolean.rs`). No other product's code
was read.

## 1. What the feature is

Four small conveniences that together remove most manual sequencing work:

- **Closest join.** Each object's exit and the next object's entry are placed at the closest
  pair of points on their outlines, re-applied on demand after moves, so connectors are the
  shortest possible.
- **Auto start and end.** The design's first and last needle positions are set automatically
  (the design centre, a hoop position, or the first object's entry), so the machine starts
  and finishes where the operator expects.
- **Continue digitizing.** A closed or open object is extended from its start or end point
  with the Pen tool without creating a new object or resequencing.
- **Overlap removal (auto clip).** Stitches of a lower object hidden under an upper one are
  removed before generation, keeping an overlap band so no fabric shows.

Who has it: Wilcom Closest Join (global, "Apply Closest Join" shortcut J on two or more
selected objects), Auto Start & End, Continue Digitizing (ES 2026, Hatch 4), Remove Overlaps
[[ES-closest]](https://docs.wilcom.com/embroiderystudio/e4/en/MainHelp/Quality/connectors/Apply_closest_join.htm),
[[ES-closest-blog]](https://wilcom.com/resources/blog/closest-join),
[[ES-designing]](https://wilcom.com/embroiderystudio/designing),
[[Hatch-dig]](https://hatchembroidery.com/features/digitizing); Pulse Auto Start/Stop, Design
Connections, Remove Overlap Stitches and Auto Clip with an overlap distance
[[DG15]](https://embroidery.pulsemicro.com/dg15-levels.html),
[[DG16-IE]](https://tajimaeurope.com/en/pulse-software-solutions/embroidery-software-tajima-software-dg16-pulse/illustrator-extreme/);
FTCU Optimize Entry/Exit Points [[FTCU]](https://www.mulqueen.com/shop/c/p/Floriani-Total-Control-Professional-Embroidery-Software---FTCU-x44586193.htm);
Janome remove overlaps [[AD]](https://janome.com.au/product/janome-artistic-digitizer).

PE-DESIGN 11: Optimize Entry/Exit Points (11.01) moves entry and exit points of shapes to
where they come closest to the same-colour shapes before and after (OpenStitch
`order.linkEntryExit`, reference tier, `optimize-entry-exit`); Remove Overlap works on
shapes (OpenStitch `shape.merge` / `shape.combine`). No per-object re-apply, no automatic
start and end position (the sewing area setting decides where the needle starts), no
continue digitizing, no stitch-level clipping of imported data.

## 2. Closest join per object

`order.linkEntryExit` already does the geometry for a range of consecutive objects: for
objects `k` and `k+1` of the same colour it sets `exit(k)` and `entry(k+1)` to the closest
pair of outline points (`route::nearest` on each outline, iterated once each way). B11 adds:

- **Scope.** `order.closestJoin` with `ids` (two or more selected objects, in their sewing
  order) or `all`; it calls the same routine over the selection, including across colour
  changes when `acrossColours: true` (PE-DESIGN leaves both points alone at a colour change,
  which stays the default).
- **Fixed points.** An entry or exit the user placed by hand (`object.setEntryExit`) is
  marked `fixed` and not moved; the routine then only moves the free end of the pair.
- **Regions whose start is constrained.** Fills start on an outline point (travel from the
  entry to the first row is inside the region); satin regions start at the entry rung;
  motif and decorative fills start at the pattern reference. The closest pair is found on
  the outline for all, and the generator's own travel does the rest, as today.
- **Automatic mode.** A design setting `closestJoin: auto` re-applies the routine to the
  affected pair whenever an object moves, scales or is resequenced (one call in the
  engine's post-edit hook, cheap because it touches two objects). Off by default to keep
  PE-DESIGN behaviour; the Sewing Order pane shows a toggle.

## 3. Auto start and end

A Design Page setting `autoStartEnd: none | centre | hoopPosition | firstObject`:

- `centre` (Wilcom's default): the compiler emits a leading jump from the Design Page
  centre to the first stitch and a trailing jump from the last stitch back to the centre,
  so the machine's needle begins and ends at the hoop centre. PE-DESIGN's "sewing area"
  setting already decides whether the origin is the page centre or the design centre
  (`SewingArea`); `centre` is that origin.
- `hoopPosition`: a point the user drags on the canvas (stored in the Design Page).
- `firstObject`: no leading jump (today's behaviour), the design ends at the last object's
  exit.

The leading and trailing jumps are trimmed as any jump above the trim length. Formats that
encode the start position themselves (DST's header extents, PES's design centre) are
unaffected: the jump is stitch data.

## 4. Continue digitizing

The Pen tool gets a "continue" mode: with a line or region object selected and the pointer
near its start or end vertex (within the handle radius), clicking appends anchors to that
end of the object's path (`object.setPoint` / a new `object.appendPoint` command that pushes
a vertex and its handles at the start or end), closing the path if the first anchor is
clicked. The object keeps its id, attributes, sewing order and entry/exit; a closed region
being extended becomes open until closed again (the inspector warns). One undo step per
anchor as for the Pen tool today.

## 5. Overlap removal (auto clip)

### 5.1 Shape level (automatic)

A design setting `removeOverlaps: off | auto` and a per-object `clipUnder: bool`. With
`auto`, before generation of a region `L` the compiler subtracts from its outline every
later region `U` that overlaps it and has `clipUnder` (default true for fills and satin,
false for open fills, lines and text), each `U` shrunk by `overlap` (default 0.5 mm, or half
the cover width of a satin, Pulse's "overlap distance" and Wilcom's "cutting overlap = half
the cover width"): `boolean::combine(L, offset(U, −overlap), Difference)`. The user's
outline is untouched; only the generated stitches change, and `shape.merge` remains the
destructive command for those who want the outline cut. Holes created by the subtraction
are sewn as hole sewing, which the region code handles.

### 5.2 Stitch level (imported data)

`stitches.clip` on a stitch block under a region or another block: needle points of the
lower block inside the shrunken upper outline are removed, the run is re-linked with travel
along the clip boundary (`route::travel` on the boundary polygon), and lock stitches are
inserted at the cut ends (`stitch_edit` has insert and delete; the travel and locks are
new). This is what lets a purchased design sit under a new satin border without a doubled
layer.

## 6. Parameters and defaults

| Setting / command | Type | Default |
|---|---|---|
| `order.closestJoin` | command (`ids` / `all`, `acrossColours`) | — |
| design `closestJoin` | `off` \| `auto` | `off` |
| entry/exit `fixed` | bool | false |
| design `autoStartEnd` | `none` \| `centre` \| `hoopPosition` \| `firstObject` | `none` (today) |
| design `removeOverlaps` | `off` \| `auto` | `off` |
| object `clipUnder` | bool | true for fills and satin |
| `overlap` | mm | 0.5, or half the cover width for satin |
| `stitches.clip` | command | — |
| `object.appendPoint` | command | — |

## 7. Model and code changes

- `model.rs`: `DesignPage::auto_start_end`, `closest_join`, `remove_overlaps`;
  `RegionSewing` / `LineSewing`: `entry_fixed`, `exit_fixed`, `clip_under`.
- `order.rs`: `closest_join(d, ids, across)` factored out of `link_entry_exit` with the
  fixed-point rule; the post-edit hook for `auto`.
- `compile.rs`: the leading and trailing jumps; the per-region subtraction before
  `region::layers_with` (using `boolean::combine` and `offset`).
- `drawing.rs` / `commands.rs`: `object.appendPoint`; the Pen tool's continue mode in the
  app's pen state machine.
- `stitch_edit.rs`: `clip(block, polygon, overlap)` with re-linking and locks.
- App: Sewing Order pane toggles, Design Page settings rows, the pen mode.

Effort: S for closest join and auto start/end, S for continue digitizing, S for shape-level
overlap removal, S–M for stitch-level clipping.

## 8. Test cases (internal regression tier, plus a reference guard)

| Spec | Script | Expected |
|---|---|---|
| `join-linkentryexit-is-today` | `optimize-entry-exit` re-run | identical output (guard) |
| `join-three-shapes` | three same-colour squares in a row, `order.closestJoin all` | each connector is the minimum distance between consecutive outlines (± 0.2 mm) |
| `join-fixed-entry` | same with the second square's entry fixed on its far side | that entry unchanged; the first square's exit moves to the point nearest it |
| `join-across-colours` | two colours, `acrossColours` false / true | points untouched at the colour change / moved |
| `join-auto-on-move` | `closestJoin: auto`, move the middle square | the two connectors are recomputed; the others unchanged |
| `start-end-centre` | two objects, `autoStartEnd: centre` | the first stitch record is a jump from the page centre; the last is a jump back to it; both trimmed |
| `continue-open-line` | a 3-point line, `object.appendPoint` at the end twice | same object id, 5 points, attributes and order unchanged; one undo step each |
| `continue-close` | append the first anchor's position | the path closes; the object becomes a region if it has region sewing |
| `clip-fill-under-satin` | a fill with a 4 mm satin border on top, `removeOverlaps: auto` | no fill needle point inside the border's outline shrunk by 2 mm; fill stitches still present inside the band between the shrunk and the full border outline; the fill's own outline object unchanged |
| `clip-open-fill-kept` | an open contour fill under a satin | not clipped (`clipUnder` false by default for open fills) |
| `clip-stitch-block` | an imported stitch block with a satin region on top, `stitches.clip` | block points inside the shrunk outline removed; the block's runs re-linked along the boundary; lock stitches at each cut end |
| `clip-off-is-today` | every reference spec with `removeOverlaps: off` explicit | identical output (guard) |

Measurement helpers needed: "needle points inside a polygon" (shared), "connector length
between objects" (exists for the entry/exit spec), "command sequence" (exists).

## 9. Compatibility and licensing

All four are off by default; the reference entry/exit spec is a guard. Nearest points,
boolean subtraction and path appending are elementary. "Closest Join", "Auto Start & End",
"Continue Digitizing" and "Auto Clip" are vendor names; OpenStitch uses "Closest join",
"Start and end position", "Continue" and "Remove overlaps (automatic)".

## 10. Open questions

- Whether `closestJoin: auto` should be the default for new designs once it has been used
  for a while; PE-DESIGN users expect manual points, so no for now.
- The overlap default for satin (half the cover width) versus a fixed 0.5 mm: the vendors
  differ; make it a Fabric Selector field.
