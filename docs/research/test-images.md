# Test images for the image features

A library of test images for Auto Punch, Photo Stitch 1 and 2, Cross Stitch and Design
Center (#127, #128, #129, #132). They cover every kind of image a user may bring, from photos
to logos to pixel art.

Each image is run through both PE-DESIGN 11.50, by the reference session, and OpenStitch:
- PE-DESIGN's output stays in the owner's notes folder, never in the repository.
- OpenStitch's output is compared with it using `tools/screencmp.py` and the digitize measures.
- OpenStitch's results, next to the sources, feed the README's "From image to embroidery"
  gallery.

## Provenance and rules

- **Source:** every image is generated for OpenStitch with Higgsfield (GPT Image 2.5, medium
  quality, 2026-10-30), at the owner's request.
- **Derived variants:** a few are made from those images in code (JPEG artefacts, noise, size).
- **Content:** original only. There are no real people's likenesses, brands, logos or
  copyrighted characters, and nothing that resembles Brother's or PE-DESIGN's samples. Portraits
  show invented people.
- **Record:** `examples/images/PROVENANCE.md` lists each file's prompt, model, settings, date
  and Higgsfield job id. The model takes no seed, so the job id identifies the generation.
  docs/LEGAL_AND_LICENSING.md records the library.
- **Size:** images are stored at most 768 px on the long side, as JPEG (photos, soft
  images) or palette PNG (flat graphics, transparency). Line art is greyscale PNG, and the
  pixel art is scaled with nearest neighbour. The large case `p06-sunset-large.jpg`
  (2688 × 1520) is a separate 2k generation of the same scene. The library is 4.4 MB.
- **Files:** each image's file name starts with its id (`p01-portrait.jpg`,
  `g07-pixel-strawberry.png`); the full list is in `examples/images/PROVENANCE.md`.

## The set

Columns: the image, its type, the shape (aspect), the wizards it exercises and their settings,
and what it verifies or measures. Every wizard runs at its defaults unless a setting is named.

### Photos

