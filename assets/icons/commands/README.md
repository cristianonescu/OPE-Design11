# Command icons

One SVG per ribbon icon, rendered by `crates/openstitch-app/src/icons.rs`. They were generated
for OpenStitch with Higgsfield (Recraft V4.1, `model_type` `utility_vector`, 1:1, 1k, white
background), one request per icon, with the colours listed in `prompts.tsv` and this prompt:

> Chunky flat vector UI icon of {subject}. One single bold silhouette filling 80 percent of the
> square, simple geometric shapes, thick dark charcoal outline, no tiny details, no text, no
> shadow, no gradient, plain white background, readable at 16 pixels.

(The six icons of the approved style sample, folder, disk, scissors, needle, hoop and zoom_in,
used the same words in a slightly different order.)

The 93 icons from `connect` on in `prompts.tsv` were added on 2026-10-30 in two rounds for
commands that showed another command's icon (docs/research/asset-audit.md), with the white
background passed as `background_color` and the colours as `colors`; their Higgsfield job ids
are in `higgsfield-jobs.tsv`. `stitch_factory`, `view_solid` and `import_svg` are second
attempts. In 16 of them (`csv`, `fabric`, `measure`, `new_folder`, `thread_chart`,
`thumbs_small`, `wireless`, `add_to_design`, `attribute_sets`, `image_delete`, `image_html`,
`image_modify`, `image_paste`, `network`, `region_reset`, `stitch_simulator`) the fills the snap
left alone were set to the nearest palette colour by hand.
Editor commands get them through `glyph` in `icons.rs`, the suite windows' commands by their
key through `suite_glyph`.

`tools/clean_icons.py RAW_DIR assets/icons/commands` strips the generator metadata and the
background and snaps fills to the palette. The charcoal outline (`#2b2f36`) is replaced by the
theme's text colour when an icon is drawn. To add an icon: add a row to `prompts.tsv`, generate
it the same way, clean it, list it in `own!` in `icons.rs` and map it in `glyph` or
`suite_glyph` (a test checks that every SVG is used and renders). See docs/LEGAL_AND_LICENSING.md for where the icons come from.
