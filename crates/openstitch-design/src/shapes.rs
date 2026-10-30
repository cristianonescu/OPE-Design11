//! Arcs and preset shapes, built from independent geometry (regular polygons, stars, Bézier
//! approximations of elliptical arcs). Lengths are 0.1 mm, angles degrees, +y down, positive
//! angles clockwise on screen.

use crate::fmath::Det;
use crate::geometry::{Bezier, Contour, Rect, Transform, Vec2, v2};
use std::f64::consts::{FRAC_PI_2, TAU};

/// How an arc is closed (PE-DESIGN's arc, fan shape and arc-and-string tools).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcMode {
    /// Open curve along the ellipse.
    Arc,
    /// Closed through the centre (pie slice).
    Fan,
    /// Closed by the straight chord between the ends.
    Chord,
}

/// Elliptical arc around `center` from `start` through `sweep` degrees (|sweep| ≤ 360), as cubic
/// Béziers of at most 90° each. A full-turn sweep yields a closed ellipse whatever the mode.
pub fn arc(center: Vec2, rx: f64, ry: f64, start: f64, sweep: f64, mode: ArcMode) -> Contour {
    let sweep = sweep.clamp(-360.0, 360.0);
    let full = sweep.abs() >= 360.0;
    let n = ((sweep.abs() / 90.0).ceil() as usize).max(1);
    let step = sweep.to_radians() / n as f64;
    let k = 4.0 / 3.0 * (step / 4.0).dtan();
    let at = |a: f64| v2(center.x + rx * a.dcos(), center.y + ry * a.dsin());
    let tangent = |a: f64| v2(-rx * a.dsin(), ry * a.dcos());
    let a0 = start.to_radians();
    let mut points: Vec<Vec2> = (0..=n).map(|i| at(a0 + step * i as f64)).collect();
    let mut curves: Vec<Option<Bezier>> = (0..n)
        .map(|i| {
            let (a, b) = (a0 + step * i as f64, a0 + step * (i + 1) as f64);
            Some(Bezier {
                c1: at(a) + tangent(a) * k,
                c2: at(b) - tangent(b) * k,
            })
        })
        .collect();
    let closed = if full {
        // The last vertex coincides with the first: the closing segment carries the last curve.
        points.pop();
        true
    } else {
        match mode {
            ArcMode::Arc => false,
            ArcMode::Chord => {
                curves.push(None);
                true
            }
            ArcMode::Fan => {
                points.push(center);
                curves.extend([None, None]);
                true
            }
        }
    };
    let mut c = Contour::new(points, closed);
    for (i, curve) in curves.into_iter().enumerate() {
        c.set_curve(i, curve);
    }
    c
}

/// Regular polygon with `sides` vertices on a circle of radius 1, first vertex at the top.
pub fn regular_polygon(sides: usize) -> Contour {
    let n = sides.max(3);
    Contour::new(
        (0..n)
            .map(|i| unit(i as f64 / n as f64 * TAU - FRAC_PI_2))
            .collect(),
        true,
    )
}

/// Star with `points` tips on radius 1 and inner vertices on `inner` (0–1), tip at the top.
pub fn star(points: usize, inner: f64) -> Contour {
    let n = points.max(3);
    Contour::new(
        (0..2 * n)
            .map(|i| {
                let r = if i % 2 == 0 { 1.0 } else { inner };
                unit(i as f64 / (2 * n) as f64 * TAU - FRAC_PI_2) * r
            })
            .collect(),
        true,
    )
}

fn unit(a: f64) -> Vec2 {
    v2(a.dcos(), a.dsin())
}

/// Symmetric heart of four cubic segments, point at the bottom.
pub fn heart() -> Contour {
    let mut c = Contour::new(
        vec![v2(0.0, -0.45), v2(-1.0, -0.4), v2(0.0, 1.0), v2(1.0, -0.4)],
        true,
    );
    let segs = [
        (v2(-0.05, -1.0), v2(-1.0, -1.05)),
        (v2(-1.0, 0.15), v2(-0.35, 0.55)),
        (v2(0.35, 0.55), v2(1.0, 0.15)),
        (v2(1.0, -1.05), v2(0.05, -1.0)),
    ];
    for (i, (c1, c2)) in segs.into_iter().enumerate() {
        c.set_curve(i, Some(Bezier { c1, c2 }));
    }
    c
}

