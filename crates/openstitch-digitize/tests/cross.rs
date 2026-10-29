mod common;

use common::*;
use openstitch_core::Command;
use openstitch_design::{Compiler, Design, DesignPage};
use openstitch_digitize::cross::{self, CrossStitch, CrossType};
use openstitch_digitize::{DigitizeError, Frame};

#[test]
fn grid_cells_colours_and_crosses() {
    // 40×20 px at 0.5 mm/px = 20×10 mm → 8×4 cells of 2.5 mm (5 px). Red covers cells
    // 1–3 of rows 1–2, blue cell 6 of row 1; white background.
    let img = rects(40, 20, WHITE, &[(5, 5, 20, 15, RED), (30, 5, 35, 10, BLUE)]);
    let frame = Frame::new(5.0, 5.0, 20.0, 40);
    let out = cross::convert(
        &img,
        &frame,
        &CrossStitch {
            colors: 3,
            ..Default::default()
        },
    )
    .expect("converts");
    assert_eq!((out.columns, out.rows), (8, 4));
    assert_eq!(out.cells, vec![6, 1]);
    assert_eq!(out.objects.len(), 2);
    assert!(out.background.is_some());
    let red = out.objects[0].stitch_block.as_ref().expect("block");
    // 2 runs of 3 cells: jump + anchor + 3×3 forward + 3×3 back = 20 each (six stitches a
    // cell, as PE-DESIGN 11.50 sews them).
    assert_eq!(red.stitches.len(), 2 * 20);
    assert_eq!(
        red.stitches
            .iter()
            .filter(|s| s.command == Command::Jump)
            .count(),
        2
    );
    // Every needle point is a cell corner or a cell centre inside the frame, and every
    // stitch lies on a diagonal.
    for s in &red.stitches {
        let (u, v) = ((s.x - 50.0) / 12.5, (s.y - 50.0) / 12.5);
        assert!(
            u.fract() == 0.0 && v.fract() == 0.0 && (u as i64 % 2 == v as i64 % 2),
            "{s:?}"
        );
        assert!(s.x >= 50.0 && s.x <= 50.0 + 100.0 && s.y >= 50.0 && s.y <= 150.0);
    }
    for w in red.stitches.windows(2) {
        if w[1].command == Command::Stitch && (w[0].x, w[0].y) != (w[1].x, w[1].y) {
            assert_eq!((w[1].x - w[0].x).abs(), (w[1].y - w[0].y).abs(), "{w:?}");
        }
    }
    let blue = out.objects[1].stitch_block.as_ref().expect("block");
    let pts: Vec<(f64, f64)> = blue.stitches.iter().map(|s| (s.x, s.y)).collect();
    // Cell (6, 1) with the frame at (5, 5) mm: TL (200,75) → centre → BL (200,100) → "/" to
    // TR (225,75) → centre → BR (225,100) → "\" to TL.
    assert_eq!(
        pts,
        vec![
            (200.0, 75.0),
            (200.0, 75.0),
            (212.5, 87.5),
            (200.0, 100.0),
            (225.0, 75.0),
            (212.5, 87.5),
            (225.0, 100.0),
            (200.0, 75.0)
        ]
    );
}

#[test]
fn half_crosses_and_determinism() {
    let img = rects(10, 10, WHITE, &[(0, 0, 10, 5, BLACK)]);
    let frame = Frame::new(0.0, 0.0, 10.0, 10);
    let p = CrossStitch {
        cell_mm: 5.0,
        colors: 2,
        kind: CrossType::Half,
        ..Default::default()
    };
    let a = cross::convert(&img, &frame, &p).expect("converts");
    assert_eq!(a, cross::convert(&img, &frame, &p).expect("converts"));
    let b = a.objects[0].stitch_block.as_ref().expect("block");
    let pts: Vec<(f64, f64)> = b.stitches.iter().map(|s| (s.x, s.y)).collect();
    assert_eq!(
        pts,
        vec![
            (0.0, 50.0),
            (0.0, 50.0),
            (50.0, 0.0),
            (50.0, 50.0),
            (100.0, 0.0)
        ]
    );
    let mut d = Design::new("t", DesignPage::default());
    for o in a.objects {
        d.add(o);
    }
    assert!(Compiler::default().compile(&d).count(Command::Stitch) >= 4);
}

