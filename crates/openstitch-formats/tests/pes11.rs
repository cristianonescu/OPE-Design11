//! PE-DESIGN 11 (`#PES0110`) editable shapes, read from the owner's minimal PE-DESIGN files in
//! tests/fixtures/pe11 (provenance in tests/fixtures/pe11/PROVENANCE.md). Each rN file is the
//! baseline r01 with one property changed, so each test pins one mapped field.

mod common;

use common::data;
use openstitch_formats::pes;
use openstitch_formats::pes11::{self, Letter, Objects, RegionSewType, Shape, Text, Vertex};

fn shapes(name: &str) -> (Vec<Shape>, Option<String>) {
    let section = pes::read_objects(&data(&format!("pe11/{name}.pes"))).expect("objects");
    assert_eq!(section.version, "0110");
    (section.shapes, section.unparsed)
}

fn one(name: &str) -> Shape {
    let (mut s, unparsed) = shapes(name);
    assert_eq!((s.len(), unparsed), (1, None), "{name}");
    s.remove(0)
}

/// Vertex positions in mm on the design page (file units are 0.1 mm + 500).
fn mm(s: &Shape) -> Vec<(f32, f32)> {
    s.vertices
        .iter()
        .map(|v| ((v.x - 500.0) / 10.0, (v.y - 500.0) / 10.0))
        .collect()
}

#[test]
fn baseline_rectangle_with_pe_default_attributes() {
    // Rectangle (10,10)–(20,20) mm; line Zigzag, region Fill, PE defaults.
    let s = one("r01-rect");
    assert_eq!(s.bounds, [600, 600, 700, 700]);
    assert!(s.closed && s.line_sewn && s.under_sewing && !s.gradation);
    assert_eq!(s.line_type, 0, "zigzag");
    assert_eq!(s.region, RegionSewType::Fill);
    assert_eq!(
        (s.density, s.direction, s.frequency, s.running_path),
        (4.5, 45, 30, 0)
    );
    // Starts in the middle of the bottom edge, then runs round the corners.
    assert_eq!(
        mm(&s),
        [
            (15.0, 20.0),
            (10.0, 20.0),
            (10.0, 10.0),
            (20.0, 10.0),
            (20.0, 20.0)
        ]
    );
    assert!(s.vertices.iter().all(|v| v.kind == 0));
}

#[test]
fn each_changed_property_reads_back() {
    assert_eq!(one("r02-rect-w20").bounds, [600, 600, 800, 700]);
    let moved = one("r03-rect-moved");
    assert_eq!(moved.bounds, [800, 800, 900, 900]);
    assert_eq!(mm(&moved)[0], (35.0, 40.0));
    assert_eq!(one("r04-rect-satin").region, RegionSewType::Satin);
    assert!(!one("r06-rect-nounder").under_sewing);
    let no_line = one("r07-rect-noline");
    assert!(!no_line.line_sewn);
    assert_eq!(no_line.line_type, 9);
    assert_eq!(one("r08-rect-dir90").direction, 90);
    assert_eq!(one("r09-rect-density").density, 6.0);
    // The region colour lives in the thread table, not in these fields.
    assert_eq!(one("r05-rect-blue"), one("r01-rect"));
}

#[test]
fn circle_is_four_smooth_bezier_vertices() {
    let s = one("r10-circle");
    assert!(s.closed);
    assert_eq!(
        mm(&s),
        [(15.0, 20.0), (10.0, 15.0), (15.0, 10.0), (20.0, 15.0)]
    );
    assert!(s.vertices.iter().all(|v| v.kind == 0x11));
    // Handles 0.5523 × the radius along the tangent: the cubic Bézier circle.
    let v = s.vertices[0];
    let mut hx = [v.handles[0].0, v.handles[1].0];
    hx.sort_by(f32::total_cmp);
    assert!(
        (hx[0] - 622.39).abs() < 0.01 && (hx[1] - 677.61).abs() < 0.01,
        "{hx:?}"
    );
    assert!(v.handles.iter().all(|h| h.1 == 700.0));
}