/// Rectangle `w` × `h` from (0, 0) whose corners are quarter ellipses of radius `r` (clamped to
/// half the shorter side; 0 gives a plain rectangle).
pub fn rounded_rectangle(w: f64, h: f64, r: f64) -> Contour {
    let r = r.clamp(0.0, w.min(h) / 2.0);
    if r <= 0.0 {
        return Contour::rectangle(0.0, 0.0, w, h);
    }
    let k = r * (4.0 / 3.0 * (std::f64::consts::PI / 8.0).dtan());
    // Straight edge then corner curve, clockwise from the top edge.
    let points = vec![
        v2(r, 0.0),
        v2(w - r, 0.0),
        v2(w, r),
        v2(w, h - r),
        v2(w - r, h),
        v2(r, h),
        v2(0.0, h - r),
        v2(0.0, r),
    ];
    let corners = [
        (v2(w - r + k, 0.0), v2(w, r - k)),
        (v2(w, h - r + k), v2(w - r + k, h)),
        (v2(r - k, h), v2(0.0, h - r + k)),
        (v2(0.0, r - k), v2(r - k, 0.0)),
    ];
    let mut c = Contour::new(points, true);
    for (i, (c1, c2)) in corners.into_iter().enumerate() {
        c.set_curve(2 * i + 1, Some(Bezier { c1, c2 }));
    }
    c
}

/// Arrow pointing right: shaft `shaft` (0–1) of the height thick, head `head` (0–1) of the
/// width long, in the unit square.
pub fn arrow(shaft: f64, head: f64) -> Contour {
    let (t, hx) = ((1.0 - shaft) / 2.0, 1.0 - head);
    Contour::new(
        vec![
            v2(0.0, t),
            v2(hx, t),
            v2(hx, 0.0),
            v2(1.0, 0.5),
            v2(hx, 1.0),
            v2(hx, 1.0 - t),
            v2(0.0, 1.0 - t),
        ],
        true,
    )
}

/// Plus sign whose arms are `arm` (0–1) of the size thick, in the unit square.
pub fn cross(arm: f64) -> Contour {
    let (a, b) = ((1.0 - arm) / 2.0, (1.0 + arm) / 2.0);
    Contour::new(
        vec![
            v2(a, 0.0),
            v2(b, 0.0),
            v2(b, a),
            v2(1.0, a),
            v2(1.0, b),
            v2(b, b),
            v2(b, 1.0),
            v2(a, 1.0),
            v2(a, b),
            v2(0.0, b),
            v2(0.0, a),
            v2(a, a),
        ],
        true,
    )
}

/// Crescent: a unit circle minus a circle shifted right by `bite` (0–1) radius, as two arcs.
pub fn crescent(bite: f64) -> Contour {
    // Intersections of x² + y² = 1 and (x − d)² + y² = 1 lie at x = d/2.
    let d = bite.clamp(0.05, 1.95);
    let half = (d / 2.0).dacos().to_degrees();
    let outer = arc(
        v2(0.0, 0.0),
        1.0,
        1.0,
        half,
        360.0 - 2.0 * half,
        ArcMode::Arc,
    );
    let inner = arc(
        v2(d, 0.0),
        1.0,
        1.0,
        180.0 + half,
        -2.0 * half,
        ArcMode::Arc,
    );
    let mut points = outer.points.clone();
    let mut curves: Vec<Option<Bezier>> =
        (0..outer.segment_count()).map(|i| outer.curve(i)).collect();
    // The inner arc starts where the outer one ends; skip its duplicate first vertex.
    points.extend(inner.points.iter().skip(1).take(inner.points.len() - 2));
    curves.extend((0..inner.segment_count()).map(|i| inner.curve(i)));
    let mut c = Contour::new(points, true);
    for (i, curve) in curves.into_iter().enumerate() {
        c.set_curve(i, curve);
    }
    c
}

/// Teardrop pointing up: a round bottom of two quarter curves joined to a tip.
pub fn teardrop() -> Contour {
    let mut c = Contour::new(vec![v2(0.0, -1.0), v2(0.5, 0.5), v2(-0.5, 0.5)], true);
    c.set_curve(
        0,
        Some(Bezier {
            c1: v2(0.15, -0.5),
            c2: v2(0.5, 0.0),
        }),
    );
    c.set_curve(
        1,
        Some(Bezier {
            c1: v2(0.5, 0.5 + 2.0 / 3.0),
            c2: v2(-0.5, 0.5 + 2.0 / 3.0),
        }),
    );
    c.set_curve(
        2,
        Some(Bezier {
            c1: v2(-0.5, 0.0),
            c2: v2(-0.15, -0.5),
        }),
    );
    c
}

