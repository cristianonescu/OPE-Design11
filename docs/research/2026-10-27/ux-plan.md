# UX/graphics pass with Higgsfield assets — plan (session higgsfield-assets-57536e)

Owner: this session (branch claude/higgsfield-assets-57536e, one PR for v0.1.0-alpha.2).
Brand session owns logo, window/packaging icons, splash, About, Application button — not touched.

## Survey: weakest visual / UX spots (code + docs/images tour)

| # | Spot | Today | Impact |
|---|---|---|---|
| 1 | Empty canvas (new design, no file) | blank white page; only hint is a weak text line in Sewing Order | high: first thing a new user sees |
| 2 | Design Database empty states | "Choose a folder on the left." / "No embroidery designs here." plain centred text | high |
| 3 | Database thumbnails while loading / unreadable | blank white square while loading, a "⚠" glyph when unreadable | medium-high |
| 4 | Pattern wizard dialogs (Background Fill, Appliqué, Patch, Cutwork) | text-only dialogs, nothing shows what the wizard makes | medium-high |
| 5 | Image to Embroidery dialog | text-only | medium |
| 6 | Realistic view | stitches drawn on a flat white page; no fabric, so "realistic" reads flat | high |
| 7 | Programmable Stitch Creator library empty ("No saved patterns yet.") | weak text | medium |
| 8 | Name Drop window with no Name Drop text | weak text | low-medium |
| 9 | Start/welcome panel with recent files + templates | none (recent files only in the File menu) | high, but changes the window layout |
| 10 | Stitch Design Factory / thread chart thumbnails | text lists | medium, needs per-item art or rendered previews |
| 11 | Realistic thread texture (per-stitch sheen bitmap) | 3-stroke shading in code | medium, render-path change |

## Chosen for this PR (bounded, layout and command arrangement unchanged)

1. Empty canvas illustration + short hint (drawn on the Design Page only while the design has
   no objects and no background image; disappears with the first object).
2. Design Database empty states: illustrations for "choose a folder" and "empty folder" and
   "no search results".
3. Database thumbnail placeholders: a faded spool illustration while a file loads, a
   "can't read" illustration instead of the ⚠ glyph.
4. Header art for the four pattern wizards: a small illustration left of the selection line.
5. Header art for Image to Embroidery.
6. Realistic view: a small tileable woven-fabric PNG under the stitches, on the Design Page.
7. Empty states with illustrations in the Programmable Stitch Creator library and Name Drop.

All illustrations: Recraft V4.1 utility_vector via Higgsfield, the same palette and charcoal
outline as the command icons (outline recoloured to the theme text colour at draw time, so they
work in light and dark). Rendered through the existing resvg path in icons.rs. Code fallback:
when an SVG fails, the text is shown alone (as before).

## Phase 2 (not in this PR)

- Start/welcome panel (recent files with thumbnails, templates, New/Open) — needs a layout
  decision against PE-DESIGN 11's behaviour (it opens straight into a new design).
- Thumbnails in the Stitch Design Factory and user thread charts (rendered previews, not art).
- Realistic view thread texture as a bitmap brush, plus a fabric choice tied to the Fabric
  Selector (cotton, denim, felt, towel).
- Header art for Template and Monogram windows; illustrated tooltips for wizards.
- Loading spinners for long commands (Auto Punch, Photo Stitch).
