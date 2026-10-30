//! Contour fill, beyond PE-DESIGN (backlog B1; internal regression tier, geometric expected
//! values from docs/research/beyond/contour-fill.md): a circle, a rounded rectangle, a square
//! with a hole, a U shape whose rings split; the three strategies, echo lines and joins.
//! Lengths are 0.1 mm.

use crate::contour::{ContourParams, contour};
use crate::geometry::{Contour, Vec2, contains, v2};
use crate::model::{ContourStrategy, RegionSewing, RegionType};
use openstitch_core::Thread;

const C: Vec2 = Vec2 { x: 0.0, y: 0.0 };

/// 2 mm between rings, 2 mm stitches.
fn params(strategy: ContourStrategy) -> ContourParams {
    ContourParams {
        spacing: 20.0,
        pitch: 20.0,
        strategy,
        rounded: true,
        echo_lines: 0,
        counterclockwise: false,
        start_inside: false,
        entry: None,
    }
}

fn circle(r: f64) -> Contour {
    crate::shapes::arc(C, r, r, 90.0, 360.0, crate::shapes::ArcMode::Chord)
}

/// 60 × 40 mm with 8 mm corner radii.
fn rounded_rectangle() -> Contour {
    let (w, h, r) = (300.0, 200.0, 80.0);
    let mut pts = Vec::new();
    for (cx, cy, a0) in [
        (w - r, h - r, 0.0),
        (-w + r, h - r, 90.0),
        (-w + r, -h + r, 180.0),
        (w - r, -h + r, 270.0),
    ] {
        for k in 0..=12 {
            let t = (a0 + 90.0 * k as f64 / 12.0_f64).to_radians();
            pts.push(v2(cx + r * t.cos(), cy + r * t.sin()));
        }
    }
    Contour::new(pts, true)
}

/// A U: 60 × 40 mm, a 20 mm notch 30 mm deep from the top (bottom bar 10 mm thick).
fn u_shape() -> Contour {
    Contour::new(
        vec![
            v2(-300.0, -200.0),
            v2(-100.0, -200.0),
            v2(-100.0, 100.0),
            v2(100.0, 100.0),
            v2(100.0, -200.0),
            v2(300.0, -200.0),
            v2(300.0, 200.0),
            v2(-300.0, 200.0),
        ],
        true,
    )
}

fn longest(path: &[Vec2]) -> f64 {
    path.windows(2)
        .map(|w| w[0].distance(w[1]))
        .fold(0.0, f64::max)
}

fn to_outline(p: Vec2, contours: &[Contour]) -> f64 {
    contours
        .iter()
        .flat_map(Contour::edges)
        .map(|(a, b)| {
            let ab = b - a;
            let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / (ab.x * ab.x + ab.y * ab.y))
                .clamp(0.0, 1.0);
            a.lerp(b, t).distance(p)
        })
        .fold(f64::INFINITY, f64::min)
}