#[test]
fn open_line_and_two_objects() {
    let line = one("r11-line");
    assert!(!line.closed && line.line_sewn);
    assert_eq!(mm(&line), [(10.0, 10.0), (30.0, 10.0)]);
    // The second rectangle refers to the class by MFC tag 0x8001 instead of naming it.
    let (two, unparsed) = shapes("r16-two-rects");
    assert_eq!(unparsed, None);
    assert_eq!(
        two.iter().map(|s| s.bounds).collect::<Vec<_>>(),
        [[600, 600, 700, 700], [900, 600, 1000, 700]]
    );
}

#[test]
fn text_letters_and_attributes_read() {
    // r12: `A` in Lato at the Text tool's defaults, clicked at (10, 20) mm.
    let t = &objects_of(&owner_file("r12-text-a")).texts[0];
    assert_eq!(t.position, 0);
    assert_eq!(t.bounds, [599, 541, 700, 700]);
    assert_eq!(
        t.origin,
        (600.0, 700.0),
        "the click: the first baseline's left end"
    );
    assert_eq!(
        (t.line_spacing, t.vertical, t.breaks.clone()),
        (100.0, false, vec![])
    );
    assert_eq!(t.transform, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let a = &t.letters[0];
    assert_eq!(
        a,
        &Letter {
            line_color: 1,
            ..Letter::with_defaults(u16::from(b'A'))
        }
    );
    // r13: size 20 set on the placed text is a scale, then the text was dragged 10 mm down.
    let t = &objects_of(&owner_file("r13-text-a-20")).texts[0];
    assert_eq!((t.letters[0].size, t.letters[0].scale), (10.0, 2.0));
    assert_eq!(t.transform[4..], [1.0, 100.0]);
    assert_eq!(t.origin, (600.0, 700.0));
    // r14: two letters; r25: the letter B; r26: clicked 10 mm further right.
    let ab = &objects_of(&owner_file("r14-text-ab")).texts[0];
    let chars: Vec<u16> = ab.letters.iter().map(|l| l.ch).collect();
    assert_eq!(chars, [u16::from(b'A'), u16::from(b'B')]);
    assert_eq!(
        objects_of(&owner_file("r25-text-b")).texts[0].letters[0].ch,
        u16::from(b'B')
    );
    assert_eq!(
        objects_of(&owner_file("r26-text-moved")).texts[0].origin.0,
        700.0
    );
    // r23: region Fill; r24: the colour is in the thread table, not the letter.
    let fill = &objects_of(&owner_file("r23-text-fill")).texts[0].letters[0];
    assert_eq!(fill.region, RegionSewType::Fill);
    let blue = objects_of(&owner_file("r24-text-blue"));
    assert_eq!(blue.texts[0].letters[0].region_color, 0);
    assert_eq!(blue.threads[0].catalog_number.as_deref(), Some("007"));
    // r15: a rectangle (position 0), then the text (position 1) in the line's red.
    let both = objects_of(&owner_file("r15-rect-text"));
    assert_eq!((both.shapes.len(), both.texts.len()), (1, 1));
    assert_eq!(both.texts[0].position, 1);
    assert_eq!(both.texts[0].letters[0].region_color, 1);
    assert_eq!(both.texts[0].origin, (900.0, 1000.0));
    // Ten letters at size 8; vertical text; a line break and the line spacing %.
    let ten = &objects_of(&owner_file("o-text-10-letters")).texts[0];
    assert_eq!(ten.letters.len(), 10);
    assert!(
        ten.letters
            .iter()
            .all(|l| l.size == 8.0 && l.font == "Lato")
    );
    assert!(objects_of(&owner_file("o-text-vertical")).texts[0].vertical);
    for (name, spacing) in [
        ("text-line-spacing-50", 50.0),
        ("text-line-spacing-100", 100.0),
        ("text-line-spacing-150", 150.0),
    ] {
        let t = &objects_of(&owner_file(name)).texts[0];
        assert_eq!(
            (t.breaks.clone(), t.line_spacing),
            (vec![1], spacing),
            "{name}"
        );
    }
    // A sewn line on text: Running in the letter's line fields.
    let sewn = &objects_of(&owner_file("text-sewing-order")).texts[0].letters[0];
    assert!(sewn.line_sewn);
    assert_eq!(sewn.line_type, Shape::LINE_RUNNING);
    // Every file still reads its stitches from the PEC section.
    for name in ["r01-rect", "r12-text-a", "r15-rect-text", "r16-two-rects"] {
        let p = pes::read(&data(&format!("pe11/{name}.pes"))).expect("read");
        assert!(p.stitches.len() > 100, "{name}");
    }
}

#[test]
fn unmapped_layouts_stop_reading_instead_of_misreading() {
    // A Manual Punch block and the eight Shape-tool start-point shapes from the compat
    // references: layouts not mapped yet must end the list as "unparsed", with the shapes read
    // before them intact and every vertex inside its bounds.
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../compat/reference/stitches"
    );
    for (name, read, stop) in [
        ("satin-zigzag-column", 0, "CEmbLine (unmapped variant)"),
        ("shape-start-points", 3, "CEmbSpecialShape"),
    ] {
        let bytes = std::fs::read(format!("{dir}/{name}.pes")).expect("reference");
        let section = pes::read_objects(&bytes).expect("objects");
        assert_eq!(
            (section.shapes.len(), section.unparsed.as_deref()),
            (read, Some(stop)),
            "{name}"
        );
        for s in &section.shapes {
            assert!(s.vertices.len() >= 2, "{name}");
            for v in &s.vertices {
                assert!(
                    f32::from(s.bounds[0]) - 1.0 <= v.x && v.x <= f32::from(s.bounds[2]) + 1.0,
                    "{name}"
                );
            }
        }
        assert!(pes::read(&bytes).is_ok(), "{name}");
    }
}

