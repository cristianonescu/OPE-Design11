# Asset audit — 2026-10-30

Where the editor lacks its own icon, illustration or other graphic, made against `5b8f1cb`
(origin/main). The owner asked on 2026-10-30 that missing resources be generated with
Higgsfield, like every other asset ([LEGAL_AND_LICENSING.md](../LEGAL_AND_LICENSING.md)).

## How the audit was made

- **Commands.** A script listed the 134 commands of the Layout & Editing window
  (`editor/ribbon.rs`) with the glyph each draws (`icons.rs`), and the 111 commands of the
  Programmable Stitch Creator, Stitch Design Factory and Design Database (`editor/suite.rs`)
  with the editor command whose icon each borrows. At `5b8f1cb` 134 commands share 75 icons,
  so 59 commands show an icon drawn for another command.
- **Text-only places.** `crates/openstitch-app/src` was searched for empty states, dialogs that
  open on a bare sentence, and symbol characters used as icons.
- **Screens.** `tools/screenshots.py` was run before and after (`--screenshot`, scripted
  scenes; no screen control), and the images in `docs/images` were checked by eye.

Arrows such as ▲ ▼ ↑ ↓ on small reorder buttons, × and ⧉ on pane title bars and ▾ on
dropdowns are standard text controls and stay as they are.

## Priorities

- **P1**: the icon says something else, or the opposite (Connect drew a split, Refresh drew
  Redo, Parent Folder drew Undo), or a place is text only where a picture is expected.
- **P2**: the icon is a sibling's (five image commands share one picture); it is not wrong,
  but the buttons cannot be told apart.
- **P3**: the shared icon fits well enough (all zoom tools, Search as a magnifier); no change
  planned.

A full set is about 105 images (about 263 credits at 2.5 each). Following the rule for sets
over about 60 images, the P1 half was generated first: 50 icons and 3 illustrations (plus
three regenerated, 140 credits in all, balance 1,564.75 before). The owner approved the P2
half the same day: 45 icons and 4 illustrations plus one regenerated, 125 credits (see Phase 2
below).

## Editor (Layout & Editing) commands

