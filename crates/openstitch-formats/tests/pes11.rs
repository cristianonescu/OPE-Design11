//! PE-DESIGN 11 (`#PES0110`) editable shapes, read from the owner's minimal PE-DESIGN files in
//! tests/fixtures/pe11 (provenance in tests/fixtures/pe11/PROVENANCE.md). Each rN file is the
//! baseline r01 with one property changed, so each test pins one mapped field.

mod common;

use common::data;
use openstitch_formats::pes;
use openstitch_formats::pes11::{RegionSewType, Shape};

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
fn text_is_reported_unparsed_and_stitches_still_read() {
    for name in ["r12-text-a", "r13-text-a-20", "r14-text-ab"] {
        let (s, unparsed) = shapes(name);
        assert!(s.is_empty(), "{name}");
        assert_eq!(unparsed.as_deref(), Some("CEmbNText"), "{name}");
    }
    // A rectangle, then text: the shape is read, reading stops at the text.
    let (s, unparsed) = shapes("r15-rect-text");
    assert_eq!((s.len(), unparsed.as_deref()), (1, Some("CEmbNText")));
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
