# B15. Lettering depth: baselines, saved kerning, team names, closest-point connections

Backlog item B15 of [competitive-features.md](../competitive-features.md) (features D9–D11,
D13). Research date: 2026-10-28. A design note for `openstitch-design` (`text.rs`,
`fonts.rs`) and `openstitch-engine` (`lettering.rs`). No other product's code was read.

## 1. What the feature is

Lettering is most of hobby embroidery, and the professional products go deeper than
PE-DESIGN in four places:

- **Baselines as a model.** The text sits on a baseline that is a line, an arc (clockwise or
  counter-clockwise, by radius and sweep), a full circle or ellipse, a vertical line, or any
  drawn path; letters are placed by arc length and rotated to the tangent; a fixed-length
  baseline compresses letter spacing as it fills.
- **Saved kerning.** Pair adjustments made by hand are saved with the font and applied
  next time; kerning classes apply one adjustment to a whole class of letter pairs.
- **Team names.** One design, a list of names (and numbers, and sizes) from a CSV, one
  output file per row, named by a pattern, with the lettering refitted per row.
- **Closest-point connections.** Letters of a word are joined at the nearest points of
  consecutive glyphs with a hidden run, so a word sews with no trim between letters; small
  text fonts use shortened stitches and compensation so 3–5 mm lettering stays readable.