| Place | Current state | Asset needed | Priority | Status |
|---|---|---|---|---|
| Connect | split icon (the opposite) | two lines joined at a dot (`connect`) | P1 | done |
| Divide by Color | split icon | square in three colour strips (`divide_color`) | P1 | done |
| Split Stitch Data | split icon | zigzag broken in two (`split_stitches`) | P1 | done |
| Import to Design Center | Auto Punch's wand | sheet with a star and an arrow (`design_center`) | P1 | done |
| Sew Embroidery (Send) | needle icon | sewing machine (`machine`) | P1 | done |
| Programmable Stitch Creator | needle icon | lattice fill tile (`stitch_creator`) | P1 | done |
| Stitch Design Factory | needle icon | ornament tile (`stitch_factory`) | P1 | done |
| Font Creator | font icon | letter with edit nodes (`font_creator`) | P1 | done |
| Text Size | font icon | large and small T (`text_size`) | P1 | done |
| Font Filter, Font Filter Settings | font icon | funnel with a letter (`font_filter`) | P1 | done |
| Running | open line | red running stitch (`running`) | P1 | done |
| Feed | open line | thin line with an arrow (`feed`) | P1 | done |
| Solid View | eye | solid blue circle (`view_solid`) | P1 | done |
| Stitch View | eye | parallel stitch lines (`view_stitch`) | P1 | done |
| Realistic View | eye | glossy thread rows (`view_realistic`) | P1 | done |
| View Thread Trimming | eye | scissors cutting a thread (`thread_trim`) | P1 | done |
| Sewing Order pane | generic pane | stacked colour cards (`pane_order`) | P1 | done |
| Stitch Simulator pane | generic pane | play button over stitches (`pane_simulator`) | P1 | done |
| Attributes Setting pane | generic pane | slider panel (`pane_attributes`) | P1 | done |
| Reference Window | generic pane | map with a view frame (`reference_window`) | P1 | done |
| Delete Direction Line | Add Direction Line's icon | arrow crossed out (`dir_delete`) | P1 | done |
| Reverse Direction Line | Add Direction Line's icon | U-turn arrow (`dir_reverse`) | P1 | done |
| Center/Reference Point | Delete Point's icon | crosshair (`sewing_points`) | P1 | done |
| Show Grid with Axes | grid | grid with red axes (`grid_axes`) | P1 | done |
| Fabric Selector | palette | folded cloth (`fabric`) | P1 | done |
| User Thread Charts | palette | card of spools (`thread_chart`) | P1 | done |
| Measure | ruler (same as Ruler) | tape measure (`measure`) | P1 | done |
| Paste / Save / Modify Image, Background Image Density | Open Image's picture | picture with clipboard / disk / pencil / half-tone | P2 | done (phase 2) |
| Rotate Left / Right, Delete Image | Rotate/Flip, Delete | picture with a turn arrow; picture with a cross | P2 | done (phase 2) |
| Split at Point, Delete Point | split, point | node with a cut; node with a cross | P2 | done (phase 2) |
| Design Settings vs Options | same gear | page with a gear for Design Settings | P2 | done (phase 2) |
| Sewing Attribute Sets | Region sew type | stack of attribute cards | P2 | done (phase 2) |
| Edge radius, Column width, Straight/Curved Block, Satin Skeleton, Satin Skeleton from Shape | rectangle, size, polygon, curve, outline | rounded corner; satin column width; block tools; skeleton line | P2 | done (phase 2) |
| Export Sections | hoop | hoop halves with an export arrow (`sections_export`); Move Sections keeps the hoop | P2 | done (phase 2) |
| Photo Stitch 2 | Photo Stitch 1's | photo in a second style | P2 | done (phase 2) |
| Edit Character, Pencil (Open Curve), Convert Region to Line, Convert to Outlines, Change Color | sibling icons | own variants | P2 | done (phase 2) |
| Edit Point, Open Curve, Import SVG, Paste Attributes, Text and Grid settings | sibling icons | roof-shaped point; open S curve; page with a curve; dropper with a drop; letter with a gear; grid with a spacing arrow | P2 | done (phase 2) |
| Zoom tools, Rectangle | shared within a family | — | P3 | keep |

## Suite windows (Stitch Creator, Stitch Design Factory, Design Database)

