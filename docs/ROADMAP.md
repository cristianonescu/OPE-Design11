# Roadmap

| Phase | Goal | State |
|---|---|---|
| 1 Tooling | Rust workspace, CI on three OSes, fixture generators | Done. |
| 2 Reference research | Guide-based feature inventory; reference files from a licensed PE-DESIGN 11 | Done: inventory from the public guide; reference files made in a licensed PE-DESIGN 11.50 since 2026-10-27 (`compat/reference/`, provenance in `MANIFEST.json`). |
| 3 Core engine | Stitch model, threads, document operations, stitch generation | Done: design model, all guide sew types, compiler, command registry, undo, `.osd` projects, CLI and JSON control, MCP. |
| 4 File interoperability | Every format PE-DESIGN 11 reads or writes; round-trip tests | DST, EXP, JEF, PCS, SEW, XXX, HUS and VIP written record for record as PE-DESIGN 11.50 exports them (reference tier, 2026-10-28); PES, PEC and the readers at oracle tier with fuzzing; SHV, PHC, CSD read-only; PES v1/v6 object section; PE-DESIGN 11 (`#PES0110`) metadata, thread table and editable shapes read, and shapes written as editable objects (`pesVersion` 11, 2026-10-29, unverified in PE-DESIGN); SVG and image import; split exports. |
| 5 Desktop interface | Cross-platform editor (egui) | Done: drawing, Bézier pen and point editing, selection, inspector, object list, undo, simulator, wizards, Design Database mode, Stitch Creator, Stitch Design Factory, printing, USB transfer, self-update, unsaved-changes prompt on close/new/open (macOS: Cmd+Q and Cmd+W too, no system menu). |
| 6 Advanced | Digitizing, lettering, complex fills, suite programs | Done as independent approximations: Auto Punch, Cross Stitch, Photo Stitch, Design Center; full lettering suite and Font Creator. |
| 7 Validation | Reference suite, gap closing | In progress: the compatibility harness (`stitchtool compat`) compares OpenStitch's output with PE-DESIGN 11.50's files spec by spec; [COMPATIBILITY_STATUS.md](COMPATIBILITY_STATUS.md) shows the tier each row has reached. No sew-out on a machine yet. |

## Platform decision (2026-10-18)

The target is every OS, so OpenStitch is a Rust workspace (`crates/`) with one egui/eframe app
for macOS, Windows and Linux. CI builds and tests all three.

## Done (2026-10-19)