#[test]
fn thread_table_and_colour_indexes() {
    // r01: region 005 SILVER, line 800 RED. The table is in sewing order (region first).
    let section = pes::read_objects(&data("pe11/r01-rect.pes")).expect("objects");
    let table: Vec<(String, Option<&str>)> = section
        .threads
        .iter()
        .map(|t| (t.hex(), t.catalog_number.as_deref()))
        .collect();
    assert_eq!(
        table,
        [
            ("#a8a8a8".into(), Some("005")),
            ("#ed171f".into(), Some("800"))
        ]
    );
    assert_eq!(section.threads[0].brand.as_deref(), Some("EMBROIDERY"));
    let s = &section.shapes[0];
    assert_eq!((s.region_color, s.line_color), (0, 1));
    // r17: the line takes the region's silver.
    assert_eq!(one("r17-line-silver").line_color, 0);
    // r18: colours swapped; the table swaps with them, so the indexes stay.
    let swapped = pes::read_objects(&data("pe11/r18-colours-swapped.pes")).expect("objects");
    assert_eq!(swapped.threads[0].hex(), "#ed171f");
    let s = &swapped.shapes[0];
    assert_eq!((s.region_color, s.line_color), (0, 1));
    // Three shapes in three colours (compat reference shape-start-points).
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../compat/reference/stitches/shape-start-points.pes"
    );
    let section = pes::read_objects(&std::fs::read(path).expect("reference")).expect("objects");
    let colours: Vec<String> = section
        .shapes
        .iter()
        .map(|s| section.threads[usize::from(s.region_color)].hex())
        .collect();
    let catalog: Vec<&str> = section
        .shapes
        .iter()
        .filter_map(|s| {
            section.threads[usize::from(s.region_color)]
                .catalog_number
                .as_deref()
        })
        .collect();
    assert_eq!(catalog, ["800", "007", "027"], "{colours:?}");
}

#[test]
fn line_attributes() {
    let b = one("r01-rect");
    assert_eq!(
        (
            b.zigzag_width,
            b.zigzag_density,
            b.run_pitch,
            b.line_under_sewing
        ),
        (2.0, 4.5, 2.0, true)
    );
    let running = one("r19-line-running");
    assert_eq!((running.line_type, running.run_pitch), (1, 2.0));
    assert_eq!(one("r20-line-running-pitch").run_pitch, 3.0);
    let wide = one("r21-zigzag-width");
    assert_eq!(wide.zigzag_width, 3.0);
    // The outline's bounds don't change with the zigzag width (only the sewn extent does).
    assert_eq!(wide.bounds, [600, 600, 700, 700]);
    assert!(!one("r22-line-noUnder").line_under_sewing);
}