Who has it: Wilcom 9 baselines, saved kerning per font, Enveloping, Team Names with Matrix
sequencing, Expert Resequencing (tucking overlaps under satin), small-lettering fonts
[[ES-lettering]](https://wilcom.com/embroiderystudio/lettering),
[[ES-baselines]](https://docs.wilcom.com/embroiderystudio/26/en/OnlineHelp/Lettering/lettering_create/Apply_baselines.htm),
[[ES-curved]](https://docs.wilcom.com/embroiderystudio/e4/en/MainHelp/Lettering/lettering_edit/Adjust_curved_baseline_settings.htm);
Pulse Visual Kerning, kerning classes, TTF kerning import, closest-point connected fonts
(180–200 fonts), lock stitches inside text, team names, WordIT
[[DG16-notes]](https://cloud.pulsemicro.com/Download/DownloadReleaseNotes?name=DG16),
[[DG16]](https://fonts.pulsemicro.com/Home/Embroidery); Embird Font Engine baselines (lines,
circle, spirals, node-editable), envelopes, nearest-point connections
[[Embird-font]](https://www.embird.net/machine-embroidery-tutorials/common/font_engine.htm),
[[Embird-baseline]](https://www.embird.net/sw/Embird/tutorial/custbsln/custbsln.htm);
Ink/Stitch font.json with `hkern` pairs, lettering along a path, batch lettering
[[IS-lettering]](https://inkstitch.org/docs/lettering/), [[IS-font]](https://inkstitch.org/tutorials/font-creation/);
Embrilliance connecting scripts, AlphaTricks automatic kerning, circle and spiral text
[[EM-alpha]](https://embrilliance.com/product/alphatricks/), [[EM-ess]](https://www.embrilliance.com/essentials);
FTCU Closest Point Connect fonts, Name Drops [[FTCU]](https://www.mulqueen.com/shop/c/p/Floriani-Total-Control-Professional-Embroidery-Software---FTCU-x44586193.htm);
Melco's lettering property list as a public description of the baseline parameters
[[Melco]](https://melco-service.com/docs/DS_V9_090309/prop_lettering.htm).

PE-DESIGN 11: 26 Transform presets (arches, fans, …), Fit Text to Outline (text on a path
with horizontal/vertical alignment, spread, other side), per-character kerning and edits,
Connection Points (Default / Closest / Farther, text only), small-text fonts, Name Drop (one
field replaced from a list), monograms, Font Creator, 130 fonts. OpenStitch today: `TextSpec`
with `fit: PathFit` (valign, upright, spread, other_side) on a path object, `transform` with
24 of 26 presets (`envelope_edges`), `kerning: bool` from the font's OpenType tables,
`CharEdit` per character, `NameDrop` (`text.rs:311`), `DesignFont` (bundled fonts, user
TrueType carried in the design, embroidery fonts from Font Creator), shaping with rustybuzz;
Connection Points and small-text fonts are open items of the compatibility roadmap.

## 2. Baseline model

### 2.1 Definition

`Baseline` on `TextSpec` (default `Line`, which is today's layout):

```
Line                                     // horizontal; today's text
FixedLine { length }                     // letter spacing compresses to fit, expands to fill (optional)
Vertical                                 // existing vertical text (letters stacked)
Arc { radius, sweep_deg, clockwise }     // centre below (cw) or above (ccw) the text
Circle { rx, ry, start_deg, clockwise }  // full ellipse; text starts at start_deg
Path { object id, fit: PathFit }         // today's Fit Text to Outline
```

`Arc` and `Circle` are `Path` with a generated contour, so one placement routine serves all:
letters are placed along the baseline by arc length, each rotated to the tangent at its
anchor (`upright` keeps letters vertical, today's `PathFit::upright`), kerning and letter
spacing measured along the arc. Counter-clockwise arcs make the letter tops converge, which
the published guides handle with extra spacing: the engine adds `spacing_ccw` (default
+10 % of the letter width at the top line, proportional to the curvature) automatically so
the user does not.

The Transform presets stay as they are (they warp the whole text block; a baseline places
letters), and the inspector shows "Baseline" and "Transform" as two rows; a preset that is
really an arc (the arch presets) can be re-expressed as `Arc` later without changing output.

### 2.2 Editing

Drag handles on the canvas: for `Arc`, the two ends and the midpoint (changes the radius);
for `Circle`, the centre and the radii; for `FixedLine`, the length; for `Path`, the path's
own point editing. `object.setBaseline` sets the model; one undo step per drag.

## 3. Saved kerning and kerning classes

### 3.1 Storage

`DesignFont` (and the user's font library entries) gains `kerning: KerningTable`:

```
KerningTable {
    pairs:   BTreeMap<(char, char), f64>,     // em fraction, added after the font's own tables
    classes: Vec<KerningClass { name, left: Vec<char>, right: Vec<char>, value }>,
}
```

Today's per-character kerning on a text object (`CharEdit`) stays per object; "Save kerning
to font" copies the current pair adjustments into the font's table (for the bundled fonts,
into a user override stored in the user settings, as user thread charts are), and the
table is applied in `layout` after the OpenType `kern`/`GPOS` adjustments. Classes: a pair
`(l, r)` with no explicit entry takes the first class whose `left` contains `l` and `right`
contains `r`. The Font Creator shows a kerning pane with a sample word and sliders
("Visual kerning"), writes pairs and classes, and can import a TrueType font's pair table
into an embroidery font made from it (`ttf-parser` already reads `kern`).

### 3.2 Automatic kerning for embroidery fonts

Fonts made from stitch files (user-mapped text, Font Creator from images) have no kerning.
"Auto kern" computes, for each pair in the font's sample set (the 26 × 26 letters plus
digits), the horizontal distance between the glyphs' outline profiles at 8 heights and sets
the pair so the minimum gap equals the font's default side bearing (what AlphaTricks
describes as automatic kerning). Deterministic, one command, results saved into the table
for the user to adjust.

## 4. Team names

`lettering.teamNames` with a design, a CSV (header row names the fields), a mapping from
field to text object (several text objects per design: name, number, department), optional
per-row size or hoop columns, and an output pattern (`{row}`, `{field}` tokens) for file
names and formats. For each row: replace the texts (Name Drop's fitting rule: the
replacement is refitted to the original's box, which `NameDrop::fit` already does), compile,
export (every format the design would export to), and collect a worksheet row (file name,
thread list, stitch count). "Matrix sequencing": optionally one file per row **and** one
combined file with all rows arranged on a page grid (for multi-head shops), using the Arrange
Copies placement. Rows that do not fit (a long name at the minimum letter size) are reported,
not silently shrunk below `minLetterHeight` (default 4 mm).

This is Name Drop generalised from one field to a table and from one file to a batch; the
engine's batch export and the Design Database's conversion loop exist.

## 5. Closest-point connections and small text

### 5.1 Connections

PE-DESIGN's Connection Points (Default / Closest / Farther) are on the roadmap; this note
fixes the Closest rule since it is the one other products use for "no-trim" fonts: for
consecutive glyphs `i` and `i+1`, choose `exit(i)` and `entry(i+1)` as the nearest pair of
points on their outlines (`route::nearest`, as B11's closest join), then link them with a
running stitch that is hidden: along the glyph outline where the satin will cover it, or
straight across the gap when the gap is under `maxOpenConnector` (default 1.5 mm, the
distance a loose running stitch between letters is accepted in script fonts), else a jump
(and trim if over the trim length). Within a glyph, the strokes are ordered so the glyph's
exit is on the side of the next glyph (the sewing order inside a text is at the reference
tier for the default; Closest reorders strokes only when it changes the exit side).

Script fonts whose glyphs touch (Lato Bold does not; Pacifico does) get `connected: true`
in their font data, which makes Closest the default connection for them.

### 5.2 Small text

A `smallText` preset of sewing attributes applied when the letter height is below 6 mm (the
Wilcom lettering threshold band): satin density 5.5 lines/mm, no under sewing below 5 mm
and a centre run between 5 and 6 mm (B5's width table), pull compensation as a percentage
(B5, 15 %), short stitches on (B3), split length off, and a stitch length floor of 0.5 mm.
The preset is applied by the text tool when the size is set, shown in the inspector, and
editable; PE-DESIGN's small-text fonts (a separate font list) are matched by letting any
font use the preset, which is what the roadmap item needs.

## 6. Parameters and defaults

| Attribute / command | Type | Default |
|---|---|---|
| `baseline` | `line` \| `fixedLine{length}` \| `vertical` \| `arc{radius, sweep, cw}` \| `circle{rx, ry, start, cw}` \| `path{id, fit}` | `line` |
| `spacingCcw` | % of letter width | 10, automatic |
| font `kerning.pairs`, `kerning.classes` | table | empty |
| `font.saveKerning`, `font.autoKern`, `font.importKerning` | commands | — |
| `lettering.teamNames` | command (`csv`, `mapping`, `pattern`, `formats`, `matrix`) | `minLetterHeight` 4 mm |
| `connection` | `default` \| `closest` \| `farther` | `default` (`closest` for connected fonts) |
| `maxOpenConnector` | mm | 1.5 |
| `smallText` preset threshold | mm | 6 |

## 7. Model and code changes

- `text.rs`: `Baseline` on `TextSpec` (serde default `line`); arc and circle contour
  generation; the ccw spacing rule in `layout_in`; `KerningTable` application after shaping;
  the Closest connection in the glyph sequencing (`Layout::stitch_block` and the per-glyph
  compile path).
- `fonts.rs`: `kerning` on `DesignFont`; user overrides for bundled fonts in the user
  settings.
- `lettering.rs`: `object.setBaseline`, `font.*` kerning commands, `lettering.teamNames`
  (built on Name Drop and the export loop), `text.setConnection`; the `smallText` preset
  in the text tool's defaults.
- App: baseline handles and the Baseline row; the Font Creator kerning pane; a Team Names
  dialog (CSV preview, mapping, pattern, run); a Connection row.

Effort: S for team names, S for saved kerning, M for the baseline model and handles, M for
closest connections (the quality is in the hidden-run choice), S for the small-text preset
once B3 and B5 exist.

## 8. Test cases (internal regression tier, plus reference guards)

| Spec | Script | Expected |
|---|---|---|
| `baseline-line-is-today` | every text reference spec with `baseline: line` explicit | identical output (guard) |
| `baseline-arc-cw` | "ARCH" at 15 mm on an arc of radius 40 mm, 120° sweep, clockwise | letters' anchors on the arc within 0.2 mm; each letter rotated to the tangent within 1°; letter order along the arc left to right |
| `baseline-arc-ccw-spacing` | same, counter-clockwise | top-line gaps between letters ≥ the bottom-line gaps of the cw case (no overlap at the tops) |
| `baseline-circle` | "AROUND" on a circle rx = ry = 30, start −90° | anchors on the circle; first letter at the top |
| `baseline-fixed-line` | "TOO LONG FOR THIS" in a 50 mm fixed line | letter spacing compressed so the text width is 50 ± 0.5 mm; letters not scaled |
| `baseline-path-equals-fit` | `path` baseline vs today's Fit Text to Outline with the same options | identical output |
| `kerning-pair-saved` | kern "AV" by −0.1 em on an object, save to font, new text "AVA" | the pair applied in the new text; the font's table has the entry |
| `kerning-class` | class left {T, V, W, Y} right {a, e, o} value −0.08 em | "To", "Ye" kerned; "Ta" with an explicit pair uses the pair |
| `kerning-auto` | an embroidery font from stitch files, `font.autoKern` | every sample pair's minimum profile gap equals the side bearing ± 0.1 mm; deterministic |
| `team-names-batch` | a design with name and number texts, a 3-row CSV | three PES files named by the pattern; each name refitted into the original box; the worksheet lists three rows; a 40-character name reported as not fitting at 4 mm |
| `team-names-matrix` | same with `matrix` | one combined file with the three rows on a grid within the hoop |
| `connection-closest-script` | "name" in Pacifico, `connection: closest` | zero trims and zero jumps over 1.5 mm between letters; connectors lie under the next glyph's satin or within the glyph's outline |
| `connection-default-is-today` | the text sewing-order reference spec | identical output (guard) |
| `small-text-preset` | "Hi" at 4 mm | density 5.5; no under sewing; compensation 15 %; no stitch under 0.5 mm except ties |

Measurement helpers needed: "glyph anchors and rotations from the layout" (the layout's
own output), "gaps between consecutive glyph outlines at a height", "jumps and trims between
glyphs" (exists), file-count checks for the batch.

## 9. Compatibility and licensing

`line` baseline, default connection and the Transform presets reproduce today's output
(guards). Bundled fonts stay SIL OFL; kerning overrides for them live in user settings, not
in the font files. Baseline placement by arc length, pair kerning and nearest-point joins
are generic; "Enveloping", "Team Names", "Matrix sequencing", "Visual Kerning",
"AlphaTricks" and "Closest Point Connect" are vendor names; OpenStitch uses "Baseline",
"Team names", "Kerning", "Connection: closest".

## 10. Open questions

- Whether `Arc` should replace the arch Transform presets in the UI (one concept instead of
  two); keep both until the preset list is complete (presets 13 and 14 are still open on the
  compatibility roadmap).
- Team names and the Design Database: the batch could run from the Database window over
  several designs; defer to a later iteration.
