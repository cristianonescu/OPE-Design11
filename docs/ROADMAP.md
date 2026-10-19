# Roadmap

| Phase | Goal | State |
|---|---|---|
| 1 Tooling | Rust workspace, CI on three OSes, fixture generators | Done. |
| 2 Reference research | Guide-based feature inventory; Windows VM with a licensed PE-DESIGN; reference fixtures | Inventory done. Reference fixtures need a licensed PE-DESIGN installation. |
| 3 Core engine | Stitch model, threads, document operations, stitch generation | Done: design model, all guide sew types, compiler, command registry, undo, `.osd` projects, CLI and JSON control, MCP. |
| 4 File interoperability | Every format PE-DESIGN 11 reads or writes; round-trip tests | PES, PEC, DST, EXP, JEF, PCS, SEW, XXX, HUS at oracle tier with fuzzing; VIP spec-derived; SHV, PHC, CSD read-only; PES v1/v6 object section; SVG and image import; split exports. |
| 5 Desktop interface | Cross-platform editor (egui) | Done: drawing, Bézier pen and point editing, selection, inspector, object list, undo, simulator, wizards, Design Database mode, Stitch Creator, Stitch Design Factory, printing, USB transfer. |
| 6 Advanced | Digitizing, lettering, complex fills, suite programs | Done as independent approximations: Auto Punch, Cross Stitch, Photo Stitch, Design Center; full lettering suite and Font Creator. |
| 7 Validation | Full reference suite, gap closing | Not started; needs a licensed PE-DESIGN installation. |

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
  - Sewing direction (round fill, centre and reference points); Manual Punch.
  - User settings: saved attribute sets and user thread charts.
- **Data types:** conversions between shape, text and stitch patterns, region → line, and
  stitch-pattern editing (needle points, split, connect, divide by colour).
- **Stitch types:**
  - Line types: motif (own library), stem, candlewicking, E/V, chain, cutting, zigzag half stitch.
  - Satin and fill options: auto direction, underlays, gradation, feather, split, travel.
  - Region types: programmable fill, motif fill, piping, cross stitch, concentric circle,
    radial, spiral, flexible spiral, stippling, net fill, zigzag net fill, decorative fill.
- **Lettering:** text objects with Lato Bold and Pacifico (SIL OFL), text on a path, user
  TrueType/OpenType fonts (CJK through any font with the glyphs), user-mapped text, monograms,
  Name Drop, templates and the Template Wizard, Font Creator.
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
  MSI + zip, Linux tarball with thumbnailer); tag-only draft releases.

## Still to do

1. **Editing details:** an editor tool for drawing satin skeletons and deriving one from a
   shape (the engine keeps Bézier curves through shape operations and sews variable-direction
   satin along a skeleton).
2. **Lettering:** a shaping engine for right-to-left and complex scripts, vertical CJK text,
   kerning, per-character edits, Font Creator characters from background images in one step.
3. **Formats:** CSD export (no public writer); editable PES shapes and text (no public
   description; needs reference files and the legal opinion).
4. **Design Database:** printing through the OS print dialog.
5. **Distribution:** code signing (owner's certificates), macOS Apple-Event file opening,
   AppImage/Flatpak, PES thumbnails on macOS and Windows (need `unsafe` FFI, which the workspace
   forbids).
6. **Reference tier:** once a licensed PE-DESIGN is available (VM_PLAN.md), generate fixtures,
   calibrate defaults and estimates, and promote rows to Verified Compatible.

Wireless transfer and Link are Blocked: their protocols are undocumented and will not be
reverse-engineered.

## Blocked on a licensed PE-DESIGN installation

| Task | Blocked part | Proceeding meanwhile |
|---|---|---|
| Verified Compatible status for any row | Reference outputs | Oracle tests, public specs |
| Stitch-generation parameter calibration (running, satin, fill) | Reference stitch files | Algorithms from first principles, unit tests |
| PE-DESIGN default DST jumps-for-trim, colour mapping, PES object section | Reference files | Configurable defaults |
| Wizard behaviour (Auto Punch, Photo Stitch) | Observation | Independent algorithms, documented as approximations |

## Dependencies on the owner

- Reference tier: a licensed PE-DESIGN installation (VM_PLAN.md).
- Legal opinion on studying PE-DESIGN output files for undocumented format fields (EU Directive
  2009/24 Art. 5(3)/6/8).
- Real PES files from the owner's own designs for reader tests.
- A test sew-out of an OpenStitch-written file on the owner's Brother machine.
