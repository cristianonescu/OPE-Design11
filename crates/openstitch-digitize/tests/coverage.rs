use openstitch_design::geometry::contains;
use openstitch_design::{Vec2, v2};
use openstitch_digitize::coverage::{self, CoverageOptions};

/// Back-and-forth rows filling `[x0, x1] × [y0, y1]` (0.1 mm), `spacing` apart, stitches ≤ 40.
fn fill(x0: f64, y0: f64, x1: f64, y1: f64, spacing: f64, vertical: bool) -> Vec<Vec2> {
    let mut run = Vec::new();
    let mut k = 0;
    let (a0, a1, b0, b1) = if vertical {
        (y0, y1, x0, x1)
    } else {
        (x0, x1, y0, y1)
    };
    let mut b = b0;
    while b <= b1 + 1e-9 {
        let (s, e) = if k % 2 == 0 { (a0, a1) } else { (a1, a0) };
        let n = ((a1 - a0) / 40.0).ceil() as usize;
        for i in 0..=n {
            let a = s + (e - s) * i as f64 / n as f64;
            run.push(if vertical { v2(b, a) } else { v2(a, b) });
        }
        b += spacing;
        k += 1;
    }
    run
}

#[test]
fn a_filled_rectangle_traces_to_its_outline() {
    let runs = vec![fill(100.0, 100.0, 400.0, 250.0, 4.0, false)];
    let areas = coverage::trace(&runs, &CoverageOptions::default());
    assert_eq!(areas.len(), 1);
    let a = &areas[0];
    assert_eq!(a.contours.len(), 1);
    assert_eq!(a.angle, 0.0, "rows run horizontally");
    // About 30 × 15 mm (plus half a thread on each side).
    assert!(
        (a.area - 300.0 * 150.0).abs() < 300.0 * 150.0 * 0.06,
        "{}",
        a.area
    );
    for p in a.contours[0].path() {
        assert!(
            p.x > 95.0 && p.x < 405.0 && p.y > 95.0 && p.y < 255.0,
            "{p:?}"
        );
    }
    assert!(contains(v2(250.0, 175.0), &a.contours));
    assert!(!contains(v2(50.0, 175.0), &a.contours));
}

#[test]
fn separate_patches_holes_and_directions() {
    // A vertical-stitch bar and, apart from it, a ring of horizontal rows around a hole.
    let mut ring = Vec::new();
    for (x0, y0, x1, y1) in [(500.0, 100.0, 800.0, 160.0), (500.0, 340.0, 800.0, 400.0)] {
        ring.push(fill(x0, y0, x1, y1, 4.0, false));
    }
    for (x0, y0, x1, y1) in [(500.0, 160.0, 560.0, 340.0), (740.0, 160.0, 800.0, 340.0)] {
        ring.push(fill(x0, y0, x1, y1, 4.0, true));
    }
    let mut runs = vec![fill(100.0, 100.0, 160.0, 400.0, 4.0, true)];
    runs.extend(ring);
    let areas = coverage::trace(&runs, &CoverageOptions::default());
    assert_eq!(areas.len(), 2);
    assert_eq!(areas[0].angle, 90.0, "bar stitches run vertically");
    assert!(
        (areas[0].thickness - 60.0).abs() < 8.0,
        "{}",
        areas[0].thickness
    );
    assert_eq!(areas[1].contours.len(), 2, "ring has a hole");
    assert!(!contains(v2(650.0, 250.0), &areas[1].contours));
    assert!(contains(v2(650.0, 130.0), &areas[1].contours));
}

#[test]
fn hostile_runs_are_bounded() {
    assert!(coverage::trace(&[], &CoverageOptions::default()).is_empty());
    assert!(coverage::trace(&[vec![v2(0.0, 0.0)]], &CoverageOptions::default()).is_empty());
    let nan = vec![vec![v2(f64::NAN, 0.0), v2(1.0, f64::INFINITY)]];
    assert!(coverage::trace(&nan, &CoverageOptions::default()).is_empty());
    // A huge pattern gets coarser cells instead of a huge grid.
    let far = vec![vec![v2(-1e6, -1e6), v2(1e6, 1e6)]];
    let areas = coverage::trace(&far, &CoverageOptions::default());
    assert_eq!(areas.len(), 1);
    // Degenerate options are clamped.
    let o = CoverageOptions {
        cell: 0.0,
        thread_width: -1.0,
        close: -5.0,
        min_area: -1.0,
        ..Default::default()
    };
    let line = vec![vec![v2(0.0, 0.0), v2(100.0, 0.0)]];
    assert_eq!(coverage::trace(&line, &o).len(), 1);
}
