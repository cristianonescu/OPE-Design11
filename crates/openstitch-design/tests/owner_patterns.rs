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
fn a_decorative_file_with_a_free_line_is_kept_as_a_tile() {
    let deco = import("psc-deco-1line.plf");
    assert_eq!(
        deco.role,
        TextRole::Decorative {
            width: 10000,
            height: 10000,
            repeat: 1
        }
    );
    // The line does not run edge to edge, so OpenStitch's row-chaining decorative fill
    // cannot sew it; it is kept as a fill tile (see docs/FILE_FORMATS.md).
    assert_eq!(deco.pattern.kind, PatternKind::Fill);
    assert_eq!(
        deco.pattern.paths,
        vec![cell(&[(128.0, 128.0), (896.0, 896.0)])]
    );
}
