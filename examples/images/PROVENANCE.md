# Test images: provenance

Every image here was generated for OpenStitch with Higgsfield at the project owner's request,
under the owner's account, on 2026-10-30:
- **Model:** GPT Image 2.5 (`gpt_image_2_5`), quality medium, at the model's default 1k
  resolution unless a row says 2k.
- **Reference images:** none.
- **Content:** every prompt describes original content. There are no real people's likenesses,
  no brands, logos or copyrighted characters, and nothing modelled on Brother's or PE-DESIGN's
  sample images. The portraits show invented people.
- **Seed:** the model takes no seed, so the Higgsfield job id identifies each generation.

What each image tests, and in which wizard, is in
[docs/research/test-images.md](../../docs/research/test-images.md). The licensing record is in
[docs/LEGAL_AND_LICENSING.md](../../docs/LEGAL_AND_LICENSING.md).

## Storage

- **Size:** the originals (1024 px class) were reduced to at most 768 px on the long side.
  `p06-sunset-large.jpg` keeps its 2688 × 1520 pixels as the large-input case.
- **Photos and soft images:** JPEG quality 85.
- **Flat graphics:** PNG with a 256-colour palette (median cut, no dither).
- **Black line art and silhouettes** (g03, g04, g11, g12): 8-bit greyscale PNG.
- **Transparency:** `g10-leaf-transparent.png` keeps its alpha channel in its palette.
- **Pixel art:** `g07-pixel-strawberry.png` was scaled with nearest neighbour, so its 32 × 32
  grid stays exact (24 px cells).

## Images

