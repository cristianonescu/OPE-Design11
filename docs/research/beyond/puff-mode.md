# B7. 3D puff (foam) satin mode: algorithm sketch

Backlog item B7 of [competitive-features.md](../competitive-features.md) (feature F10).
Research date: 2026-10-28. A design note for `openstitch-design` (`model.rs`, `column.rs`,
`skeleton.rs`, `lines.rs`, `compile.rs`) and `openstitch-engine` (`order.rs`). No other
product's code was read; the rules come from published digitizing guides.

## 1. What the feature is

3D puff embroidery lays a sheet of foam (3 mm standard, 4 mm maximum, never doubled) on the
fabric and sews satin over it; the needle penetrations along the column edges perforate the
foam so the excess tears away, and the satin stands up. Digitizing for it differs from flat
satin in a fixed set of ways, which the professional products bundle as a per-object mode:

- satin only, width about 3–12 mm (narrower over-compresses, wider sags; one source says
  7 mm to avoid splitting);
- about twice the density (0.16–0.18 mm spacing, versus 0.3–0.4 mm flat) so the edge
  perforation is continuous;
- no pull compensation (foam pushes the thread outward) and little or no underlay (opt-in
  at stress points);
- **capped ends**: a column's open ends must be closed with a short satin "cap" sewn across
  the end before the top satin, so the foam is cut there too and does not show;
- no stitch shortening on inner corners (gaps in the perforation);
- a machine **stop** before the first puff element so the operator lays the foam, and all
  puff elements sewn last, after the flat embroidery;
- where a satin border surrounds a flat fill, the border alone cuts the foam.

