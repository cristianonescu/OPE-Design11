//! Concentric Circle and Spiral Stitch as PE-DESIGN 11.50 sews them (issue #138): the
//! structure its reference files show (compat/reference/stitches/concentric-*.pes,
//! spiral-*.pes) on a 40 mm circle, a 40 × 30 mm rectangle and a 40 mm ring with a 15 mm hole,
//! at PE's defaults (density 2.0 lines/mm, run pitch 2.0 mm). Lengths are 0.1 mm.

use crate::concentric::{ConcentricParams, concentric};
use crate::generate::Run;
use crate::geometry::{Contour, Vec2, contains, v2};
use crate::model::{RegionSewing, RegionType};
use crate::spiral::{SpiralParams, spiral};
use openstitch_core::Thread;

const C: Vec2 = Vec2 { x: 0.0, y: 0.0 };

fn circle(r: f64) -> Contour {
    crate::shapes::arc(C, r, r, 90.0, 360.0, crate::shapes::ArcMode::Chord)
}

/// 40 × 30 mm about the origin, drawn as the Rectangle tool draws it.
fn rectangle() -> Contour {
    Contour::rectangle(-200.0, -150.0, 400.0, 300.0)
}

fn concentric_params() -> ConcentricParams {
    ConcentricParams {
        center: C,
        spacing: 5.0,
        pitch: 20.0,
        start_inside: false,
        entry: None,
    }
}

fn spiral_params() -> SpiralParams {
    SpiralParams {
        spacing: 5.0,
        pitch: 20.0,
        reverse_winding: false,
        start_inside: false,
        entry: None,
    }
}

fn longest(run: &[Vec2]) -> f64 {
    run.windows(2)
        .map(|w| w[0].distance(w[1]))
        .fold(0.0, f64::max)
}

/// Where a path crosses the positive x axis (y = 0, x > 0), in path order.
fn crossings_x(path: &[Vec2]) -> Vec<f64> {
    path.windows(2)
        .filter(|w| (w[0].y > 0.0) != (w[1].y > 0.0) && w[0].y != w[1].y)
        .map(|w| w[0].x + (w[1].x - w[0].x) * w[0].y / (w[0].y - w[1].y))
        .filter(|&x| x > 0.0)
        .collect()
}

/// Where a path crosses the positive y axis (x = 0, y > 0: the bottom on screen).
fn crossings_y(path: &[Vec2]) -> Vec<f64> {
    let swapped: Vec<Vec2> = path.iter().map(|p| v2(p.y, p.x)).collect();
    crossings_x(&swapped)
}

#[test]
fn concentric_circle_sews_whole_circles_outside_in_then_the_cut_ones() {
    let shape = [circle(200.0)];
    let cc = concentric(&shape, &[], &concentric_params());
    // Whole circles at 0.5, 1.0, … 19.5 mm, outermost first, one row apart; each starts and
    // ends on one seam and steps one row in along it.
    let r: Vec<f64> = cc.full.iter().map(|p| p.distance(C)).collect();
    assert!((r[0] - 195.0).abs() < 1.0, "first circle 19.5 mm: {}", r[0]);
    assert!(r[r.len() - 1] < 8.0, "ends on the 0.5 mm circle");
    let mut radii: Vec<i64> = r.iter().map(|x| (x / 5.0).round() as i64).collect();
    radii.dedup();
    assert_eq!(radii, (1..=39).rev().collect::<Vec<_>>());
    // The seam: every circle starts on one radial line, where the cut 20 mm circle first meets
    // the outline walking counter-clockwise from the bottom. (On PE-DESIGN's own circle that is
    // near 9 o'clock; where a 20 mm circle meets the flattened outline depends on the
    // flattening, so the angle itself is not pinned.)
    let seam = cc.full[0].y.atan2(cc.full[0].x);
    let mut starts = 0;
    for w in cc.full.windows(2) {
        if (w[0].distance(C) - w[1].distance(C)).abs() > 4.0 {
            let a = w[1].y.atan2(w[1].x);
            let d = (a - seam)
                .abs()
                .min(std::f64::consts::TAU - (a - seam).abs());
            assert!(
                d.to_degrees() < 2.0 + 60.0 / w[1].distance(C),
                "{}",
                d.to_degrees()
            );
            starts += 1;
        }
    }
    assert!(starts >= 30, "steps in on the seam: {starts}");
    // Counter-clockwise on screen (y down): the angle falls.
    let a = |p: Vec2| p.y.atan2(p.x);
    let turn = (a(cc.full[3]) - a(cc.full[2]) + 3.0 * std::f64::consts::PI) % std::f64::consts::TAU
        - std::f64::consts::PI;
    assert!(turn < 0.0, "counter-clockwise: {turn}");
    // 64 needle points on the large circles (the chord fits 2 mm), 16 on the small ones.
    let count = |k: i64| r.iter().filter(|x| (*x / 5.0).round() as i64 == k).count();
    assert!((64..=80).contains(&count(39)), "{}", count(39));
    assert!((16..=20).contains(&count(5)), "{}", count(5));
    assert!(longest(&cc.full) <= 20.0 + 1e-9);
    // The cut circle (20 mm) comes after the trim.
    assert!(!cc.arcs.is_empty() && !cc.arcs_first);
    for arc in &cc.arcs {
        for p in arc {
            assert!((p.distance(C) - 200.0).abs() < 2.0);
        }
    }
}

