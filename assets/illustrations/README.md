# Illustrations

Spot illustrations for empty states, thumbnail placeholders and the wizard dialogs, drawn by
`crates/openstitch-app/src/art.rs` with the same renderer and palette as the command icons
(`assets/icons/commands`). They were generated for OpenStitch with Higgsfield (Recraft V4.1,
`model_type` `utility_vector`, 1:1, 1k, white background), one request per illustration, with
the colours listed in `prompts.tsv` and this prompt:

> Chunky flat vector spot illustration of {subject}. A few bold simple geometric shapes centred
> in the square, filling 70 percent of it, thick dark charcoal outline, flat fills, no tiny
> details, no text, no letters, no shadow, no gradient, plain white background, readable at 64
> pixels.

`tools/clean_icons.py RAW_DIR assets/illustrations` cleans them; five off-palette greens and
blues the snap left alone were then set to the palette green `#2e9d58` and blue `#2a7ad8` by
hand. As with the icons, the charcoal outline (`#2b2f36`) is drawn in the theme's text colour,
so they read on the light and the dark theme. A test in `art.rs` checks that every SVG renders
and is used. See docs/LEGAL_AND_LICENSING.md for where they come from.

`sew_empty` (Color pane with nothing sewn), `no_media` (Send to USB Media with no stick or card),
`no_image` (Modify Image without a background image), `objects_empty` (the objects list of an
empty design) and `update_available`, `update_current`, `update_failed` (the update dialog) were
added on 2026-10-30 the same way; in `update_available` and `update_current` three fills were
set to the palette green and blue by hand. Job ids are in `higgsfield-jobs.tsv`.

## Textures

`../textures/fabric_cotton.png` (192 × 192, greyscale) is the cloth under the stitches in the
Realistic view. It was generated with Higgsfield (Recraft V4.1, `model_type` `utility`, 1:1, 1k)
from the prompt "Flat top-down macro photo texture of plain-weave natural cotton fabric, fine
even crosshatch of warp and weft threads, uniform soft lighting, very light warm off-white
colour, no folds, no wrinkles, no shadows, no vignette, seamless tileable texture filling the
whole frame", then cropped to one repeat of the weave (382 × 383 px), cross-faded over a 28 px
seam so it tiles, scaled down and levelled to a light grey.