| Id | Image | Aspect | Wizards and settings | Verifies / measures |
|---|---|---|---|---|
| p01 | Portrait, invented adult, warm brown skin, window light | 2:3 | Photo Stitch 1 colour; Photo Stitch 2 Auto Select | Skin tones: colour error per region, threads chosen, stitches, trims |
| p02 | Black-and-white portrait, invented older man, side light | 3:4 | Photo Stitch 1 mono; Photo Stitch 2 mono | #128 mono tones: needle density per grey band |
| p03 | Children's-book gouache: fox and rabbit picnic | 4:3 | Auto Punch; Photo Stitch 1 colour | Soft illustration: regions, satin/fill, colour order |
| p04 | Long-haired ginger cat, fur close-up | 1:1 | Photo Stitch 1 colour; Photo Stitch 2 Auto | Fine texture: density, trims |
| p05 | Black-and-white dog on grass | 4:5 | Photo Stitch 1 colour and mono | Mixed hard and soft contrast |
| p06 | Landscape: sunset sky gradient over hills and a lake | 16:9 | Photo Stitch 1 colour; Photo Stitch 2 Auto; also a 2k copy | Smooth gradients, banding; a large image |
| p07 | Bowl of fruit on a wooden table | 1:1 | Photo Stitch 1 colour; Photo Stitch 2 Auto | Saturated food colours; Auto Select's four threads (#129) |
| p08 | Red poppies, macro | 1:1 | Photo Stitch 2 CMYK, RGBK, CRYK, BMYK | Colour Set mixing (#129) |
| p09 | Modern facade with strong lines, blue sky | 3:4 | Photo Stitch 2 Auto; Design Center | Straight lines, angle aliasing |
| p10 | City street at night, warm lamps, no signs | 16:9 | Photo Stitch 1 colour | Low light: dark-tone handling |
| p11 | White teacup on a white cloth, high key | 1:1 | Photo Stitch 1 colour and mono | Near-white tones: page colour, light cut-off |
| p12 | Backlit tree silhouette against a bright sky | 16:9 | Photo Stitch 1 mono; Auto Punch | High contrast; large dark regions |

### Graphics

| Id | Image | Aspect | Wizards and settings | Verifies / measures |
|---|---|---|---|---|
| g01 | Flat emblem: mountain and sun in a circle, four colours | 1:1 | Auto Punch; Cross Stitch; small copy (128 px) | Region count, satin/fill, colour order and picks (#127); small input |
| g02 | Flat illustration: hot-air balloon over hills, six colours | 4:3 | Auto Punch; Cross Stitch | Colour reduction, region order |
| g03 | Ink line art: a teapot | 1:1 | Design Center; Auto Punch | Thin strokes: satin bounds; line tracing |
| g04 | Colouring-page butterfly, bold closed outlines | 1:1 | Design Center | Figure regions between lines |
| g05 | Original cartoon robot, bold outlines, flat colours | 1:1 | Auto Punch; Cross Stitch | Outlines next to fills |
| g06 | Watercolour lemons, soft edges, paper texture | 2:3 | Auto Punch; Photo Stitch 1 colour | Noisy edges: small-region cleanup (#127) |
| g07 | 16-colour pixel-art strawberry, large pixels | 1:1 | Cross Stitch; Auto Punch | Grid alignment (#132), exact flat colours |
| g08 | Bold two-colour lettering "STITCH" | 3:2 | Auto Punch | Text strokes: satin across letters |
| g09 | Smooth abstract multi-colour gradient | 16:9 | Photo Stitch 2 Auto, CMYK and RGBK; Photo Stitch 1 colour | Mixing law per hue (#129); banding |
| g10 | Leaf sticker on a transparent background | 1:1 | Auto Punch; Photo Stitch 1 | Transparency: no background region |
| g11 | Fine mandala, thin black lines | 1:1 | Auto Punch; Design Center | Tiny detail: speck removal, thin-stroke handling (#127) |
| g12 | Monochrome stencil deer silhouette | 1:1 | Auto Punch; Cross Stitch | Two colours: background omitted |
| g13 | Geometric mosaic, twelve distinct flat colours | 1:1 | Auto Punch (all colours kept); Photo Stitch 2 Auto | Thread picks per colour (#127), Auto Select (#129) |
| g14 | Two-colour poster: orange circle on navy | 1:1 | Photo Stitch 2 Auto; Auto Punch | Auto Select on two colours (#129: does it still pick four?) |
| g15 | Folk-art bird motif, flat colours | 1:1 | Cross Stitch; Auto Punch | Cross Stitch cells and colours (#132) |

### Derived variants (made in code, no extra credits)

| Id | From | Change | Verifies |
|---|---|---|---|
| d01 | p07 | Heavy JPEG artefacts (quality 15) and Gaussian noise | Noise and artefact robustness: regions, specks |
| d02 | g01 | 128 px small copy | Small input: upscaling and stroke widths |

d01 adds Gaussian noise (σ 14 per channel, seed 7) to the 768 px p07 and saves it at JPEG
quality 15. d02 is g01 scaled to 128 × 128 with Lanczos.

## Gallery

`tools/gallery.py` converts 14 of the images, chosen to cover each wizard and image type, at
the settings it names. It opens each result in the editor's Realistic view and rewrites the
README's "From image to embroidery" table. It is part of the release checklist
(docs/RELEASING.md). Once the improved "OpenStitch" mode exists, the gallery shows it next to
the PE-compatible result.

## Findings

- **Census exhaustion on anti-aliased images (2026-10-30).** Auto Punch at defaults on g01 sews
  the emblem's orange as Silver and its yellow as Lavender (3 threads instead of 4). The 768 px
  image has 5308 distinct colours at the 400 px working resolution, mostly edge shades. The
  100-colour census, filled first met first, is full by row 31, before the sun's orange and
  yellow are reached. Those colours then count under their nearest tracked shade (greys and
  lavenders). The same happens on other generated flat graphics (the strawberry's yellow
  seeds in Cross Stitch). **PE-DESIGN differs** (round-5 capture, defaults, 100 mm): it
  fills in 4 colours and keeps orange and yellow (Brother 001 White, 206, 209 Tangerine, 007
  Prussian Blue; 11 342 stitches, the white background left out, the white rays sewn). So
  PE's census does not fill up with edge shades. The fix is its own PR.
- **Small input (d02, round 5).** On the 128 px emblem, scaled up, PE fills in 10 colours,
  and the anti-aliased edge shades become regions of their own (9 Brother threads). PE
  behaves like OpenStitch there: edge shades count once they are large enough after
  upscaling.

## PE-DESIGN round 5, priority 1 (2026-10-30)

The reference session's captures are in the owner's notes folder (`round5\images`), never in
the repository. What they settle:

| Image, wizard | PE-DESIGN 11.50 |
|---|---|
| g13 mosaic, Auto Punch | 10 automatic colours for 12 flat colours |
| g13 mosaic, Photo Stitch 2 Auto Select | Four Ver4 threads: 89, 90, 66, 70 |
| g14 two colours, Photo Stitch 2 Auto Select | Still four threads: 66, 90, 89, 75 (#129) |
| g14 two colours, Auto Punch | 2 automatic colours; under the Brother chart the background is left out and only the circle is sewn (one thread, 5096 stitches) |
| g09 gradient, Photo Stitch 2 | Auto Select 70, 89, 76, 90. CMYK is C 70, M 90, Y 89, K 75. RGBK is R 84, G 71, B 67, K 75 |
| p08 poppies, Photo Stitch 2 | The same set threads. CRYK is 70, 84, 89, 75; BMYK is 67, 90, 89, 75 |
| p02 mono portrait | Photo Stitch 1 and 2 mono both default to one thread, Ver4 75; 10 949 and 19 812 stitches |
| g07 pixel art, Cross Stitch | 2.5 mm cells, 8 automatic colours with the background left out, 7 threads, 4158 stitches. The grid does not follow the pixel blocks |

Priority 2 (36 of 40; the four Design Center captures are missing):
- **g10, transparency.** PE shows the transparent area as black. Auto Punch leaves it out,
  but Photo Stitch 1 Color sews it in Black 900, because Sew Page Color is on by default.
- **Photo Stitch 1 Color defaults:** the Brother chart, Auto Select and 10 colours.
- **g11, thin lines.** Auto Punch breaks the mandala's thin lines into fragments.
- **Photo Stitch 1 Color defaults, in full:** the Brother Embroidery chart, Auto Select on, 10
  colours, Detail middle, Run Pitch 3.0 mm, Sew Page Color on.
- **Large input (p06-sunset-large, 2688 px).** PE gives no downsampling notice. Auto Select
  picks a different set from the 768 px copy, while the stitch count stays about the same
  (10 530 against 10 599).
- **Noise (d01).** Photo Stitch 1's Auto Select is identical to the clean p07, and its stitch
  count is +0.6 %. In Auto Punch the JPEG blocks make ragged region edges.
- **Mono.** Photo Stitch 1 and 2 mono always use one thread (Ver4 75). The high-key p11 sews
  only 2684 stitches.
- **Cross Stitch.** On g15 and g12 PE stops responding for 30–60 s, then recovers.
- **Photo Stitch 2 Auto Select.** It always picks four Ver4 threads, from a small recurring
  set: maroon 75, pink-red 90, lime 89, and a blue, teal or cyan.
- **Design Center:** not captured. Windows SmartScreen blocks it, and PE then reports that it
  cannot find Design Center. This is the owner's to clear.

PE also refuses a result that does not fit the Design Page ("keep all designs within the
Design Page"), so these captures use a 180 × 130 mm page.

## Measures

| Wizard | Measures |
|---|---|
| Auto Punch | Region count; satin or fill per region and direction; threads and colour order; stitch count; trims; colour error per region (mean ΔE between the source and the thread) |
| Photo Stitch 1 | Stitch count; trims; needle points per cm² per tone band or colour area |
| Photo Stitch 2 | Per thread: line angle, spacing, needle density per colour area (`tools/screencmp.py`); threads and order |
| Cross Stitch | Cells per colour; stitches; threads |
| Design Center | Line count; region count; stitches |

The same measures judge the improved "OpenStitch" mode
(docs/research/beyond/image-digitizing.md) against PE-DESIGN and against OpenStitch's
PE-compatible mode.
