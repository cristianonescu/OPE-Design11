# B16. Image digitizing beyond PE-DESIGN: an "OpenStitch" mode

Research date: 2026-10-31. Requested by the owner, who called PE-DESIGN's image wizards
"super ultra outdated". This note asks how the wizards could give better results. It keeps
PE-DESIGN's behaviour as the default ("PE-DESIGN" mode) and adds an improved "OpenStitch"
mode. Each method is judged by measurement on the test images in `examples/images`
([../test-images.md](../test-images.md)). No other product's code was read.

## 1. Summary

- **Recommended first:** perceptual colour reduction (3.1) and region cleanup (3.2), together
  as Auto Punch's OpenStitch mode (`compatMode: openStitch`; the name is shared with the
  region attribute of the same name for sewing modes). On nine of the ten Auto Punch test
  images it:
  - lowers the mean colour error from 9.1 to 8.7 ΔE00;
  - sews 20 % fewer stitches and 52 % fewer trims;
  - leaves no specks (220 before).

  It costs about 300 lines, needs no dependency, and carries no known patent risk.
- **Not proposed now:**
  - **Cross Stitch:** perceptual matching makes it slightly worse (+0.4 ΔE).
  - **Photo Stitch 1 and 2:** need their own palette work (3.6).
  - **Stitch direction from image structure (3.5):** close to Brother's US 7,996,103.
  - **Machine learning (3.7):** model licences and size.
- **The limit is the thread chart.** The mosaic's brown has no Brother Embroidery thread
  nearer than ΔE00 19, and its cyan none nearer than 15. Better reduction cannot cross that
  floor; other brands' charts could (B13's thread-brand conversion).

## 2. How the methods are judged

`tools/image_eval.py` converts each test image with `stitchtool`:
- The Design Page is the image's size, with the long side 100 mm.
- The objects are converted to stitches and drawn as 0.4 mm thread lines on white, at the
  image's own pixel scale.
- The image and the drawing are compared by CIEDE2000 over 2 mm cells.

The measures:

| Measure | What it shows |
|---|---|
| **ΔE** | Mean colour error over every cell. A background the wizard leaves out counts as fabric against the image. |
| **ΔE sewn** | Mean over the cells at least 30 % covered by thread: the error of what is sewn. |
| **Stitches**, **trims** | Sewing time and thread cuts, from `design.info`. |
| **Specks** | Objects whose stitches cover less than 2 mm². Each costs a jump, a trim and a tie-in for a dot nobody sees. |

The ten Auto Punch images run at defaults, with the automatic colour count. The OpenStitch
column is the same command with `compatMode: openStitch`.

## 2.1 Results

| Image | ΔE | ΔE sewn | Stitches | Trims | Specks |
|---|---|---|---|---|---|
| g01 emblem | 4.3 → 4.1 | 8.3 → 7.9 | 8,690 → 7,935 (−9 %) | 10 → 10 | 0 → 0 |
| g02 balloon | 15.1 → 14.2 | 8.8 → 6.7 | 7,923 → 7,632 (−4 %) | 12 → 11 | 1 → 0 |
| g05 robot | 5.2 → 4.2 | 12.2 → 10.1 | 11,903 → 8,223 (−31 %) | 231 → 162 | 11 → 0 |
| g06 watercolour | 5.1 → 4.7 | 7.7 → 6.9 | 18,291 → 12,998 (−29 %) | 284 → 109 | 36 → 0 |
| g08 lettering | 7.8 → 8.1 | 8.7 → 10.8 | 1,981 → 1,552 (−22 %) | 16 → 8 | 5 → 0 |
| g11 mandala | 4.5 → 4.9 | 6.8 → 4.5 | 2,590 → 211 (−92 %) | 99 → 10 | 19 → 6 |
| g13 mosaic | 12.8 → 11.7 | 12.8 → 11.7 | 24,299 → 23,517 (−3 %) | 68 → 59 | 6 → 0 |
| g15 folk bird | 8.6 → 9.3 | 8.8 → 9.1 | 9,945 → 8,923 (−10 %) | 63 → 59 | 0 → 0 |
| p03 storybook | 10.9 → 10.2 | 10.9 → 10.2 | 41,665 → 30,124 (−28 %) | 701 → 280 | 101 → 0 |
| d01 noisy fruit | 11.8 → 11.7 | 11.8 → 11.6 | 40,563 → 31,300 (−23 %) | 324 → 130 | 60 → 0 |

Each cell reads PE-DESIGN mode → OpenStitch mode.

Reading it:
- **Painted and noisy images gain the most** (p03, g06, d01, g05): trims fall by 30–62 %,
  stitches by 23–31 %, and the colour error falls.
- **Flat graphics gain colour accuracy** where RGB distance picked a dull thread: g02's
  dark-green trees, the mosaic's greens and browns, the robot's teal.
- **The lettering (g08) and folk bird (g15) lose 0.3–0.7 ΔE.** k-means moves a small accent
  colour towards its larger neighbours, and the anti-aliased letter edges merge.
