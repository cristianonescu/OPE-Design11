# B5. Pull and push compensation, automatic underlay by width, fabric table: algorithm sketch

Backlog item B5 of [competitive-features.md](../competitive-features.md) (features D4–D6).
Research date: 2026-10-28. A design note for `openstitch-design` (`model.rs`, `region.rs`,
`column.rs`, `skeleton.rs`, `generate.rs`) and `openstitch-engine` (`fabric.rs`). No other
product's code was read; the numbers come from published digitizing practice.

## 1. What the feature is

Three linked pieces of "sew-out quality" control that the professional products expose and
PE-DESIGN 11 reduces to one slider and five under-sewing presets:

- **Pull compensation as a model.** Satin pulls the fabric in along the stitch, so columns
  sew narrower than drawn; fills pull across their rows. Compensation widens the stitch by an
  **absolute** amount per side (published starting points: 0.2 mm on woven, 0.35 mm on
  knits, 0.4 mm on fleece and towelling), or by a **percentage** of the local width (scales
  with the column, right for small lettering), with **separate values per side** (negative
  allowed to shrink), and **push compensation** that shortens the ends of a column because
  the thread pushes out along the stitch direction.
- **Automatic underlay by width.** The underlay type is chosen from the local column width
  and the region size rather than fixed per object: none below about 1 mm, a centre run for
  1–3 mm, an edge run inset about 0.3–0.5 mm for 3–4 mm, zigzag at 2–3 mm spacing above
  that (often with the edge run), tatami or double tatami under large fills; optionally a
  first and a second underlay.
- **Fabric table.** A per-fabric bundle of density scale, compensation, underlay choice and
  stitch length, editable by the user and applied per design or per object.

