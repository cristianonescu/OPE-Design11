# PE-DESIGN 11 minimal designs (`pes11` tests)

Made by the owner's PE-DESIGN 11.50 (Layout & Editing) on Windows 11 Pro 10.0.26200, on
2026-10-27, by black-box GUI use only (track `pe-gui`), for mapping the `#PES0110` object list
(docs/FILE_FORMATS.md, "PES 0110 object list"). Studying these files is allowed as project
policy: docs/LEGAL_AND_LICENSING.md. Each rN file is the baseline B with one property changed.

**Common:** PE-DESIGN freshly restarted at 20:41 (it had closed); every file: App menu > New, Design Page 100 x 100 mm (as last used), grid 1 mm, Snap to Grid on; Save As, type "Layout & Editing Ver.11.0 File (*.PES)" (#PES0110) unless stated. Page origin top-left, y down.

**Baseline B:** Home > Shapes > Rectangle/Circle/Arc > Rectangle, dragged corner to corner (10,10)-(20,20) mm. Attributes as the freshly restarted tool shows them (= PE defaults recorded in round 1 notes.md, house buttons): Edge radius 0.0; Line sew Zigzag Stitch, colour 800 EMBROIDERY RED, Under sewing on, Zigzag width 2.0 mm, Density 4.5 line/mm, Half Stitch on, Sharp Corners first button, Miter Limit 0.0; Region sew Fill Stitch, colour 005 EMBROIDERY SILVER, Under sewing on (first type, Density Medium), Density 4.5 line/mm, Gradation off, Direction Manual straight line 45 degrees, Stitch Type first, Running Stitch Path first, Step pitch 4.0 mm, Frequency 30 %, Pull compensation 0.0 mm. Status: 12,00 x 12,00 mm (the 2 mm zigzag straddles the outline), 471 stitches, Sewing Order: 1 fill (silver), 2 zigzag (red).

**Text tool (r12–r15):** Home > Text > first tool "A"; as the freshly restarted tool shows it: font Lato (the repo's Lato-Bold.ttf, installed per-user earlier; PE lists "Lato", style Bold), size 10.0, line Not Sewn (Line), region Satin Stitch, region colour 800 EMBROIDERY RED, Text Attributes all default (Kerning 0, Vertical Offset 0, Rotate 0, Character Spacing 0, Line Spacing 100 %, Alignment Left, horizontal, Transform off). The click sets the baseline-left of the text. A first r12 placed with the click at (10,10) put the letter at y 0..10 mm; Save As warned "Please change the object size or position. Do you want to save the data anyway?" -> No (discarded), so the click was made at (10,20) instead: the letter's top-left is at about (10,10).

**Round 2:** (2026-10-27 22:24-22:33): PE-DESIGN 11.50 freshly restarted at 22:24 (it had closed), so tool attributes were at defaults again; same COMMON and BASELINE B as above. Colours are set in the Color pane (Line / Region radio, BROTHER EMBROIDERY chart). Tools remember the last colour/type, so each file was checked and set back to B before its one change.

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
| `r17-line-silver.pes` | `d5d9ce74499647568d12ef8d45e2d7fa144305338bb80227f1aa800b394bf361` | B, Color pane > Line > 005 EMBROIDERY SILVER (same as the region). PE then shows ONE colour block (Sewing Order 1 item with both). 12,00 x 12,00 mm, 467 st. |
| `r18-colours-swapped.pes` | `9f656206e6884199bbdbd01e2eda39135435a9f1dfc48a985cba8710091fa007` | B, region 800 EMBROIDERY RED and line 005 EMBROIDERY SILVER. 12,00 x 12,00 mm, 471 st (blocks: 1 red fill, 2 silver zigzag). |
| `r19-line-running.pes` | `13c8942d2ef77b97399d87d52d97ee118b5ccfcbc9b1573e13a2930407114c33` | B, Shapes tab > Sew > line type Running Stitch (defaults Run pitch 2.0 mm, Run time(s) 1). 10,00 x 10,00 mm, 256 st. |
| `r20-line-running-pitch.pes` | `87ee85d649052fc934e9e019e5e1b64eb8df5720f155c3f6a11f7f4f5b78e1b6` | r19 (same design), Run pitch 3.0 mm. 10,00 x 10,00 mm, 251 st. |
| `r21-zigzag-width.pes` | `4217cc26142986c7a81a40b69befdc0995ca8a6d538a14f0cbec2c73f85242ce` | B, line Zigzag width 3.0 mm (default 2.0). 13,00 x 13,00 mm, 484 st. |
| `r22-line-noUnder.pes` | `84a72272dd2b3d7fefa688f7e453822c121e478721460d0ce2c36e3570259b0e` | B, Sewing Attributes > Line sew > Under sewing unticked. 12,00 x 12,00 mm, 426 st. |
| `r23-text-fill.pes` | `f7e7e2b6d7799c46139981403bd4db7b0dd7081472c5bf2b020d80d1c0e27e61` | r12's text `A` (Text tool, Lato, size 10, click (10,20)), region Fill Stitch (the text fill defaults: under sewing off, density 5.0). 187 st. Copied into the repo 2026-10-29 for the `#PES0110` text writer (#120). |
| `r24-text-blue.pes` | `8c8c7fef1f2d2d1a69300b7fb484a9f8c7e431e5c27bd84db96319528dc0914c` | r12's text `A`, region Satin, region colour 007 EMBROIDERY PRUSSIAN BLUE. 155 st. |
| `r25-text-b.pes` | `e5e0bc6b7e8b545e3bde1944852c661789279882ad5032b7c6f9d399d5203063` | r12 but the letter `B`. 7,70 x 10,30 mm, 207 st. |
| `r26-text-moved.pes` | `1450b6f8f655ec92a930c10be2afcf489e7cab43e24c9e1adea2a4c083c2861f` | r12 with the click 10 mm further right, at (20,20): box about (20,9.7)-(30.1,20). 155 st. |
| `r27-text-a-space-b.pes` | `70c2c73817f116c07a28ea3334fce45e64788b74e2fdb497484c70f7529c5ba5` | Round 4 (2026-10-29, session pe-refs-4, PE-DESIGN 11.50, Windows 11 Pro 10.0.26300): New design (page as PE-DESIGN started: Hoop Size 100 x 100 mm `nv880e-S`), Text tool at its defaults (Lato, 10,0, line Not Sewn, Satin Stitch 800 RED), click at (10,20) mm, typed `A B`, Select. 22,30 x 15,90 mm, 364 st; Save As Ver.11.0 (#PES0110). Shows that a space is stored as a letter. |

## Round 3 (2026-10-28): the remaining object kinds

Made by the owner's PE-DESIGN 11.50 (Layout & Editing, restarted at 14:26) on Windows 11 Pro 10.0.26300, 2026-10-28, by black-box GUI use only (session pe-refs-3), one object kind per file so that every class PE-DESIGN writes from owner-drawn content has a sample. Common: App menu > New, Design Page 100 x 100 mm (grid 1 mm, Snap to Grid on) unless the row says otherwise; one object drawn with the tool named, attributes as the tool showed them (the Shapes tools remember the last attributes: each row records the Sew group); Select tool; Save As "Layout & Editing Ver.11.0 File (*.PES)" (#PES0110). Sizes and counts from the status bar. Tool tooltips (Home > Shapes menu): Outline row = Closed Straight Line, Closed Curve (shortcut X), Pencil(Closed Curve), Open Straight Line, Open Curve, Pencil(Open Curve); Rectangle/Circle/Arc row = Rectangle, Ellipse, Arc and String, Round Rectangle, Arc, Fan; Manual Punch row = Straight Block (Z), Curve Block, Running, Feed (B); Shapes = the gallery. The Small Text and Monogram tools offer only PE's built-in fonts and were not recorded; Motif Stitch and Decorative Fill use PE's built-in patterns and were skipped; a Manual Punch Feed-only design saves with no PEC section and was discarded. Text uses Lato (the repo's Lato-Bold.ttf installed per user). Class names seen (a plain scan of the object section): `CEmbLine` (the Outline, Rectangle/Circle/Arc and Manual Punch tools, every region type), `CEmbSpecialShape` (gallery heart; also in the applique), `CEmbNText` (vertical text), `CEmbDText` (Text Transform), `CEmbAText` (Fit Text to Outline, in the compat text-fit-* references).

| File | SHA-256 | Made as |
|---|---|---|
| `o-applique-heart.pes` | `183c343363828033d2f8d030a27c88b9149ef3bd02d821a9e015358dde3569a5` | New; Shapes gallery heart dragged (10,10)-(40,40) (the tool kept the last attributes: line Running Stitch, region Zigzag Net Fill Stitch; 30,00 x 30,00 mm, 1081 st); Ctrl+A; Home > Wizards > Applique Wizard, dialog defaults: Applique Material Yes / Running Stitch (Cutting is the other choice), Applique Position (picture only), Tack down Yes / V Stitch, Covering Stitch Zigzag Stitch Width 3.0 mm Density 4.5 line/mm, Output Pattern Add (Replace is the other), Distance from original pattern 3.0 mm; OK. Sewing Order after: 1 the heart, 2 material outline, 3 position/tack-down outline, 4 covering zigzag (group). Whole design 42,60 x 42,60 mm, 2019 st. |
| `o-arc-string.pes` | `4b33d7075c8edc6ab0e145f2705a853c90fb866c28aa95b854d5a1e14d420048` | Arc and String tool, circle dragged (10,10)-(40,40) mm, then "Choose Arc start point" click at 3 o'clock (40,25), end point at 12 o'clock (25,10). Line Running Stitch (red), region Not Sewn (Region) (the tool's attributes). 30,00 x 30,00 mm, 65 st. |
| `o-arc.pes` | `886ddb7c23a9571ef7f527723b5875e582700ca171a81a7371dc37c462f8e2fe` | Arc tool, circle (10,10)-(40,40), start 3 o'clock, end 12 o'clock. Line Running Stitch (open arc, region greyed). 30,00 x 30,00 mm, 53 st. |
| `o-closed-curve.pes` | `44f9fd64f9ae6d4eb29ece237580c7151d7dcaabe185712714d50c626ecf5d54` | Closed Curve tool, clicks (10,10) (40,10) (40,40), double-click (10,40) (the curve bulges outside the points). Line Running Stitch, region Not Sewn (as the tool showed). Total 42,00 x 42,60 mm. |
| `o-closed-freehand.pes` | `2ba0cdc0554c17abf9581603ec71b75ffc02a0865d41c565750634602acbaa3a` | Pencil(Closed Curve), one stroke (10,10)->(20,8)->(30,10)->(40,14)->(44,24)->(38,34)->(26,38)->(14,32)->(10,22). Line Running Stitch, region Not Sewn. 33,20 x 29,80 mm, 97 st. |
| `o-fan.pes` | `82bba468cfcb728847970f6015fdf605f643bbf6baaf899aa83eccdbf1ddb33a` | Fan tool, same circle and points. Line Running Stitch, region Not Sewn. 30,00 x 30,00 mm, 69 st. |
| `o-gallery-heart.pes` | `e03bd1d06d31205fade6ab5dd7225f841bb7896a088418089108603d2595df5e` | Shapes gallery, row 1 second icon (heart), dragged (10,10)-(40,40). Line Running Stitch, region Not Sewn. 30,00 x 30,00 mm, 74 st. |
| `o-group-two-rects.pes` | `272151ab267fe12900369e93eafe34678f4a95cf1f25ff27316fb8a7ae68968f` | Rectangle tool (its attributes now line Running Stitch red, region Not Sewn) dragged (10,10)-(30,30) and (40,10)-(60,30) (the second needs the tool picked again); Ctrl+A; Home > Edit > Group > Group. 50,00 x 20,00 mm, 90 st. |
| `o-hoop-100x100-s.pes` | `d55099ad0a87ef4baa9a31fca31a5b8f04cc20a0c405e38f957220721d57de76` | Page Size = Hoop Size, list (multi-needle hoops: 360x60 Wide Cap Frame, 90x80 Cylinder, 180x100 Border, 300x100 Border, 70x200 Sleeve, 44x38/50x50/70x41/33x75 Compact, diam. 100/130/160 Round, 260x160 nv880e-L, 150x150 nv880e-M, 100x100 nv880e-S) -> "100 x 100 mm nv880e-S". 767 st. |
| `o-hoop-130x180-single.pes` | `e70179fd9a452e11d11743c91169a2d511702289d1274aa6a8f7e08243bdba10` | Machine Type = first radio (single-needle icon); Hoop Size list then shows 100x100, 100x180, 130x180, 150x150 (Quilt Frame), 160x260 (Color LCD), 160x260 (Monochrome LCD), 180x300, 180x360, 200x200 (Quilt Frame), 200x300, 240x240 (Quilt Frame L), 240x360, 254x254, 272x272 (Quilt Frame LL), 272x408 -> "130 x 180 mm". 767 st. |
| `o-machine-single-100x100.pes` | `d75d240ce0f326a82f89f9da8ceb8788f2b54a0d5f47fc1a106f44f076e09909` | single-needle, Custom Size 100 x 100 mm. 767 st. |
| `o-manual-curve.pes` | `868693d1dc9dbad7de458782abed777dd61ae58ffd28bdb97ad508a4cb332688` | Manual Punch > Curve Block, the same six points. 40,00 x 20,00 mm, 379 st. |
| `o-manual-running.pes` | `c03ca0bf406c5298074ae7094914b7ac5fb3660b1af47c08488e03c2133107fe` | Manual Punch > Running: clicks (10,10) (30,20), double-click (50,10). Running Stitch (red). 40,00 x 10,00 mm, 29 st. |
| `o-manual-straight.pes` | `3dfed0fd985de5b2badef1bf798b5c76fefdfba72012f98f22c6ebbfb73ca08e` | Manual Punch > Straight Block: clicks (10,10) (10,30) (30,10) (30,30) (50,10), double-click (50,30). Manual Punch tab shows Straight/Curve/Running/Feed; block = Satin Stitch (red, tool default), line Not Sewn. 40,00 x 20,00 mm, 379 st. |
| `o-open-curve.pes` | `5ee5d3ad1d3b83c5ac68d29ff4c6fa864369091a19525dd565e186b2bf8da016` | Open Curve tool, clicks (10,20) (30,10), double-click (50,20). Line Running Stitch. 40,00 x 10,00 mm, 37 st. |
| `o-open-freehand.pes` | `36a68a6d01e0ca2fb9063805abd967496ee5e9901a8cf4cee1fa9127ff2a67fc` | Pencil(Open Curve), one stroke (10,20)->(20,12)->(30,10)->(40,14)->(50,22)->(60,20). Line Running Stitch. 42,70 x 11,80 mm, 55 st. |
| `o-page-200x200.pes` | `9e96834ec6543149ca80f889a768d819022217dba4922fef9088bffb9d431c3c` | Custom Size Width 200, Height 200 mm. (The view re-zoomed to 55 %; the rectangle stays at the same page coordinates (10,10)-(30,30) of the larger page.) 767 st. |
| `o-region-concentric.pes` | `93e15d8013ddcec5d2e1cd9c92900577be0fe004acbdd7e1c1be5ac1b4f24674` | Concentric Circle Stitch. 20,00 x 20,00 mm, 729 st. |
| `o-region-cross.pes` | `56be95108f92b69978d7cb87d14d7aadb2173a5fc26fd62128a5d14c130db915` | Cross Stitch. 20,00 x 20,00 mm, 450 st. |
| `o-region-flexible-spiral.pes` | `36fc316bf3d2efeedb7cb158d39a8d4b5852876c6af225867375501efaf3ad69` | Flexible Spiral Stitch. 20,00 x 20,00 mm, 175 st. |
| `o-region-net-fill.pes` | `bd8cfff018fccb9f823aa3e419e8c92d3a8339790a80e818a1a2825922fe379b` | Net Fill Stitch. 20,00 x 20,00 mm, 251 st. |
| `o-region-piping.pes` | `be5f1877b296c8f26235a62a358f9c90ed3a454fddd44348fe005cf6b7acd22f` | Piping Stitch. 20,00 x 20,00 mm, 203 st. |
| `o-region-radial.pes` | `e7659eb22e642ed7ab68d3e12758e2143335511762e321ac9dba5c5c32f0c93a` | Radial Stitch. 20,00 x 20,00 mm, 994 st. |
| `o-region-spiral.pes` | `97577498fb4e37142d806b63a68cf43c3797a7d7fc7a8830d61ec57b091cd187` | Spiral Stitch. 20,00 x 20,00 mm, 500 st. |
| `o-region-stippling.pes` | `0f74f7c9098f987acdd9b134e57990a826c7cb1e9086977c7cac09330d7cbfc7` | Stippling Stitch. 20,00 x 20,00 mm, 108 st. |
| `o-region-zigzag-net.pes` | `051274cc60a2ab8171d26c17475a1e862c1daaceb3066417bde0ee43bb64220f` | Zigzag Net Fill Stitch. 20,00 x 20,00 mm, 767 st. |
| `o-rounded-rect.pes` | `393e7e0fa7e52f53279193593d6b49852e2b521544c695a87029f60a408c03de` | Round Rectangle tool dragged (10,10)-(40,30). Line Running Stitch, region Not Sewn. 30,00 x 20,00 mm, 56 st. |
| `o-text-transform-1.pes` | `4e7d824f7dc8840d27bd32f6db2074d571ac9d1cc15ac3df8c6e5688c8c7600e` | that text, Text Attributes > Transform ticked, first preset (row 1 left, the arched "ABC"). 23,00 x 8,00 mm, 348 st. |
| `o-text-vertical.pes` | `f72efd958c8b031c44fd3314fbafd62669abb41fcec2d56511fcc7fe0a6b2e45` | the same text, Transform unticked again, Direction = vertical button (second). 8,00 x 24,40 mm, 407 st. |
| `o-text-10-letters.pes` | `b873248689736452a1a09915048a5635c38c7d1feb329cb0c607a691853a2e3b` | Home > Text > first tool "A" (Lato, size 8.0, line Not Sewn, region Satin Stitch, 800 RED), click (10,50) mm, ABCDEFGHIJ, Direction horizontal; ten letter records in one text. 69,30 x 8,40 mm, 1141 st. |

## Round 4 (2026-10-29): owner-drawn programmable-fill and motif patterns

Made by the owner's PE-DESIGN 11.50 (Layout & Editing) on Windows 11 Pro 10.0.26300, 2026-10-29, by black-box GUI use only (session pe-refs-4). The patterns were drawn in Programmable Stitch Creator (Option menu) and saved as `owner-v.pas` (Fill/Stamp, Default Size 7.5 mm) and `owner-v.pmf` (Motif, Default Size 7.5 mm), both kept in `compat/reference/pes/` (ASCII: `@VERSION 10000`, `@SYSENV 1`, `@TILE 1` + `@TILELINE 0 3 0 0 512 768 1024 0`, or `@LINE 3 0 512 512 768 1024 512`). App menu > New (100 x 100 mm), one rectangle, Save As "Layout & Editing Ver.11.0 File (*.PES)" (#PES0110). The same three files are the compat references of `pes-objects-owner-patterns`. Test `round_3_object_files_read_their_stitches_and_stop_cleanly` covers them too.

| File | SHA-256 | Made as |
|---|---|---|
| `obj-progfill-owner-v.pes` | `5adffe08327bccf391412e2ecf4264f3ec40258658685f239e7b554b6b7db4b3` | Rectangle tool dragged (20,20)-(50,40) mm, line Running Stitch (red, pitch 2.0), region Prog. Fill Stitch with the owner-drawn pattern owner-v.pas (compat/reference/pes; a V across the 8 x 8 cell, made in Programmable Stitch Creator), 7.5 x 7.5 mm, Direction 0, Offset row 0 %, Base Sewing off; region density 4.5, direction 45, under sewing off, 007 PRUSSIAN BLUE. 30,00 x 20,00 mm, 641 st. |
| `obj-motif-fill-owner-v.pes` | `d88daafad7dd68b04669a663f940bc3d83fcb2e97da821d2de72f26f25900f07` | The same rectangle, region Motif Stitch with the owner-drawn motif owner-v.pmf (compat/reference/pes; a V), Pattern1 5.0 x 5.0 mm, H/V-Arrange first buttons, offsets and spacing 0.0, Pattern2 off, Direction 0, Row offset 0.0, Run pitch 5.0. 30,00 x 20,00 mm, 144 st. |
| `obj-motif-line-owner-v.pes` | `fc5acdc25795d540f46b9b7353b64203420057f69ae41d38129b6611e29f7082` | The same rectangle, region Not Sewn, line Motif Stitch with owner-v.pmf, 5.0 x 5.0 mm, H/V-Arrange first buttons, Offset 0.0, Spacing 0.0, Run pitch 5.0, Start/End Type Feed. 30,00 x 20,00 mm, 47 st. |

**Round 4 machine exports (#135):** made by the owner's PE-DESIGN 11.50 (Layout & Editing) on
Windows 11 Pro 10.0.26300, 2026-10-29 11:43-11:47, by black-box GUI use only (track
pe-refs-4). Design Page 100 x 100 mm. Text tool "S", size 20.0, region Satin Stitch, 800
EMBROIDERY RED (14,00 x 21,10 mm, 278 st); Rectangle (60,60)-(70,68) mm, line Not Sewn,
region Fill Stitch, 007 EMBROIDERY PRUSSIAN BLUE (10,00 x 8,00 mm, 197 st). Sewing order: the
S, then the rectangle. Save As PES, then App menu > Export... to each type, no options dialog,
no warning. Three of the eleven files are kept here for the readers' tests:

| File | SHA-256 | Made as |
|---|---|---|
| `sample-basic.csd` | `b405685a744df0dd4395279c561822aea4010d8460c7887404aaa9b7e52b841b` | Export, type Singer (*.CSD). |
| `sample-basic.dst` | `c29378ba32fda42d1b5e5499cdec5c4594ab70ac49dc5fa69ee710000c8b855b` | Export, type Tajima (*.DST). |
| `sample-basic.shv` | `508c945b5a4e9047da8cda1fd69240a6add83cd9183d1460bffdb3ba0c9dc727` | Export, type Husqvarna/Viking (*.SHV). |

## Round 4 (2026-10-30): Manual Punch blocks

Made by the owner's PE-DESIGN 11.50 (Layout & Editing) on Windows 11 Pro 10.0.26300,
2026-10-30 08:58-09:06 (m09-m11 later the same day), by black-box GUI use only (session pe-refs-4, round4/log.txt). App menu
> New (Hoop Size 100 x 100 mm, grid snap on); Home > Shapes > Manual Punch row (first icon
Straight Block, second Curve Block); clicks at page mm from the top-left. A block object comes in
with line Not Sewn, region Satin Stitch, 007 PRUSSIAN BLUE (the session's last colour), under
sewing on, density 4,5, Direction "Manual straight line" (angle greyed: the block sets it), Half
Stitch on, Running Stitch Path first. Save As "Layout & Editing Ver.11.0 File" (#PES0110).
`m06-straight-1-blue` (m01 with 007 chosen explicitly) equals m01 but for the PEC label and is
not kept.

| File | SHA-256 | Made as |
|---|---|---|
| `m01-straight-1.pes` | `0197c470e310435900c7810747d7f97bd41268ecfe887bfb9f0ffe9f95419ec6` | Straight Block: clicks (10,10) (10,30) (30,10), double-click (30,30). One block. 20,00 x 20,00 mm, 208 st. |
| `m03-straight-3.pes` | `2f6a091c27ee6a13511540200e5410e5aa463c0f4a2b5017bf53b14da9b39244` | Straight Block: clicks (10,10) (10,30) (30,10) (30,30) (50,10) (50,30) (70,10), double-click (70,30). One object, 60,00 x 20,00 mm, 550 st. |
| `m04-straight-1-fill.pes` | `d169dcc1cfbd659d455496468fb8f3cd293a02ab76980bd2e199f8c1e4e7be53` | m01, then region Fill Stitch (the block offers the whole region list): under sewing on, density 4,5, direction greyed at 45, step 4,0, frequency 30 %. 652 st. |
| `m05-straight-1-density.pes` | `23ebb6122a4c4778f742461ccbb2d5b4bd83d4865070a44cf57cc00c49a271f5` | m01 (Satin), then density 6,0. 238 st. (Saved as "4,5.pes" by a missed click and renamed; content unchanged.) |
| `m07-curve-1.pes` | `b970e05335f2d22649b139b7491ba5370b25209e7eed9bec5f04ccb631cab6c2` | Curve Block, the same four clicks as m01. 20,00 x 20,00 mm, 208 st. |
| `m08-straight-1-line.pes` | `4f46a7198c97f11a443e139eae84b6316ea600a5e870a7bb31de6f441443a553` | m01, then line Running Stitch (run pitch 2,0 mm, run time 1, 007). 253 st. |
| `m09-curve-bent.pes` | `9cc2dee22b3f5821ec5671751016e35cd6d580d4fc296a903fb991b5a3189f66` | Curve Block: clicks (10,10) (10,30) (30,20) (30,40) (50,10), double-click (50,30); the block sags through the middle pair. 40,00 x 30,00 mm, 421 st. |
| `m10-curve-uneven.pes` | `ba4087375f473715217951a32b1b1f54f9cdcaac387e76dcb96c213c06124142` | Curve Block: clicks (10,10) (10,30) (20,25) (20,45) (60,10), double-click (60,30); unequal chords. 50,80 x 35,00 mm, 529 st. |
| `m11-curve-4.pes` | `ce26049eadee96817d4bb716df8dc7bc2af4597a7762e7afc934820cef72d837` | Curve Block: clicks (10,10) (10,30) (25,25) (25,45) (45,25) (45,45) (60,10), double-click (60,30); four lines. 50,00 x 37,00 mm, 537 st. |

## Round 5 (2026-10-30): Convert to Blocks

Made by the owner's PE-DESIGN 11.50 (Layout & Editing) on Windows 11 Pro 10.0.26300,
2026-10-30 10:37-11:04, by black-box GUI use only (session pe-refs-4 for the pattern-formats
track, round4/log.txt "STROKE-WISE LETTERING", round5/text-strokes). App menu > New (Hoop Size
100 x 100 mm); Home > Text > Text tool, Lato (Lato-Bold.ttf from crates/openstitch-design/fonts,
installed for the user), size 20,0, line Not Sewn, region Satin at the text defaults (under
sewing off, density 5,0); click (50,50) mm; the letter; Select tool; Text ribbon > Convert to
Blocks; Save As "Layout & Editing Ver.11.0 File" (#PES0110).

| File | SHA-256 | Made as |
|---|---|---|
| `stroke-blocks-H.pes` | `f687c993943d9f3323713a8f1bfa1bc696ec48e69834c28d42e1e574066b59f0` | H converted to blocks: one group of five blocks and three running pieces; 285 st (as the text). |
| `stroke-blocks-A.pes` | `01e8ebc43e92ce52a5bc97659dbbf37bdd367473799bc67ede5bd4946218b2b7` | A converted to blocks: one group of five blocks and four running pieces; 293 st (the text: 290). |
## Round 4 (2026-10-30): Applique Wizard

Made by the owner's PE-DESIGN 11.50 (Layout & Editing) on Windows 11 Pro 10.0.26300,
2026-10-30 09:06-09:10, by black-box GUI use only (session pe-refs-4, round4/log.txt). App menu
> New (Hoop Size 100 x 100 mm); Rectangle tool (10,10)-(30,30) mm at the tool state (line Not
Sewn, region Satin Stitch, Direction 45, under sewing on, density 4,5, 007), 280 st; Ctrl+A,
Home > Wizards > Applique Wizard; Save As "Layout & Editing Ver.11.0 File" (#PES0110); Ctrl+Z
back to the square before the next variant. Dialog defaults: Applique Material Yes / Running
Stitch, Applique Position, Tack down Yes / V Stitch, Covering Stitch Zigzag 3,0 mm / 4,5
line/mm, Output Pattern Add, Distance 3,0 mm.

| File | SHA-256 | Made as |
|---|---|---|
| `a01-rect-applique.pes` | `5d93cddbf8afad0f280a7f3b4a931a8473fe7c2692763c7ef017232e75e95931` | Dialog defaults: four Sewing Order items (square; material; position; covering group with the tack-down). 32,60 x 32,80 mm, 1063 st; PEC colours 1, 62, 63, 64. |
| `a02-applique-cutting.pes` | `15ffc177e7850e94283a37f6d996fe44927374d6b674e1a0bb1295ab3c5e6c3b` | Applique Material Cutting: 1390 st; PEC colours 1, 65 (four cutting blocks), 63, 64. |
| `a03-applique-no-tackdown.pes` | `fee6f2f751623e9ebeb3701ac6c5e00e1b97bd50b51d3775d178b005f28a5b7e` | Tack down No: 970 st; PEC colours 1, 62, 63, 64. |
| `a04-applique-cover-4mm.pes` | `d446d00ea341cdcd5953d0c86530e77ef810391ac513e51d35300196b8d3c021` | Covering width 4,0 mm: 34,60 x 34,70 mm, 1107 st. |
| `a05-applique-replace.pes` | `ed820109683b353ef717c697dcffcc280a15598a7399b7e56dcbc55226121ad1` | Output Pattern Replace (distance greyed): the square replaced; three items, 23,00 x 23,00 mm, 537 st; PEC colours 62, 63, 64. |