- **Drawing and editing:**
  - Bézier outlines and point editing; arcs and preset shapes; group/ungroup; arrange copies
    with a canvas preview and drag placement; snap to grid.
  - Merge, remove overlap, boolean combine, split, offset lines (`i_overlay`).
  - Paste sewing attributes; entry/exit points with draggable markers.
  - Floral pattern and original decorative frames; embossing, engraving and stamps.
  - Sewing order (`order.move`, `order.byColor`, `order.optimize`) with per-object jump trimming;
    `order.optimize` in PE-DESIGN's mode by default, with OpenStitch's shortest-jump (2-opt) mode
    as a design setting (beyond PE-DESIGN, #137).
  - Sewing direction (round fill, centre and reference points); Manual Punch; Satin Skeleton
    tool and skeletons derived from shapes.
  - User settings: saved attribute sets and user thread charts.
- **Data types:** conversions between shape, text and stitch patterns, region → line, and
  stitch-pattern editing (needle points, split, connect, divide by colour).
- **Stitch types:**
  - Line types: motif (own library), stem, candlewicking, E/V, chain, cutting, zigzag half stitch.
  - Satin and fill options: auto direction, underlays, gradation, feather, split, travel.
  - Region types: programmable fill, motif fill, piping, cross stitch, concentric circle,
    radial, spiral, flexible spiral, stippling, net fill, zigzag net fill, decorative fill;
    concentric circle and spiral calibrated to PE-DESIGN 11.50 (#138, 14 references;
    open: the spiral's inner-turn needle counts, the concentric chord splits and its seam on
    Bézier outlines; the spiral's winding and closing run settled with round 5).
- **Lettering:** text objects with Lato Bold and Pacifico (SIL OFL), text on a path, user
  TrueType/OpenType fonts (CJK through any font with the glyphs), shaping of right-to-left and
  complex scripts (`rustybuzz`, `unicode-bidi`), kerning, vertical text, per-character edits
  (on the canvas with Point Edit),
  user-mapped text, monograms, Name Drop, templates and the Template Wizard, Font Creator (including
  characters traced from an image in one step).
- **Wizards:** background fill (echo, stippling, decorative, crosshatch, stripes), appliqué,
  embroidered patch, cutwork.
- **Suite programs:** Programmable Stitch Creator (`openstitch.pattern`), Stitch Design Factory
  (`openstitch.sewing`, SVG and experimental PMV export and import), Design Database (`openstitch-library`:
  browse, search, organise, convert, catalogues; `library.*` commands).
- **Machine and workspace:** machine types, Brother and user hoops, sewing area, thread length
  and sewing time, measure tool, rulers, grid, Reference window, realistic preview; split
  designs, multi-position hoops and jumbo frame; printing (actual size, reduced, positioning
  sheets, Print and Stitch; `openstitch-print`); transfer via USB media.
- **Automation:** per-command capabilities (read-only mode, file access off) for MCP and
  `stitchtool serve`; loopback socket transport for the live editor (`openstitch --listen`,
  per-launch token, file roots; `stitchtool connect`, `openstitch-mcp --live`).
- **Distribution:** per-OS packages on every merge to `main` (macOS universal `.app`, Windows
  MSI + zip, Linux tarball with thumbnailer, Linux AppImage with per-user desktop integration);
  Flatpak manifest built in CI; tag-only draft releases.

## Done (2026-10-27)

Everything below was measured black-box on a licensed PE-DESIGN 11.50 through its normal
interface, with the resulting files committed under `compat/reference/` and compared by the
harness. The details per item are in [COMPATIBILITY_AUDIT.md](COMPATIBILITY_AUDIT.md) and
[COMPATIBILITY_TESTS.md](COMPATIBILITY_TESTS.md).

- **Fill:** PE-DESIGN's defaults (4.5 lines/mm, under sewing on), its under sewing structure,
  density and pull-compensation ranges and effects, Frequency as a row-to-row needle shift,
  gradation as PE's Max/Min pattern with a linear fall, the feathered edge's width and the
  guide's exclusions.
- **Satin:** PE-DESIGN's sawtooth, tie-in and lock, under sewing, section order, split at the
  start point, feathered edge; Auto satin's spacing, corner fan, band ends and start travel.
- **Lines:** PE-DESIGN's defaults, tie-in and lock for running, triple, zigzag (mitred rails,
  under sewing, half stitch), E/V, stem, candlewicking and chain.
- **Lettering:** text size (0.7 em) and line spacing (% of the character height), the text
  tool's sewing defaults (Satin, density 5.0, no under sewing), sewing order inside a text, Fit
  Text to Outline's options, 24 of the 26 Text Transform presets.
- **Shapes and order:** the shape tools' start and end points; Optimize Sewing Order keeps the
  drawing order within a colour; direction angles turn counter-clockwise on screen.
- **PES:** PE-DESIGN opens OpenStitch's v6 files with their stitches, colours and Design
  Property; Design Property strings in Windows-1252 with CR LF; no zero-length stitch records;
  trims kept PEC-only as PE-DESIGN does; the reader parses PE-DESIGN 11's `#PES0110` metadata,
  thread table and editable shapes (`CEmbLine`; chapter "PES 0110 object list" in
  [FILE_FORMATS.md](FILE_FORMATS.md)); a missing PEC section is an error.
- **Editor and project:** OpenStitch logo, command icons and illustrations generated with
  Higgsfield (code-drawn icons as the fallback); DirectX 12 on Windows with OpenGL as the
  fallback; a gitleaks job in CI; the repository renamed to `cristianonescu/OpenStitch`;
  releases v0.1.0-alpha.1 and v0.1.0-alpha.2 ([docs/releases](releases/)).

## Still to do

Assets (owner's rule of 2026-10-30, [research/asset-audit.md](research/asset-audit.md)): the
audit is done in two rounds (93 command icons for commands that showed another command's icon,
including the suite windows' own commands, and seven empty-state and dialog illustrations, all
generated with Higgsfield, 265 credits). New windows and commands get their icon the same way
when they are added.

Compatibility, from the open items in [COMPATIBILITY_AUDIT.md](COMPATIBILITY_AUDIT.md):

Done 2026-10-30: stitch generation is bit-identical on Windows, macOS and Linux (transcendental
math through the `libm` crate instead of each platform's C library); the arched Text Transform
spec, deferred at alpha.6 because Linux measured it 0.2 mm lower, has its PE-DESIGN reference
back.

1. **Fill:** Round 4 (2026-10-29) confirmed PE-DESIGN has no random tatami stagger
   (Frequency and two Stitch Type buttons only) and a 0.1 lines/mm density floor. PE-DESIGN's row structure (halves from the start point, brick row ends, 30 %
   Frequency default); the programmable fill density; Stitch Type (turning ends) and Half
   Stitch; feathered-edge patterns; gradation presets, per-step editing and Blend. Gradation
   rows follow PE-DESIGN's cumulative-density placement since 2026-10-28 (#95: every row of
   the U within 0.12 mm, overlap 0.83 → 0.89); left for the U: PE's section order for fills in
   several parts (right leg, left leg, then the top bar ending at the first vertex). Under
   sewing Density Dense / Medium / Light and the combinable under sewing parts (edge, rows,
   lattice, piping) are done since 2026-10-29 (#124: sweep-fill-us-density-dense/-light at the
   reference tier; the second type, `cross`, since 2026-10-28, specs sweep-fill-us-type-2 and
   sweep-satin-us-type-2); left: the button-to-capture mapping of a Manual Punch column's five
   under sewing types once pe-refs-4 notes it, and the zigzag's exact split points.
   Density floor: 0.1 lines/mm for fill and satin, as PE-DESIGN 11.50's field and the
   `sweep-*-density-min` references (#139's 0.2 floor was checked against them on 2026-10-29
   and not adopted; `attribute-ranges` covers the boundaries).
2. **Satin:** Half Stitch for Auto satin (the corner fan now produces the crowding it acts
   on); Miter limit. Auto satin's needle lattice (whole spacings from the far end, the
   remainder at the start point), its forward start travel and its end on the start point
   follow PE-DESIGN since 2026-10-28 (satin-auto-bend-60: mean needle distance 0.20 → 0.09 mm);
   left: PE's exact remainder handling on the start leg (0.05 mm) and the fan's inner crowding
   (0.3-0.4 mm against our 0.25-0.3 mm), both below the file's 0.1 mm grid in effect. Done 2026-10-28 as well: the gradation pattern on a split
   satin (satin-gradation, needle for needle). Also done 2026-10-28: the minimum-density
   sawtooth (sweep-satin-density-min, needle for needle: the crossings count back from the
   start point's line, which the round-3 column at 0.2, 0.3 and 0.5 lines/mm confirmed record
   for record). Done 2026-10-29 (#122): plain satin has no minimum-crossing threshold (the
   far end's single-line gap is crossed at 0.22–5 mm, pe-refs-4's satin-length-sweep), its
   lattice stretches to whole gaps, and a column started at one end is sewn from its far end
   back to the start point; left: PE's entry/exit optimisation between objects (it moves start
   points). The Auto satin fan on sharp bends follows PE-DESIGN since 2026-10-30
   (satin-auto-bend-120 at the reference tier: 171 stitches to PE's 161, was 273).
3. **Lines:** Start/End Type Feed/Run for stem, candlewicking and chain, with the L2 marker and
   the Feed/Run choice in the editor; open lines and chain diamonds; candlewicking Low and
   High. The motif line cannot be compared: PE-DESIGN's default motif is Brother's own artwork.
4. **Lettering:** stroke-wise sewing of TrueType text as PE-DESIGN does it (every letter is
   split into strokes, each sewn as a column with its rows across the stroke, for Fill and Satin
   text alike; the 'H' references match only because an H's stems are vertical; measured and
   planned in [research/stroke-lettering.md](research/stroke-lettering.md), 2026-10-30; the
   captures are in and the stroke cut (step 1, `strokes::strokes`) is done; next, satin and fill
   per stroke, then the order, travel and trims) — the structural
   gap behind text-fit-*, text-transform-* and text-sewing-order; the fitted circle's running
   stitch follows PE-DESIGN's Bézier halving since 2026-10-28 (text-fit-default mean needle
   distance 0.27 → 0.20 mm); Text Transform presets 13 and 14 (fan out / in) done 2026-10-28; round 4 (2026-10-29)
   showed PE-DESIGN's transform level is a dragged handle whose value is stored per file
   (percent of the font size, degrees for slants, fans and arches): the level parameter and the
   cap-height box are in since 2026-10-30 (`envelopeLevel`; 105 of 108 level captures and 51 of
   52 default-level captures at the reference tier); left: arch 2 on "Agjy 123" and at level 25,
   taper 4 and slant 9 at level 100 (0.6-1.1 mm).
   distance 0.27 → 0.20 mm; #121 kept it and lifted the 256-chord ceiling, 2026-10-29); Text
   Transform presets 13 and 14 (fan out / in) done 2026-10-28.
   Left: per-character colour and
   multi-character selection; Connection Points; minimum font sizes and small-text fonts;
   kerning that reflows the line; editing several characters at once on the canvas; shaping for
   embroidery fonts.