| Place | Current state | Asset needed | Priority | Status |
|---|---|---|---|---|
| Database ▸ View ▸ Refresh | Redo | circular arrows (`refresh`) | P1 | done |
| Database ▸ Parent Folder | Undo | folder with an up arrow (`parent_folder`) | P1 | done |
| Database ▸ Create New Folder | Open's folder | folder with a plus (`new_folder`) | P1 | done |
| Database ▸ Rename | Edit Text | text field with a cursor (`rename`) | P1 | done |
| Database ▸ Large / Small Thumbnails | Shapes, Grid | 2×2 and 3×3 tiles (`thumbs_large`, `thumbs_small`) | P1 | done |
| Database ▸ Details | Sewing Order pane | list rows (`details`) | P1 | done |
| All three ▸ Status Bar; Database ▸ Toolbar | Attributes pane | window with a bottom bar / top strip (`status_bar`, `toolbar`) | P1 | done |
| Database ▸ USB Link, Wireless Link, Card Reading, Writing List | needle, Import | USB stick, signal arcs, memory card, list with arrow (`usb`, `wireless`, `read_card`, `writing_list`) | P1 | done |
| Database ▸ Create CSV, Convert Format | Export | table page; two pages with an arrow (`csv`, `convert_format`) | P1 | done |
| Database ▸ Print Preview, Print Setup | Print, Design Settings | page with a magnifier; printer with a gear (`print_preview`, `print_setup`) | P1 | done |
| Creator, Factory ▸ Flip vertically / horizontally | Rotate/Flip | mirrored triangles (`flip_v`, `flip_h`) | P1 | done |
| Creator ▸ Region (Engrave), Region (Emboss) | Stamp | sunken and raised plate (`region_engrave`, `region_emboss`) | P1 | done |
| Factory ▸ Swap | Connect (then a split icon) | two opposite arrows (`swap`) | P1 | done |
| Creator ▸ New Fill/Stamp, New Motif, New Decorative Fill | New page | page with a fill / motif / ornament | P2 | done (phase 2) |
| Creator ▸ Region (Reset), Add to Design | Stamp, needle | plate reset; tile into a design | P2 | done (phase 2) |
| Database ▸ Create Image File and HTML, DST Settings, Network Machine Settings, Customer Support | Open Image, gears, Help | picture with a page; gear variants; headset | P2 | done (phase 2) |
| Factory ▸ Needle Drop Point, Horizontal Orientation | crosshair, rotate | own variants | P2 | done (phase 2) |
| Creator, Factory ▸ Open / Modify Template, Preview Window; Database ▸ Preview; Factory ▸ Stitch Simulator | picture, pane icons | folder with a template sheet; sheet with a pencil; preview window; needle with a play button | P2 | done (phase 2) |
| Database ▸ Search, About, Manual, Application Color, Select System Unit | magnifier, info, help, palette, ruler | — | P3 | keep |

## Empty states and dialogs

| Place | Current state | Asset needed | Priority | Status |
|---|---|---|---|---|
| Color pane, "Nothing is sewn yet" | text only | empty spools (`sew_empty`) | P1 | done |
| Send to USB Media, "No USB stick or SD card is mounted." | text only | USB stick and card by an empty slot (`no_media`) | P1 | done |
| Modify Image, "No background image" | text only | empty picture frame (`no_image`) | P1 | done |
| Objects list, "No objects…" (`panels.rs`) | text only | empty tray illustration | P2 | done (phase 2) |
| Update dialog (available, up to date, failed) | text only | three small update illustrations | P2 | done (phase 2) |
| Stitch Creator pattern library, "No user folder" | header illustration exists | — | P3 | keep |

## Brand and packaging

The app icon (SVG, PNG 16–1024, `.ico`, `.icns`), wordmarks, splash, About window and
Application button come from the Higgsfield logo in `assets/brand` and are complete. No gap.

## Phase 2 (2026-10-30)

Approved by the owner after the P1 PR (#215). Generated with the same prompt templates: 45
icons and 4 illustrations (`objects_empty`, `update_available`, `update_current`,
`update_failed`), plus `import_svg` a second time (the first read as a crown), 125 credits.
`sections_move` was generated but left out: Move Sections keeps the existing hoop icon, which
would otherwise be unused. Every command in the four windows now has its own icon except the
P3 families above. To add more, generate with the prompts in `assets/icons/commands/README.md`
and `assets/illustrations/README.md`, list the file in `own!` in `icons.rs` and map it in
`glyph` (editor commands) or `suite_glyph` (suite commands, by key).

## Round 3 (2026-10-31)

Approved by the owner: the P3 icons that were still generic plus the update dialog's last
text-only states, 12 images, 30 credits. Icons: `auto_punch` (shared the Wizard wand),
`edit_text` (shared the Text tool), `font_filter_settings` (shared Font Filter),
`triple_stitch` (Factory, borrowed Running), and the Design Database's `db_search`, `about`,
`manual`, `app_color`, `system_unit`. Illustrations: `update_downloading`,
`update_restarting`, `update_unavailable`. The Recent Files list and the Background Image
Density menu reuse `db_empty` and `no_image` (#222). What still shares an icon: the zoom tools,
and suite commands that mean the same as an editor command (Open, Save, Cut, Copy and so on).