#[test]
fn round_3_object_files_read_their_stitches_and_stop_cleanly() {
    // One file per remaining object kind (tests/fixtures/pe11/PROVENANCE.md, "Round 3"): the
    // PEC stitches always read, and the object list either reads or stops at a named class
    // without panicking. The classes are mapped by later work.
    for name in [
        "o-applique-heart",
        "o-arc-string",
        "o-arc",
        "o-closed-curve",
        "o-closed-freehand",
        "o-fan",
        "o-gallery-heart",
        "o-group-two-rects",
        "o-hoop-100x100-s",
        "o-hoop-130x180-single",
        "o-machine-single-100x100",
        "o-manual-curve",
        "o-manual-running",
        "o-manual-straight",
        "o-open-curve",
        "o-open-freehand",
        "o-page-200x200",
        "o-region-concentric",
        "o-region-cross",
        "o-region-flexible-spiral",
        "o-region-net-fill",
        "o-region-piping",
        "o-region-radial",
        "o-region-spiral",
        "o-region-stippling",
        "o-region-zigzag-net",
        "o-rounded-rect",
        "o-text-transform-1",
        "o-text-vertical",
        "o-text-10-letters",
    ] {
        let bytes = data(&format!("pe11/{name}.pes"));
        let p = pes::read(&bytes).expect(name);
        assert!(
            p.stitches.len() > 20,
            "{name}: {} stitches",
            p.stitches.len()
        );
        if let Ok(section) = pes::read_objects(&bytes) {
            for s in &section.shapes {
                assert!(s.vertices.len() >= 2, "{name}");
            }
        }
    }
}

/// The owner files whose object list the reader parses completely (every object a `CEmbLine`
/// or a `CEmbNText`, groups included): the writer's round-trip and byte-comparison set.
const FULLY_PARSED: [&str; 55] = [
    "r12-text-a",
    "r13-text-a-20",
    "r14-text-ab",
    "r15-rect-text",
    "r23-text-fill",
    "r24-text-blue",
    "r25-text-b",
    "r26-text-moved",
    "o-text-10-letters",
    "o-text-vertical",
    "text-defaults-fill",
    "text-defaults-h",
    "text-line-spacing-100",
    "text-line-spacing-150",
    "text-line-spacing-50",
    "text-sewing-order",
    "text-size-h",
    "text-size-hg",
    "r01-rect",
    "r02-rect-w20",
    "r03-rect-moved",
    "r04-rect-satin",
    "r05-rect-blue",
    "r06-rect-nounder",
    "r07-rect-noline",
    "r08-rect-dir90",
    "r09-rect-density",
    "r10-circle",
    "r11-line",
    "r16-two-rects",
    "r17-line-silver",
    "r18-colours-swapped",
    "r19-line-running",
    "r20-line-running-pitch",
    "r21-zigzag-width",
    "r22-line-noUnder",
    "o-arc-string",
    "o-arc",
    "o-closed-curve",
    "o-closed-freehand",
    "o-fan",
    "o-group-two-rects",
    "o-open-curve",
    "o-open-freehand",
    "o-page-200x200",
    "o-region-concentric",
    "o-region-cross",
    "o-region-flexible-spiral",
    "o-region-net-fill",
    "o-region-piping",
    "o-region-radial",
    "o-region-spiral",
    "o-region-stippling",
    "o-region-zigzag-net",
    "o-rounded-rect",
];

/// The fixture's stitches with the PEC label dropped: it becomes the pattern's `name`, which
/// the writer would put in the (empty) Design Property name.
fn stitches_of(bytes: &[u8]) -> openstitch_core::StitchPattern {
    let mut p = pes::read(bytes).expect("stitches");
    p.metadata.remove("name");
    p
}

fn objects_of(bytes: &[u8]) -> Objects {
    let s = pes::read_objects(bytes).expect("objects");
    assert_eq!(s.version, "0110");
    Objects {
        page_mm: s.page_mm,
        threads: s.threads,
        shapes: s.shapes,
        texts: s.texts,
        groups: s.groups,
        unparsed: s.unparsed,
    }
}