#[test]
fn concentric_circle_on_a_rectangle_seams_where_the_first_cut_circle_leaves_the_bottom() {
    let shape = [rectangle()];
    let cc = concentric(&shape, &[], &concentric_params());
    // The largest whole circle touches the long sides: 15 mm.
    assert!((cc.full[0].distance(C) - 150.0).abs() < 1.0);
    // PE-DESIGN's seam: where the 15.5 mm circle crosses the bottom edge, 75.4° below the
    // horizontal on the right (y down).
    let seam = cc.full[0].y.atan2(cc.full[0].x).to_degrees();
    assert!((seam - 75.4).abs() < 1.0, "seam {seam}");
    // The cut circles run out to the corners (25 mm) as arcs inside the rectangle.
    let far = cc
        .arcs
        .iter()
        .flatten()
        .map(|p| p.distance(C))
        .fold(0.0, f64::max);
    assert!(far > 240.0, "{far}");
    for p in cc.arcs.iter().flatten() {
        assert!(p.x.abs() <= 200.5 && p.y.abs() <= 150.5, "{p:?}");
    }
}

#[test]
fn concentric_circle_skips_a_hole_and_start_point_two_reverses_it() {
    let outer = [circle(200.0)];
    let hole = [circle(75.0)];
    let cc = concentric(&outer, &hole, &concentric_params());
    for p in cc.full.iter().chain(cc.arcs.iter().flatten()) {
        assert!(p.distance(C) > 74.0, "inside the hole: {p:?}");
    }
    // The second Start Point button: the same stitches backwards.
    let mut p = concentric_params();
    p.start_inside = true;
    let back = concentric(&outer, &hole, &p);
    let mut full = cc.full.clone();
    full.reverse();
    assert_eq!(back.full, full);
    assert!(back.arcs_first);
    assert_eq!(back.arcs.len(), cc.arcs.len());
}

#[test]
fn spiral_starts_on_the_outline_and_closes_in_a_row_per_turn() {
    let shape = [circle(200.0)];
    let mut p = spiral_params();
    p.entry = Some(v2(0.0, 200.0));
    let paths = spiral(&shape, &[], &p).paths;
    assert_eq!(paths.len(), 1, "one continuous path");
    let path = &paths[0];
    assert!(
        path[0].distance(v2(0.0, 200.0)) < 2.0,
        "starts at R1 on the outline"
    );
    assert!(
        path[path.len() - 1].distance(C) < 15.0,
        "ends in the middle"
    );
    // Turns 0.5 mm apart along the axis, as PE-DESIGN's (19.65, 19.15, 18.65, …).
    let xs = crossings_x(path);
    assert!(xs.len() >= 35, "{}", xs.len());
    for w in xs.windows(2).take(20) {
        assert!((w[0] - w[1] - 5.0).abs() < 0.6, "{} then {}", w[0], w[1]);
    }
    // 64 needle points a turn on the outer turns (5.6° apart), as PE-DESIGN's.
    let a: Vec<f64> = path[..40].iter().map(|q| q.y.atan2(q.x)).collect();
    for w in a.windows(2).skip(1) {
        let d = (w[1] - w[0]).abs();
        let step = d.min(std::f64::consts::TAU - d).to_degrees();
        assert!((step - 5.625).abs() < 0.6, "{step}");
    }
    assert!(longest(path) <= 20.5, "{}", longest(path));
}

#[test]
fn spiral_turns_on_a_rectangle_are_rectangles_a_row_apart_on_both_axes() {
    let shape = [rectangle()];
    let mut p = spiral_params();
    p.entry = Some(v2(0.0, 150.0));
    let paths = spiral(&shape, &[], &p).paths;
    assert_eq!(paths.len(), 1);
    let path = &paths[0];
    // PE-DESIGN: crossings 19.85, 19.35, … on x and 14.85, 14.35, … on y: 0.5 mm apart on
    // both axes (offset turns, not shrunk copies, which would be 0.67 and 0.5).
    for xs in [crossings_x(path), crossings_y(path)] {
        let gaps: Vec<f64> = xs
            .windows(2)
            .map(|w| w[0] - w[1])
            .filter(|g| *g > 1.0)
            .collect();
        assert!(gaps.len() >= 20, "{gaps:?}");
        for g in gaps.iter().take(20) {
            assert!((g - 5.0).abs() < 0.6, "{gaps:?}");
        }
    }
    // Sides in equal stitches of up to 2 mm (PE-DESIGN's median 1.90 mm), corners kept.
    let lengths: Vec<f64> = path.windows(2).map(|w| w[0].distance(w[1])).collect();
    let mut sorted = lengths.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    assert!((median - 19.0).abs() < 1.5, "median {median}");
    // Corners stay corners: the first turn has a needle point at each corner, a fraction of
    // a row inside the outline's corner as the turn moves in.
    for corner in [v2(-200.0, 150.0), v2(-200.0, -150.0)] {
        assert!(
            path.iter().any(|q| q.distance(corner) < 3.0),
            "a needle point at {corner:?}"
        );
    }
}