Sources: Wilcom's 3D puff webinar Q&A [[Wilcom-puff]](https://wilcom.com/resources/blog/qa-from-3d-puff-embroidery-webinar);
Ignition Drawing's two-part guide [[Ignition-1]](https://blog.ignitiondrawing.com/working-with-3d-puff-embroidery-part-1/print/),
[[Ignition-2]](https://blog.ignitiondrawing.com/working-with-3d-puff-embroidery-part-2/print/);
two digitizing-service guides [[EmbPunch]](https://www.embpunch.com/blog/step-by-step-guide-to-3D-puff-digitizing),
[[Digitizing.us]](https://digitizing.us/embroidery-digitizing-and-artwork-blog/3d-puff-complete-guide/).
The sources disagree on underlay (one adds it at stress points, the webinar says none), so
underlay is opt-in. Products: Hatch Raised Embroidery and ES Raised Satin
[[Hatch-dig]](https://hatchembroidery.com/features/digitizing); Embrilliance "Satin 3D Foam
Auto-Underlay" that "adds capping and joins" [[EM-L2]](https://embrilliance.com/product/stitchartist-level-2-upgrade);
Pulse DG17 "Automatic Processing for 3D Embroidery" generating foam-cutting corners
[[DG17]](https://www.tajima.com/product/dg/) (thin detail); Ink/Stitch documents only a
Stop command for foam [[IS-commands]](https://inkstitch.org/docs/commands/).

PE-DESIGN 11 has no foam attribute; satin density presets are Dense/Medium/Light
**(inferred from the attribute pages)**. OpenStitch today: satin density is a number
(lines/mm, PE-DESIGN default 4.5 for regions, 5.0 for text), `Command::Stop` exists in
`openstitch-core`, cutwork already inserts stops between needle stages, and `order.rs`
sequences objects.

## 2. The mode

`puff: bool` on `RegionSewing` (satin regions, skeleton satin, Manual Punch columns) and on
`LineSewing` (zigzag lines). When set, the object compiles with these overrides, each of
which is also an ordinary attribute the user can still edit (the mode sets defaults and
shows them in a "Puff" pane; it does not hide them):

| Setting | Puff default | Flat default | Rule |
|---|---|---|---|
| density | 6.0 lines/mm (0.167 mm) | 4.5 | twice the flat spacing, from the guides' 0.16–0.18 mm |
| pull compensation | 0 | per fabric | foam pushes outward |
| push compensation (B5) | +0.4 mm outward at the ends | 0 | the guides give 0.3–0.4 mm; sign is outward, so this is a negative push in B5's terms |
| underlay | none | per fabric | opt-in: `puffUnderlay: true` gives a centre run only |
| short stitches (B3) | off | per fabric | no thinning on inner curves |
| corner style (B3) | `turn` | `auto` | mitres and caps make short stitches at the corner; a turn keeps full-length stitches over the foam |
| caps | on | — | section 3 |
| width warning | outside 3–12 mm | — | inspector warning, not a clamp |

Puff text (satin text) takes the same mode from the text object's sewing attributes.

## 3. Caps

A cap closes an open end of a column so the foam is perforated across it.

1. At each open end of the column (the first and last rung of a Manual Punch column, each
   end of a skeleton branch, each end of an open zigzag line; a closed ring has none), take
   the end rung `[L, R]` with width `w` and the column's end direction `û` (pointing out of
   the column).
2. The cap region is the rectangle from the end rung outward by `capDepth` (default
   `max(1.5 mm, w/3)`), that is the four points `L, R, R + û·capDepth, L + û·capDepth`,
   extended `capOverlap` (default 0.5 mm, from the guides' "0.5 mm past the end") beyond the
   drawn end so the cut lies outside the column. The guides' "push the ends out 0.3–0.4 mm"
   is this overlap plus B5's push.
3. The cap is sewn as a satin whose stitches run **along** the column direction `û` (that is,
   perpendicular to the column's own stitches), at the puff density, across the width `w`:
   rungs from `L + û·k·d` to `R + û·k·d`... turned 90°: each cap stitch goes from the end
   rung line to the cap's far edge, stepping across the width by `d`. So the cap is a small
   satin column of length `capDepth` and width `w`, rotated 90° to the main column.
4. Order: caps first (entry-end cap, then exit-end cap reached by a travel run along the
   column's centreline), then the column's own satin from the entry end, so the column covers
   the caps' inner edges. For a skeleton satin with several branches, every branch end gets a
   cap, sewn in the branch walk order of B4 before the branch's satin.
5. Caps are not added where a column end meets another puff object's side within `w/2`
   (the junction is covered) or where the end lies on a closed puff border (the border cuts
   the foam).

## 4. Sequencing and the foam stop

- `order.optimize` and the Sewing Order pane treat `puff` objects as a group that sews after
  every non-puff object (a new rule in `optimized`: puff objects are never "free" while a
  flat object remains), keeping their colours grouped among themselves.
- The compiler inserts a `Command::Stop` before the first stitch of the first puff object,
  on a thread named "Lay foam" in the puff object's colour (the same device the cutwork and
  appliqué wizards use to make the machine stop between stages); on single-needle machines
  this is a colour change the machine pauses on, on multi-needle machines a stop command.
- No second stop is needed for a second foam colour unless the thread changes, in which
  case the colour change stops the machine anyway.
- A flat fill inside a puff border is sewn before the border (it is a flat object), which
  matches the guides.

## 5. Parameters and defaults

| Attribute | Type | Default | Range |
|---|---|---|---|
| `puff` | bool | false | — |
| `puffDensity` | lines/mm | 6.0 | 4.5–8 |
| `puffUnderlay` | bool | false | — |
| `capDepth` | mm | max(1.5, w/3) | 1–5 |
| `capOverlap` | mm | 0.5 | 0–1 |
| `foamThickness` | mm | 3 | 2–4 (informational: shown in the print sheet and the design properties) |

## 6. Model and code changes

- `model.rs`: `puff`, `puff_density`, `puff_underlay`, `cap_depth`, `cap_overlap` on
  `RegionSewing` and `LineSewing` (serde defaults); `DesignProperties::foam_thickness`.
- `column.rs`: `caps(rungs, depth, overlap, spacing) -> (Run, Run)` and the ordering in
  `satin`; `skeleton.rs`: caps per branch end; `lines.rs::zigzag_rails`: caps at open ends.
- `region.rs::layers_with`: apply the overrides when `puff` (density, no pull, underlay
  choice, corner style, short stitches), add the cap layers before the top layer.
- `compile.rs`: the "Lay foam" stop before the first puff object; `order.rs::optimized`:
  puff after flat.
- Engine: attributes in `object.setRegion` / `setLine`; `design.info` reports foam
  thickness and the puff stitch count; the print sheet lists "foam 3 mm" with the colours.
- App: a "3D puff" toggle in the satin attributes with the pane of overrides; the canvas
  draws puff objects with a raised shading in Realistic View (a thicker thread width, 0.6 mm,
  and a stronger highlight; the renderer already takes a thread width).

Effort: S–M. The cap generator and the ordering rule are the work; the rest is attribute
plumbing.

## 7. Test cases (internal regression tier)

| Spec | Script | Expected |
|---|---|---|
| `puff-density-and-comp` | 8 mm Manual Punch column, `puff` on | stitch spacing along the column `1/6 mm ± 0.01`; rung ends on the rails (no pull); no under-sewing layer |
| `puff-caps` | same | two cap groups: stitches parallel to the column direction within ±3°, spanning the width `w ± 0.2 mm`, depth `max(1.5, w/3) + 0.5 mm` outward from each end; caps sewn before the column's satin; the travel between the two caps lies inside the column outline |
| `puff-closed-ring` | puff satin border around a 30 mm circle | no caps; one run |
| `puff-skeleton-branches` | a Y-shaped skeleton satin, `puff` | three caps (one per branch end), none at the junction |
| `puff-junction-no-cap` | two puff columns, one ending on the other's side | the ending column has one cap (its free end only) |
| `puff-stop-and-order` | a flat fill, a flat satin, two puff columns in the drawing order fill, puff, satin, puff | compiled order: fill, satin, puff, puff; exactly one `Stop` (or colour change to "Lay foam") before the first puff stitch |
| `puff-inside-border` | flat fill inside a puff border | fill first, then the border; the border has no caps |
| `puff-width-warning` | 2 mm and 14 mm puff columns | `object.setRegion` returns the width warning for both and still applies |
| `puff-off-is-today` | every satin reference spec with `puff: false` explicit | identical output (reference guard) |
| `puff-text` | "PUFF" in Lato Bold at 25 mm, satin text, `puff` | each stroke end capped; density 6.0; sewn after any flat object in the design |

Measurement helpers needed: "stitch spacing along a column" (exists for the satin specs),
"stitch direction groups" (shared with B3's cap test), "command sequence" (stops and colour
changes; exists).

## 8. Compatibility and licensing

Off by default; the guard re-runs the satin reference specs. PES export: the stop is a
colour change in PES/PEC (PE-DESIGN has no stop command in that format) and a `Stop` in DST
and EXP, as the cutwork stages already are. The rules are published digitizing practice
from several independent sources; "Raised Embroidery" (Hatch) and "3D Foam Auto-Underlay"
(Embrilliance) are product names; OpenStitch labels the mode "3D puff".

## 9. Open questions

- Whether the "Lay foam" stop should be a colour change on single-needle machines (the
  machine pauses and asks for the next colour, which is what home users expect) or a true
  Stop where the format has one; proposal: colour change for PES/PEC, Stop elsewhere, as
  the cutwork wizard does.
- Cap depth: `w/3` is a reading of the guides' pictures, not a published number; adjust
  after a sew-out.