#[test]
fn hostile_grids_fail_cleanly() {
    let img = rects(4, 4, RED, &[]);
    // 4 px at 1000 mm wide with 1 mm cells → 1 000 000 cells.
    let huge = Frame::new(0.0, 0.0, 1000.0, 4);
    assert!(matches!(
        cross::convert(
            &img,
            &huge,
            &CrossStitch {
                cell_mm: 1.0,
                ..Default::default()
            }
        ),
        Err(DigitizeError::TooLarge(_))
    ));
    let clear = rects(4, 4, CLEAR, &[]);
    let frame = Frame::new(0.0, 0.0, 10.0, 4);
    assert!(matches!(
        cross::convert(&clear, &frame, &CrossStitch::default()),
        Err(DigitizeError::Empty(_))
    ));
    assert!(matches!(
        cross::convert(&rects(4, 4, WHITE, &[]), &frame, &CrossStitch::default()),
        Err(DigitizeError::Empty(_))
    ));
    // Tiny frame: one cell.
    let tiny = Frame::new(0.0, 0.0, 0.1, 4);
    let out = cross::convert(
        &img,
        &tiny,
        &CrossStitch {
            skip_background: false,
            ..Default::default()
        },
    )
    .expect("one cell");
    assert_eq!(out.cells, vec![1]);
}

/// #132: the documented defaults, and the needle points per cell they imply.
#[test]
fn defaults_are_documented_and_configurable() {
    let d = CrossStitch::default();
    assert_eq!(d.cell_mm, 2.5);
    assert_eq!(d.cell_mm, CrossStitch::CELL_MM);
    assert_eq!(d.colors, 5, "PE-DESIGN's Max. Number of Colors");
    assert_eq!(d.colors, CrossStitch::COLORS);
    assert_eq!(d.kind, CrossType::Full);
    assert!(!d.triple);
    assert!(d.skip_background);
    assert_eq!(d.stitches_per_cell(), 6, "PE-DESIGN's six stitches a cell");
    let triple = CrossStitch {
        triple: true,
        ..Default::default()
    };
    assert_eq!(triple.stitches_per_cell(), 10);
    let half = CrossStitch {
        kind: CrossType::Half,
        ..Default::default()
    };
    assert_eq!(half.stitches_per_cell(), 2);
}

/// #132: the grid of a sample image is stable: 20 × 10 mm at 2.5 mm cells is 8 × 4 cells, and
/// a 3 mm cell gives round(20 / 3) × round(10 / 3) = 7 × 3.
#[test]
fn grid_follows_the_cell_size() {
    let img = rects(40, 20, WHITE, &[(5, 5, 20, 15, RED)]);
    let frame = Frame::new(0.0, 0.0, 20.0, 40);
    let a = cross::convert(&img, &frame, &CrossStitch::default()).expect("converts");
    assert_eq!((a.columns, a.rows), (8, 4));
    let b = cross::convert(
        &img,
        &frame,
        &CrossStitch {
            cell_mm: 3.0,
            ..Default::default()
        },
    )
    .expect("converts");
    assert_eq!((b.columns, b.rows), (7, 3));
    assert_eq!(
        a,
        cross::convert(&img, &frame, &CrossStitch::default()).expect("converts")
    );
}

