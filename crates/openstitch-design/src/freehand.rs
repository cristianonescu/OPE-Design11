//! Pencil tools (PE-DESIGN's freehand open and closed curves): the pointer trail is reduced to
//! the few vertices that keep it within a tolerance (Douglas–Peucker), and the segments
//! between smooth vertices become cubic Béziers (Catmull-Rom tangents); sharp turns stay
//! corners. Own algorithm. Lengths are 0.1 mm.

use crate::fmath::Det;
use crate::geometry::{Bezier, Contour, Vec2};

/// Most trail points one stroke may have.
pub const MAX_POINTS: usize = 20_000;
/// Turns sharper than this (degrees between the incoming and outgoing direction) stay corners.
pub const CORNER_DEGREES: f64 = 75.0;

fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let len2 = d.x * d.x + d.y * d.y;
    if len2 == 0.0 {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * d.x + (p.y - a.y) * d.y) / len2).clamp(0.0, 1.0);
    p.distance(a + d * t)
}

/// Indices Douglas–Peucker keeps on `points[first..=last]` (iterative).
fn keep(points: &[Vec2], first: usize, last: usize, tolerance: f64, kept: &mut [bool]) {
    let mut stack = vec![(first, last)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (mut worst, mut at) = (0.0, a);
        for i in a + 1..b {
            let d = segment_distance(points[i], points[a], points[b]);
            if d > worst {
                (worst, at) = (d, i);
            }
        }
        if worst > tolerance {
            kept[at] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
}

/// The trail without repeated points (closer than a hundredth of the tolerance).
fn dedup(points: &[Vec2], tolerance: f64) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last().is_none_or(|q| q.distance(p) > tolerance * 0.01) {
            out.push(p);
        }
    }
    out
}

/// Vertices of the simplified trail. A closed trail drops its closing repeat.
pub fn simplify(points: &[Vec2], closed: bool, tolerance: f64) -> Vec<Vec2> {
    let mut pts = dedup(points, tolerance);
    if closed && pts.len() > 2 && pts[0].distance(pts[pts.len() - 1]) <= tolerance {
        pts.pop();
    }
    let n = pts.len();
    if n <= 2 {
        return pts;
    }
    let mut kept = vec![false; n];
    kept[0] = true;
    if closed {
        // Split the loop at the point farthest from the start so both halves are open runs.
        let far = (1..n)
            .max_by(|&i, &j| pts[i].distance(pts[0]).total_cmp(&pts[j].distance(pts[0])))
            .unwrap_or(n / 2);
        kept[far] = true;
        let mut ring = pts.clone();
        ring.push(pts[0]);
        let mut ring_kept = kept.clone();
        ring_kept.push(true);

        keep(&ring, 0, far, tolerance, &mut ring_kept);
        keep(&ring, far, n, tolerance, &mut ring_kept);
        kept.copy_from_slice(&ring_kept[..n]);
    } else {
        kept[n - 1] = true;
        keep(&pts, 0, n - 1, tolerance, &mut kept);
    }
    pts.iter()
        .zip(kept)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

fn unit(d: Vec2) -> Option<Vec2> {
    let l = d.length();
    (l > 1e-9).then(|| d * (1.0 / l))
}

/// The contour for a pencil stroke: `None` when the trail is too short (an open line needs
/// two distinct points, a closed one three).
pub fn fit(points: &[Vec2], closed: bool, tolerance: f64, smooth: bool) -> Option<Contour> {
    let v = simplify(points, closed, tolerance.max(0.1));
    let n = v.len();
    if n < if closed { 3 } else { 2 } {
        return None;
    }
    let mut c = Contour::new(v.clone(), closed);
    if !smooth || n < 3 {
        return Some(c);
    }
    let at = |i: isize| -> Option<Vec2> {
        if closed {
            Some(v[i.rem_euclid(n as isize) as usize])
        } else {
            usize::try_from(i).ok().and_then(|i| v.get(i).copied())
        }
    };
    let cos_corner = CORNER_DEGREES.to_radians().dcos();
    // Tangent (scaled by the neighbour chord) at a smooth vertex; None at corners and ends.
    let tangent = |i: usize| -> Option<Vec2> {
        let (p, q, r) = (at(i as isize - 1)?, at(i as isize)?, at(i as isize + 1)?);
        let (a, b) = (unit(q - p)?, unit(r - q)?);
        (a.x * b.x + a.y * b.y >= cos_corner).then(|| (r - p) * (1.0 / 6.0))
    };
    for s in 0..c.segment_count() {
        let e = (s + 1) % n;
        let (a, b) = (v[s], v[e]);
        let (t0, t1) = (tangent(s), tangent(e));
        if t0.is_none() && t1.is_none() {
            continue;
        }
        // A missing tangent points along the chord, a third of the way.
        let chord = (b - a) * (1.0 / 3.0);
        let limit = a.distance(b) * 0.5;
        let clamp = |t: Vec2| {
            let l = t.length();
            if l > limit { t * (limit / l) } else { t }
        };
        c.set_curve(
            s,
            Some(Bezier {
                c1: a + clamp(t0.unwrap_or(chord)),
                c2: b - clamp(t1.unwrap_or(chord)),
            }),
        );
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::v2;

    fn circle(n: usize, r: f64) -> Vec<Vec2> {
        (0..n)
            .map(|k| {
                let t = k as f64 / n as f64 * std::f64::consts::TAU;
                v2(r * t.cos(), r * t.sin())
            })
            .collect()
    }

    #[test]
    fn straight_trail_becomes_one_segment() {
        let trail: Vec<Vec2> = (0..=100)
            .map(|k| v2(f64::from(k), 0.3 * f64::from(k % 2)))
            .collect();
        let c = fit(&trail, false, 2.0, true).expect("line");
        assert_eq!(c.points, vec![v2(0.0, 0.0), v2(100.0, 0.0)]);
        assert!(!c.has_curves());
        assert!(fit(&[v2(0.0, 0.0), v2(0.0, 0.0)], false, 2.0, true).is_none());
    }

    #[test]
    fn circle_trail_becomes_a_few_smooth_curves_close_to_it() {
        let trail = circle(400, 300.0);
        let c = fit(&trail, true, 3.0, true).expect("closed");
        assert!(c.closed);
        assert!(
            c.points.len() >= 4 && c.points.len() <= 40,
            "{}",
            c.points.len()
        );
        assert!(
            c.curves.iter().all(Option::is_some),
            "a circle has no corners"
        );
        // The flattened outline stays near the circle.
        for p in c.path() {
            assert!((p.length() - 300.0).abs() < 6.0, "{p:?}");
        }
    }

    #[test]
    fn sharp_turns_stay_corners() {
        // An L: along x, then straight down.
        let mut trail: Vec<Vec2> = (0..=50).map(|k| v2(f64::from(k) * 4.0, 0.0)).collect();
        trail.extend((1..=50).map(|k| v2(200.0, f64::from(k) * 4.0)));
        let c = fit(&trail, false, 1.0, true).expect("open");
        assert_eq!(
            c.points,
            vec![v2(0.0, 0.0), v2(200.0, 0.0), v2(200.0, 200.0)]
        );
        assert!(!c.has_curves(), "the corner is kept sharp");
        let rough = fit(&circle(200, 100.0), true, 2.0, false).expect("polygon");
        assert!(!rough.has_curves());
    }
}