5. **Machine stream:** the per-format compares of DST, EXP, JEF, PCS, SEW, XXX, HUS and VIP
   against PE-DESIGN's exports pass since 2026-10-28 (the writers follow PE-DESIGN's record
   layout; FILE_FORMATS.md "PE-DESIGN's record layout"; the SEW stream from PE-DESIGN's own
   start point since the round-3 SEW exports). Since 2026-10-28 a move of up to 2.3 mm between
   same-colour objects is sewn across as PE-DESIGN does (machine-jumps, machine-trim-bound,
   machine-trim-fine). Left: how the folding formats write a move shorter than one record (one
   export each, listed in the audit); PE's optimized exit and entry 0.5 mm inside the edges
   (fill track) and the multi-needle minimum-length field's value; the same compares on a
   design with short moves. The parked references (satin gradation, Text Transform presets 13
   and 14, under sewing type 2, satin density minimum, the trim bound) have all landed.
   Round 4 (2026-10-29): PE-DESIGN's DST divides an over-long move evenly into
   121-unit records (long-jump-300, reference tier) and its oversize export carries no
   hoop check (hoop-oversize-export). Parked with their numbers: the ten-format export of
   a two-colour design (the PES reader drops PE-DESIGN's tie-in needle points, 4 per
   file; `stitches.import` of a DST merges its two colours; the CSD reader misreads a real
   file; SHV one colour change over), PE-DESIGN's PES of the 350 mm move (PEC encoding
   beyond 204.7 mm unknown) and of the oversize design (211 x 229 mm read for 129 x 105).
