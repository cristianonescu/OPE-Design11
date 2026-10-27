# Command icons

One SVG per ribbon icon, rendered by `crates/openstitch-app/src/icons.rs`. They were generated
for OpenStitch with Higgsfield (Recraft V4.1, `model_type` `utility_vector`, 1:1, 1k, white
background), one request per icon, with the colours listed in `prompts.tsv` and this prompt:

> Chunky flat vector UI icon of {subject}. One single bold silhouette filling 80 percent of the
> square, simple geometric shapes, thick dark charcoal outline, no tiny details, no text, no
> shadow, no gradient, plain white background, readable at 16 pixels.

(The six icons of the approved style sample, folder, disk, scissors, needle, hoop and zoom_in,
used the same words in a slightly different order.)

`tools/clean_icons.py RAW_DIR assets/icons/commands` strips the generator metadata and the
background and snaps fills to the palette. The charcoal outline (`#2b2f36`) is replaced by the
theme's text colour when an icon is drawn. To add an icon: add a row to `prompts.tsv`, generate
it the same way, clean it and map its glyph in `icons.rs` (a test checks that every SVG is
used and renders). See docs/LEGAL_AND_LICENSING.md for where the icons come from.