- **The mandala (g11), thin line art, lost its lines in the first version.** The light-grey
  line pixels matched the white thread, which is the background, and the 3 mm² cleanup
  merged the line pieces. Fixed 2026-10-31 (section 2.2).

Ablation:
- **Perceptual matching of the median-cut colours alone:** robot 5.2 → 4.2, watercolour
  5.1 → 4.6.
- **Adding k-means:** watercolour 4.6 → 4.55, storybook 10.1 → 9.7.
- **Matching pixels to the thread colours instead of the centres:** worse on photos
  (storybook 12.8), so it was dropped.
- **Cleanup at 3 mm² with one majority pass:** removes every speck and halves the trims, at
  a cost of 0.3–0.7 ΔE when used alone.
- **6 mm² with two passes:** costs more colour (storybook 12.1) for few further trims, so
  3 mm² was kept.

## 2.2 Line art (2026-10-31)

The mandala sewed 211 stitches in OpenStitch mode, against PE-DESIGN mode's 2,590. Three
changes, each measured, bring it back to 2,195 stitches at ΔE 4.6 (PE-DESIGN mode 4.5):

- **Background by colour, not by thread.** OpenStitch mode leaves out the background
  colour's own pixels, not every colour that shares its thread. The mandala's light-grey
  edge shades (centre 214, 9.7 ΔE00 from white) share the white thread with the background.
  They were left out with it, and now stay.
- **Thin pieces stay on line art.** When the left-out background covers at least 75 % of the
  image, a region under the minimum area stays if its mean width (area over its longest side)
  is at most 0.5 mm and it is at least twice as long. Lines and their dots stay; compact
  specks still merge.
  - Applying the rule to every image undid the speck cleanup on photos: the storybook went
    back to 603 trims and 345 specks, because paintings are full of thin edge slivers. So it
    applies to line art only.
- **No majority pass.** The pass erased 1 px lines and cost the lettering 0.6 ΔE (8.1 →
  7.6). Without it, OpenStitch mode is the perceptual palette and the 3 mm² cleanup.

The nine other images, against the first version:

| Version | Mean ΔE | Stitches vs PE mode | Trims vs PE mode | Specks |
|---|---|---|---|---|
| First version | 8.7 | −20 % | −52 % | 0 |
| With these changes | 8.6 | −14 % | −48 % | 10 |

The ten specks are the lettering's edge slivers (its cream background is over 75 %). The
added stitches come from dropping the majority pass.

## 3. Methods

For each method: what it would improve, its cost, the Rust crates, the licence, and the
patent risk. The risks are checked against [patents.md](patents.md), which is not legal
advice. Crate licences are as their crates.io pages state; check them again before adopting
one.

### 3.1 Perceptual colour reduction (CIELAB or OKLab, k-means, CIEDE2000)