6. **PES:** PE-DESIGN 11's own `#PES0110` layout is written (`pesVersion` 11, issue #120):
   shapes as `CEmbLine`, Lato Bold text as `CEmbNText`, Text Transform text as `CEmbDText`,
   fitted text as `CEmbAText`, gallery shapes as `CEmbSpecialShape`, stitch patterns as `CEmbOne`
   stitch groups, Manual Punch Straight and Curve Blocks and the Applique Wizard's objects
   (2026-10-29/30). PE-DESIGN opens the shapes, the text, the Text Transform and the fitted text
   editable (references `pes-0110-editable-shapes`, `-text`, `-text-transform`, `-text-fit`). The
   selected hoop is written to the v6 header in mm and an over-size export warns instead of
   failing, as PE-DESIGN does (2026-10-29, #125), with its "Design Page Is Custom" and "Hoop
   Rotation" flags and a custom page's size since 2026-10-30 (opened in PE-DESIGN 11.50 as the
   hoop and page they describe). Round 4 added owner-drawn Programmable Stitch
   Creator sources (`.pas`, `.pmf`, `.plf`), imported since 2026-10-29 (#131); Font Creator fonts
   wait for the owner (#134). Left: PE-DESIGN opening a written stitch group
   (`pes-0110-stitches`), Straight and Curve Block (`pes-0110-manual-punch`,
   `pes-0110-curve-block`) and appliqué (`pes-0110-applique`; the covering line's fields of unused
   sew types not mapped); the fitted text's box as PE-DESIGN computes it (PE-DESIGN recomputes it
   on opening); PE-DESIGN's confirmation of the Hoop Size page (probe p3); the design's machine
   type and hoop in the header; turn the `#PES0110` shapes and texts the reader parses into
   editable OpenStitch objects; right-aligned and vertical text; the other region and line types,
   hole-sewing pairs and several outlines per object; the hoop in v1 (the public
   description contradicts itself); PES v7–v10 (from the owner's own files only, as
   [LEGAL_AND_LICENSING.md](LEGAL_AND_LICENSING.md) allows).
7. **Editing details:** skeletons that branch (a derived skeleton follows a shape's longest
   stroke only) and an exact medial axis instead of the grid approximation.
8. **Formats:** CSD export (no public writer). The CSD and SHV readers match PE-DESIGN's own
   exports since 2026-10-29 (#135), PEC colour indexes are the plain nearest entry per
   block as in PE-DESIGN's own files (2026-10-30), and a DST import keeps its colour blocks apart. The eight
   machine-format writers (DST, EXP, PCS, HUS, VIP, JEF, SEW, XXX) match PE-DESIGN's exports of
   its own design since 2026-10-30 (`machine-sample-basic`, zero-length stitches aside and
   counted per side in the report; PCS and HUS write a jump landing twice as PE does, so only
   the two repeats PE's PES cannot carry are left). PE's PEC keeps zero-length stitches (28 of
   249 references), so the planned "drop them in the PEC writer" was not done. PEC moves over
   204.7 mm split as PE-DESIGN's DST does since 2026-10-29 (#126); PE's own PEC does not encode
   such a move (its `long-jump-300` PES has a stand-in row instead), settled 2026-10-30. PE's
   oversize PES has an empty machine section, which the reader reports instead of misreading
   (2026-10-30). Thread charts: OpenStitch's own CSV is documented and exported since 2026-10-29
   (#136; PE-DESIGN keeps charts as binary `.btc2`, no CSV). Colour matching is equal-weight
   squared Euclidean with an "approximate" city-block matcher since 2026-10-29 (#119), checked
   against the owner's twelve-swatch capture; the 61 BROTHER EMBROIDERY catalogue codes are known
   since the round-4 chart reading, PEC 42 (307 LINEN) has PE-DESIGN's RGB and every coded thread
   in PE's files matches the chart (2026-10-30), and SVG imports snap to them as PE-DESIGN's do
   since 2026-10-30 (`thread-match-swatches` compares the thread table). The 94-entry sew palette is
   listed since 2026-10-30 (PE's Ver4 Option Color chart as ids 66–94, `sewPalette`; the 0110 writer writes those
   threads with their ids as PE does); where
   PE-DESIGN matches colours to it is not verified (no capture). Left: the user-chart CSV schema
   (#136), per-machine export limits and hoop codes (#125).
9. **Machine tier:** a first sew-out of an OpenStitch-written file on a Brother machine, recorded
   in `compat/machine/`. No spec has reached this tier.
10. **Distribution:** code signing (owner's certificates); Flathub submission (offline cargo
    sources, owner's account). Blocked on `unsafe` FFI, which the workspace forbids: macOS
    Apple-Event file opening (winit 0.30/eframe 0.37 deliver no file-open event; a delegate
    needs objc2 `define_class!` with `unsafe` blocks, see DISTRIBUTION.md "macOS double-click")
    and PES thumbnails on macOS and Windows.
11. **Image digitizing (Phase 6, #127 #128 #132, #129 optional):** Photo Stitch mono tone
    pipeline done 2026-10-29 (#128: integer 30/59/11 luminance, full-range min/max stretch
    with the (1, 255) guard); Auto Punch's 3×3 modal colour census capped at 100 colours, the
    `max(4, round(param · 100 / mmPerPx²))` small-region floor at 4-connectivity and the
    centerline-thinness satin-vs-fill test done 2026-10-29 (#127); Cross Stitch shares that
    prepass with documented grid and per-cell defaults since 2026-10-29 (#132); Auto Punch's
    satin bounds hold at every working resolution since 2026-10-30 (exact stroke widths for
    even pixel counts), its fill direction and region order are PE's on every capture since
    2026-10-30 (largest row spread; scan order alternating by colour; specks kept and dropped
    as PE; PE's thread picks after reduction checked on 20 flat colours: a per-colour mapping
    no tested distance reproduces, kept as a documented difference). Compared with
    PE-DESIGN 11.50's round-4 captures on 2026-10-29: Cross Stitch's six diagonal stitches per
    cell (−0.9 % stitches), Photo Stitch 1's Sew Page Color, 3.0 mm Run Pitch and 10 colours
    (+5 %); second round (bars, moved shapes, mono, Photo Stitch 2): Auto Punch's satin/fill
    bounds 0.7–6.5 mm, Auto-direction satin without under sewing, fill across strokes up to
    13 mm and at 45° beyond, automatic colour count, PE's colour order (R + G + B, highest
    first, also in Cross Stitch), Sew Page Color in Color mode only (Auto Punch within 3 % of
    PE's stitches on all four captures). Photo Stitch 1's runs are joined nearest first since
    2026-10-30 (mono +11 % stitches, was +39 %; trims 99 and 249, were 426 and 1158). Next:
    run-to-run travel in Cross Stitch; PE's contour-following meanders for Photo Stitch 1 and
    Photo Stitch 2's exact colour transitions within a Color Set and Auto Select's thread
    choice (its line
    screen, #129, is built since 2026-10-30 after the owner's decision: −3.1 % stitches, angles,
    spacing and densities as PE's; named Color Sets, Direction, interval/density and Mono's
    tone ramp as PE's; harness `tools/screencmp.py`, design note
    docs/research/photo-stitch-2-halftone.md).

## Beyond PE-DESIGN

PE-DESIGN compatibility stays the primary goal and the phases above come first. After that,
OpenStitch adds what other digitizing products offer and PE-DESIGN 11 does not. The survey
of Wilcom EmbroideryStudio and Hatch, Bernina V9, mySewnet, Janome Artistic Digitizer, Tajima
DG by Pulse, Embird, Ink/Stitch (concepts only), Embrilliance and Floriani is in
[research/competitive-features.md](research/competitive-features.md): a comparison matrix,
per-feature notes with the stitch-generation idea, effort and licensing, and a ranked
backlog. Every item lands as a new attribute or command that is off unless chosen, so the
PE-DESIGN reference specs stay green. The rows are listed as "Planned (beyond PE-DESIGN)" in
[FEATURE_MATRIX.md](FEATURE_MATRIX.md).

Done beyond PE-DESIGN so far: the seeded random tatami stagger (#123, 2026-10-29: `stagger:
random` with `staggerSeed`, reproducible; PE-DESIGN's Fill pane offers the regular Frequency
brick only, which stays the default).

| Phase | Backlog items | Scope |
|---|---|---|
| A Fill library | B1 contour/spiral fill · B6 density profiles and two-colour blend · B7 3D puff mode · B8 crosshatch and lace · B12 carving lines and knockdown fill | `openstitch-design` generators and inspector rows |
| B Satin quality | B3 corner handling (mitre/cap/lap) and short stitches · B5 pull/push compensation, automatic underlay by width, fabric table · B10 variable-width satin line | model change with `.osd` migration; needs a sew-out |
| C Routing | B4 auto-routed running stitches and Redwork, then satin branching · B11 closest join, auto start/end, continue digitizing, overlap removal | `route.rs`, `order.rs`, Pen tool |
| D Turning fills | B2 two-guide fills and stitch-angle maps · B9 maze and tiled meander fills, randomisation | builds on A and C |
| E Tools and lettering | B13 density map, troubleshoot, thread-brand conversion, auto hoop · B14 stitch-file resize with density · B15 baselines, saved kerning, team names, closest-point letter connections | engine commands and editor panes |

Status of each item (sketch = the algorithm, parameters, model change and test specs, written
before any code; the ranking and the proposed tracks are in
[research/beyond/readiness.md](research/beyond/readiness.md)):

| Item | Feature | Phase | Sketch | Status |
|---|---|---|---|---|
| B1 | Contour (offset) fill | A | [research/beyond/contour-fill.md](research/beyond/contour-fill.md) | Done 2026-10-30: region type `contour` with rings, single and double spiral, echo lines (0–30) and round or mitred corners, on the ring tree of PE-DESIGN's Spiral Stitch (`crate::spiral`); open: repeats, bevel join, compat specs |
| B2 | Two-guide fills and stitch-angle maps | D | [research/beyond/guided-fill-and-angle-maps.md](research/beyond/guided-fill-and-angle-maps.md) | Sketched; not started |
| B3 | Satin corners and short stitches | B | [research/beyond/satin-corners-and-short-stitches.md](research/beyond/satin-corners-and-short-stitches.md) | Sketched; not started |
| B4 | Auto-routed running stitches, Redwork, satin branching | C | [research/beyond/auto-route-and-branching.md](research/beyond/auto-route-and-branching.md) | Sketched; not started |
| B5 | Compensation model, automatic underlay by width, fabric table | B | [research/beyond/compensation-and-underlay.md](research/beyond/compensation-and-underlay.md) | Sketched; not started |
| B6 | Density profiles, two-colour blend, hidden travel | A | [research/beyond/density-profiles-and-blend.md](research/beyond/density-profiles-and-blend.md) | Sketched; not started |
| B7 | 3D puff mode | A | [research/beyond/puff-mode.md](research/beyond/puff-mode.md) | Sketched; not started |
| B8 | Crosshatch, curved crosshatch, lace | A | [research/beyond/crosshatch-and-lace.md](research/beyond/crosshatch-and-lace.md) | Sketched; not started |
| B9 | Maze and meander fills, fur, randomisation | D | [research/beyond/meander-maze-and-randomisation.md](research/beyond/meander-maze-and-randomisation.md) | Sketched; not started |
| B10 | Variable-width satin line | B | [research/beyond/variable-width-satin-line.md](research/beyond/variable-width-satin-line.md) | Sketched; not started |
| B11 | Closest join, auto start/end, continue digitizing, overlap removal | C | [research/beyond/closest-join-and-overlap.md](research/beyond/closest-join-and-overlap.md) | Sketched; not started. Related, shipped 2026-10-29 (#137): `order.optimize` mode `minJump`, a 2-opt shortest-jump order per colour block with every neighbouring pair linked, as a design setting (`optimizeOrder`); PE-DESIGN's order stays the default |
| B12 | Carving lines, knockdown fill | A | [research/beyond/carving-and-knockdown.md](research/beyond/carving-and-knockdown.md) | Sketched; not started |
| B13 | Density map, troubleshoot, thread conversion, auto hoop | E | [research/beyond/quality-tools.md](research/beyond/quality-tools.md) | Sketched; not started |
| B14 | Stitch-file resize with density | E | [research/beyond/stitch-file-resize.md](research/beyond/stitch-file-resize.md) | Sketched; not started |
| B15 | Lettering: baselines, kerning, team names, connections | E | [research/beyond/lettering-depth.md](research/beyond/lettering-depth.md) | Sketched; not started |

Deferred: linear colour-gradient and tartan fills, shape and vine fills, photo
thread-painting, sequin/chenille/bead types (machine-specific), the TrueType turning-satin
rework, an intelligent multi-hoop split with editable lines, VP3 and other extra formats.
The patents named in the research document are checked by number before the related item
ships; the facts per patent and the items each touches are in
[research/beyond/patents.md](research/beyond/patents.md) (shown active: Brother's
US 7996103 for the deferred direction-aware photo stitch, and BriTon Leap's families for cut
files from stitch data and for tracing a bitmap of stitch data, which concern
`object.toShapes`, B12 on stitch blocks and B14's fill fallback; the rest shown expired or
ceased). Not legal advice: the list for the owner's lawyer.

## Pattern and font files (Phase 6, #130 #131 #134)

Only files the owner makes in PE-DESIGN's own tools (Programmable Stitch Creator, Stitch
Design Factory, Font Creator) and saves through the normal interface are studied; Brother's
bundled patterns, motifs and fonts are never read. Done: PMV read (2026-10-29, the documented
layout, every byte checked). Waiting for the owner-made samples: PE-DESIGN's own PMV files (a
2- and a 4-point decorative stitch), Programmable Stitch Creator saves (a 2-point line, a
4-point square, two separate lines; a motif; a decorative-fill row) with a programmable fill
and a motif fill sewn from them, and a Font Creator font of one and two owner-drawn glyphs
with text set in it. Each lands as its own PR with the fixtures' provenance in a MANIFEST.
bundled patterns, motifs and fonts are never read. Done: import of `.pas`/`.pmf`/`.plf` text
sources in the documented `@DIRECTIVE` layout, checked against six owner-made Programmable
Stitch Creator files, and programmable fill, motif fill and motif line tiled as PE-DESIGN 11.50
sews them (2026-10-29: four reference specs); the motif fill's travel and ties and the motif
line's tie-in and lock as PE-DESIGN (2026-10-30: 130 and 23 needle points as PE); free decorative
lines placed and spaced as PE-DESIGN sews them (2026-10-30, `deco-fill-1line-100mm`). Next: how often
PE-DESIGN repeats a free decorative line and its outline travel (needs a second reference). Waiting for
the owner-made samples: PE-DESIGN's own PMV files and a Font Creator font of one and two
owner-drawn glyphs with text set in it (pe-refs-4 could not reach Font Creator and Stitch Design
Factory). Each lands as its own PR with the fixtures' provenance in a MANIFEST.

## Blocked

Wireless transfer, Link, USB Link, network machine settings and the Design Database's Card
Reading (Brother's USB card reader/writer) are Blocked: their protocols are undocumented and
will not be reverse-engineered. PHX and PEN import are Blocked for the same reason (PEN is an
encrypted purchase format). See the Blocked rows in [FEATURE_MATRIX.md](FEATURE_MATRIX.md).

## Dependencies on the owner

- New reference files, made in the owner's licensed PE-DESIGN 11.50 for the specs that ask for
  one ([compat/reference/README.md](../compat/reference/README.md)). Recorded so far (2026-10-28):
  fill and satin sweeps, the gradation sweep (Max/Min, the eight presets, Blend, Direction, no
  under sewing), Auto satin bends 30–120°, the Fit-text sweep (spacing, size, AHHHH, satin), one
  sample of every PES object kind, and two machine-stream designs (trim threshold, colour order).
  Still to record: satin bends on a curved band; Fit text on a line and a square, kerning on "AVA"; Stamp from an
  owner-drawn pattern, a two-colour text, an editable hole-sewing pair, the Cutwork and Embroidered
  Patch wizards, non-ASCII Design Property text; frame moves between 2.2 and 2.4 mm for the exact trim bound (0.5–2.0 mm gaps and four-colour
  ordering are recorded), satin columns over 30 mm wide (16–30 mm are sent whole), and the
  other-format exports once their writers match.
- A test sew-out of an OpenStitch-written file on the owner's Brother machine (the machine
  tier).
- A lawyer's review of the policy on studying PE-DESIGN output files (allowed as project policy
  since 2026-10-27; [LEGAL_AND_LICENSING.md](LEGAL_AND_LICENSING.md)).
- Code-signing certificates (macOS, Windows) and a Flathub account.
- The decision on `unsafe` in one small, audited crate per OS (macOS file opening, Quick Look
  and Explorer thumbnails).
