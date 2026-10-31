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
    // A space is a letter with the same attributes (r27, `A B`).
    let spaced = &objects_of(&owner_file("r27-text-a-space-b")).texts[0];
    let chars: Vec<u16> = spaced.letters.iter().map(|l| l.ch).collect();
    assert_eq!(chars, [65, 32, 66]);
    assert_eq!(spaced.letters[1].density, spaced.letters[0].density);
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
fn manual_punch_blocks_read_as_shapes_with_their_direction_lines() {
    // Manual Punch > Straight or Curve Block: a CEmbLine whose geometry lists hold the lines'
    // ends (FILE_FORMATS.md, "PES 0110 object list"). Clicks (10,10) (10,30) (30,10) (30,30)
    // make one line per click pair: the outline is the first ends, then the second ends back.
    let read = |name: &str| objects_of(&data(&format!("pe11/{name}.pes"))).shapes;
    let m01 = read("m01-straight-1");
    let [s] = m01.as_slice() else {
        panic!("one block object: {m01:?}")
    };
    assert_eq!((&s.punch, s.punch_exit), (&vec![[0, 3], [1, 2]], 2));
    assert!(s.closed);
    let xy: Vec<(f32, f32)> = s.vertices.iter().map(|v| (v.x, v.y)).collect();
    assert_eq!(
        xy,
        [
            (600.0, 600.0),
            (800.0, 600.0),
            (800.0, 800.0),
            (600.0, 800.0)
        ]
    );
    assert_eq!(
        (s.region, s.density, s.line_sewn, s.line_type),
        (RegionSewType::Satin, 4.5, false, Shape::LINE_NOT_SEWN)
    );
    assert_eq!(read("m03-straight-3")[0].punch.len(), 4);
    assert_eq!(read("o-manual-straight")[0].punch.len(), 3);
    assert_eq!(read("m04-straight-1-fill")[0].region, RegionSewType::Fill);
    assert_eq!(read("m05-straight-1-density")[0].density, 6.0);
    let m08 = &read("m08-straight-1-line")[0];
    assert_eq!(
        (m08.line_sewn, m08.line_type, m08.run_pitch),
        (true, Shape::LINE_RUNNING, 2.0)
    );
    // The Curve Block: the same lines, the sides as smooth curves through their ends.
    let m07 = &read("m07-curve-1")[0];
    assert_eq!(m07.punch.len(), 2);
    assert!(m07.vertices.iter().any(|v| v.kind != 0));
    assert_eq!(read("m09-curve-bent")[0].punch.len(), 3);
    assert_eq!(read("m11-curve-4")[0].punch.len(), 4);
    // Manual Punch > Running is an open line with one entry per list: an ordinary shape.
    assert!(read("o-manual-running")[0].punch.is_empty());
    // A block object written from these fields reads back the same.
    let objects = objects_of(&data("pe11/m03-straight-3.pes"));
    let written =
        pes11::write(&objects, &stitches_of(&data("pe11/m03-straight-3.pes"))).expect("write");
    assert_eq!(objects_of(&written).shapes, objects.shapes);
}

#[test]
fn applique_wizard_objects_read_with_their_lines_and_colours() {
    // Applique Wizard on a 20 mm square at its defaults (a01): the square, then the material
    // (running), position (running) and tack-down (V, 2.0 mm) lines on an outline 3 mm out,
    // then the covering zigzag (3.0 mm), all in one group; the stage colours are PE-DESIGN's
    // appliqué colours 100, 101, 102.
    let a01 = objects_of(&data("pe11/a01-rect-applique.pes"));
    assert!(a01.unparsed.is_none());
    let lines: Vec<(u16, u16)> = a01
        .shapes
        .iter()
        .map(|s| (s.line_type, s.line_color))
        .collect();
    assert_eq!(
        lines,
        [
            (Shape::LINE_NOT_SEWN, 1),
            (Shape::LINE_RUNNING, 2),
            (Shape::LINE_RUNNING, 3),
            (Shape::LINE_E_OR_V, 4),
            (Shape::LINE_ZIGZAG, 4)
        ]
    );
    assert_eq!(a01.shapes[3].ev_width, 2.0);
    assert_eq!(a01.shapes[4].zigzag_width, 3.0);
    // One group: the four lines, then the square.
    assert_eq!(a01.groups, [vec![1, 2, 3, 4, 0]]);
    for (t, catalog) in a01.threads[2..].iter().zip(["100", "101", "102"]) {
        assert_eq!(t.catalog_number.as_deref(), Some(catalog));
        assert_eq!((t.kind, t.rgb_value()), (Some(pes11::APPLIQUE_KIND), 0));
    }
    // Material Cutting (a02): a cutting line, and bit 0x80 of the signature code.
    let a02 = data("pe11/a02-applique-cutting.pes");
    assert_eq!(objects_of(&a02).shapes[1].line_type, Shape::LINE_CUTTING);
    assert_eq!(a02[14..16], [0xB1, b'0']);
    // No tack-down (a03): one object fewer; a 4 mm cover (a04); Replace (a05): the square gone.
    assert_eq!(
        objects_of(&data("pe11/a03-applique-no-tackdown.pes"))
            .shapes
            .len(),
        4
    );
    let a04 = objects_of(&data("pe11/a04-applique-cover-4mm.pes"));
    assert_eq!(a04.shapes.last().map(|s| s.zigzag_width), Some(4.0));
    let a05 = objects_of(&data("pe11/a05-applique-replace.pes"));
    assert_eq!(a05.shapes.len(), 4);
    assert!(
        a05.shapes
            .iter()
            .all(|s| s.region == RegionSewType::NotSewn)
    );
}

