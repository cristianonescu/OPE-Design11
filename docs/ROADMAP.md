# Roadmap

| Phase | Goal | State |
|---|---|---|
| 1 Tooling | Rust workspace, CI on three OSes, fixture generators | Done. |
| 2 Reference research | Guide-based feature inventory; reference files from a licensed PE-DESIGN 11 | Done: inventory from the public guide; reference files made in a licensed PE-DESIGN 11.50 since 2026-10-27 (`compat/reference/`, provenance in `MANIFEST.json`). |
| 3 Core engine | Stitch model, threads, document operations, stitch generation | Done: design model, all guide sew types, compiler, command registry, undo, `.osd` projects, CLI and JSON control, MCP. |
| 4 File interoperability | Every format PE-DESIGN 11 reads or writes; round-trip tests | DST, EXP, JEF, PCS, SEW, XXX, HUS and VIP written record for record as PE-DESIGN 11.50 exports them (reference tier, 2026-10-28); PES, PEC and the readers at oracle tier with fuzzing; SHV, PHC, CSD read-only; PES v1/v6 object section; PE-DESIGN 11 (`#PES0110`) metadata, thread table and editable shapes read; SVG and image import; split exports. |
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
  - Sewing order (`order.move`, `order.byColor`, `order.optimize`) with per-object jump trimming.
  - Sewing direction (round fill, centre and reference points); Manual Punch; Satin Skeleton
    tool and skeletons derived from shapes.
  - User settings: saved attribute sets and user thread charts.
- **Data types:** conversions between shape, text and stitch patterns, region → line, and
  stitch-pattern editing (needle points, split, connect, divide by colour).
- **Stitch types:**
  - Line types: motif (own library), stem, candlewicking, E/V, chain, cutting, zigzag half stitch.
  - Satin and fill options: auto direction, underlays, gradation, feather, split, travel.
  - Region types: programmable fill, motif fill, piping, cross stitch, concentric circle,
    radial, spiral, flexible spiral, stippling, net fill, zigzag net fill, decorative fill.
- **Lettering:** text objects with Lato Bold and Pacifico (SIL OFL), text on a path, user
  TrueType/OpenType fonts (CJK through any font with the glyphs), shaping of right-to-left and
  complex scripts (`rustybuzz`, `unicode-bidi`), kerning, vertical text, per-character edits
  (on the canvas with Point Edit),
  user-mapped text, monograms, Name Drop, templates and the Template Wizard, Font Creator (including
  characters traced from an image in one step).
- **Wizards:** background fill (echo, stippling, decorative, crosshatch, stripes), appliqué,
  embroidered patch, cutwork.
- **Suite programs:** Programmable Stitch Creator (`openstitch.pattern`), Stitch Design Factory
  (`openstitch.sewing`, SVG and experimental PMV export), Design Database (`openstitch-library`:
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

Compatibility, from the open items in [COMPATIBILITY_AUDIT.md](COMPATIBILITY_AUDIT.md):

1. **Fill:** PE-DESIGN's row structure (halves from the start point, brick row ends, 30 %
   Frequency default); the programmable fill density; Stitch Type (turning ends) and Half
   Stitch; feathered-edge patterns; gradation presets, per-step editing and Blend. Gradation
   rows follow PE-DESIGN's cumulative-density placement since 2026-10-28 (#95: every row of
   the U within 0.12 mm, overlap 0.83 → 0.89); left for the U: PE's section order for fills in
   several parts (right leg, left leg, then the top bar ending at the first vertex); under
   sewing Density Dense / Light (the second under sewing type, `cross`, is done for fill and
   satin since 2026-10-28, specs sweep-fill-us-type-2 and sweep-satin-us-type-2).
2. **Satin:** Half Stitch for Auto satin (the corner fan now produces the crowding it acts
   on); Miter limit. Auto satin's needle lattice (whole spacings from the far end, the
   remainder at the start point), its forward start travel and its end on the start point
   follow PE-DESIGN since 2026-10-28 (satin-auto-bend-60: mean needle distance 0.20 → 0.09 mm);
   left: PE's exact remainder handling on the start leg (0.05 mm) and the fan's inner crowding
   (0.3-0.4 mm against our 0.25-0.3 mm), both below the file's 0.1 mm grid in effect. Done 2026-10-28 as well: the gradation pattern on a split
   satin (satin-gradation, needle for needle). Also done 2026-10-28: the minimum-density
   sawtooth (sweep-satin-density-min, needle for needle: the crossings count back from the
   start point's line, which the round-3 column at 0.2, 0.3 and 0.5 lines/mm confirmed record
   for record; left: the bound under which a single-line gap next to the far end is not
   crossed, between 0.33 and 2 mm). Next on this
   track: the second under sewing type (sweep-satin-us-type-2).
