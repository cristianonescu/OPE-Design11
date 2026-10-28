# Text track (phase 2) — analysis notes

Session higgsfield-assets-57536e. Requests T1–T5 are in TRACKS.md ("[text, 2026-10-27]").
Font for every reference: Lato Bold (OFL, bundled by OpenStitch as `lato-bold`), used in PE-DESIGN
as an installed TrueType font. Specs live in `compat/specs/text/`.

## PE UI survey (pe-gui fills this in)

- Text Attributes: size (what it measures?), character spacing, line spacing (unit, range, step,
  default), alignment.
- Fit Text to Outline Attribute Setting: Horizontal Alignment (choices, default), Vertical
  Alignment (choices, default), Text Orientation (choices, default), the other side (control,
  default). What happens with a second text on the same outline. Does Text Transform release it?
- Text Transform: number of presets, order, a one-line description each, further settings.
- User-mapped text: which per-character edits are disabled.

### Survey results (pe-gui, PE-DESIGN 11.50, 2026-10-27; files and log in PE-reference-notes round2 text folder)

- Font: Lato-Bold.ttf installed for the user; PE lists it as "Lato" among the TrueType fonts (Text tab font
  drop-down; recently used fonts first, then Brother's numbered fonts, then TrueType A–Z). TTF Settings dialog
  "TrueType Font Attribute Setting": Font Lato, Font style Bold (only one), Script Western.
- Size: a spin field next to Font Filter on the Text tab, no label or tooltip, default 10.0. At size 20.0 Lato
  Bold 'H' is 17.40 x 20.60 mm and 'Hxg' 49.20 x 25.80 mm, so size is close to the capital height (H 20.6 mm).
- Text Attributes defaults: Kerning 0.0 mm, Vertical Offset 0.0 mm, Rotate Angle 0 degree, Character Spacing
  0.0 mm, Line Spacing 100 "% of char height", Alignment Left (left / centre / right), Direction horizontal
  (horizontal / vertical), Transform off. Text region default Satin Stitch (choices Satin, Fill, Prog. Fill),
  default text colour 209 TANGERINE.
- Line Spacing: % of char height, default 100. Typing 0 is shown as 0; typing stops at three digits (9999 ->
  999); the spin arrows step by 1 and go past 999 (999 -> 1000), so the maximum is at least 1000 (not found).
  Two 10 mm lines of 'H': 26.20 mm tall at 100 %, 34.10 at 150 %, 18.20 at 50 %.
- Multi-line text: Enter on the canvas does not break the line; in the Text Attributes text box Ctrl+Enter
  inserts the break and Enter applies the edit.
- Fit Text to Outline Attribute Setting (Home > Edit > Fit Text > Fit Text to Outline, with a text and an
  outline selected): Horizontal Alignment drop-down of 5 icons: left, CENTRE (default), right, stretched to
  fill the outline (<-ABC->, scales the letters: 8 mm text became 140 x 139 mm round a 60 mm circle),
  spaced evenly round the whole outline (A B C). Vertical Alignment 3 icons: ABOVE the line (default),
  centred on it, below it. Text Orientation 2 icons: TURNED ALONG the outline (default), upright. Check box
  "the other side", default OFF (text on the inside, reading the other way). The dialog remembers the last
  choices. Fit Text > Release Text from Outline undoes it. The circle's start and end point is at 6 o'clock
  (50, 80) mm: left alignment starts the text there (running clockwise); centre centres it at 12 o'clock.
- Second text on the same outline: clicking the outline then selects the outline and its fitted text as one,
  and Fit Text is greyed (no message): PE does not fit a second text to an outline that has one.
- Text Transform on a fitted text: Text Attributes, Transform included, are all greyed while the text is fitted,
  so Transform cannot be applied; the fit is not released (Release Text from Outline first).
- Sewing order inside a text (AB, Fill red + Running outline black): Sewing Order rows 1 = fills (1-1 A, 1-2 B),
  2 = outlines (2-1 A, 2-2 B): all regions first, then all outlines, one colour block each.
- User-mapped text: not surveyed (needs characters registered with Brother's tools or built-in content).
- Text Transform (T4): Text Attributes > Transform check box, default off; ticking it applies preset 3. Palette:
  2 round icons on top, then a 4-column grid of 7 rows: 30 presets, no tooltips. In reading order, 1-12:
  1 arch over the top of a circle (letters radial), 2 arch along the bottom, 3 taper growing to the right,
  4 taper shrinking to the right, 5/6 mild tapers (top edge sloping, mirror pair), 7/8 tapers with a straight
  baseline, 9 slant forward, 10 slant back, 11/12 symmetric tapers growing / shrinking. Files for 1-12 only.
  Below the palette: Name Drop (check box, off), Name Drop List..., Maintain aspect ratio (greyed).

## OpenStitch today (before the text PRs)

- Size = capital height ('H' is `size` mm tall). Line pitch = font line height (ascender −
  descender + line gap) × scale + `lineSpacing` mm.
- Text on a path: along the path by `align` (left/center/right); baseline on the path; letters turn
  with it. Several texts could share one outline.
- Inside a text: every glyph's region first, then every glyph's outline.

## Work in progress

- Branch `text/fit-outline-pe11`: `TextSpec.fit` (`PathFit`: valign bottom/middle/top, upright,
  otherSide), `text.setPath` options, one text per outline (refused, as the guide says), Fit Text
  dropdown (Fit Text to Outline… dialog, Release Text from Outline). Names and defaults are
  provisional until the PE UI survey; spec `text-fit-default` waits for its reference.

- Branch `text/line-spacing-pe11`: `TextSpec.line_spacing_percent` (default 100, % of the font's
  line height, provisional basis; the old mm `lineSpacing` adds on top for old files), engine
  `lineSpacingPercent`, Character Spacing and Line Spacing rows in the Text Attributes pane. Specs
  text-size-h (internal: H = 20.0 mm passes), text-size-hg, text-line-spacing-100/150/50 wait
  for T1/T2.
- Branch `text/sewing-order-pe11`: spec text-sewing-order waits for T5. No code change yet.
- Text Transform: nothing built yet; waits for the preset survey (T4).
- User-mapped text: the guide says "some editing of individual characters" is disabled; which
  ones waits for T5's observation.

## Measurements (text track, 2026-10-27)

- T1/T2 → PR #54 (merged). PE size = 0.7 em: H at 20 is 17.4 x 20.6 mm, Hxg 49.2 x 25.8 (OpenStitch
  before: 16.8 x 20.0, 47.7 x 24.9). H stem 3.8 vs 3.7, gap 9.8 vs 9.4: a 1.03 scale, not an
  overhang. Line pitch at size 10: 7.9 / 15.9 / 23.8 mm at 50/100/150 %, linear; 100 % = OS/2
  usWinAscent (2233/2000 em) on the 0.7-em scale (15.95). OpenStitch now matches all five to 0.1 mm.
  Old .osd (< v3) and templates (< v2) keep `metrics: capHeight`.
- T3 → PR #57. Measured with a new `polar` compat measure about (50, 50), circle ring excluded.
  The circle's outline starts at 6 o'clock (50, 80), clockwise; left starts there, right ends
  there, centre centres at 12. Centred / below drop by size/2 and size (8.0 mm), not the cap
  height (8.26). Spaced: each character centred in an equal share of the outline. All eight
  files match within 1 deg and 0.1 mm. Shape tools now give closed outlines #51's bottom-centre
  line entry too (PE sews the circle's line from the bottom); badge golden re-recorded.
  Stretch: PE's file is off the page; see T4 note, no spec.
- T5 → PR #58. Two colour blocks (all fills, then all outlines), as OpenStitch already did. Fill
  block 1157 vs 1127; outline block 157 vs 114 because PE's running pitch is 2.0 mm with tie-in
  and lock (lines track, #55).
- T4: files 1–12 unreadable. The text started at x = −3 mm, PE saved "anyway" and wrote NO PEC
  section (no "LA:"; the header's PEC offset points at zeros); stitches only in the undocumented
  #PES0110 object data. OpenStitch's PES reader then returns all-zero stitches silently instead of
  an error. Follow-up for pes-io: refuse a PES whose PEC offset has no PEC label. Asked pe-gui to
  remake 1–12 on the page.

## Text Transform (T4, branch text/transform-pe11)

- PE has 26 presets (2 arches + 4 x 6 grid). First set unreadable (off page, no PEC); pe-gui remade
  all 26 on the page. tools/fit_envelopes.py fits each preset's top/bottom edge from the H stems
  (cosine basis, outliers dropped, pairs/symmetric pooled, knots k/32 = outermost nearby sample).
  24 presets laid out (all but fans 13/14); 24 specs pass (bounds and 14 stem columns within 0.5 mm).
  Arches: rigid letters on an arc, inner line keeps the width (1.879 rad), shifted −0.55 % of width.
- FOLLOW-UP (text defaults): pe-gui found PE's text region defaults are its own: Satin Stitch,
  under sewing OFF, density 5.0 (Fill), colour 209 TANGERINE. OpenStitch's text.add uses the shape
  defaults (Fill 4.5 with under sewing on). The T1–T5 specs measure geometry only, so they are not
  affected; T5's fill block (1157 PE vs 1127) would move. Worth its own small PR with a spec.