/// An owner file: `text-*` from compat/reference/text, the others from tests/fixtures/pe11.
fn owner_file(name: &str) -> Vec<u8> {
    if name.starts_with("text-") {
        let path = format!(
            "{}/../../compat/reference/text/{name}.pes",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    } else {
        data(&format!("pe11/{name}.pes"))
    }
}

#[test]
fn region_not_sewn_groups_and_page_size_read() {
    let o = objects_of(&data("pe11/o-group-two-rects.pes"));
    assert_eq!(o.shapes.len(), 2);
    assert!(o.shapes.iter().all(|s| s.region == RegionSewType::NotSewn));
    assert_eq!(o.groups, vec![vec![0, 1]]);
    assert_eq!(o.page_mm, Some((100, 100)));
    assert_eq!(
        objects_of(&data("pe11/o-page-200x200.pes")).page_mm,
        Some((200, 200))
    );
    assert_eq!(
        objects_of(&data("pe11/r01-rect.pes")).groups,
        Vec::<Vec<u16>>::new()
    );
}

#[test]
fn written_files_read_back_to_the_same_objects() {
    for name in FULLY_PARSED {
        let fixture = owner_file(name);
        let objects = objects_of(&fixture);
        assert_eq!(objects.unparsed, None, "{name}");
        let pattern = stitches_of(&fixture);
        let written = pes11::write(&objects, &pattern).expect("write");
        let back = objects_of(&written);
        assert_eq!(back, objects, "{name}");
        let reread = pes::read(&written).expect("read back");
        assert_eq!(reread.stitches.len(), pattern.stitches.len(), "{name}");
        assert_eq!(reread.threads.len(), pattern.threads.len(), "{name}");
    }
}

/// Differing byte ranges `(start, end)` between two slices.
fn diff_ranges(a: &[u8], b: &[u8]) -> Vec<(usize, usize)> {
    let n = a.len().min(b.len());
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if a[i] != b[i] {
            let s = i;
            while i < n && a[i] != b[i] {
                i += 1;
            }
            out.push((s, i));
        } else {
            i += 1;
        }
    }
    if a.len() != b.len() {
        out.push((n, a.len().max(b.len())));
    }
    out
}

/// Where the object list starts (the u16 count before `FFFF 0000 0800 CEmbLine`).
fn list_offset(bytes: &[u8]) -> usize {
    bytes
        .windows(7)
        .position(|w| w[..4] == [0xFF, 0xFF, 0, 0] && w[6] == b'C')
        .expect("object list")
        - 2
}

/// Byte offsets of the leading `CEmbLine` bodies in `bytes` (the byte after the class name or
/// tag); `count` of them, which must come first in the list.
fn body_offsets(bytes: &[u8], count: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut at = list_offset(bytes) + 16;
    for _ in 0..count {
        out.push(at);
        let n = u32::from_le_bytes(bytes[at + 4516..at + 4520].try_into().expect("n")) as usize;
        at += 4520 + 28 * n + 2; // the next object's 0x8001 tag
    }
    out
}