#[test]
fn spiral_round_a_hole_never_sews_into_it_and_start_point_two_reverses_it() {
    let outer = [circle(200.0)];
    let hole = [circle(75.0)];
    let paths = spiral(&outer, &hole, &spiral_params()).paths;
    assert_eq!(
        paths.len(),
        2,
        "one spiral from the outline in, one round the hole"
    );
    for p in paths.iter().flatten() {
        assert!(p.distance(C) > 73.0, "inside the hole: {p:?}");
        assert!(p.distance(C) < 201.0);
    }
    let mut p = spiral_params();
    p.start_inside = true;
    let back = spiral(&outer, &hole, &p).paths;
    let mut fwd: Vec<Vec2> = paths.iter().flatten().copied().collect();
    fwd.reverse();
    let rev: Vec<Vec2> = back.iter().flatten().copied().collect();
    assert_eq!(fwd, rev);
}

#[test]
fn both_types_default_to_pe_designs_density_and_sew_through_the_region_code() {
    for kind in [RegionType::ConcentricCircle, RegionType::Spiral] {
        let r = RegionSewing::new(kind, Thread::rgb(0xff0000));
        assert_eq!((r.density, r.run_pitch_mm()), (2.0, 2.0), "{kind:?}");
        let outer = [circle(200.0)];
        let runs: Vec<Run> = crate::region::layers(&outer, &r, &[]).pop().expect("top");
        let points: usize = runs.iter().map(Vec::len).sum();
        // PE-DESIGN: 1841 (concentric) and 1682 (spiral) stitches on this circle.
        let expect = if kind == RegionType::Spiral {
            1682.0
        } else {
            1841.0
        };
        assert!(
            (points as f64 - expect).abs() / expect < 0.05,
            "{kind:?}: {points} needle points"
        );
        for run in &runs {
            for q in run {
                assert!(contains(*q, &outer) || q.distance(C) > 199.0);
            }
        }
    }
}

#[test]
fn spiral_winding_and_ending_follow_where_the_start_point_lies() {
    // PE-DESIGN 11.50 round 5: from a start point inside an edge (a rectangle's bottom middle)
    // the spiral runs clockwise to the corner and ends back at the start after a trimmed
    // closing run; from a vertex (a circle's bottom, a triangle's corner) it turns
    // counter-clockwise and ends in the middle.
    let mut p = spiral_params();
    p.entry = Some(v2(0.0, 150.0));
    let rect = spiral(&[rectangle()], &[], &p);
    let path = &rect.paths[0];
    assert_eq!(path[0], v2(0.0, 150.0), "starts at R1");
    assert!(
        path[3].x < -50.0 && (path[3].y - 150.0).abs() < 1e-6,
        "runs left along the bottom"
    );
    let closing = rect.closing.as_ref().expect("a closing run");
    assert!(
        closing[0].distance(v2(-200.0, 150.0)) < 1e-6,
        "from the bottom-left corner"
    );
    assert!(
        closing.last().expect("run").distance(v2(0.0, 150.0)) < 1e-6,
        "back to R1"
    );
    assert!(longest(closing) <= 20.0 + 1e-9);
    assert!(!rect.closing_first);

    p.entry = Some(v2(0.0, 200.0));
    let round = spiral(&[circle(200.0)], &[], &p);
    assert!(round.closing.is_none());
    let path = &round.paths[0];
    assert!(
        path[3].x > 10.0,
        "runs right from the bottom: {:?}",
        path[3]
    );
    assert!(
        path.last().expect("path").distance(C) < 15.0,
        "ends in the middle"
    );

    let triangle = Contour::new(
        vec![v2(0.0, -200.0), v2(200.0, 200.0), v2(-200.0, 200.0)],
        true,
    );
    p.entry = Some(v2(-200.0, 200.0));
    let tri = spiral(&[triangle], &[], &p);
    assert!(tri.closing.is_none());
    assert!(
        tri.paths[0][3].x > -150.0 && tri.paths[0][3].y > 190.0,
        "runs right along the base"
    );

    // The second Start Point button: the closing run comes first, reversed.
    p.entry = Some(v2(0.0, 150.0));
    p.start_inside = true;
    let back = spiral(&[rectangle()], &[], &p);
    assert!(back.closing_first);
    assert_eq!(back.closing.as_ref().expect("run")[0], v2(0.0, 150.0));
}