#[test]
fn convert_to_blocks_reads_as_blocks_and_running_pieces() {
    // Text > Convert to Blocks on a Lato Bold H and A (20 mm, Satin): one group of Manual
    // Punch blocks with running pieces between them. A block's lines can skip outline points
    // (A's second block: lines [0, 11] [1, 10] [4, 7] [5, 6]); the running pieces have empty
    // position lists; the blocks carry under sewing type 0 and +178 0.
    let h = objects_of(&data("pe11/stroke-blocks-H.pes"));
    assert!(h.unparsed.is_none());
    assert_eq!(h.shapes.len(), 8);
    assert_eq!(h.groups, [(0..8).collect::<Vec<u16>>()]);
    let blocks: Vec<usize> = h.shapes.iter().map(|s| s.punch.len()).collect();
    assert_eq!(blocks, [3, 0, 4, 0, 2, 4, 0, 7]);
    assert!(h.shapes[1].no_positions && !h.shapes[1].closed);
    assert_eq!(
        (h.shapes[0].under_sewing_type, h.shapes[0].flag_178),
        (0, 0)
    );
    assert_eq!(h.shapes[0].punch_exit, 2);
    let a = objects_of(&data("pe11/stroke-blocks-A.pes"));
    assert!(a.unparsed.is_none());
    assert_eq!(a.shapes[1].punch, [[0, 11], [1, 10], [4, 7], [5, 6]]);
    assert_eq!(a.shapes[1].punch_exit, 5);
}

#[test]
fn hole_sewing_pairs_read() {
    // Modify Overlap > Set hole sewing on a 15 mm circle inside a 40 mm one: the inner circle
    // Not Sewn, the pair [outer, inner] before the groups. A vector import pairs the pentagon
    // with a Not Sewn copy of the star placed after it.
    for (name, pair, inner_region) in [
        ("concentric-ring40-hole15", [1, 0], 0),
        ("spiral-ring40-hole15", [1, 0], 0),
        ("hole-sewing-pentagon-star", [0, 1], 1),
    ] {
        let o = objects_of(&owner_file(name));
        assert!(o.unparsed.is_none(), "{name}");
        assert_eq!(o.holes, [pair], "{name}");
        assert_eq!(
            o.shapes[inner_region].region,
            RegionSewType::NotSewn,
            "{name}"
        );
    }
    // The vector import's shapes keep the plain values in the unused sew types' fields.
    let star = objects_of(&owner_file("hole-sewing-pentagon-star"));
    assert!(star.shapes[0].plain_settings && !star.shapes[3].plain_settings);
}

#[test]
fn programmable_fill_and_motif_patterns_read_from_the_header() {
    // The owner's V, drawn in Programmable Stitch Creator (compat/reference/pes/owner-v.pas:
    // `@TILELINE 0 3 0 0 512 768 1024 0`, Default Size 7.5 mm) and as a motif (owner-v.pmf:
    // `@LINE 3 0 512 512 768 1024 512`, 5 mm), listed in the header before the thread table.
    let fill = objects_of(&data("pe11/obj-progfill-owner-v.pes"));
    assert_eq!(
        fill.fill_patterns,
        [pes11::PesPattern {
            size: 750,
            polylines: vec![vec![(0, 0), (512, 768), (1024, 0)]],
        }]
    );
    assert!(fill.motif_patterns.is_empty());
    assert_eq!(fill.shapes[0].region, RegionSewType::ProgrammableFill);
    let motif = objects_of(&data("pe11/obj-motif-line-owner-v.pes"));
    assert_eq!(
        motif.motif_patterns,
        [pes11::PesPattern {
            size: 500,
            polylines: vec![vec![(0, 512), (512, 768), (1024, 512)]],
        }]
    );
    // Files without patterns list none.
    assert!(
        objects_of(&data("pe11/r01-rect.pes"))
            .fill_patterns
            .is_empty()
    );
}

#[test]
fn region_types_and_the_decorative_pattern_read() {
    // Each Round 3 region file at PE-DESIGN's defaults differs from a fill only in its code.
    for (name, region) in [
        ("o-region-cross", RegionSewType::Cross),
        ("o-region-concentric", RegionSewType::Concentric),
        ("o-region-radial", RegionSewType::Radial),
        ("o-region-spiral", RegionSewType::Spiral),
        ("o-region-piping", RegionSewType::Piping),
        ("o-region-stippling", RegionSewType::Stippling),
        ("o-region-net-fill", RegionSewType::NetFill),
        ("o-region-zigzag-net", RegionSewType::ZigzagNet),
        ("o-region-flexible-spiral", RegionSewType::FlexibleSpiral),
        ("obj-motif-fill-owner-v", RegionSewType::Motif),
    ] {
        assert_eq!(
            objects_of(&data(&format!("pe11/{name}.pes"))).shapes[0].region,
            region,
            "{name}"
        );
    }
    // A Decorative Fill keeps its pattern in the object's pattern block: the owner's one-line
    // pattern (psc-deco-1line.plf, `@TILELINE 0 2 128 128 896 896`) at 100 mm.
    let deco = objects_of(&owner_file("deco-fill-1line-100mm"));
    let s = &deco.shapes[0];
    assert_eq!(s.region, RegionSewType::Decorative);
    assert_eq!(
        s.decorative,
        Some(pes11::PesPattern {
            size: 10000,
            polylines: vec![vec![(128, 128), (896, 896)]],
        })
    );
}

