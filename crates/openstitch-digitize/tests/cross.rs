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
    // 2 runs of 3 cells: jump + anchor + 3×2 forward + 3×2 back = 14 each.
    assert_eq!(red.stitches.len(), 2 * 14);
    assert_eq!(
        red.stitches
            .iter()
            .filter(|s| s.command == Command::Jump)
            .count(),
        2
    );
    // Every needle point is on the 2.5 mm grid inside the frame.
    for s in &red.stitches {
        assert!(
            (s.x - 50.0) % 25.0 == 0.0 && (s.y - 50.0) % 25.0 == 0.0,
            "{s:?}"
        );
        assert!(s.x >= 50.0 && s.x <= 50.0 + 100.0 && s.y >= 50.0 && s.y <= 150.0);
    }
    let blue = out.objects[1].stitch_block.as_ref().expect("block");
    let pts: Vec<(f64, f64)> = blue.stitches.iter().map(|s| (s.x, s.y)).collect();
    // Cell (6, 1) with the frame at (5, 5) mm: BL (200,100) → TR (225,75) → BR (225,100) →
    // TL (200,75) → BL.
    assert_eq!(
        pts,
        vec![
            (200.0, 100.0),
            (200.0, 100.0),
            (225.0, 75.0),
            (225.0, 100.0),
            (200.0, 75.0),
            (200.0, 100.0)
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
