# B10. Satin line with variable width: algorithm sketch

Backlog item B10 of [competitive-features.md](../competitive-features.md) (feature O2).
Research date: 2026-10-28. A design note for `openstitch-design` (`model.rs`, `lines.rs`,
`skeleton.rs`, `column.rs`) and the editor. No other product's code was read.

## 1. What the feature is

A satin outline whose width changes along the path: wide in the middle of a brush stroke,
tapered to a point at the ends, thick on one side of a letter and thin on the other. Three
ways the products expose it: two independently editable rails (Wilcom Column B); a
centreline with a width at points along it (Pulse Steil Thickness, Floriani single-line
satin input, mySewnet Satin Column); pen pressure (Embird). Tapered ends ("brush" or
calligraphy strokes) are the common use.

Who has it: Wilcom Column B and Column C [[ES-corners]](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/quality/quality-11.htm);
mySewnet Satin Column (pairs of points, about 8–10 mm advised) **(single source)**
[[MSW-area]](https://softwarehelp.mysewnet.com/MSW/140/Digitizing/Topics/Area_Types.htm);
Pulse Column, Steil Thickness and the Brush Tool with tapered ends
[[DG17-notes]](https://cloud.pulsemicro.com/Download/DownloadReleaseNotes?name=DG17),
[[DG16-IE]](https://tajimaeurope.com/en/pulse-software-solutions/embroidery-software-tajima-software-dg16-pulse/illustrator-extreme/);
Embird pressure-sensitive columns **(single source)** [[Embird-tools]](https://www.embird.net/digitizing-tools/);
Ink/Stitch satin column with rungs and the stroke-width "Stroke to Satin" tool
[[IS-satin]](https://inkstitch.org/docs/stitches/satin-column/),
[[IS-satintools]](https://inkstitch.org/docs/satin-tools/); FTCU single-line satin input
[[FTCU]](https://www.mulqueen.com/shop/c/p/Floriani-Total-Control-Professional-Embroidery-Software---FTCU-x44586193.htm).

PE-DESIGN 11: the Zigzag line sew type has one width; a variable width needs a Manual Punch
column (a region). OpenStitch today has **more than PE-DESIGN here already**: the Satin
Skeleton tool (`shape.skeletonSatin`) makes a satin region from a centreline with a width at
every point (`Skeleton { points, widths }`), `skeleton::satin` sews it with rungs
perpendicular to the line, `skeleton::outline` derives the region's outline, and skeletons
are derived from shapes. What is missing is the line form: a satin outline on **any** line
object (open or closed, drawn with the pen tool, text converted to outlines, imported SVG
strokes) with width keys the user drags, asymmetric widths, tapered ends, and a stroke-width
import from SVG. B10 is that, built on the skeleton generator.

## 2. Geometry

### 2.1 Width keys

A line object with `LineType::Satin` carries a width profile along its arc length:

```
WidthKey { s: f64 /* 0..1, fraction of the line's length */, left: f64, right: f64 /* mm */ }
widthKeys: Vec<WidthKey>   // sorted by s; at least one
```

The left and right widths are measured from the centreline along the normal, so the rails
are `C(s) ± n̂(s) · w_side(s)` with `w_side` interpolated linearly between keys (or by a
monotone cubic when `smoothWidth` is on, so a width change does not show a crease). One key
with `left = right = w/2` is today's Zigzag line of width `w`; so the Zigzag line type keeps
its name and output, and `Satin` is the new type whose single-key default is the same
geometry sewn with the skeleton generator (which handles bends by rotating rungs, as
`zigzag_rails` leans them; the two coincide on straight legs and differ on curves, which is
why this is a new type and not a change to Zigzag).

Ends: `taperStart` and `taperEnd` (mm, default 0) ramp both widths to zero over that
distance from each end, producing the brush-stroke point; a tapered end gets no cap or
lock across it (the last rung is a point), and the tie-in moves to the first full-width
rung.

Closed lines: `s` wraps, the profile is periodic, and the first key at `s = 0` is also the
last.

### 2.2 Sewing

Convert the line and its profile to a `Skeleton` at sewing time: sample the line at the
rung spacing (`10 / density` along the longer side, as `skeleton::rungs` does), evaluate
`left(s)` and `right(s)`, and build rungs `[C − n̂·left, C + n̂·right]`; then `skeleton::satin`
(sawtooth, tie-in and lock, under sewing from the region code, pull compensation from B5,
corners and short stitches from B3, puff from B7). Where the left and right widths differ
the rung is off-centre, which is exactly what an asymmetric column is. The existing
skeleton validation (widths 0–30 mm, at least one non-zero) applies.

Under sewing for a satin line: the zigzag line's rule today (a running stitch a quarter of
the width to each side); with `UnderlayStyle::Auto` (B5) it follows the width table.

### 2.3 Editing

- **Handles.** The canvas shows a handle on each rail at every key; dragging a handle along
  the normal changes that side's width at that key (both sides with Shift, keeping the
  column symmetric); double-clicking a rail adds a key at that `s`; Delete removes one.
  Dragging the centreline's vertices edits the path as for any line (the keys keep their
  `s`).
- **Width tool.** A "Satin width" inspector row sets the current key's left/right, the taper
  lengths and `smoothWidth`; "Reset to constant" leaves one key.
- **Pressure.** The Pencil tool, when the pointer reports pressure, records a key every
  2 mm of arc length with `left = right = pressure × maxWidth / 2`; without pressure it
  behaves as today. egui exposes pointer pressure on platforms that have it (`force`), so
  this is a small addition to `freehand.rs`'s pointer trail.
- **Convert.** Line ▸ "Convert to column" turns the satin line into a Manual Punch column
  (the rungs become direction lines) for users who want rail-level editing (Wilcom's Column
  B); and "Satin line from column" does the reverse by fitting keys to the rungs' midpoint
  chain and widths.

### 2.4 SVG import

`import.svg` maps a stroked path with a stroke width to a `Satin` line with one key
(`left = right = stroke-width / 2`) when the width is at least `minSatinWidth` (1 mm
default), and to a running line below it; this is what the Ink/Stitch "stroke to satin"
tool does by hand and what SVG from a drawing program carries for free. Dashed strokes stay
running lines.

## 3. Parameters and defaults

| Attribute | Type | Default | Range |
|---|---|---|---|
| `type` | `satin` (line) | — | new `LineType::Satin` |
| `widthKeys` | list of `{s, left, right}` | one key, 1.5 / 1.5 mm (a 3 mm column) | widths 0–15 mm per side |
| `smoothWidth` | bool | true | — |
| `taperStart`, `taperEnd` | mm | 0 | 0–20 |
| `density` | lines/mm | 4.5 | existing |
| `underlay` | bool | true | existing zigzag-line rule; `Auto` with B5 |
| `pullCompensation` | existing / B5 | per fabric | |
| `minSatinWidth` (import) | mm | 1.0 | 0.5–3 |

## 4. Model and code changes

- `model.rs`: `LineType::Satin`; `LineSewing::width_keys`, `smooth_width`, `taper_start`,
  `taper_end` (serde defaults; absent on every other line type).
- `lines.rs`: `satin_line(path, keys, tapers, spacing, …) -> Run` that builds the skeleton
  and calls `skeleton::satin`; closed-path handling; the under-sewing rule.
- `skeleton.rs`: a constructor from `(path, left(s), right(s))` and asymmetric rungs (today
  the skeleton is symmetric: `widths` is the full width; add an optional `offsets` vector
  or build rungs directly in `lines.rs`).
- `compile.rs`: the line type dispatch.
- Engine: `object.setLine` accepts the keys; `object.setWidthKey` / `removeWidthKey` for
  the handles (one undo step per drag); `import.svg` stroke mapping; `object.lineToColumn`
  and `object.columnToLine`.
- App: handles and the width row; the Pencil's pressure keys; the Outline ribbon tab gets
  "Satin" next to Zigzag.

Effort: S–M. The generator exists; the work is the key model, the conversions and the
canvas handles.

## 5. Test cases (internal regression tier, plus a reference guard)

| Spec | Script | Expected |
|---|---|---|
| `satinline-constant-equals-skeleton` | a 40 mm straight satin line, one key 2/2 mm | identical stitches to `shape.skeletonSatin` with width 4 on the same points |
| `satinline-zigzag-unchanged` | every zigzag-line reference script | identical output (the Zigzag type is untouched) |
| `satinline-keys` | keys at s = 0 (1/1), 0.5 (3/3), 1 (1/1) on a 60 mm line | rung half-widths measured along the normal follow the piecewise-linear profile within 0.1 mm (and the smooth profile when `smoothWidth`, monotone between keys) |
| `satinline-asymmetric` | keys with left 1, right 3 | rails at 1 mm and 3 mm from the centreline; rungs off-centre |
| `satinline-taper` | 40 mm line, 3 mm wide, `taperEnd` 8 | widths fall linearly to a point over the last 8 mm; the last stitch is on the centreline at the end; no lock stitch across the taper (the lock sits at the last full-width rung) |
| `satinline-closed` | a 30 mm circle with keys 1 → 3 → 1 around it | periodic profile; one run; the seam at s = 0 has no width jump |
| `satinline-curve` | an S-curve, constant 4 mm | every rung perpendicular to the centreline within 3° (skeleton behaviour), rung spacing on the outer side `10/density ± 0.05` |
| `satinline-corners` | an L with `cornerStyle: mitre` (B3) | the mitre construction present at the corner |
| `satinline-svg-import` | an SVG with a 3 mm stroked path and a 0.4 mm one | the first becomes a satin line with one 1.5/1.5 key, the second a running line |
| `satinline-column-roundtrip` | satin line → column → satin line | keys fitted within 0.1 mm of the originals; stitches identical within 0.05 mm |
| `satinline-pressure` | a scripted pencil trail with pressure values | one key per 2 mm with `left = right = pressure × maxWidth / 2` |

Measurement helpers needed: "rung extraction" and "rail distance per rung end" (shared with
B3 and B5), "rung angle to the centreline tangent" (small).

## 6. Compatibility and licensing

The Zigzag line type and the Satin Skeleton region are unchanged; `Satin` is a new line
type. PES export writes stitches; an editable `#PES0110` save writes a stitch group (the
line has no PE-DESIGN equivalent). The geometry is OpenStitch's own skeleton satin with a
width profile; "Column B/C", "Steil Thickness" and "Brush Tool" are vendor names; OpenStitch
labels the type "Satin" (line) with "Width keys" and "Taper".

## 7. Open questions

- Whether `Satin` should replace `Zigzag` as the default satin outline for new users (it
  handles curves better); proposal: no, Zigzag stays the PE-DESIGN-compatible default and
  the Fabric Selector or a saved attribute set can prefer Satin.
- Minimum width before a satin line degrades to a running stitch (the taper's last
  millimetre): the skeleton's `Thin` validation covers the whole line; for the taper use a
  0.3 mm floor below which rungs are dropped and the path is sewn as running stitch.