/// #132: colour reduction is Auto Punch's census prepass: a one-pixel speck inside a cell takes
/// its 3×3 modal colour and never reaches the vote, so the cell is its surround's colour and
/// the speck's colour is not a thread at all.
#[test]
fn colours_come_from_the_shared_census() {
    // 10 px per cell (1 mm/px, 10 mm cells): one blue pixel in each red cell.
    let img = rects(
        40,
        40,
        WHITE,
        &[
            (0, 0, 20, 20, RED),
            (5, 5, 6, 6, BLUE),
            (15, 15, 16, 16, BLUE),
        ],
    );
    let frame = Frame::new(0.0, 0.0, 40.0, 40);
    let out = cross::convert(
        &img,
        &frame,
        &CrossStitch {
            cell_mm: 10.0,
            colors: 8,
            ..Default::default()
        },
    )
    .expect("converts");
    assert_eq!((out.columns, out.rows), (4, 4));
    assert_eq!(out.threads.len(), 1, "{:?}", out.threads);
    assert!(out.threads[0].red > 150 && out.threads[0].blue < 100);
    assert_eq!(out.cells, vec![4]);
}

/// #132: triple sews every full diagonal there, back and there again; the needle still ends
/// where the single cross does, so the run continues the same way.
#[test]
fn triple_sews_each_diagonal_three_times() {
    let img = rects(10, 10, WHITE, &[(0, 0, 5, 5, BLACK)]);
    let frame = Frame::new(0.0, 0.0, 10.0, 10);
    let single = cross::convert(
        &img,
        &frame,
        &CrossStitch {
            cell_mm: 5.0,
            colors: 2,
            ..Default::default()
        },
    )
    .expect("converts");
    let triple = cross::convert(
        &img,
        &frame,
        &CrossStitch {
            cell_mm: 5.0,
            colors: 2,
            triple: true,
            ..Default::default()
        },
    )
    .expect("converts");
    let pts = |c: &cross::Converted| -> Vec<(f64, f64)> {
        c.objects[0]
            .stitch_block
            .as_ref()
            .expect("block")
            .stitches
            .iter()
            .map(|s| (s.x, s.y))
            .collect()
    };
    // One cell: TL anchor, halves through the centre to BL, "/" to TR, halves through the
    // centre to BR, "\" to TL.
    assert_eq!(
        pts(&single),
        vec![
            (0.0, 0.0),
            (0.0, 0.0),
            (25.0, 25.0),
            (0.0, 50.0),
            (50.0, 0.0),
            (25.0, 25.0),
            (50.0, 50.0),
            (0.0, 0.0)
        ]
    );
    assert_eq!(
        pts(&triple),
        vec![
            (0.0, 0.0),
            (0.0, 0.0),
            (25.0, 25.0),
            (0.0, 50.0),
            (50.0, 0.0),
            (0.0, 50.0),
            (50.0, 0.0),
            (25.0, 25.0),
            (50.0, 50.0),
            (0.0, 0.0),
            (50.0, 50.0),
            (0.0, 0.0)
        ]
    );
}

/// PE-DESIGN 11.50's Cross Stitch on the owner's four-shape image (round-4 capture
/// `crossstitch-default.pes`): six stitches on the diagonals per cell, half of them halves
/// through the centre, none along a cell side.
#[test]
fn cells_are_six_diagonal_stitches_as_pe_design_sews_them() {
    let img = rects(30, 30, WHITE, &[(5, 5, 25, 25, RED)]);
    let out = cross::convert(
        &img,
        &Frame::new(0.0, 0.0, 30.0, 30),
        &CrossStitch::default(),
    )
    .expect("converts");
    let cells: usize = out.cells.iter().sum();
    let b = out.objects[0].stitch_block.as_ref().expect("block");
    let mut halves = 0;
    let mut fulls = 0;
    for w in b.stitches.windows(2) {
        if w[1].command != Command::Stitch || (w[0].x, w[0].y) == (w[1].x, w[1].y) {
            continue;
        }
        let (dx, dy) = ((w[1].x - w[0].x).abs(), (w[1].y - w[0].y).abs());
        assert_eq!(dx, dy, "only diagonals: {w:?}");
        if dx == 12.5 {
            halves += 1;
        } else {
            assert_eq!(dx, 25.0);
            fulls += 1;
        }
    }
    assert_eq!((fulls, halves), (2 * cells, 4 * cells));
}