/// Scales and moves `c` so its flattened bounds fill `width` × `height` centred on `center`,
/// then rotates it by `rotation` degrees about the centre.
pub fn fit(c: &Contour, center: Vec2, width: f64, height: f64, rotation: f64) -> Contour {
    // Flattening tolerance is absolute: measure the bounds at a large scale so they are exact
    // to well under a micrometre whatever the input size.
    let c = &c.transformed(&Transform::scaling(1e6, 1e6, v2(0.0, 0.0)));
    let Some(b) = Rect::enclosing(c.outline_points()) else {
        return c.clone();
    };
    let sx = if b.width() > 0.0 {
        width / b.width()
    } else {
        1.0
    };
    let sy = if b.height() > 0.0 {
        height / b.height()
    } else {
        1.0
    };
    let t = Transform::translation(-b.center().x, -b.center().y)
        .then(&Transform::scaling(sx, sy, v2(0.0, 0.0)))
        .then(&Transform::translation(center.x, center.y))
        .then(&Transform::rotation(rotation, center));
    c.transformed(&t)
}

/// PE-DESIGN 11's default start and end point of a Shape-tool region (rectangle, round
/// rectangle, ellipse, heart, diamond, stars, drop): where the vertical line through the middle
/// of the bounds meets the outline lowest, e.g. the middle of a rectangle's bottom edge, a
/// heart's tip or the notch between a five-point star's lower points.
pub fn bottom_centre(c: &Contour) -> Option<Vec2> {
    let b = Rect::enclosing(c.outline_points())?;
    let x = b.center().x;
    c.edges()
        .into_iter()
        .filter_map(|(a, e)| {
            let (lo, hi) = if a.x <= e.x { (a, e) } else { (e, a) };
            if x < lo.x || x > hi.x {
                return None;
            }
            if hi.x == lo.x {
                return Some(v2(x, a.y.max(e.y)));
            }
            Some(lo.lerp(hi, (x - lo.x) / (hi.x - lo.x)))
        })
        .max_by(|p, q| p.y.total_cmp(&q.y))
}

