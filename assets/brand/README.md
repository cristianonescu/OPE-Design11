# OpenStitch logo

The mark is a needle with its thread: the thread sweeps round the needle in a wide C, closes
into a small o-loop and ends in a running stitch. It reads first as needle and thread; the C
and the o are only hinted. The lettering is "OpenStitch" in rounded strokes with a coral i.

| File | Use |
|---|---|
| `../icons/openstitch.svg` | App icon master (mark on the navy tile); `tools/gen_icons.py` writes the PNG, ICO, ICNS and RGBA files beside it |
| `openstitch-mark.svg` | The mark without the tile (Application button) |
| `openstitch-wordmark.svg`, `.png` | Icon and lettering, for light backgrounds (README, About, splash) |
| `openstitch-wordmark-dark.svg`, `.png` | The same with cream lettering, for dark backgrounds |

Palette: navy `#1f3a5f`, coral `#e2445c`, cream `#f6f1e7`, tile edge `#3a5f8f`.

Both parts were generated for OpenStitch with Higgsfield (Recraft V4.1, `model_type` `vector`,
1:1 and 16:9, 1k, white background, palette as above), one request each, with no reference
images. The mark:

> Original flat vector app logo mark for an embroidery design program: a slim cream sewing
> needle drawn diagonally, its coral red thread sweeping in an open curve that subtly suggests
> a letter C, then making one small closed round loop that subtly suggests a letter O, and
> finishing as a short line of dashed running stitches. The letters are only hinted by the
> thread path, not written; it reads first as a needle and thread. Bold simple shapes, even
> thick thread, centered on a rounded square deep navy tile, high contrast, readable at 16
> pixels, no text, no gradient, no shadow, plain white background.

The lettering:

> Wordmark lettering only: the single word "OpenStitch" set in a clean, friendly, geometric
> sans-serif, medium-bold weight, even letter spacing, "Open" in deep navy and "Stitch" in deep
> navy, the dot of the letter i replaced by a small coral red dot. Flat vector, one line of
> text centered, no symbol, no icon, no other text, no shadow, no gradient, plain white
> background.

`tools/clean_logo.py MARK.svg LETTERING.svg` turns the two generated SVGs into the files above:
it removes the generator metadata, the background and the generated tile, makes the needle's
eye and the letter counters holes, centres the mark on a new tile and lays out the wordmark.
The generated shapes themselves are unchanged. Then run `tools/gen_icons.py`. See
docs/LEGAL_AND_LICENSING.md for where the logo comes from.
