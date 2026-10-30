//! The owner-made Programmable Stitch Creator files (tests/fixtures/patterns, PROVENANCE.md)
//! import into the patterns PE-DESIGN's editor showed when they were drawn.

use openstitch_design::pattern_text::{self, TextRole};
use openstitch_design::patterns::PatternKind;
use openstitch_design::{Vec2, v2};
use std::path::Path;

fn read(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/patterns")
        .join(name);
    std::fs::read_to_string(path).expect("fixture")
}

fn import(name: &str) -> pattern_text::TextPattern {
    let text = read(name);
    assert!(text.contains("\r\n"), "{name}: PE-DESIGN writes CRLF");
    assert!(pattern_text::looks_like(text.as_bytes()));
    pattern_text::parse(&text, "owner", "Owner").unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn cell(points: &[(f64, f64)]) -> Vec<Vec2> {
    points
        .iter()
        .map(|&(x, y)| v2(x / 1024.0, y / 1024.0))
        .collect()
}

#[test]
fn fill_stamp_files_are_tiles_in_the_1024_cell() {
    let line = import("psc-line-2pt.pas");
    assert_eq!(line.version, Some(10000));
    assert_eq!(line.unit, None);
    assert_eq!(line.role, TextRole::Fill { subtype: 1 });
    assert_eq!(line.pattern.kind, PatternKind::Fill);
    assert_eq!(
        line.pattern.paths,
        vec![cell(&[(0.0, 0.0), (1024.0, 1024.0)])]
    );

    let square = import("psc-square-4pt.pas");
    assert_eq!(
        square.pattern.paths,
        vec![cell(&[
            (128.0, 128.0),
            (896.0, 128.0),
            (896.0, 896.0),
            (128.0, 896.0),
            (128.0, 128.0),
        ])],
        "a closed square keeps its closing point"
    );

    let two = import("psc-two-lines.pas");
    assert_eq!(
        two.pattern.paths,
        vec![
            cell(&[(0.0, 256.0), (1024.0, 256.0)]),
            cell(&[(0.0, 768.0), (1024.0, 768.0)]),
        ],
        "separate lines are separate paths"
    );
}

#[test]
fn motif_files_run_from_0_0_to_1_0() {
    let flat = import("psc-motif-2pt.pmf");
    assert_eq!(flat.role, TextRole::Motif { height: 1 });
    assert_eq!(flat.pattern.kind, PatternKind::Motif);
    assert_eq!(flat.pattern.paths, vec![vec![v2(0.0, 0.0), v2(1.0, 0.0)]]);

    let zig = import("psc-motif-zigzag-3pt.pmf");
    // (512, 768) is 256 cell units below the base line: a quarter of the motif size, +y
    // down (PE-DESIGN sews it 1.25 mm off the line at 5 mm).
    assert_eq!(
        zig.pattern.paths,
        vec![vec![v2(0.0, 0.0), v2(0.5, 0.25), v2(1.0, 0.0)]]
    );
}

#[test]
fn a_decorative_file_with_a_free_line_keeps_it_free() {
    let deco = import("psc-deco-1line.plf");
    assert_eq!(
        deco.role,
        TextRole::Decorative {
            width: 10000,
            height: 10000,
            repeat: 1
        }
    );
    // The line does not run edge to edge: a free line, tiled as a grid centred on the
    // region (PE-DESIGN's deco-fill-1line-100mm.pes sews it corner to corner).
    assert_eq!(deco.pattern.kind, PatternKind::Decorative);
    assert!(deco.pattern.free_lines);
    assert_eq!(
        deco.pattern.paths,
        vec![cell(&[(128.0, 128.0), (896.0, 896.0)])]
    );
}

#[test]
fn free_decorative_lines_are_tiled_centred_on_the_region() {
    use openstitch_core::Thread;
    use openstitch_design::model::{RegionSewing, RegionType};
    use openstitch_design::{Contour, region};
    let deco = import("psc-deco-1line.plf").pattern;
    // PE-DESIGN's deco-fill-1line-100mm: a 40 mm square, the 100 mm tile centred on it, so
    // the line (128, 128)-(896, 896) of the cell runs from corner to corner.
    let square = Contour::rectangle(300.0, 300.0, 400.0, 400.0);
    let mut r = RegionSewing::new(RegionType::DecorativeFill, Thread::rgb(0));
    r.pattern = Some(deco.id.clone());
    r.pattern_width = Some(100.0);
    r.pattern_height = Some(100.0);
    r.angle = 0.0;
    r.run_pitch = Some(2.0);
    r.underlay = false;
    let runs = region::layers_with(std::slice::from_ref(&square), &r, &[], &[deco])
        .pop()
        .expect("top layer");
    let pts: Vec<Vec2> = runs.into_iter().flatten().collect();
    let on_outline = |p: &Vec2| {
        (p.x - 300.0).abs() < 0.5
            || (p.x - 700.0).abs() < 0.5
            || (p.y - 300.0).abs() < 0.5
            || (p.y - 700.0).abs() < 0.5
    };
    let diagonal: Vec<&Vec2> = pts
        .iter()
        .filter(|p| !on_outline(p) && (p.x - p.y).abs() < 0.5)
        .collect();
    assert!(
        pts.iter().all(|p| on_outline(p) || (p.x - p.y).abs() < 0.5),
        "every needle point lies on the outline or the line"
    );
    // 28 inner points: the 56.6 mm line split evenly at the 2 mm run pitch (1.95 mm), as
    // PE-DESIGN sews it.
    assert_eq!(diagonal.len(), 28, "{diagonal:?}");
    assert!(pts.iter().any(|p| p.distance(v2(300.0, 300.0)) < 0.5));
    assert!(pts.iter().any(|p| p.distance(v2(700.0, 700.0)) < 0.5));
}