/// The left end of the lowest edge: PE-DESIGN 11's default start and end point of its triangle.
pub fn bottom_left(c: &Contour) -> Option<Vec2> {
    let b = Rect::enclosing(c.outline_points())?;
    let tol = b.height().max(b.width()) * 1e-6;
    c.outline_points()
        .into_iter()
        .filter(|p| p.y >= b.max_y - tol)
        .min_by(|p, q| p.x.total_cmp(&q.x))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::contains;

    fn inside(p: Vec2, c: &Contour) -> bool {
        contains(p, std::slice::from_ref(c))
    }

    fn bounds(c: &Contour) -> Rect {
        Rect::enclosing(c.outline_points()).expect("bounds")
    }

    #[test]
    fn quarter_arc_is_one_curve_close_to_the_circle() {
        let c = arc(v2(0.0, 0.0), 100.0, 100.0, 0.0, 90.0, ArcMode::Arc);
        assert!(!c.closed);
        assert_eq!(c.points.len(), 2);
        assert!(c.points[1].distance(v2(0.0, 100.0)) < 1e-9);
        for p in c.path() {
            assert!((p.length() - 100.0).abs() < 0.1, "{p:?}");
        }
    }

    #[test]
    fn arc_modes_close_differently() {
        let fan = arc(v2(0.0, 0.0), 50.0, 50.0, 0.0, 180.0, ArcMode::Fan);
        assert!(fan.closed);
        assert_eq!(fan.points.last(), Some(&v2(0.0, 0.0)));
        let chord = arc(v2(0.0, 0.0), 50.0, 50.0, 0.0, 180.0, ArcMode::Chord);
        assert!(chord.closed);
        assert_eq!(chord.points.len(), 3);
        assert!(chord.curve(chord.segment_count() - 1).is_none());
        let full = arc(v2(0.0, 0.0), 50.0, 30.0, 10.0, -360.0, ArcMode::Fan);
        assert!(full.closed);
        assert_eq!(full.points.len(), 4);
        let b = bounds(&full);
        assert!((b.width() - 100.0).abs() < 0.2 && (b.height() - 60.0).abs() < 0.2);
    }

    #[test]
    fn polygon_and_star_vertices() {
        let p = regular_polygon(6);
        assert_eq!(p.points.len(), 6);
        assert!(p.points.iter().all(|q| (q.length() - 1.0).abs() < 1e-12));
        assert!(p.points[0].distance(v2(0.0, -1.0)) < 1e-12);
        let s = star(5, 0.4);
        assert_eq!(s.points.len(), 10);
        assert!((s.points[1].length() - 0.4).abs() < 1e-12);
        assert!(inside(v2(0.0, 0.0), &s));
        assert_eq!(regular_polygon(1).points.len(), 3);
    }

    #[test]
    fn fit_fills_the_box() {
        for c in [
            heart(),
            star(7, 0.5),
            teardrop(),
            crescent(0.8),
            arrow(0.4, 0.4),
            cross(0.3),
        ] {
            let f = fit(&c, v2(500.0, 400.0), 300.0, 200.0, 0.0);
            let b = bounds(&f);
            assert!((b.width() - 300.0).abs() < 0.6, "{b:?}");
            assert!((b.height() - 200.0).abs() < 0.6, "{b:?}");
            assert!(b.center().distance(v2(500.0, 400.0)) < 0.6);
        }
    }

    #[test]
    fn heart_is_one_region_with_the_point_at_the_bottom() {
        let h = fit(&heart(), v2(0.0, 0.0), 200.0, 200.0, 0.0);
        assert!(inside(v2(0.0, 0.0), &h));
        assert!(inside(v2(-50.0, -60.0), &h));
        // The dip between the lobes is outside.
        assert!(!inside(v2(0.0, -95.0), &h));
        let bottom = h
            .outline_points()
            .into_iter()
            .fold(v2(0.0, f64::MIN), |a, p| if p.y > a.y { p } else { a });
        assert!(bottom.x.abs() < 1e-6);
    }

    #[test]
    fn rounded_rectangle_corners() {
        let r = rounded_rectangle(100.0, 60.0, 10.0);
        assert_eq!(r.points.len(), 8);
        let b = bounds(&r);
        assert!((b.width() - 100.0).abs() < 1e-9 && (b.height() - 60.0).abs() < 1e-9);
        assert!(!inside(v2(0.5, 0.5), &r));
        assert!(inside(v2(5.0, 30.0), &r));
        assert_eq!(rounded_rectangle(100.0, 60.0, 0.0).points.len(), 4);
        // Radius larger than half the side is clamped: the short sides become semicircles.
        assert!(rounded_rectangle(100.0, 60.0, 500.0).points[2].distance(v2(100.0, 30.0)) < 1e-9);
    }

    #[test]
    fn crescent_excludes_the_bite() {
        let c = crescent(0.8).transformed(&Transform::scaling(100.0, 100.0, v2(0.0, 0.0)));
        assert!(inside(v2(-90.0, 0.0), &c));
        assert!(!inside(v2(50.0, 0.0), &c));
        assert!(!inside(v2(0.0, 0.0), &c));
    }

    #[test]
    fn default_start_points() {
        // PE-DESIGN 11.50 (compat spec shape-start-points): the middle of a rectangle's bottom
        // edge, the notch between a five-point star's lower points, a triangle's lower left corner.
        let r = Contour::rectangle(20.0, 20.0, 200.0, 100.0);
        assert_eq!(bottom_centre(&r), Some(v2(120.0, 120.0)));
        let star = fit(&star(5, 0.4), v2(0.0, 0.0), 200.0, 200.0, 0.0);
        let notch = bottom_centre(&star).expect("notch");
        assert!(
            notch.x.abs() < 1e-6 && notch.y > 0.0 && notch.y < 100.0 - 1.0,
            "{notch:?}"
        );
        let t = fit(&regular_polygon(3), v2(0.0, 0.0), 200.0, 200.0, 0.0);
        let p = bottom_left(&t).expect("corner");
        assert!(
            (p.x + 100.0).abs() < 1e-6 && (p.y - 100.0).abs() < 1e-6,
            "{p:?}"
        );
    }
}