Sources: Wilcom underlay types with their width rules and lettering thresholds (< 5 mm none,
6–10 mm centre run, > 10 mm edge run), Auto Fabric, pull compensation and the ES 2026
separate per-side value
[[ES-underlay]](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/stabilizing/stabilizing-7.htm),
[[ES-pull]](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/stabilizing/stabilizing-17.htm),
[[Wilcom-pushpull]](https://wilcom.com/resources/blog/push-and-pull-compensation),
[[Hatch-fabric]](https://blog.hatchembroidery.com/how-to-use-auto-fabric-or-create-your-own-custom-fabric-settings/);
Bernina underlay by type [[Bernina-underlay]](https://softwarehelp.bernina.com/en/MainHelp/PART_Digitizing/quality/quality-5.htm);
Pulse X/Y pull and push compensation, four underlay types with order, fabric recipes
[[DG-12]](https://dgml14.pulsemicro.com/pulse-products-12reasonsforpulse.htm); Embird
underlay chosen by object size, Styles Table [[Embird-tools]](https://www.embird.net/digitizing-tools/);
Ink/Stitch pull compensation in mm and % with two values for asymmetry, push compensation,
centre-walk / contour / zigzag satin underlays with fixed and proportional insets
[[IS-satin]](https://inkstitch.org/docs/stitches/satin-column/); Embrilliance compensation
min / % / max [[EM-SA]](https://www.embrilliance.com/stitchartist); FTCU Save2Sew
[[FTCU]](https://www.mulqueen.com/shop/c/p/Floriani-Total-Control-Professional-Embroidery-Software---FTCU-x44586193.htm);
an underlay primer [[Underlay-101]](https://support.coldesi.com/Portals/8/Documents/Liberty%20Docs/Underlay%20101.pdf);
a pull-compensation explainer [[MrX]](https://mrxstitch.com/pull-compensation/).

PE-DESIGN 11: one unitless "pull compensation" slider on satin, fill and programmable fill;
under sewing on/off with the presets Edge only, Single, Region only, Edge and region, Double;
the Fabric Selector as a global preset ([region attributes](http://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/16_1293518.html)).
OpenStitch today: `pull_compensation: f64` in mm, applied as a row-end extension on both
ends of every fill row (`generate.rs`) and a rung extension at both ends of satin
(`column::satin`, `skeleton::rungs`); `UnderlayStyle` {Edge, Zigzag, EdgeZigzag, Double,
EdgeFill} with `underlay_spacing` (2 mm default) and the PE-DESIGN insets measured
black-box (satin outline 1 mm in, rows 2 mm apart; fill edge run 1 mm in); `fabric.rs` with
eight fabrics, each setting fill and satin density, stitch length, pull compensation and
an underlay style. All of that is at the reference tier and must not change by default.

## 2. Compensation model

### 2.1 Definition

Replace the scalar by a structure, with the scalar kept as a compatibility alias:

```
Compensation {
    left_mm:  f64,   // absolute, per side (left/right of the stitch direction, or start/end of a fill row)
    right_mm: f64,
    percent:  f64,   // of the local width, split equally over both sides
    max_mm:   f64,   // cap on the total per side (Embrilliance's "max"); 0 = no cap
    push_mm:  f64,   // shorten each column end by this amount along the stitch direction
}
```

The effective widening of a stitch of local width `w` on each side is

```
side = clamp(side_mm + (percent / 100) · w / 2,  −w/4,  max_mm or ∞)
```

(the lower clamp stops a negative value from inverting the stitch). The PE-DESIGN scalar
`pull_compensation = p` is exactly `left_mm = right_mm = p, percent = 0, push = 0`, so every
existing design and reference spec compiles unchanged, and the `.osd` file keeps writing the
scalar when the other fields are at their defaults.

### 2.2 Where it applies

- **Satin columns and skeleton satin** (`column::satin`, `skeleton::rungs`): each rung is
  lengthened by `left` at its left end and `right` at its right end (today: `pull` at both).
  The sides are named by the rung direction, which is consistent along a column; the
  inspector shows "Side A / Side B" with the column's rails highlighted, as Manual Punch
  draws them.
- **Fills** (`generate::fill*`, row spans): each row span is extended by `left` at its start
  and `right` at its end (today: both ends by `pull`). "Left" is the row's start side in the
  fill's angle frame, so the two values mean "the two edges across the stitch direction".
- **Zigzag lines** (`lines::zigzag_rails`): the rails are offset `w/2 + left` and
  `w/2 + right`; today the line has no compensation at all, so the default stays 0.
- **Push**: the first and last `push_mm` of a column (measured along the centreline) are
  removed: rungs within `push_mm` of either end are dropped and the end rung is moved
  inward by `push_mm`. For fills, push is the across-row direction (the first and last rows),
  where it is usually 0; the attribute is still honoured for symmetry. Push is ignored for
  closed satin rings (no ends).
- **Under sewing** keeps its own insets (unchanged): compensation is for the top stitching;
  the published rule is that underlay stays inside the compensated outline.

### 2.3 Percent and small lettering

`percent` is what makes small text sew readably: a 1.5 mm stroke with 20 % gets 0.15 mm per
side, a 6 mm stroke 0.6 mm, where a fixed 0.3 mm would be too much for the first and too
little for the second. The text tool's sewing attributes get the same structure; PE-DESIGN's
"small text" fonts, which OpenStitch does not yet have (ROADMAP "Lettering"), are where the
Fabric Selector would set `percent`.

## 3. Automatic underlay by width

### 3.1 A new style, not a change to the existing ones

`UnderlayStyle::Auto` is added; the five PE-DESIGN presets keep their exact behaviour. Auto
chooses per object (and, for columns, per section) from a table of width thresholds; the
chosen construction is one of the existing generators, so nothing new is sewn, only chosen:

| Local width `w` (satin) / size (fill) | First underlay | Second underlay |
|---|---|---|
| `w < 1.0 mm` | none | — |
| `1.0 ≤ w < 3.0 mm` | centre run (one running stitch down the centreline and back, 2.5 mm pitch) | — |
| `3.0 ≤ w < 4.5 mm` | edge run (today's satin outline inset 1 mm, or `0.3–0.5 mm` for narrow columns: inset = `min(1.0, w/4)` mm) | — |
| `4.5 ≤ w < 10 mm` | edge run | zigzag across the column at 2.5 mm spacing (today's satin rows) |
| `w ≥ 10 mm` (a wide satin) | edge run | zigzag, and a warning that satin over 10 mm should become a fill (the Baby Lock note) |
| fill, area `< 100 mm²` | edge run | — |
| fill, `100 ≤ area < 2500 mm²` | edge run | tatami underlay perpendicular to the rows (today's `EdgeFill`) |
| fill, `area ≥ 2500 mm²` or stretch fabrics | edge run | double tatami at ±45° (today's `Double`) |

The centre run is a new, trivial generator (`generate::running` along the centreline,
forward and back). For a skeleton satin the width varies along the column, so the choice is
made per rung section with the median width of the section, and the chosen layers are
clipped to that section; a column with a narrow neck and wide ends gets a centre run in the
neck and edge+zigzag at the ends, joined by the usual travel.

Thresholds come from the published rules above (Wilcom's lettering thresholds and the
2–3 mm centre-run band, the Underlay 101 primer); they are attributes of the fabric (section
4) so a user can shift them. The lettering rule is applied through the text tool's sewing
attributes: text gets `Auto` by default only when the user picks it, because PE-DESIGN's text
tool sews satin at density 5.0 without under sewing (reference tier) and that stays the
default.

### 3.2 Order and spacing

First underlay, then second, then the top stitching, each starting where the previous
ended (today's `link_underlay` for satin travels between layers without a jump). The zigzag
underlay spacing is `underlay_spacing` (2 mm default) scaled by the fabric's underlay
density factor; the edge-run inset is the table value; the second underlay is rotated 90°
from the top stitching as today.

## 4. Fabric table

### 4.1 Model

`fabric.rs` becomes data the user can edit (a user-settings file, as the user thread charts
and saved attribute sets are), each entry:

```
Fabric {
    id, name, description, advice,
    fill_density, satin_density, stitch_length,     // as today
    compensation: Compensation,                     // replaces pull_compensation
    underlay: UnderlayStyle,                        // may be Auto
    underlay_density_factor: f64,                   // scales underlay row spacing (1.0 = today's 2 mm)
    auto_thresholds: { none_below, centre_below, edge_below, zigzag_from } in mm,
    short_stitch_distance: Option<f64>,             // B3, e.g. 0.25 mm on knits
}
```

Bundled defaults (the eight fabrics today) get `compensation` from their current
`pull_compensation` as `left = right`, plus `percent` 0, and the thresholds from the table
above; the published starting points (0.2 woven, 0.35 knit, 0.4 fleece/towel) already match
the bundled values.

### 4.2 Application

Today the Fabric Selector applies its values to every object (`fabric.apply`). Add:

- **Per object**: `object.applyFabric` sets the object's attributes from a fabric; the
  object remembers the fabric id in an informational field so re-applying after edits is one
  click (Pulse's "recipe", Embird's Styles Table).
- **Design default**: the Design Page carries a fabric id; new objects take its values.
- **Re-digitize**: `fabric.apply` with `recompute: true` also resets Auto underlay choices
  and the short-stitch rule; this is Floriani's Save2Sew in effect.

## 5. Parameters and defaults

| Attribute | Type | Default | Range |
|---|---|---|---|
| `pullCompensation` (alias) | mm | 0 (PE-DESIGN default) | −2–2 |
| `compensation.leftMm` / `rightMm` | mm | = alias | −2–2 |
| `compensation.percent` | % | 0 | −20–50 |
| `compensation.maxMm` | mm | 0 (off) | 0–3 |
| `compensation.pushMm` | mm | 0 | 0–1.5 |
| `underlayStyle` | existing + `auto` | PE-DESIGN preset | — |
| fabric `autoThresholds` | mm | 1.0 / 3.0 / 4.5 / 10 | ordered, 0.5–15 |
| fabric `underlayDensityFactor` | — | 1.0 | 0.5–2 |

## 6. Model and code changes

- `model.rs`: `Compensation` with a custom serde that reads a bare number (old files, the
  PE-DESIGN scalar) or the structure, and writes the bare number when only `left == right`
  is set; `UnderlayStyle::Auto`; `RegionSewing::fabric: Option<String>`.
- `generate.rs`: `RegionParams.pull_compensation` becomes `(left, right)`; call sites pass
  both.
- `column.rs`, `skeleton.rs`: per-end lengthening, push at the ends, per-section width.
- `region.rs`: the Auto branch in the under-sewing match, the centre-run generator, the
  per-section clipping for skeleton satin, and the wide-satin warning (returned through the
  existing attribute validation path so the inspector can show it).
- `lines.rs`: optional compensation on zigzag rails.
- Engine: `object.setRegion` / `setLine` accept the structure (and still the scalar);
  `fabric.list` / `fabric.apply` read the user table; `object.applyFabric`; the Design
  Page's fabric id in `design.setPage`.
- App: the compensation row becomes a disclosure with Side A / Side B / % / max / push; the
  underlay dropdown gains "Automatic (by width)"; the Fabric Selector gains an editor with
  the new fields and "apply to selection".
- `.osd` migration: none needed (serde reads both forms); a round-trip test proves old files
  re-save byte-identically when untouched.

Effort: M. The arithmetic is small; the model change touches every generator's signature and
the engine's parameter schema, and the inspector work is a full pane.

## 7. Test cases (internal regression tier, plus reference guards)

| Spec | Script | Expected |
|---|---|---|
| `comp-scalar-is-today` | every reference spec that sets `pullCompensation` re-run through the structure with `left = right` | identical output (reference-tier guard) |
| `comp-asymmetric-column` | 6 mm Manual Punch column, `leftMm` 0.4, `rightMm` 0 | every rung's left end is 0.4 mm ± 0.05 outside the left rail, right ends on the right rail |
| `comp-percent-lettering` | the word "mini" in Lato Bold at 5 mm and at 20 mm, `percent` 20 | widening per side measured on the straight strokes = 10 % of the stroke width ± 0.05 mm at both sizes |
| `comp-max-cap` | 12 mm column, `percent` 50, `maxMm` 0.8 | widening per side = 0.8 mm, not 3 mm |
| `comp-negative-clamp` | 2 mm column, `leftMm` −1.5 | widening clamped at −0.5 mm (−w/4): stitches never invert (every rung length > 0) |
| `comp-push-ends` | 40 mm straight 6 mm column, `pushMm` 0.5 | first and last rungs 0.5 mm ± 0.1 inside the drawn ends; rung count reduced accordingly; closed satin ring unchanged |
| `comp-fill-rows` | 40 × 20 mm fill, `leftMm` 0.3, `rightMm` 0 | row starts extended 0.3 mm on the start side only |
| `underlay-auto-narrow` | 2 mm column, `auto` | one underlay layer: a centre run out and back along the midline (all points within 0.2 mm of the centreline) |
| `underlay-auto-medium` | 4 mm column | edge run only, inset `min(1.0, w/4)` = 1.0 mm |
| `underlay-auto-wide` | 8 mm column | edge run then zigzag rows at 2 mm; the two layers and the top stitching are one run (no jump) |
| `underlay-auto-neck` | skeleton satin with a 2 mm neck between 8 mm ends | centre run in the neck section, edge + zigzag in the end sections; travel between sections inside the outline |
| `underlay-auto-fill-sizes` | fills of 50, 400 and 4000 mm² | edge run; edge + tatami; edge + double tatami |
| `underlay-presets-unchanged` | every reference spec with under sewing re-run | identical output (guard) |
| `fabric-apply-object` | two fills, `object.applyFabric` knit on one | only that object's density, compensation and underlay change; `fabric` id recorded |
| `fabric-user-table-roundtrip` | add a fabric with custom thresholds, save, reload | the table round-trips; `underlay-auto-*` results follow the custom thresholds |
| `osd-compensation-roundtrip` | load an alpha.4 `.osd` with scalar compensation, save untouched | byte-identical file |

Measurement helpers needed: "rung extraction" and "rail distance per rung end" (shared with
B3), "layer identification" (underlay vs top by order and density, exists for the satin
specs), "distance of points to a polyline" (exists for the line specs).

## 8. Compatibility and licensing

The scalar alias and the untouched presets keep every reference spec and every existing
`.osd` file as they are; the guards above enforce it. PES export is unaffected (stitches).
The width thresholds and compensation values are published practice from several
independent sources; no vendor code or data was used. Wilcom's "Auto Fabric" and Floriani's
"Save2Sew" are product names; OpenStitch keeps "Fabric Selector" (PE-DESIGN's own term,
which the guide uses) and "Automatic (by width)".

## 9. Open questions

- Whether `percent` should be the default for text (the professional products' small
  lettering fonts use it) once small-text fonts exist; proposal: yes, 15 %, set by the text
  tool's attribute preset, after a sew-out.
- Whether to expose the Auto thresholds per object or only per fabric; proposal: per fabric
  only, to keep the inspector short.
- The wide-satin warning threshold (10 mm here, from the Baby Lock note; PE-DESIGN itself
  allows wider satin): make it a preference.
