# PE-DESIGN 11 minimal designs (`pes11` tests)

Made by the owner's PE-DESIGN 11.50 (Layout & Editing) on Windows 11 Pro 10.0.26200, on
2026-10-27, by black-box GUI use only (track `pe-gui`), for mapping the `#PES0110` object list
(docs/FILE_FORMATS.md, "PES 0110 object list"). Studying these files is allowed as project
policy: docs/LEGAL_AND_LICENSING.md. Each rN file is the baseline B with one property changed.

**Common:** PE-DESIGN freshly restarted at 20:41 (it had closed); every file: App menu > New, Design Page 100 x 100 mm (as last used), grid 1 mm, Snap to Grid on; Save As, type "Layout & Editing Ver.11.0 File (*.PES)" (#PES0110) unless stated. Page origin top-left, y down.

**Baseline B:** Home > Shapes > Rectangle/Circle/Arc > Rectangle, dragged corner to corner (10,10)-(20,20) mm. Attributes as the freshly restarted tool shows them (= PE defaults recorded in round 1 notes.md, house buttons): Edge radius 0.0; Line sew Zigzag Stitch, colour 800 EMBROIDERY RED, Under sewing on, Zigzag width 2.0 mm, Density 4.5 line/mm, Half Stitch on, Sharp Corners first button, Miter Limit 0.0; Region sew Fill Stitch, colour 005 EMBROIDERY SILVER, Under sewing on (first type, Density Medium), Density 4.5 line/mm, Gradation off, Direction Manual straight line 45 degrees, Stitch Type first, Running Stitch Path first, Step pitch 4.0 mm, Frequency 30 %, Pull compensation 0.0 mm. Status: 12,00 x 12,00 mm (the 2 mm zigzag straddles the outline), 471 stitches, Sewing Order: 1 fill (silver), 2 zigzag (red).

**Text tool (r12–r15):** Home > Text > first tool "A"; as the freshly restarted tool shows it: font Lato (the repo's Lato-Bold.ttf, installed per-user earlier; PE lists "Lato", style Bold), size 10.0, line Not Sewn (Line), region Satin Stitch, region colour 800 EMBROIDERY RED, Text Attributes all default (Kerning 0, Vertical Offset 0, Rotate 0, Character Spacing 0, Line Spacing 100 %, Alignment Left, horizontal, Transform off). The click sets the baseline-left of the text. A first r12 placed with the click at (10,10) put the letter at y 0..10 mm; Save As warned "Please change the object size or position. Do you want to save the data anyway?" -> No (discarded), so the click was made at (10,20) instead: the letter's top-left is at about (10,10).

| File | SHA-256 | Made as |
|---|---|---|
| `r01-rect.pes` | `97acefa9c7e11b1966093e067444dcb0b1934a0fd9e5808007c3730c7b466c38` | B. 12,00 x 12,00 mm, 471 st. |
| `r02-rect-w20.pes` | `2b934d9489a142bc7296ef19d53bd76f8ac26e56c02c908db589045da7414697` | B but (10,10)-(30,20). 22,00 x 12,00 mm, 745 st. |
| `r03-rect-moved.pes` | `f7063e489ff4bc34c5160414b44141ec9a41611891aac05ee9a0f30240922114` | B but (30,30)-(40,40). 12,00 x 12,00 mm, 473 st. |
| `r04-rect-satin.pes` | `6b8809fa0873f9e26d09563704cff29b3a278abd8c9bb5363099ffdfbe6002d7` | B, then (shape selected) Shapes tab > Sew > region type Satin Stitch (satin defaults: Under sewing on Medium, Density 4.5, Direction 45). 12,00 x 12,00 mm, 370 st. |
| `r05-rect-blue.pes` | `dbc2885ad9d9513819c3f35a925bcbd73513c665e94a3fbcf7ea20f59eb3b30d` | B, then Color pane, Region, BROTHER EMBROIDERY 007 PRUSSIAN BLUE. 12,00 x 12,00 mm, 471 st. |
| `r06-rect-nounder.pes` | `59bbed3f276f34fa158f1b8f80b6cab5efa9e0c71840d3e3df6ebd7c6f567021` | B, then Sewing Attributes > Region sew > Under sewing unticked. 12,00 x 12,00 mm, 423 st. |
| `r07-rect-noline.pes` | `a70cd1de6427d49f57fc7bf09a1a60226ab6b1c896fcb7df65715a6e4086486c` | B, then line type Not Sewn (Line). 10,00 x 10,00 mm, 230 st. |
| `r08-rect-dir90.pes` | `61e49a05e2639cccf817a569e4df4d0e020d7f2fdfb26ca6b0e4bd9ff49fd8fb` | PE's default fill Direction is already 45 degrees, so a 45-degree file would equal r01; made with Direction Manual straight line 90 degrees instead. 12,00 x 12,00 mm, 462 st. |
| `r09-rect-density.pes` | `91031276d49066f181f88b7f16b64177b02638a159087c333dca2e4e8973fbd5` | B, region Density 6.0 line/mm. 12,00 x 12,00 mm, 530 st. |
| `r10-circle.pes` | `4fda5676bbbf02151b825fef23c46b251754c83768a0a887cd3f6a8777c4dbe0` | Shapes > Ellipse (Rectangle/Circle/Arc row, 2nd) dragged (10,10)-(20,20), same default attributes as B (Zigzag red / Fill silver, values as above). 12,00 x 12,00 mm, 400 st. |
| `r11-line.pes` | `bad5682da96054a4d82a2d3da2949c29e3949720c5f97c1246936ae86ca9bd30` | Shapes > Outline > open straight line, click (10,10), double-click (30,10); Select tool. Line Zigzag Stitch 800 RED, defaults (under sewing on, width 2.0, density 4.5, half stitch on). 20,00 x 2,00 mm, 118 st. |
| `r12-text-a.pes` | `505ed96e7340d306a55b6b2b3836e0a5f1356959c46d1e31a1feed6bf773d047` | "A", click (10,20). 10,10 x 10,30 mm, 155 st; box about (10,9.7)-(20.1,20). |
| `r13-text-a-20.pes` | `3afdeeec119e211c82084c8cb77163fc9134843f9fb7dd6b881904f236c09ebb` | r12, then (text selected) size 20.0: the letter grows upward past the page top, so it was dragged down 10 mm (snap) to stay on the page: box about (10,9.4)-(30.1,30), baseline y 30. 20,10 x 20,60 mm, 290 st. |
| `r14-text-ab.pes` | `a1e941016670496f251799e46bdee13243cf305698a66f0f7545a3dba4f990a9` | "AB" (size set back to 10.0 on the tool), click (10,20). 18,80 x 10,30 mm, 363 st. |
| `r15-rect-text.pes` | `e06980cb3a1fec55ece376f50897a430d68a6dd04f9cfc1b52d50857846a0b63` | B, then Text tool "A" size 10 with the click at (40,50) (top-left about (40,40)). Whole design 41,00 x 41,00 mm, 626 st; Sewing Order: 1 fill silver; 2 red = zigzag + A (same colour, one block). |
| `r16-two-rects.pes` | `c79ddb9b51d78a7337fce6b4fe70a3c2e22ef14b15945e74490bced9633b7827` | B, then a second Rectangle (40,10)-(50,20), same attributes. 42,00 x 12,00 mm, 944 st; Sewing Order 1 silver, 2 red, 3 silver, 4 red. |