#[test]
fn unmapped_layouts_stop_reading_instead_of_misreading() {
    // Layouts not mapped yet must end the list as "unparsed", with the shapes read before them
    // intact and every vertex inside its bounds. The Manual Punch column of
    // satin-zigzag-column reads as one block object since its layout was mapped (m01, m03),
    // and the eight Shape-tool start-point shapes (five of them gallery shapes) read
    // completely.
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../compat/reference/stitches"
    );
    for (name, read, stop) in [
        ("satin-zigzag-column", 1, ""),
        ("shape-start-points", 8, ""),
    ] {
        let bytes = std::fs::read(format!("{dir}/{name}.pes")).expect("reference");
        let section = pes::read_objects(&bytes).expect("objects");
        assert_eq!(
            (section.shapes.len(), section.unparsed.as_deref()),
            (read, Some(stop).filter(|s| !s.is_empty())),
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
    // All eight shapes now read (five are gallery shapes): the region colours in order.
    assert_eq!(
        catalog,
        ["800", "007", "027", "019", "058", "030", "070", "005"],
        "{colours:?}"
    );
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
        "obj-progfill-owner-v",
        "obj-motif-fill-owner-v",
        "obj-motif-line-owner-v",
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
const FULLY_PARSED: &[&str] = &[
    "hole-sewing-pentagon-star",
    "concentric-ring40-hole15",
    "spiral-ring40-hole15",
    "deco-fill-1line-100mm",
    "deco-fill-1line-offcentre",
    "a01-rect-applique",
    "a02-applique-cutting",
    "a03-applique-no-tackdown",
    "a04-applique-cover-4mm",
    "a05-applique-replace",
    "stroke-blocks-H",
    "stroke-blocks-A",
    "o-manual-straight",
    "o-manual-curve",
    "o-manual-running",
    "pes-default-v6",
    "pes-csewseg-trims",
    "pes-design-properties",
    "o-gallery-heart",
    "shape-start-points",
    "text-fit-ahhhh",
    "text-fit-default",
    "text-fit-halign-left",
    "text-fit-halign-right",
    "text-fit-halign-spaced",
    "text-fit-orient-upright",
    "text-fit-otherside",
    "text-fit-satin-ahhhh",
    "text-fit-size-12",
    "text-fit-size-6",
    "text-fit-spacing-m1",
    "text-fit-spacing-p2",
    "text-fit-valign-below",
    "text-fit-valign-center",
    "o-text-transform-1",
    "text-transform-1",
    "text-transform-2",
    "text-transform-3",
    "text-transform-4",
    "text-transform-5",
    "text-transform-6",
    "text-transform-7",
    "text-transform-8",
    "text-transform-9",
    "text-transform-10",
    "text-transform-11",
    "text-transform-12",
    "text-transform-13",
    "text-transform-14",
    "text-transform-15",
    "text-transform-16",
    "text-transform-17",
    "text-transform-18",
    "text-transform-19",
    "text-transform-20",
    "text-transform-21",
    "text-transform-22",
    "text-transform-23",
    "text-transform-24",
    "text-transform-25",
    "text-transform-26",
    "r27-text-a-space-b",
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
    "m01-straight-1",
    "m03-straight-3",
    "m04-straight-1-fill",
    "m05-straight-1-density",
    "m07-curve-1",
    "m08-straight-1-line",
    "m09-curve-bent",
    "m10-curve-uneven",
    "m11-curve-4",
];

/// The fixture's stitches with the header's own name (the PEC label would otherwise become the
/// pattern's `name`, which the writer puts in the Design Property name).
fn stitches_of(bytes: &[u8]) -> openstitch_core::StitchPattern {
    let mut p = pes::read(bytes).expect("stitches");
    // The name is the header's Design Property name when it has one, else nothing (the PEC
    // label is not written into the header).
    match pes::read_objects(bytes)
        .ok()
        .and_then(|s| s.metadata.get("name").cloned())
    {
        Some(name) => p.set_name(name),
        None => {
            p.metadata.remove("name");
        }
    }
    p
}

fn objects_of(bytes: &[u8]) -> Objects {
    let s = pes::read_objects(bytes).expect("objects");
    assert_eq!(s.version, "0110");
    Objects {
        page_mm: s.page_mm,
        hoop_page: s.hoop_page,
        header: s.header,
        code: s.code,
        threads: s.threads,
        shapes: s.shapes,
        texts: s.texts,
        stitch_groups: s.stitch_groups,
        groups: s.groups,
        holes: s.holes,
        fill_patterns: s.fill_patterns,
        motif_patterns: s.motif_patterns,
        unparsed: s.unparsed,
    }
}

/// An owner file: `text-*` from compat/reference/text, the others from tests/fixtures/pe11.
fn owner_file(name: &str) -> Vec<u8> {
    if name.starts_with("pes-") {
        let path = format!(
            "{}/../../compat/reference/pes/{name}.pes",
            env!("CARGO_MANIFEST_DIR")
        );
        return std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
    if [
        "shape-start-points",
        "hole-sewing-pentagon-star",
        "concentric-ring40-hole15",
        "spiral-ring40-hole15",
        "deco-fill-1line-100mm",
        "deco-fill-1line-offcentre",
    ]
    .contains(&name)
    {
        let path = format!(
            "{}/../../compat/reference/stitches/{name}.pes",
            env!("CARGO_MANIFEST_DIR")
        );
        return std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
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
    // r01 was saved on a Custom Size page, r27 on PE-DESIGN's default Hoop Size page.
    assert!(!objects_of(&data("pe11/r01-rect.pes")).hoop_page);
    assert!(objects_of(&data("pe11/r27-text-a-space-b.pes")).hoop_page);
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
    for &name in FULLY_PARSED {
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

/// Byte offsets of the leading `CEmbLine` / `CEmbSpecialShape` bodies in `bytes` (the byte after
/// the class name or tag); `count` of them, which must come first in the list.
/// One `CEmbLine` record in a file: where its body, pattern block and geometry start, and where
/// the geometry header's marks end.
#[derive(Clone, Copy)]
struct Body {
    start: usize,
    pattern_end: usize,
    geometry: usize,
    marks_end: usize,
}

fn u16_at(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("u32")) as usize
}

/// The `count` `CEmbLine` records after the list start: the pattern block (u16 10000, u16
/// polyline count, per polyline 6 zero bytes, u16 1, u16 point count, points) is as long as
/// the pattern it holds, and the geometry header's two lists as long as their counts.
fn bodies(bytes: &[u8], count: usize) -> Vec<Body> {
    let mut out = Vec::new();
    let mut at = list_offset(bytes) + 2;
    for _ in 0..count {
        // The MFC tag: a class name the first time (FFFF, schema, u16 length, name), else u16.
        at += if bytes[at..at + 2] == [0xFF, 0xFF] {
            6 + u16_at(bytes, at + 4)
        } else {
            2
        };
        let start = at;
        let mut o = start + 388;
        let polylines = u16_at(bytes, o);
        o += 2;
        for _ in 0..polylines {
            o += 10 + 4 * u16_at(bytes, o + 8);
        }
        let pattern_end = o;
        let geometry = pattern_end + 72;
        let mut g = geometry;
        for _ in 0..2 {
            g += 4 + 12 * u32_at(bytes, g);
        }
        let marks_end = g + 52;
        let n = u32_at(bytes, marks_end);
        out.push(Body {
            start,
            pattern_end,
            geometry,
            marks_end,
        });
        at = marks_end + 4 + 28 * n;
    }
    out
}

/// `fixture` with every record's pattern block replaced by `placeholder` (the writer's own),
/// so the rest of each record compares at the writer's offsets.
fn with_pattern(fixture: &[u8], records: &[Body], placeholder: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(fixture.len());
    let mut from = 0;
    for b in records {
        out.extend_from_slice(&fixture[from..b.start + 386]);
        out.extend_from_slice(placeholder);
        from = b.pattern_end;
    }
    out.extend_from_slice(&fixture[from..]);
    out
}

/// Two geometry-header mark blocks (52 bytes: A x y, u32, f64 t, u16, B x y, u32, f64 t, C x y,
/// u16) equal but for the floats' last bits: the integers equal, coordinates within 0.001 unit,
/// parameters within 1e-4.
fn marks_close(a: &[u8], b: &[u8]) -> bool {
    let f32_at = |m: &[u8], o: usize| f32::from_le_bytes(m[o..o + 4].try_into().expect("f32"));
    let f64_at = |m: &[u8], o: usize| f64::from_le_bytes(m[o..o + 8].try_into().expect("f64"));
    let ints = [8..12, 20..22, 30..34, 50..52];
    ints.iter().all(|r| a[r.clone()] == b[r.clone()])
        && [0, 4, 22, 26, 42, 46]
            .iter()
            .all(|&o| (f32_at(a, o) - f32_at(b, o)).abs() <= 0.001)
        && [12, 34]
            .iter()
            .all(|&o| (f64_at(a, o) - f64_at(b, o)).abs() <= 1e-4)
}

#[test]
fn written_object_sections_match_pe_designs_bytes_outside_the_documented_differences() {
    // docs/FILE_FORMATS.md, "Writer": the pattern block inside every body is OpenStitch's own;
    // the two parameters of the geometry header's first marks vary between files of the same
    // shape (r02, r08), and the marks' coordinates follow the closed-shape rule, which the open
    // and curved owner files do not all share (the circles the text-fit files' texts follow
    // too). Everything else in the object section is identical.
    let marks_differ = [
        "o-gallery-heart",
        "shape-start-points",
        // A rectangle whose position fields differ from r01's (the segment of its second
        // position: the documented geometry-header difference).
        "deco-fill-1line-offcentre",
        "r10-circle",
        "r11-line",
        // A vector import's pentagon and star: outlines that are not rectangles.
        "hole-sewing-pentagon-star",
        // Two circles each (a hole sewing pair), placed like r10's.
        "concentric-ring40-hole15",
        "spiral-ring40-hole15",
        // Manual Punch > Running: an open running line, placed like r11's.
        "o-manual-running",
        // The Applique Wizard's lines follow a curved outline offset from the pattern.
        "a01-rect-applique",
        "a02-applique-cutting",
        "a03-applique-no-tackdown",
        "a04-applique-cover-4mm",
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
    for &name in FULLY_PARSED {
        let fixture = owner_file(name);
        let objects = objects_of(&fixture);
        let pattern = stitches_of(&fixture);
        let written = pes11::write(&objects, &pattern).expect("write");
        // Every pattern block is OpenStitch's placeholder (4 + 3970 bytes); PE-DESIGN's is as
        // long as the programmable pattern it holds (53 polylines in most files, one in m01).
        let records = bodies(&fixture, objects.shapes.len());
        let original_len = fixture.len();
        let fixture = match bodies(&written, objects.shapes.len()).first() {
            Some(w) => with_pattern(&fixture, &records, &written[w.start + 386..w.pattern_end]),
            None => fixture,
        };
        let shift = fixture.len() as isize - original_len as isize;
        let pec = (u32::from_le_bytes(fixture[8..12].try_into().expect("offset")) as isize + shift)
            as usize;
        let pec_w = u32::from_le_bytes(written[8..12].try_into().expect("offset")) as usize;
        // The header up to the object list: the fields after the metadata strings are the
        // file's own, written back verbatim (page, hoop and machine fields). The metadata
        // strings are written in Windows-1252 with CR LF, as PE-DESIGN shows them;
        // pes-design-properties holds OpenStitch's former UTF-8 bytes, which PE-DESIGN kept
        // (FILE_FORMATS, header strings), so its list and PEC offsets move by the difference.
        let (list, list_w) = (list_offset(&fixture), list_offset(&written));
        assert_eq!(&written[12..21], &fixture[12..21], "{name}: header");
        assert_eq!(
            objects_of(&written).header,
            objects_of(&fixture).header,
            "{name}: header"
        );
        if name != "pes-design-properties" {
            assert_eq!(&written[12..list_w], &fixture[12..list], "{name}: header");
        }
        assert_eq!(list - list_w, pec - pec_w, "{name}: PEC offset");
        // The object list and the trailer, compared from the list start.
        let (fixture, written) = (&fixture[list..pec], &written[list_w..pec_w]);
        let records = bodies(fixture, objects.shapes.len());
        // The circle's sewn extent is one unit wider than the bounds plus half the zigzag width,
        // and an open zigzag line's extent grows across the line only, not past its ends.
        let extent_differs = name == "r10-circle" || name == "r11-line";
        let allowed = |from: usize, to: usize| {
            records.iter().enumerate().any(|(k, r)| {
                let (b, gh) = (r.start, r.geometry);
                // +162: PE-DESIGN's default-colour index, OpenStitch's line colour.
                (b + 162 <= from && to <= b + 164)
                    || (extent_differs && b + 8 <= from && to <= b + 16)
                    // The two lists' positions on a shape (one entry each): their t values.
                    || (r.marks_end - r.geometry == 84
                        && ((gh + 8 <= from && to <= gh + 16) || (gh + 24 <= from && to <= gh + 32)))
                    || ((marks_differ.contains(&name) || name.starts_with("text-fit"))
                        && gh <= from
                        && to <= r.marks_end)
                    // Convert to Blocks' running pieces are open lines: their marks follow the
                    // open-line placement the writer doesn't reproduce (the blocks' own marks
                    // are compared byte for byte).
                    || (name.starts_with("stroke-blocks")
                        && objects.shapes.get(k).is_some_and(|s| !s.closed)
                        && gh <= from
                        && to <= r.marks_end)
                    // A diagonal block's marks: PE-DESIGN's crossing differs from OpenStitch's in
                    // the last bits of the floats (t within 1e-4, coordinates within 0.001 unit;
                    // the integer fields equal), as decoded below.
                    || (name.starts_with("stroke-blocks")
                        && r.marks_end - 52 <= from
                        && to <= r.marks_end
                        && marks_close(
                            &written[r.marks_end - 52..r.marks_end],
                            &fixture[r.marks_end - 52..r.marks_end],
                        ))
            })
        };
        // Text: a letter field that is f32 -0.0 in r12–r26 and 0.0 in the later owner files
        // (OpenStitch writes r12's), and a last sewing-order field PE-DESIGN leaves unset in the
        // line-spacing files (OpenStitch writes 1).
        let text_quirk = |from: usize, to: usize| {
            (to - from <= 4 && written[from..to].iter().all(|&b| b == 0 || b == 0x80))
                // A sewing-order entry's last field, 1, that PE-DESIGN sometimes leaves unset.
                || (to - from <= 4 && from + 200 > written.len() && written[from..to].iter().all(|&b| b <= 1))
                || (name.starts_with("text-line-spacing") && from >= written.len() - 4)
        };
        let unexplained: Vec<(usize, usize)> = diff_ranges(written, fixture)
            .into_iter()
            .filter(|&(s, e)| !allowed(s, e) && !text_quirk(s, e))
            .collect();
        let located: Vec<String> = unexplained
            .iter()
            .map(|&(f, t)| match records.iter().rposition(|r| r.start <= f) {
                Some(k) => {
                    let r = records[k];
                    if f >= r.geometry {
                        format!("#{k} geo+{}..{}", f - r.geometry, t - r.geometry)
                    } else {
                        format!("#{k} body+{}..{}", f - r.start, t - r.start)
                    }
                }
                None => format!("{f}..{t}"),
            })
            .collect();
        assert!(unexplained.is_empty(), "{name}: {located:?}");
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
        hoop_page: false,
        header: None,
        code: None,
        threads: vec![openstitch_core::Thread::rgb(0x12_34_56)],
        shapes: vec![shape, second],
        texts: Vec::new(),
        stitch_groups: Vec::new(),
        groups: vec![vec![0], vec![1]],
        holes: Vec::new(),
        fill_patterns: Vec::new(),
        motif_patterns: Vec::new(),
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
    let mut back = objects_of(&written);
    back.header = None;
    back.code = None;
    // A thread with no name is written with its chart index, as PE-DESIGN writes its colours.
    for (b, t) in back.threads.iter_mut().zip(&objects.threads) {
        if t.name.is_none() {
            b.name = None;
        }
    }
    assert_eq!(back, objects);
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
        hoop_page: false,
        header: None,
        code: None,
        threads: r01.threads.clone(),
        shapes: vec![r01.shapes[0].clone(), second],
        texts: vec![text],
        stitch_groups: Vec::new(),
        groups: vec![vec![1, 2]],
        holes: Vec::new(),
        fill_patterns: Vec::new(),
        motif_patterns: Vec::new(),
        unparsed: None,
    };
    let pattern = stitches_of(&owner_file("r01-rect"));
    let written = pes11::write(&objects, &pattern).expect("write");
    let mut back = objects_of(&written);
    back.header = None;
    back.code = None;
    // A thread with no name is written with its chart index, as PE-DESIGN writes its colours.
    for (b, t) in back.threads.iter_mut().zip(&objects.threads) {
        if t.name.is_none() {
            b.name = None;
        }
    }
    assert_eq!(back, objects);
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
        hoop_page: false,
        header: None,
        code: None,
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
            align: 0,
            origin: (600.0, 700.0),
            envelope: None,
            char_spacing: 0.0,
            fit: None,
        }],
        stitch_groups: Vec::new(),
        groups: Vec::new(),
        holes: Vec::new(),
        fill_patterns: Vec::new(),
        motif_patterns: Vec::new(),
        unparsed: None,
    };
    let written = pes11::write(&objects, &stitches_of(&fixture)).expect("write");
    let pec = u32::from_le_bytes(fixture[8..12].try_into().expect("offset")) as usize;
    assert_eq!(&written[..pec], &fixture[..pec]);
}

#[test]
fn the_pec_header_carries_pe_designs_fields() {
    // PE-DESIGN's own files with OpenStitch's object header (150 × 150 mm hoop, custom page)
    // carry these PEC header fields; the PEC writer's own values made PE-DESIGN show a Hoop
    // Settings tab on a written text.
    for name in [
        "r01-rect",
        "r12-text-a",
        "r14-text-ab",
        "text-defaults-h",
        "r27-text-a-space-b",
    ] {
        let fixture = owner_file(name);
        let written = pes11::write(&objects_of(&fixture), &stitches_of(&fixture)).expect("write");
        let pec_of = |b: &[u8]| u32::from_le_bytes(b[8..12].try_into().expect("offset")) as usize;
        let (p, q) = (pec_of(&fixture), pec_of(&written));
        assert_eq!(
            &written[q + 0x13..q + 0x30],
            &fixture[p + 0x13..p + 0x30],
            "{name}"
        );
        if !objects_of(&fixture).hoop_page {
            assert_eq!(
                &written[q + 0x208..q + 0x20C],
                &fixture[p + 0x208..p + 0x20C],
                "{name}"
            );
        }
    }
}

#[test]
fn text_transform_presets_read() {
    // compat/reference/text/text-transform-N: HHHHHHH with Text Transform preset N at the
    // default level. Each preset has its own code; the strength is the default level's.
    let codes: [(u16, f32); 26] = [
        (24, 30.0),
        (25, 30.0),
        (0, 50.0),
        (1, 50.0),
        (8, 20.0),
        (9, 20.0),
        (4, 50.0),
        (5, 50.0),
        (10, 10.0),
        (11, 10.0),
        (3, 50.0),
        (2, 50.0),
        (7, 30.0),
        (6, 30.0),
        (18, 20.0),
        (19, 20.0),
        (12, 50.0),
        (14, 50.0),
        (20, 20.0),
        (21, 20.0),
        (17, 50.0),
        (16, 50.0),
        (22, 20.0),
        (23, 20.0),
        (13, 50.0),
        (15, 50.0),
    ];
    for (k, &(code, strength)) in codes.iter().enumerate() {
        let name = format!("text-transform-{}", k + 1);
        let t = &objects_of(&owner_file(&name)).texts[0];
        let e = t.envelope.expect("CEmbDText");
        assert_eq!((e.code, e.strength), (code, strength), "{name}");
        assert_eq!(t.align, 1, "{name}: made centred");
        assert_eq!(
            t.origin,
            (1000.0, 1000.0),
            "{name}: the click, the alignment point"
        );
    }
    // The arches: the circle round which HHHHHHH is bent (radius, centre x, centre y).
    let over = objects_of(&owner_file("text-transform-1")).texts[0]
        .envelope
        .expect("arch");
    for (got, want) in over.geometry[..3].iter().zip([408.025, 1000.0, 1408.025]) {
        assert!((got - want).abs() < 0.01, "{:?}", over.geometry);
    }
    let plain = &objects_of(&owner_file("o-text-transform-1")).texts[0];
    assert_eq!(plain.align, 0, "left aligned");
}

#[test]
fn fit_text_to_outline_reads() {
    // compat/reference/text/text-fit-*: ABCDE (or AHHHH) fitted to a 60 mm circle, one Fit
    // Text to Outline option changed per file.
    let fit = |name: &str| {
        let o = objects_of(&owner_file(name));
        assert_eq!((o.shapes.len(), o.texts.len()), (1, 1), "{name}");
        o.texts[0].fit.expect("CEmbAText")
    };
    let default = fit("text-fit-default");
    assert_eq!(default.path, 0, "the circle, written first");
    assert_eq!(
        (
            default.halign,
            default.valign,
            default.upright,
            default.other_side
        ),
        (1, 0, false, false)
    );
    assert_eq!(fit("text-fit-halign-left").halign, 0);
    assert_eq!(fit("text-fit-halign-right").halign, 2);
    assert_eq!(fit("text-fit-halign-spaced").halign, 4);
    assert_eq!(fit("text-fit-valign-center").valign, 1);
    assert_eq!(fit("text-fit-valign-below").valign, 2);
    assert!(fit("text-fit-orient-upright").upright);
    assert!(fit("text-fit-otherside").other_side);
    let spacing = |name: &str| objects_of(&owner_file(name)).texts[0].char_spacing;
    assert_eq!(
        (
            spacing("text-fit-spacing-m1"),
            spacing("text-fit-spacing-p2")
        ),
        (-10.0, 20.0)
    );
}

#[test]
fn gallery_shapes_read_as_shapes() {
    // A Shapes-gallery heart is a CEmbLine record under the class CEmbSpecialShape: the same
    // attributes and Bézier outline; the writer keeps the class.
    let heart = objects_of(&owner_file("o-gallery-heart"));
    assert_eq!(heart.shapes.len(), 1);
    let s = &heart.shapes[0];
    assert!(s.gallery && s.closed);
    assert_eq!(s.bounds, [600, 600, 900, 900]);
    assert_eq!(s.line_type, Shape::LINE_RUNNING);
    let start = objects_of(&owner_file("shape-start-points"));
    assert_eq!(
        start.shapes.iter().filter(|s| s.gallery).count(),
        5,
        "three Shape-tool shapes, five gallery shapes"
    );
}

#[test]
fn stitch_groups_read_in_pes_0110() {
    // PE-DESIGN's re-saves of OpenStitch's v6 files hold their stitches as a CEmbOne group with
    // CSewSeg runs: flag, thread, FFFF, a u32 point count, points from the bottom-left corner.
    let g = objects_of(&owner_file("pes-default-v6"))
        .stitch_groups
        .remove(0);
    assert_eq!(g.position, 0);
    assert_eq!(g.bounds, [800, 850, 1200, 1150]);
    assert_eq!(g.transform[4..], [800.0, 1150.0]);
    assert_eq!((g.width, g.height), (400, 300));
    let runs: Vec<(u16, u16, usize)> = g
        .segments
        .iter()
        .map(|s| (s.flag, s.color, s.points.len()))
        .collect();
    assert_eq!(runs, [(0, 0, 49), (1, 1, 2), (0, 1, 17)]);
    assert_eq!(g.color_log, [(0, 0), (1, 1)]);
}

#[test]
fn a_stitch_group_built_from_stitches_round_trips() {
    use openstitch_core::{Command, Stitch, StitchPattern, Thread};
    let mut p = StitchPattern::new();
    p.threads = vec![Thread::rgb(0xFF0000), Thread::rgb(0x0000FF)];
    for (x, y, c) in [
        (100.0, 100.0, Command::Stitch),
        (200.0, 100.0, Command::Stitch),
        (200.0, 200.0, Command::Stitch),
        (200.0, 200.0, Command::ColorChange),
        (300.0, 300.0, Command::Jump),
        (300.0, 300.0, Command::Stitch),
        (400.0, 300.0, Command::Stitch),
    ] {
        p.stitches.push(Stitch::new(x, y, c));
    }
    let g = pes11::StitchGroup::from_pattern(&p, 0, |k| k as u16).expect("group");
    assert_eq!(g.bounds, [600, 600, 900, 800]);
    let objects = Objects {
        page_mm: Some((100, 100)),
        hoop_page: true,
        header: None,
        code: None,
        threads: p.threads.clone(),
        shapes: Vec::new(),
        texts: Vec::new(),
        stitch_groups: vec![g],
        groups: Vec::new(),
        holes: Vec::new(),
        fill_patterns: Vec::new(),
        motif_patterns: Vec::new(),
        unparsed: None,
    };
    let written = pes11::write(&objects, &p).expect("write");
    let mut back = objects_of(&written);
    back.header = None;
    back.code = None;
    // A thread with no name is written with its chart index, as PE-DESIGN writes its colours.
    for (b, t) in back.threads.iter_mut().zip(&objects.threads) {
        if t.name.is_none() {
            b.name = None;
        }
    }
    assert_eq!(back, objects);
}

/// PE-DESIGN 11.50 saves a design outside its Design Page with an empty machine section (no
/// colours, a 13-byte stitch block that only gives the design's size, 129 x 105 mm): reading
/// it says so instead of decoding the preview icons as 169 stitches and 256 colours, and the
/// object section still reads (#125).
#[test]
fn pe_design_oversize_save_has_no_machine_stitches() {
    let bytes = data("pe11/oversize-hoop100.pes");
    let err = pes::read(&bytes).expect_err("no machine stitches");
    let msg = err.to_string();
    assert!(
        msg.contains("129.0 x 105.0 mm") && msg.contains("Design Page"),
        "{msg}"
    );
    let objects = pes::read_objects(&bytes).expect("objects");
    assert_eq!(objects.version, "0110");
    assert_eq!(objects.threads.len(), 13);
}

/// OpenStitch's own empty design writes the same "no colours" form in its PEC section: a
/// machine-only (`1t`) file reads back with no threads and no stitches (it used to read as 256
/// Sky Blue threads).
#[test]
fn empty_pes_reads_back_with_no_threads() {
    let mut p = openstitch_core::StitchPattern::new();
    p.push(openstitch_core::Command::End);
    let options = pes::WriteOptions::parse("1t").expect("1t");
    let back = pes::read(&pes::write_with(&p, &options).expect("write")).expect("read");
    assert!(back.threads.is_empty(), "{}", back.threads.len());
    assert_eq!(back.count(openstitch_core::Command::Stitch), 0);
}
/// Every BROTHER EMBROIDERY thread in PE-DESIGN 11.50's own `#PES0110` files (the pe11
/// fixtures and `compat/reference`) has OpenStitch's PEC chart RGB for its code: the chart
/// table agrees with PE wherever PE has written a colour (issues #119, #136; 42 LINEN from
/// `thread-307-linen.pes`).
#[test]
fn pe_design_thread_tables_match_the_pec_chart() {
    use openstitch_core::{PEC_CHART, pec_index_of_brother_code};
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).expect("dir").flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pes")) {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    walk(&root.join("tests/fixtures/pe11"), &mut files);
    walk(&root.join("../../compat/reference"), &mut files);
    let mut checked = std::collections::BTreeSet::new();
    for path in &files {
        let bytes = std::fs::read(path).expect("read");
        if !bytes.starts_with(b"#PES0110") {
            continue;
        }
        let Ok(section) = pes::read_objects(&bytes) else {
            continue;
        };
        for t in &section.threads {
            let (Some(code), Some("EMBROIDERY")) =
                (t.catalog_number.as_deref(), t.brand.as_deref())
            else {
                continue;
            };
            let Some(i) = pec_index_of_brother_code(code) else {
                continue;
            };
            let chart = PEC_CHART[i].as_ref().expect("chart entry");
            assert_eq!(
                chart.hex(),
                t.hex(),
                "{} code {code} (PEC {i})",
                path.display()
            );
            checked.insert(i);
        }
    }
    assert!(
        checked.contains(&42) && checked.contains(&57),
        "{checked:?}"
    );
    assert!(checked.len() >= 20, "{checked:?}");
}

/// PE-DESIGN 11.50's own file of the 29 Ver4 Option colours carries exactly OpenStitch's sew
/// palette entries 66–94: the id where a BROTHER EMBROIDERY thread has its PEC index, the RGB,
/// type 0x0B and brand "Option Color" (#136).
#[test]
fn ver4_option_colours_match_the_sew_palette() {
    use openstitch_core::{OPTION_COLOR_BRAND, OPTION_COLOR_KIND, sew_palette};
    let objects = pes::read_objects(&data("pe11/ver4-option-color-29.pes")).expect("objects");
    let option: Vec<_> = objects
        .threads
        .iter()
        .filter(|t| t.brand.as_deref() == Some(OPTION_COLOR_BRAND))
        .collect();
    assert_eq!(option.len(), 29);
    for (k, t) in option.iter().enumerate() {
        let id: usize = t.name.as_deref().and_then(|n| n.parse().ok()).expect("id");
        assert_eq!(id, 66 + k);
        let ours = sew_palette()[id].as_ref().expect("palette entry");
        assert_eq!(ours.hex(), t.hex(), "{id}");
        assert_eq!(t.kind, Some(OPTION_COLOR_KIND));
        assert_eq!(t.catalog_number, None);
    }
}

/// The 0110 writer gives a Ver4 Option Color thread PE-DESIGN's own record: its sew palette id
/// 66–94 where a BROTHER EMBROIDERY thread has its PEC index, an empty code and chart, type
/// 0x0B and brand "Option Color", both for the threads of PE-DESIGN's file and for OpenStitch's
/// sew palette entries (#136). An RGB off the chart under that brand keeps the PEC index.
#[test]
fn option_colours_are_written_with_their_sew_palette_ids() {
    use openstitch_core::{OPTION_COLOR_BRAND, Thread, sew_palette};
    let objects = pes::read_objects(&data("pe11/ver4-option-color-29.pes")).expect("objects");
    let option: Vec<_> = objects
        .threads
        .iter()
        .filter(|t| t.brand.as_deref() == Some(OPTION_COLOR_BRAND))
        .collect();
    assert_eq!(option.len(), 29);
    let strings = |t: &Thread| {
        (
            t.rgb_value(),
            t.name.clone(),
            t.catalog_number.clone(),
            t.brand.clone(),
            t.chart.clone(),
            t.kind,
        )
    };
    for (k, pe) in option.iter().enumerate() {
        assert_eq!(
            strings(&pes11::as_pe_writes(pe)),
            strings(pe),
            "row {}",
            k + 1
        );
        let ours = sew_palette()[66 + k].as_ref().expect("palette entry");
        assert_eq!(
            strings(&pes11::as_pe_writes(ours)),
            strings(pe),
            "id {}",
            66 + k
        );
    }
    let mut off = Thread::rgb(0x00_00AB);
    off.brand = Some(OPTION_COLOR_BRAND.into());
    let nearest = openstitch_core::nearest_index(&off, openstitch_core::pec_basic_chart());
    assert_eq!(
        pes11::as_pe_writes(&off).name,
        nearest.map(|i| i.to_string().into())
    );
}

/// PE-DESIGN 11.50's colour list after the PEC section of a Hoop Size page file (the RGB of
/// each colour block as R, G, B; then a u16 0 and three bytes per block) gives PEC 62–64 the
/// Applique Wizard's colours, and OpenStitch's chart has them: (255, 200, 100) material,
/// (255, 200, 150) position, (255, 200, 200) appliqué; 1 PRUSSIAN BLUE as a check.
#[test]
fn pe_design_applique_colours_match_pec_chart_62_to_64() {
    use openstitch_core::PEC_CHART;
    let mut seen = std::collections::BTreeSet::new();
    for name in [
        "a01-rect-applique",
        "a02-applique-cutting",
        "a03-applique-no-tackdown",
        "a04-applique-cover-4mm",
        "a05-applique-replace",
    ] {
        let b = data(&format!("pe11/{name}.pes"));
        let pec = u32::from_le_bytes(b[8..12].try_into().expect("u32")) as usize;
        let blocks = usize::from(b[pec + 0x30]) + 1;
        let list = &b[pec + 0x31..pec + 0x31 + blocks];
        let rgb_at = b.len() - 3 * blocks - 2 - 3 * blocks;
        assert_eq!(
            b[rgb_at + 3 * blocks..rgb_at + 3 * blocks + 2],
            [0, 0],
            "{name}"
        );
        for (k, &index) in list.iter().enumerate() {
            let rgb = &b[rgb_at + 3 * k..rgb_at + 3 * k + 3];
            let Some(chart) = PEC_CHART.get(usize::from(index)).and_then(Option::as_ref) else {
                continue; // 65, the cutting line: not in the chart
            };
            assert_eq!(
                rgb,
                [chart.red, chart.green, chart.blue],
                "{name} block {k} PEC {index}"
            );
            seen.insert(index);
        }
    }
    assert!([1, 62, 63, 64].iter().all(|i| seen.contains(i)), "{seen:?}");
}
