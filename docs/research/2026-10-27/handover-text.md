# Text track handover (session higgsfield-assets-57536e, 2026-10-27)

Worktree `<worktree>`. Details: text-analysis.md.

## Landed / in the queue
- #54 merged: size = 0.7 em, line spacing "% of char height" (100 % = OS/2 usWinAscent). `.osd` v3 and
  `.ostemplate` v2: older text keeps `metrics: capHeight`. Specs text-size-h/-hg, text-line-spacing-50/100/150.
- #57 merged: Fit Text to Outline dialog (5 H-aligns, above/centred/below by size, upright, other side),
  Release, one text per outline; shape outlines start at #51's bottom centre. compat `polar` measure. 8 text-fit-* specs.
- #58 merged: text-sewing-order (PE also sews all fills, then all outlines). No code change.
- #61 (branch text/transform-pe11, 0a0c6ec, rebased on e3caf47): Text Transform, 24 of 26 presets.
  `TextSpec.envelope`, `tools/fit_envelopes.py` → `crates/openstitch-design/src/envelope_presets.rs`,
  compat `columns` measure, 24 text-transform-N specs, palette in the Text Attributes pane.
- #64 (branch text/defaults-pe11, df45631, on bf07fe8): PE text defaults (Satin, density 5.0, under sewing off;
  Fill 5.0, no under sewing, Frequency 30 %). Specs text-defaults-h/-fill. Needs one rebase after #61
  (MANIFEST, STATUS, COMPATIBILITY_TESTS; keep both sides' entries, regenerate STATUS with `stitchtool
  compat --status docs/COMPATIBILITY_STATUS.md`). After #61 lands, #64's switch to Satin is already covered:
  #61's transform specs set `type: fill` explicitly. The coordinator pings for the rebase.

## Left
1. Text Transform presets 13/14 (fan out / in): PE skews each letter differently; not laid out, palette
   buttons disabled. References exist: compat/reference/text/text-transform-13/14 are NOT in the repo yet;
   copies in <reference-notes>\round2\text\ (MANIFEST-text.json has their SHA and box).
   Next: model each letter's top and bottom edge by its corners (not columns), add to the table, add specs.
2. User-mapped text: the guide disables "some editing of individual characters"; which ones is unknown and
   needs characters registered with Brother's tools. Open audit item; no code.
3. Text Satin stitch count: text-defaults-h 221 vs PE's 285 (box exact). Satin structure → satin track.
   Text Fill 544 vs 483 before #59's row joining; re-measure after #64 lands (expected to drop).
4. pes-io (TRACKS.md Cross-track): a PES without a PEC section (PE omits it for off-page designs) reads as
   zero stitches; should error. Example: round2\text\text-fit-halign-stretch.pes.
5. "Stretched" Fit alignment has no spec (PE's file is off the page; see 4).

## Files and references
- Code: crates/openstitch-design/src/text.rs (metrics, fit, envelope), envelope_presets.rs (generated),
  model.rs `path_contour`; engine commands.rs (`text_params`, `text.setPath`, `text_region`), lettering.rs;
  app lettering_ui.rs (Fit dialog), panes.rs (Text Attributes: spacing rows, Transform palette), ribbon.rs.
- compat: specs/text/*, reference/text/* (PE-DESIGN 11.50, Lato Bold, made by pe-gui), measures `polar`,
  `columns`. Source files + MANIFEST-text.json: <reference-notes>\round2\text\.
- Scratch helper (not in repo): scratchpad addref.py copies a round2 file into compat/reference with its
  MANIFEST entry.

## Decisions pending
- None for the owner. Owner of the fan presets and the user-mapped check: next text session.