| File | Aspect | Higgsfield job | Prompt |
|---|---|---|---|
| p01-portrait.jpg | 2:3 | 84adc052-c95f-4562-8ed0-6805077535ef | Photorealistic head-and-shoulders portrait of an invented adult woman with warm brown skin and short curly dark hair, soft natural window light from the left, calm expression, plain muted teal background, natural colours. A fictional person, not resembling any real individual. No text, no logos. |
| p02-mono-portrait.jpg | 3:4 | e378f6bb-4697-4557-94c8-60f79d9c21db | Black-and-white studio photograph of an invented elderly man with a white beard, dramatic side lighting, full tonal range from deep black shadows to bright highlights, plain dark grey background. A fictional person, not resembling any real individual. No text. |
| p03-storybook.jpg | 4:3 | e4ec11d0-fce1-40d5-9346-705d6ace47ac | Children's-book gouache illustration: a fox and a rabbit having a picnic on a checked blanket in a meadow, soft brush texture, warm friendly colours, original characters, no text. |
| p04-cat-fur.jpg | 1:1 | 6f3217d4-0df3-40b4-83e2-0759a6d921da | Photorealistic close-up of a long-haired ginger cat, detailed fluffy fur, green eyes, soft daylight, blurred neutral background. No text. |
| p05-dog.jpg | 4:5 | 0bed535e-2c8e-4687-b681-3c62aededc76 | Photorealistic black-and-white coated dog sitting on green grass, bright daylight, shallow depth of field, natural colours. No text, no collar tags. |
| p06-sunset.jpg | 16:9 | f9bfe30d-da84-40c4-8c68-eb3c3a6f4d29 | Photorealistic landscape at sunset: a smooth sky gradient from deep orange near the horizon to violet and blue above, dark rolling hills and a calm lake reflecting the sky. No people, no text. |
| p06-sunset-large.jpg | 16:9, 2k | 5d555570-17ef-4743-8a64-b78fc2203b5f | Photograph of a landscape at sunset: a smooth sky gradient from deep orange to purple over dark rolling hills and a calm lake reflecting the sky, no people, no buildings, no text. |
| p07-fruit.jpg | 1:1 | d4c76398-a416-47ba-9eb3-f1a290dabedf | Photorealistic bowl of fruit on a rustic wooden table: oranges, green apples, purple grapes, a yellow banana and red strawberries, vivid colours, soft studio light. No text, no labels. |
| p08-poppies.jpg | 1:1 | 540faa74-514e-42ea-9233-386739625dcd | Photorealistic macro photograph of red poppies with black centres in a meadow, green stems, soft blurred green background. No text. |
| p09-facade.jpg | 3:4 | 74210ba2-690a-43c1-ab36-528eb4f11dd9 | Photorealistic modern building facade with strong straight lines: a grid of windows and concrete fins in strong perspective against a clear blue sky. No signs, no text, no logos. |
| p10-night-street.jpg | 16:9 | 66bb8f74-7236-4563-8fff-ea713f15a608 | Photograph of a quiet cobbled city street at night, warm glowing street lamps, wet reflections, dark blue sky, no people, no signs, no readable text, no brand names. |
| p11-high-key.jpg | 1:1 | b0e79d77-4607-4d28-a6b5-ba34f2dc6dbe | High-key photograph: a white porcelain teacup and saucer on a white linen tablecloth, bright soft light, very light tones throughout with gentle grey shadows. No text. |
| p12-backlit-tree.jpg | 16:9 | f54aa35c-19d2-4927-9a4b-08a73ab06bac | Backlit photograph: a single large tree as a near-black silhouette on a hilltop against a very bright pale sky with the sun just behind the branches. No people, no text. |
| g01-emblem.png | 1:1 | 5b04befd-0f74-40ed-8f73-73b568b91abd | Flat vector emblem: a stylised mountain with a rising sun inside a circle, exactly four flat colours (navy, orange, yellow, white) on a plain white background, crisp edges, no gradients, no text. An original design, not any existing logo or brand. |
| g02-balloon.png | 4:3 | 25952267-ac88-4bb6-bffd-db10edcdb22f | Flat vector illustration of a striped hot-air balloon floating over rolling green hills under a light blue sky, six flat colours, no gradients, no outlines, no text. |
| g03-teapot-lineart.png | 1:1 | 20c623b3-c3c4-4957-9fe7-e722850cb6e3 | Clean black ink line drawing of a round teapot with a curved spout and handle, single-weight outlines only, no shading, no fill, pure white background. No text. |
| g04-butterfly-outline.png | 1:1 | fa66fc24-c69c-4fd0-b4e4-0a1f2c7066bc | Colouring-book page: a butterfly drawn with bold thick black outlines forming closed regions on the wings, no colour, no shading, white background. No text. |
| g05-robot-cartoon.png | 1:1 | ae5b011b-5813-4a63-a350-d9fcc7cce9b1 | Original cartoon character: a friendly round little robot with antenna and big eyes, bold black outlines, flat bright colours (teal, orange, light grey), plain white background. Not based on any existing character. No text. |
| g06-watercolour-lemons.jpg | 2:3 | d3d7f882-f214-49a2-a18b-a0e631bedf68 | Loose watercolour painting of three lemons with green leaves, soft bleeding edges, visible paper texture, white paper background. No text, no signature. |
| g07-pixel-strawberry.png | 1:1 | 3f52236a-565d-4685-9662-9ee63504bd63 | 16-colour pixel art of a strawberry with leaves, large clearly visible square pixels on a 32 by 32 grid, flat colours, no anti-aliasing, plain white background. No text. |
| g08-lettering.png | 3:2 | dd0a01c1-c516-435f-8247-dd1c83a12a9b | The word STITCH in bold heavy sans-serif capital letters, dark blue letters with a red underline bar, on a plain cream background, flat colours, centred. Only that one word. |
| g09-gradient.jpg | 16:9 | 354cccc6-9618-4c2e-9bd1-61a9d42a50ea | Smooth abstract multi-colour gradient: flowing soft bands blending from red through orange, yellow, green and blue to purple, no objects, no texture, no text. |
| g10-leaf-transparent.png | 1:1, transparent | 799d18fa-64c9-4dee-a0b5-9be32bfa54e6 | A single flat green leaf sticker icon with a darker green vein and a thin white border, on a fully transparent background. No text. |
| g11-mandala.png | 1:1 | a2b1a0ed-7f53-460b-916f-7820474f0e58 | Intricate circular mandala made of very fine thin black lines and tiny dots on a white background, highly detailed, no colour, no text. |
| g12-deer-stencil.png | 1:1 | 0197818b-abf3-4ba7-9280-fa50050c62eb | Monochrome stencil-style silhouette of a standing deer with antlers, solid black on a plain white background, crisp edges, no text. |
| g13-mosaic.png | 1:1 | aa91479a-faa4-49d3-a12d-3a568f90e08e | Geometric mosaic of triangles and squares in exactly twelve distinct flat colours (red, orange, yellow, lime, green, teal, cyan, blue, indigo, purple, pink, brown), sharp edges, no gradients, no text. |
| g14-two-colour.png | 1:1 | 28b131c6-9d5f-4c9f-bdcd-5022181e4e16 | Minimal two-colour poster: a large flat orange circle centred on a flat navy blue background, nothing else, no text, no texture. |
| g15-folk-bird.png | 1:1 | f803d328-307d-42ff-9f21-03f3f23e3557 | Original folk-art bird motif with tulips and leaves, symmetrical, flat red, green, yellow and dark blue colours on a cream background, simple bold shapes, no text. |

## Derived variants

These were made in code from the images above, with no further generation. The steps are in
docs/research/test-images.md.

| File | From | Change |
|---|---|---|
| d01-fruit-artefacts.jpg | p07 | Gaussian noise (σ 14, seed 7), then JPEG quality 15 |
| d02-emblem-small.png | g01 | Scaled to 128 × 128 (Lanczos) |