#[test]
fn written_object_sections_match_pe_designs_bytes_outside_the_documented_differences() {
    // docs/FILE_FORMATS.md, "Writer": the pattern block inside every body is OpenStitch's own;
    // the two parameters of the geometry header's first marks vary between files of the same
    // shape (r02, r08), and the marks' coordinates follow the closed-shape rule, which the open
    // and curved owner files do not all share. Everything else in the object section is
    // identical.
    let marks_differ = [
        "r10-circle",
        "r11-line",
        "o-arc",
        "o-arc-string",
        "o-closed-curve",
        "o-closed-freehand",
        "o-fan",
        "o-open-curve",
        "o-open-freehand",
        "o-rounded-rect",
        "o-region-concentric",
        "o-region-cross",
        "o-region-flexible-spiral",
        "o-region-net-fill",
        "o-region-piping",
        "o-region-radial",
        "o-region-spiral",
        "o-region-stippling",
        "o-region-zigzag-net",
    ];
    for name in FULLY_PARSED {
        let fixture = owner_file(name);
        let objects = objects_of(&fixture);
        let pattern = stitches_of(&fixture);
        let written = pes11::write(&objects, &pattern).expect("write");
        let pec = u32::from_le_bytes(fixture[8..12].try_into().expect("offset")) as usize;
        let pec_w = u32::from_le_bytes(written[8..12].try_into().expect("offset")) as usize;
        // The header up to the object list: identical, except that the 200 × 200 page file
        // carries an optional hoop field (4 bytes) the writer leaves absent (2 bytes).
        let (list, list_w) = (list_offset(&fixture), list_offset(&written));
        if name == "o-page-200x200" {
            assert_eq!(list - list_w, 2, "{name}");
            assert_eq!(pec - pec_w, 2, "{name}");
        } else {
            assert_eq!(list, list_w, "{name}");
            assert_eq!(pec, pec_w, "{name}: PEC offset");
            // The hoop fields up to the end of the machine string are the baseline's (150 ×
            // 150 mm, `nv880e-M`) whatever the owner's hoop was (o-text-*: 100 × 100 `nv880e-S`).
            let machine = |b: &[u8]| {
                b.windows(7)
                    .position(|w| w == b"nv880e-")
                    .expect("machine string")
                    + 8
            };
            assert_eq!(machine(&written), machine(&fixture), "{name}");
            let m = machine(&fixture);
            assert_eq!(&written[12..21], &fixture[12..21], "{name}: header");
            assert_eq!(&written[m..list], &fixture[m..list], "{name}: header");
        }
        // The object list and the trailer, compared from the list start.
        let (fixture, written) = (&fixture[list..pec], &written[list_w..pec_w]);
        let bodies = body_offsets(fixture, objects.shapes.len());
        // The circle's sewn extent is one unit wider than the bounds plus half the zigzag width,
        // and an open zigzag line's extent grows across the line only, not past its ends.
        let extent_differs = name == "r10-circle" || name == "r11-line";
        let allowed = |from: usize, to: usize| {
            bodies.iter().any(|&b| {
                let gh = b + 4432;
                (b + 386 <= from && to <= b + 4360)
                    // +162: PE-DESIGN's default-colour index, OpenStitch's line colour.
                    || (b + 162 <= from && to <= b + 164)
                    || (extent_differs && b + 8 <= from && to <= b + 16)
                    || (gh + 8 <= from && to <= gh + 16)
                    || (gh + 24 <= from && to <= gh + 32)
                    || (marks_differ.contains(&name) && gh <= from && to <= gh + 84)
            })
        };
        // Text: a letter field that is f32 -0.0 in r12–r26 and 0.0 in the later owner files
        // (OpenStitch writes r12's), and a last sewing-order field PE-DESIGN leaves unset in the
        // line-spacing files (OpenStitch writes 1).
        let text_quirk = |from: usize, to: usize| {
            (to - from <= 4 && written[from..to].iter().all(|&b| b == 0 || b == 0x80))
                || (name.starts_with("text-line-spacing") && from >= written.len() - 4)
        };
        let unexplained: Vec<(usize, usize)> = diff_ranges(written, fixture)
            .into_iter()
            .filter(|&(s, e)| !allowed(s, e) && !text_quirk(s, e))
            .collect();
        assert!(unexplained.is_empty(), "{name}: {unexplained:?}");
    }
}

#[test]
fn writer_refuses_inconsistent_objects() {
    let good = objects_of(&data("pe11/r01-rect.pes"));
    let pattern = pes::read(&data("pe11/r01-rect.pes")).expect("stitches");
    let mut bad_colour = good.clone();
    bad_colour.shapes[0].region_color = 7;
    assert!(pes11::write(&bad_colour, &pattern).is_err());
    let mut one_vertex = good.clone();
    one_vertex.shapes[0].vertices.truncate(1);
    assert!(pes11::write(&one_vertex, &pattern).is_err());
    let mut bad_group = good.clone();
    bad_group.groups = vec![vec![0, 3]];
    assert!(pes11::write(&bad_group, &pattern).is_err());
    let mut no_threads = good.clone();
    no_threads.threads.clear();
    assert!(pes11::write(&no_threads, &pattern).is_err());
}