- **Today:**
  - **Palette:** PE mode median-cuts the census in RGB.
  - **Thread match:** each box goes to the nearest PEC thread by equal-weight RGB distance (#119).
  - **Classification:** pixels go to the nearest box mean, also in RGB.
  - **Effect:** RGB distance treats a dull olive as close to a brown, and a dark grey as close to a dark green. That is why the balloon's dark-green trees sew grey and the mosaic's brown sews olive.
- **Method:**
  1. **Seeds:** median-cut boxes seed a weighted k-means (Lloyd) in CIELAB.
  2. **Matching:** each centre takes the thread nearest it by CIEDE2000.
  3. **Classification:** pixels go to the chosen threads' own colours, also by CIEDE2000. A region is sewn in the thread that looks closest to it, not the thread nearest a box mean.
  - **OKLab** is a simpler perceptual space with similar results for quantisation. CIELAB is used because CIEDE2000 is defined on it.
- **Cost:**
  - **Runtime:** the census holds at most 100 colours, so k-means runs over 100 points. Classification is one CIEDE2000 per pixel and thread, about 1.6 M evaluations at 400 px and 10 threads, a few milliseconds.
  - **Code:** about 200 lines, no dependency.
- **Crates:**
  - `palette` (MIT/Apache-2.0) has the colour spaces and CIEDE2000.
  - `kmeans_colors` (MIT/Apache-2.0) has k-means.
  - The code is small enough to own (`perceptual.rs`, written from the published formulas: CIE 15:2004; Sharma, Wu and Dalal, 2005).
- **Patent risk:** none known. Colour quantisation and colour-difference formulas are textbook. No patent in patents.md claims palette selection.

### 3.2 Small-region cleanup and smoothing

- **Today:** regions of at most `max(4, round(minArea / mmPerPx²))` pixels merge into the neighbour they share the longest border with (PE's rule, #127). minArea defaults to 1 mm².
- **Method:** a larger floor in mm² (a fill smaller than about 3 mm² sews as a blob of jumps and ties), with one or two 3×3 majority passes on the labels before the merge.
- **Cost:** none; both are settings of the PE pipeline (`minArea`, `smooth`).
- **Crates:** none.
- **Patent risk:** none known. The expired US 8,532,810 (Vistaprint) covers fragmenting and ordering auto-digitized regions, not merging small ones.

### 3.3 SLIC superpixels and mean-shift segmentation

- **Method:** segment the image into compact, colour-coherent superpixels (SLIC) or modes (mean shift) before colour reduction, so regions follow edges rather than noisy pixel labels.
- **Benefit:** cleaner region outlines on photos and watercolours, fewer specks.
- **Cost:**
  - **Runtime:** SLIC is linear in pixels, about 10 iterations over a 400 px raster. Mean shift is quadratic without a spatial index.
  - **Code:** about 300 lines.
- **Crates:** `simple_clustering` (MIT/Apache-2.0) implements SLIC and SNIC. Mean shift has no maintained pure-Rust crate.
- **Patent risk:** none known for SLIC (Achanta et al., 2012; published and widely implemented).

### 3.4 Skeleton (medial-axis) satin

- **Method:** for a stroke-like region, take its medial axis and sew a satin column along it, with the width from the distance transform. This is instead of today's choice of one Auto direction per region.
- **Benefit:** curved letters and lines sew as one column that turns with the stroke.
- **Cost:** a skeleton (Zhang–Suen thinning or a distance-transform ridge), a graph of branches, and a column per branch. About 600 lines, plus the `column` code that Manual Punch already has.
- **Crates:** `imageproc` (MIT) has a distance transform. Thinning is easy to own.
- **Patent risk:**
  - US 6,934,599 (VSM, TrueType to satin) is expired.
  - The survey's earlier label for US 8,532,810 as "skeleton satin" was wrong (patents.md).
  - No live patent on the list claims a medial-axis satin.

### 3.5 Stitch direction from image structure

- **Method:** turn fill rows or Photo Stitch lines to follow the image's local gradient or structure tensor, for example along fur or petals.
- **Patent risk: high.** US 7,996,103 (Brother, active until 2030-03-27) claims per-pixel angle features from the image, segments made from them, and a three-test connector decision. A per-pixel direction field is the claim's first element. Combined with segments and any connector rule based on coverage, colour or angle, it comes close to the claim.
- **Decision:**
  - Not proposed.
  - The owner's 2026-10-30 decision covers only Photo Stitch 2's fixed angles. Any image-derived direction needs the question put to the owner again (patents.md).
  - A per-region direction from the region's own shape (the principal axis Auto Punch already uses for satin) is not per pixel and is unchanged.

### 3.6 Error diffusion and thread-aware palettes for Photo Stitch 1

- **Today:**
  - **Palette:** Photo Stitch 1 already median-cuts its palette.
  - **Dithering:** it dithers cells with Floyd–Steinberg over the palette (`palette::dither`).
- **Method:** pick the palette in CIELAB (3.1) and diffuse the error in CIELAB, so dithering mixes threads by appearance rather than by RGB.
- **Benefit:** expected on skin tones and gradients.
- **Cost:** small; it reuses 3.1.
- **Patent risk:**
  - Error diffusion is long public (Floyd and Steinberg, 1976).
  - Photo Stitch 1 keeps its fixed row direction, so it does not touch US 7,996,103's direction field.

### 3.7 Offline machine learning in Rust

- **Method:** a small segmentation or matting network (for example a U-Net or a salient-object model) run on the CPU, to separate the subject from the background before Auto Punch.
- **Benefit:** background removal on photos (the portrait, the pets) without the user drawing a mask.
- **Cost:**
  - **Runtime:** `tract` (MIT/Apache-2.0) or `ort` (MIT/Apache-2.0, needs ONNX Runtime's native library) can run ONNX models offline.
  - **Size:** a model adds 5–50 MB to the download.
  - **Licence:** the weights' licence must allow redistribution, which rules out many research checkpoints (for example non-commercial U²-Net variants).
- **Patent risk:** none known on the list. The licence and size risks are larger.
- **Decision:** later, after the non-ML methods; it needs a model with a redistributable licence.

## 4. The mode switch

- **Parameter:** `compatMode` on `digitize.autoPunch`, values `peDesign` (the default) and
  `openStitch`. It is the same name and values as the region attribute for sewing modes.
- **Image dialog:** a "Mode" choice, PE-DESIGN or OpenStitch, with a tooltip that says what
  changes.
- **PE-DESIGN mode stays exactly as it is**, so every compat spec and PE capture comparison
  is unaffected.
- **OpenStitch mode changes three things:**
  - the colour reduction (3.1);
  - the default small-region area, 3 mm² instead of 1, merging compact specks only on line
    art;
  - the background left out by colour rather than by thread (2.2).

  Settings the user changes still win.
- **Other wizards:** Cross Stitch and the Photo Stitches stay PE-only until a method beats
  PE-DESIGN mode on their test images.

## 5. Next steps

1. **Auto Punch's OpenStitch mode**, as measured here. Then the gallery shows both modes side
   by side (`tools/gallery.py`).
2. **Keep line art's lines:** done 2026-10-31 (2.2).
3. **Photo Stitch 1's perceptual palette and error diffusion (3.6)**, measured on the photos
   (p01, p04, p07) with the same harness.
4. **SLIC before Auto Punch on photos (3.3)**, if 1 and 3 leave the painted images ragged.
5. **Other brands' thread charts** for the colour floor (with B13).