3. **Lines:** Start/End Type Feed/Run for stem, candlewicking and chain, with the L2 marker and
   the Feed/Run choice in the editor; open lines and chain diamonds; candlewicking Low and
   High. The motif line cannot be compared: PE-DESIGN's default motif is Brother's own artwork.
4. **Lettering:** stroke-wise sewing of TrueType text as PE-DESIGN does it (every letter is
   split into strokes, each sewn as a column with its rows across the stroke, for Fill and Satin
   text alike; the 'H' references match only because an H's stems are vertical) — the structural
   gap behind text-fit-*, text-transform-* and text-sewing-order; the fitted circle's running
   stitch follows PE-DESIGN's Bézier halving since 2026-10-28 (text-fit-default mean needle
   distance 0.27 → 0.20 mm); Text Transform presets 13 and 14 (fan out / in) done 2026-10-28; round 4 (2026-10-29)
   showed PE-DESIGN's transform level is a dragged handle whose value is stored per file
   (percent of the font size, degrees for slants, fans and arches): add a level parameter to
   `text.add`, fit each preset's envelope as a function of the value from the parked
   level files, and re-fit the cap-height box (presets 3, 9, 10, 14, 17 miss by up to 2 mm
   on descender letters).
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
6. **PES:** turn the `#PES0110` shapes the reader now parses into editable OpenStitch objects;
   write editable shapes and text; PES v7–v11 and the hoop and Design Page fields (from the
   owner's own files only, as [LEGAL_AND_LICENSING.md](LEGAL_AND_LICENSING.md) allows).
7. **Editing details:** skeletons that branch (a derived skeleton follows a shape's longest
   stroke only) and an exact medial axis instead of the grid approximation.
8. **Formats:** CSD export (no public writer).
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
    prepass with documented grid and per-cell defaults since 2026-10-29 (#132). Next: the rotating-line coverage model for
    Photo Stitch 2 only if the captures show the zigzag model is the dominant remaining
    difference (#129). Each is compared with pe-refs-4's captures of PE-DESIGN's output at
    defaults when they land.

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
| B1 | Contour (offset) fill | A | [research/beyond/contour-fill.md](research/beyond/contour-fill.md) | Sketched; not started |
| B2 | Two-guide fills and stitch-angle maps | D | [research/beyond/guided-fill-and-angle-maps.md](research/beyond/guided-fill-and-angle-maps.md) | Sketched; not started |
| B3 | Satin corners and short stitches | B | [research/beyond/satin-corners-and-short-stitches.md](research/beyond/satin-corners-and-short-stitches.md) | Sketched; not started |
| B4 | Auto-routed running stitches, Redwork, satin branching | C | [research/beyond/auto-route-and-branching.md](research/beyond/auto-route-and-branching.md) | Sketched; not started |
| B5 | Compensation model, automatic underlay by width, fabric table | B | [research/beyond/compensation-and-underlay.md](research/beyond/compensation-and-underlay.md) | Sketched; not started |
| B6 | Density profiles, two-colour blend, hidden travel | A | [research/beyond/density-profiles-and-blend.md](research/beyond/density-profiles-and-blend.md) | Sketched; not started |
| B7 | 3D puff mode | A | [research/beyond/puff-mode.md](research/beyond/puff-mode.md) | Sketched; not started |
| B8 | Crosshatch, curved crosshatch, lace | A | [research/beyond/crosshatch-and-lace.md](research/beyond/crosshatch-and-lace.md) | Sketched; not started |
| B9 | Maze and meander fills, fur, randomisation | D | [research/beyond/meander-maze-and-randomisation.md](research/beyond/meander-maze-and-randomisation.md) | Sketched; not started |
| B10 | Variable-width satin line | B | [research/beyond/variable-width-satin-line.md](research/beyond/variable-width-satin-line.md) | Sketched; not started |
| B11 | Closest join, auto start/end, continue digitizing, overlap removal | C | [research/beyond/closest-join-and-overlap.md](research/beyond/closest-join-and-overlap.md) | Sketched; not started |
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