#[test]
fn an_openstitch_shape_with_curves_round_trips() {
    // A three-vertex open curve with one Bézier segment, metadata and two groups of one.
    let v = |x: f32, y: f32, h0: (f32, f32), h1: (f32, f32), kind: u32| Vertex {
        x,
        y,
        handles: [h0, h1],
        kind,
    };
    let mut shape = Shape::with_defaults();
    shape.closed = false;
    shape.region = RegionSewType::NotSewn;
    shape.line_type = Shape::LINE_RUNNING;
    shape.run_pitch = 2.5;
    shape.vertices = vec![
        v(
            600.0,
            700.0,
            (0.0, 0.0),
            (650.0, 600.0),
            pes11::KIND_CURVE_AFTER,
        ),
        v(
            800.0,
            600.0,
            (750.0, 600.0),
            (0.0, 0.0),
            pes11::KIND_CURVE_BEFORE,
        ),
        v(1000.0, 700.0, (0.0, 0.0), (0.0, 0.0), 0),
    ];
    shape.bounds = Shape::bounds_of(&shape.vertices);
    let mut second = shape.clone();
    second.vertices.iter_mut().for_each(|p| p.y += 200.0);
    second.bounds = Shape::bounds_of(&second.vertices);
    let objects = Objects {
        page_mm: Some((130, 180)),
        threads: vec![openstitch_core::Thread::rgb(0x12_34_56)],
        shapes: vec![shape, second],
        texts: Vec::new(),
        groups: vec![vec![0], vec![1]],
        unparsed: None,
    };
    let mut pattern = openstitch_core::StitchPattern::new();
    pattern
        .threads
        .push(openstitch_core::Thread::rgb(0x12_34_56));
    for (x, y) in [(100.0, 200.0), (300.0, 100.0), (500.0, 200.0)] {
        pattern.stitches.push(openstitch_core::Stitch::new(
            x,
            y,
            openstitch_core::Command::Stitch,
        ));
    }
    pattern.metadata.insert("name".into(), "curve".into());
    pattern.metadata.insert("author".into(), "Ana Pérez".into());
    let written = pes11::write(&objects, &pattern).expect("write");
    assert_eq!(objects_of(&written), objects);
    let back = pes::read(&written).expect("read");
    assert_eq!(
        back.metadata.get("author").map(String::as_str),
        Some("Ana Pérez")
    );
    assert_eq!(
        back.count(openstitch_core::Command::Stitch),
        pattern.count(openstitch_core::Command::Stitch)
    );
}

#[test]
fn texts_between_shapes_and_in_groups_round_trip() {
    // A text between two shapes (positions 0, 2 shapes; 1 the text), grouped with the second
    // shape: the archive numbering mixes the three classes, the trailer lists the letters.
    let r01 = objects_of(&owner_file("r01-rect"));
    let mut text = objects_of(&owner_file("r14-text-ab")).texts.remove(0);
    text.position = 1;
    text.breaks = vec![1];
    text.line_spacing = 150.0;
    for l in &mut text.letters {
        l.region_color = 1;
        l.line_color = 1;
    }
    let mut second = r01.shapes[0].clone();
    second.vertices.iter_mut().for_each(|v| v.x += 300.0);
    second.bounds = Shape::bounds_of(&second.vertices);
    let objects = Objects {
        page_mm: Some((100, 100)),
        threads: r01.threads.clone(),
        shapes: vec![r01.shapes[0].clone(), second],
        texts: vec![text],
        groups: vec![vec![1, 2]],
        unparsed: None,
    };
    let pattern = stitches_of(&owner_file("r01-rect"));
    let written = pes11::write(&objects, &pattern).expect("write");
    assert_eq!(objects_of(&written), objects);
    // Two texts at one position, or a text past the end of the list, are refused.
    let mut clash = objects.clone();
    clash.texts.push(clash.texts[0].clone());
    assert!(pes11::write(&clash, &pattern).is_err());
    let mut past = objects.clone();
    past.texts[0].position = 9;
    assert!(pes11::write(&past, &pattern).is_err());
}

#[test]
fn a_text_built_from_defaults_writes_r12s_object_section() {
    // The letter defaults and r12's text fields give r12's object section byte for byte.
    let fixture = owner_file("r12-text-a");
    let read = objects_of(&fixture);
    let objects = Objects {
        page_mm: Some((100, 100)),
        threads: read.threads.clone(),
        shapes: Vec::new(),
        texts: vec![Text {
            position: 0,
            bounds: [599, 541, 700, 700],
            letters: vec![Letter {
                line_color: 1,
                ..Letter::with_defaults(u16::from(b'A'))
            }],
            breaks: Vec::new(),
            line_spacing: 100.0,
            vertical: false,
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            origin: (600.0, 700.0),
        }],
        groups: Vec::new(),
        unparsed: None,
    };
    let written = pes11::write(&objects, &stitches_of(&fixture)).expect("write");
    let pec = u32::from_le_bytes(fixture[8..12].try_into().expect("offset")) as usize;
    assert_eq!(&written[..pec], &fixture[..pec]);
}
