# Hoops and machine types

Source data for `openstitch_design::machine::BROTHER_HOOPS`. Only Brother's public pages are
used; nothing comes from the PE-DESIGN installation.

## What the PE-DESIGN 11 guide says

[Specifying the Design Page Settings](https://support.brother.com/g/s/hf/htmldoc/ped/im/ped11/en/PED11_EN/pages/03_1245955.html):

- Machine Type: the Page Size choices depend on it. A multi-needle machine icon exists; the
  guide shows the choices only as images.
- Hoop Size: the list is an image. In the text, only the multi-position hoops are named:
  100 × 172 mm and 130 × 300 mm, marked "*".
- Custom Size: width and height typed in, for split designs. No limits are given.
- Rotate 90 Degrees: not for cap, cylinder or round frames, and not for user hoops.
- User hoops: width, height, optional comment; up/down ordering; added at the bottom of the
  list. Designs in a user hoop size cannot be saved in previous-version formats.
- Output ▸ sewing area: "Design Page area" (needle starts at the page centre; the design is as
  large as the page) or "Use existing design area" (needle starts at the design centre).
- Jump Stitch Trimming: multi-needle machines only; a minimum jump length for trimming.

## Hoop sizes and where each comes from

| Hoop (mm) | Source |
|---|---|
| 100 × 100, 130 × 180 | PE-DESIGN 8 manual, "Creating Design For Multi-Position Hoops" (manualslib page 184) |
| 160 × 260 | Brother SA441 6" × 10" hoop page (brother-usa.com/products/sa441) |
| 200 × 300 | Brother SA447 and PRH300 8" × 12" frame pages |
| 240 × 360 | Brother USA category listing of a 9-1/2" × 14" embroidery area |
| 272 × 272 | PE-DESIGN 11 update history, version 11.10 |
| 180 × 360 | update history, 11.20 |
| 100 × 180, 254 × 254 | update history, 11.30 |
| 297 × 465 | update history, 11.40 |
| 200 × 360 | update history, 11.50 |
| 360 × 360 (jumbo frame) | update history, 11.23 Design Database notes |
| 100 × 172 *, 130 × 300 * | PE-DESIGN 11 guide (multi-position) |

Update history: <https://support.brother.com/g/s/hf/ped_info/ped11/one_click_update/en/ped11update_info.html>.

## Gaps

- The full image-only hoop list (cap, cylinder and round frames, small hoops) is not in text
  form anywhere public we found. Add entries only with a citable source.
- Which hoop each machine model accepts is not published in one table, so every hoop is offered
  for both machine types. PE-DESIGN, too, leaves this to the user.
- Thread-length and sewing-time models are first-principles estimates (1 mm per penetration,
  10 mm per trim, bobbin = needle / 3; 800 / 1000 stitches per minute, 3 s per trim, 5 s per
  multi-needle colour change). They are not calibrated against PE-DESIGN's Design Property
  figures, which needs a licensed installation.