fn crossings(path: &[Vec2]) -> usize {
    let o = |p: Vec2, q: Vec2, r: Vec2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
    let segs: Vec<(Vec2, Vec2)> = path.windows(2).map(|w| (w[0], w[1])).collect();
    let mut n = 0;
    for i in 0..segs.len() {
        for j in i + 2..segs.len() {
            let ((a, b), (c, d)) = (segs[i], segs[j]);
            if o(a, b, c) * o(a, b, d) < -1e-9 && o(c, d, a) * o(c, d, b) < -1e-9 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn circle_rings_are_concentric_half_a_spacing_in_then_a_spacing_apart() {
    let rings = contour(&[circle(200.0)], &[], &params(ContourStrategy::Rings));
    // 1, 3, … 19 mm inside a 20 mm radius.
    assert_eq!(rings.len(), 10);
    for (k, ring) in rings.iter().enumerate() {
        let expect = 200.0 - (k as f64 + 0.5) * 20.0;
        for p in ring {
            assert!(
                (p.distance(C) - expect).abs() < 1.0,
                "ring {k}: {}",
                p.distance(C)
            );
        }
        assert!(
            ring[0].distance(*ring.last().expect("ring")) < 1e-6,
            "closed"
        );
        assert!(longest(ring) <= 20.0 + 1e-9);
    }
    // Neighbouring rings start on different radial lines (no stacked needle points).
    let a = |p: Vec2| p.y.atan2(p.x);
    for w in rings.windows(2).take(6) {
        assert!((a(w[0][0]) - a(w[1][0])).abs() > 1e-3);
    }
}

#[test]
fn rounded_rectangle_rings_follow_the_outline_at_a_constant_distance() {
    let shape = [rounded_rectangle()];
    let rings = contour(&shape, &[], &params(ContourStrategy::Rings));
    assert_eq!(rings.len(), 10, "the short side is 40 mm");
    for (k, ring) in rings.iter().enumerate() {
        let expect = (k as f64 + 0.5) * 20.0;
        for &p in ring {
            let d = to_outline(p, &shape);
            assert!((d - expect).abs() < 1.5, "ring {k}: {d}");
        }
    }
}

#[test]
fn single_spiral_is_one_path_that_never_crosses_itself() {
    for shape in [circle(200.0), rounded_rectangle()] {
        let paths = contour(
            std::slice::from_ref(&shape),
            &[],
            &params(ContourStrategy::SingleSpiral),
        );
        assert_eq!(paths.len(), 1);
        assert_eq!(crossings(&paths[0]), 0);
        assert!(longest(&paths[0]) <= 20.5);
        // Starts half a spacing inside the outline.
        let d = to_outline(paths[0][0], std::slice::from_ref(&shape));
        assert!((d - 10.0).abs() < 1.5, "{d}");
    }
}

#[test]
fn double_spiral_ends_next_to_where_it_began() {
    let shape = [circle(200.0)];
    let paths = contour(&shape, &[], &params(ContourStrategy::DoubleSpiral));
    assert_eq!(paths.len(), 1);
    let path = &paths[0];
    let (start, end) = (path[0], path[path.len() - 1]);
    assert!(start.distance(C) > 185.0, "starts on the outer ring");
    assert!(
        end.distance(C) > 160.0,
        "ends on the second ring: {}",
        end.distance(C)
    );
    assert!(start.distance(end) < 60.0, "{}", start.distance(end));
    assert!(
        path.iter().any(|p| p.distance(C) < 20.0),
        "reaches the middle"
    );
    assert!(longest(path) <= 20.5);
    // The same rows as the rings, just ordered differently: about the same length.
    let total = |paths: &[Vec<Vec2>]| -> f64 {
        paths
            .iter()
            .map(|p| p.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>())
            .sum()
    };
    let rings = contour(&shape, &[], &params(ContourStrategy::Rings));
    let (d, r) = (total(&paths), total(&rings));
    assert!((d - r).abs() / r < 0.15, "{d} against {r}");
}

#[test]
fn a_hole_gets_rings_of_its_own_and_is_never_sewn_over() {
    let shape = [
        Contour::rectangle(-200.0, -200.0, 400.0, 400.0),
        Contour::rectangle(-50.0, -50.0, 100.0, 100.0),
    ];
    let hole = [shape[1].clone()];
    let rings = contour(&shape, &[], &params(ContourStrategy::Rings));
    // Outline and hole meet 7.5 mm apart: rings at 1, 3, 5, 7 mm from each.
    assert_eq!(rings.len(), 8);
    for ring in &rings {
        assert!(ring.iter().all(|&p| !contains(p, &hole)));
    }
    // The same as a paired hole sewing.
    let paired = contour(&shape[..1], &hole, &params(ContourStrategy::Rings));
    assert_eq!(paired.len(), 8);
    // Through the region code: one run, travel inside, never across the hole.
    let mut r = RegionSewing::new(RegionType::Contour, Thread::rgb(0xff0000));
    r.contour_strategy = ContourStrategy::SingleSpiral;
    let runs = crate::region::layers(&shape, &r, &[]).pop().expect("top");
    assert_eq!(runs.len(), 1);
    for w in runs[0].windows(2) {
        assert!(!contains(w[0].lerp(w[1], 0.5), &hole));
    }
}

#[test]
fn a_u_shape_splits_and_each_arm_is_sewn_whole() {
    let paths = contour(&[u_shape()], &[], &params(ContourStrategy::SingleSpiral));
    assert_eq!(
        paths.len(),
        2,
        "the outer turns run on into one arm; the other arm alone"
    );
    let left = |p: &Vec2| p.x < 0.0;
    let tail = &paths[0][paths[0].len() * 3 / 4..];
    let side = left(tail.last().expect("points"));
    assert!(tail.iter().all(|p| left(p) == side));
    assert!(paths[1].iter().all(|p| left(p) != side));
}

#[test]
fn echo_lines_ring_the_shape_from_the_outside_in_first() {
    let mut p = params(ContourStrategy::Rings);
    p.echo_lines = 3;
    let shape = [circle(100.0)];
    let paths = contour(&shape, &[], &p);
    // Echo rings at 16, 14 and 12 mm (2 mm apart outside a 10 mm radius), outermost first,
    // in one path that leads into the first inner ring.
    let first = &paths[0];
    let radii: Vec<i64> = first
        .iter()
        .map(|q| (q.distance(C) / 20.0).round() as i64)
        .collect();
    let mut seen = radii.clone();
    seen.dedup();
    assert_eq!(&seen[..3], &[8, 7, 6], "{seen:?}");
    assert!(first.iter().any(|q| (q.distance(C) - 160.0).abs() < 1.0));
    assert!(longest(first) <= 20.0 + 1e-6);
    let inside = paths
        .iter()
        .flatten()
        .filter(|q| q.distance(C) < 100.0)
        .count();
    assert!(inside > 50, "the inner rings follow");
}

#[test]
fn mitred_echo_rings_keep_square_corners_round_ones_do_not() {
    let square = [Contour::rectangle(-100.0, -100.0, 200.0, 200.0)];
    let corner = v2(120.0, 120.0);
    for (rounded, near) in [(false, true), (true, false)] {
        let mut p = params(ContourStrategy::Rings);
        p.echo_lines = 1;
        p.rounded = rounded;
        let echo = &contour(&square, &[], &p)[0];
        let hit = echo.iter().any(|q| q.distance(corner) < 2.0);
        assert_eq!(hit, near, "rounded {rounded}");
    }
}

#[test]
fn start_inside_reverses_and_counterclockwise_turns_the_other_way() {
    let shape = [circle(200.0)];
    let fwd = contour(&shape, &[], &params(ContourStrategy::SingleSpiral));
    let mut p = params(ContourStrategy::SingleSpiral);
    p.start_inside = true;
    let back = contour(&shape, &[], &p);
    let mut f = fwd[0].clone();
    f.reverse();
    assert_eq!(back[0], f);
    let turn = |path: &[Vec2]| {
        let (a, b) = (path[3], path[4]);
        a.x * b.y - a.y * b.x
    };
    p.start_inside = false;
    p.counterclockwise = true;
    let ccw = contour(&shape, &[], &p);
    assert!(turn(&fwd[0]) > 0.0 && turn(&ccw[0]) < 0.0);
}

#[test]
fn the_region_type_defaults_and_stays_deterministic() {
    let r = RegionSewing::new(RegionType::Contour, Thread::rgb(0xff0000));
    assert_eq!((r.spacing_mm(), r.run_pitch_mm()), (2.0, 2.0));
    assert_eq!(r.contour_strategy, ContourStrategy::Rings);
    let shape = [rounded_rectangle()];
    assert_eq!(
        crate::region::layers(&shape, &r, &[]),
        crate::region::layers(&shape, &r, &[])
    );
    let tiny = [circle(5.0)];
    assert!(contour(&tiny, &[], &params(ContourStrategy::Rings)).is_empty());
}

#[test]
fn a_ring_that_collapses_is_sewn_all_round_before_the_remnant() {
    // A heart's insets end in a small remnant in one lobe: the spirals must not leave the
    // last full ring half sewn. They sew the same rows as the rings, in about the same length.
    let heart = crate::shapes::heart().transformed(&crate::geometry::Transform::scaling(
        180.0,
        180.0,
        v2(0.0, 0.0),
    ));
    let shape = [heart];
    let total = |paths: &[Vec<Vec2>]| -> f64 {
        paths
            .iter()
            .map(|p| p.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>())
            .sum()
    };
    let mut p = params(ContourStrategy::Rings);
    p.spacing = 15.0;
    let rings = total(&contour(&shape, &[], &p));
    for strategy in [ContourStrategy::SingleSpiral, ContourStrategy::DoubleSpiral] {
        p.strategy = strategy;
        let paths = contour(&shape, &[], &p);
        let len = total(&paths);
        assert!(
            (len - rings).abs() / rings < 0.12,
            "{strategy:?}: {len} against the rings' {rings}"
        );
        for path in &paths {
            assert!(longest(path) <= 20.5, "{strategy:?}: {}", longest(path));
        }
    }
}
