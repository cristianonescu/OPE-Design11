# Split designs, multi-position hoops, jumbo frame, printing, USB transfer

Behaviour from Brother's public PE-DESIGN 11 guide (support.brother.com, `PED11_EN/pages/`),
read 2026-10-19: tutorials 10-1 to 10-3 (`07_1763923`, `07_1800398`…`07_1801158`,
`07_1925828`, `07_1925827`, `07_1925948`, `07_1943503`), Tutorial 7 and printing (`06_1757710`,
`06_1804289`, `06_1804342`, `04_1828606`), USB transfer (`08_1789292`), saving (`04_1828566`),
original card (`08_1790513`). **[image]** marks details shown only in screenshots, hence
unknown; OpenStitch's choice for each is stated.

## Split designs (Custom Size)

- Design Settings ▸ Custom Size: width and height; a 90 × 90 cm page is mentioned (implied
  maximum). **Section Size (for Hoop Size)**: each entry is a hoop with sections **10 mm smaller
  in width and height** (example "120 × 170 mm (130 × 180 mm)"); the margin "allows for
  overlap". User hoops are not offered. Full list **[image]** → OpenStitch offers every Brother
  hoop except multi-position and jumbo.
- **Use smaller section sizes**: sections "10 mm smaller than normal on all sides" →
  OpenStitch reads this as 20 mm smaller than the hoop in each direction.
- No explicit overlap setting → none by default; OpenStitch cuts stitches exactly at the split
  line.
- Order: "from left to right, top to bottom". The first section is "Aa" on the machine; the
  label scheme **[image]** → OpenStitch labels row letter (A, B, …) + column letter (a, b, …).
- **Move Sections**: split lines are dragged in the ruler; they reset when the section size
  changes. OpenStitch: split positions as numbers, validated so no section exceeds the section
  size.
- **Alignment stitching**: single running-stitch lines at section edges, pitch **7.0 mm**,
  starting with fixed stitches at **0.3 mm**; thread colour "NOT DEFINED"; printed in red.
  Before a section is sewn, its edges shared with already-sewn neighbours (left, top) are
  stitched; after it, its edges towards later neighbours (right, bottom).
- Saved/exported as **one** PES (section data in PE-DESIGN's own header); the original card
  writes sections separately. OpenStitch cannot write PE-DESIGN's section header (undocumented),
  so it exports **one machine file per section**, each centred on its section.
- Positioning sheets: PDFs per hoop, printed at 100 %; punch the arrow ends, mark the fabric,
  join marks into centre lines; points A–D on the sheet align with a–d on the fabric for the
  next section.

## Multi-position hoops (100 × 172, 130 × 300)

- Three installation positions **a (top), b (middle), c (bottom)**; each pattern must fit one
  area of **100 × 100** or **130 × 180 mm** completely; areas overlap; offsets **[image]** →
  OpenStitch spaces them evenly: a at the top, c at the bottom, b centred.
- Patterns outside every area are errors (Design Property, save).
- **Optimize hoop change** example: 1a, 2b, 3a, 4c, 5a, 6c → a(1), b(2), a(3, 5), c(4, 6): later
  patterns join an earlier group of the same position, but not across everything → OpenStitch
  pulls a later pattern forward into the current group unless it overlaps a pattern it would
  jump over (keeps visible stacking order) or an earlier pattern of its own position stayed
  behind (keeps each position's order); with 3 overlapping 2 this gives the guide's result.
- One PES; original card one pattern per position.

## Jumbo frame (360 × 360)

- Design Page **350 × 350 mm**; camera multi-needle machines only.
- Page divided automatically into **Side 1** (sewn first) and **Side 2**, with an overlapping
  area **[image]**; the frame is **rotated 180°** for Side 2 (Snowman sticker re-detection);
  no alignment stitching; prints Side 2 rotated 180°. → OpenStitch: horizontal split at the
  middle (top half Side 1, bottom half Side 2 rotated 180°), overlap configurable (default 0).

## Printing

- Paper size and orientation from the printer. **Actual Size** (1:1, info on separate pages)
  or **Reduced Size** (one page). Options: background image; sewing area box and centre axes;
  embroidery positioning mark at the centre; template grid (green, matches the hoop's
  embroidery sheet); hoop section boundaries in red; divide into 2 pages; colour list by change
  or total; stitch image normal or realistic.
- Print and Stitch: print the image (optionally flipped) on fabric or transfer sheet, then a
  positioning sheet with marks and guide lines for the embroidery area; tape it on the hooped
  fabric and align the needle with the centre mark.
- Split designs: page 1 the whole pattern with size and file name, then one positioning sheet
  per section.

## USB media

- Home ▸ Send ▸ Send to USB Media ▸ pick the drive; name collisions ask overwrite / stop /
  rename; "Finished outputting data." Format, folder and naming on the media are not stated →
  OpenStitch writes the chosen format to the root of the volume.
