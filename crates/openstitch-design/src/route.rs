//! Entry and exit points (PE-DESIGN's region entry R1 / exit R2 and line entry L1): where the
//! needle starts and ends an object's sewing. Points are snapped to the outline; regions travel
//! along the outline from the entry to the first fill stitch and from the last to the exit.
//! Lengths are 0.1 mm.

use crate::generate::{Run, subdivide};
use crate::geometry::{Contour, Vec2};

/// Where on a contour a point is closest: segment, parameter on it, and the point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnContour {
    pub segment: usize,
    pub t: f64,
    pub point: Vec2,
}

fn cubic(a: Vec2, b: Vec2, c: Vec2, d: Vec2, t: f64) -> Vec2 {
    let u = 1.0 - t;
    a * (u * u * u) + b * (3.0 * u * u * t) + c * (3.0 * u * t * t) + d * (t * t * t)
}

fn nearest_on_line(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 == 0.0 {
        return 0.0;
    }
    (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
}

impl Contour {
    /// Point of segment `i` at parameter `t` (exact on curves).
    pub fn point_at(&self, i: usize, t: f64) -> Vec2 {
        let (a, b) = self.segment_ends(i);
        match self.curve(i) {
            None => a.lerp(b, t),
            Some(c) => cubic(a, c.c1, c.c2, b, t),
        }
    }

    /// The point of the outline nearest to `p` (curves searched by sampling, then refined).
    pub fn nearest(&self, p: Vec2) -> Option<OnContour> {
        let mut best: Option<OnContour> = None;
        let mut consider = |segment: usize, t: f64, point: Vec2| {
            if best.is_none_or(|b| point.distance(p) < b.point.distance(p)) {
                best = Some(OnContour { segment, t, point });
            }
        };
        if self.points.len() == 1 {
            consider(0, 0.0, self.points[0]);
        }
        for i in 0..self.segment_count() {
            match self.curve(i) {
                None => {
                    let (a, b) = self.segment_ends(i);
                    let t = nearest_on_line(p, a, b);
                    consider(i, t, a.lerp(b, t));
                }
                Some(_) => {
                    const N: usize = 64;
                    let (mut t, mut d) = (0.0, f64::INFINITY);
                    for k in 0..=N {
                        let s = k as f64 / N as f64;
                        let q = self.point_at(i, s).distance(p);
                        if q < d {
                            (t, d) = (s, q);
                        }
                    }
                    let mut step = 1.0 / N as f64;
                    for _ in 0..30 {
                        step /= 2.0;
                        for s in [(t - step).max(0.0), (t + step).min(1.0)] {
                            let q = self.point_at(i, s).distance(p);
                            if q < d {
                                (t, d) = (s, q);
                            }
                        }
                    }
                    consider(i, t, self.point_at(i, t));
                }
            }
        }
        best
    }

    /// The same outline starting (for a closed contour) at the point nearest `p`, inserting a
    /// vertex there when needed (curves split exactly). An open contour is reversed when `p` is
    /// nearer its end. Shape and region are unchanged.
    pub fn starting_at(&self, p: Vec2) -> Contour {
        let n = self.points.len();
        if n < 2 {
            return self.clone();
        }
        if !self.closed {
            let (first, last) = (self.points[0], self.points[n - 1]);
            if last.distance(p) >= first.distance(p) {
                return self.clone();
            }
            let segs = self.segment_count();
            let mut r = Contour::new(self.points.iter().rev().copied().collect(), false);
            for i in 0..segs {
                if let Some(c) = self.curve(segs - 1 - i) {
                    r.set_curve(i, Some(crate::geometry::Bezier { c1: c.c2, c2: c.c1 }));
                }
            }
            return r;
        }
        let Some(at) = self.nearest(p) else {
            return self.clone();
        };
        let mut c = self.clone();
        const EPS: f64 = 1e-6;
        let start = if at.t <= EPS {
            at.segment
        } else if at.t >= 1.0 - EPS {
            (at.segment + 1) % n
        } else {
            c.insert_point(at.segment, at.t)
        };
        let segs = c.segment_count();
        let curves: Vec<_> = (0..segs).map(|i| c.curve((i + start) % segs)).collect();
        let points: Vec<Vec2> = (0..c.points.len())
            .map(|i| c.points[(i + start) % c.points.len()])
            .collect();
        let mut r = Contour::new(points, true);
        for (i, curve) in curves.into_iter().enumerate() {
            r.set_curve(i, curve);
        }
        r
    }
}

/// Polyline position: segment index of `poly` and distance along the whole polyline.
fn locate(poly: &[Vec2], p: Vec2) -> (f64, Vec2) {
    let mut best = (0.0, poly[0], f64::INFINITY);
    let mut walked = 0.0;
    for w in poly.windows(2) {
        let t = nearest_on_line(p, w[0], w[1]);
        let q = w[0].lerp(w[1], t);
        let d = q.distance(p);
        let len = w[0].distance(w[1]);
        if d < best.2 {
            best = (walked + t * len, q, d);
        }
        walked += len;
    }
    (best.0, best.1)
}

/// Points along the closed polyline `poly` (first == last) from arc length `from` to `to`, the
/// shorter way round, excluding the ends.
fn walk(poly: &[Vec2], from: f64, to: f64) -> Vec<Vec2> {
    let lens: Vec<f64> = poly.windows(2).map(|w| w[0].distance(w[1])).collect();
    let total: f64 = lens.iter().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let forward = (to - from).rem_euclid(total);
    let ahead = forward <= total - forward;
    // Vertex arc positions.
    let mut at = vec![0.0];
    for l in &lens {
        at.push(at.last().copied().unwrap_or(0.0) + l);
    }
    let mut out = Vec::new();
    let span = if ahead { forward } else { total - forward };
    let n = poly.len() - 1;
    let mut order: Vec<(f64, Vec2)> = (0..n)
        .map(|i| {
            let d = if ahead {
                (at[i] - from).rem_euclid(total)
            } else {
                (from - at[i]).rem_euclid(total)
            };
            (d, poly[i])
        })
        .filter(|(d, _)| *d > 1e-9 && *d < span - 1e-9)
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(order.into_iter().map(|(_, p)| p));
    out
}

/// Running stitches along the outline (the closed contour of `outline` nearest `a`) from `a`
/// to `b`, ends included, at most `pitch` long. Straight when the points are on different
/// contours.
fn travel(outline: &[Contour], a: Vec2, b: Vec2, pitch: f64) -> Run {
    let nearest = |p: Vec2| {
        outline
            .iter()
            .enumerate()
            .filter(|(_, c)| c.closed && c.points.len() >= 2)
            .filter_map(|(i, c)| c.nearest(p).map(|n| (i, n.point.distance(p))))
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .map(|(i, _)| i)
    };
    let (ca, cb) = (nearest(a), nearest(b));
    let mut path = vec![a];
    if let (Some(i), Some(j)) = (ca, cb)
        && i == j
    {
        let poly = outline[i].path();
        let (from, pa) = locate(&poly, a);
        let (to, pb) = locate(&poly, b);
        path.push(pa);
        path.extend(walk(&poly, from, to));
        path.push(pb);
    }
    path.push(b);
    path.dedup_by(|x, y| x.distance(*y) < 1e-9);
    subdivide(&path, pitch)
}

/// Region `runs` reordered to start near `entry` and end near `exit` (each snapped to the
/// nearest point of `outline`): the run sequence is reversed when that brings its ends closer,
/// then joined to the entry and exit by travel along the outline.
pub fn route_region(
    mut runs: Vec<Run>,
    outline: &[Contour],
    entry: Option<Vec2>,
    exit: Option<Vec2>,
    pitch: f64,
) -> Vec<Run> {
    runs.retain(|r| !r.is_empty());
    if runs.is_empty() || (entry.is_none() && exit.is_none()) {
        return runs;
    }
    let snap = |p: Vec2| snap(outline, p);
    let (entry, exit) = (entry.map(snap), exit.map(snap));
    let start = runs[0][0];
    let end = *runs.last().and_then(|r| r.last()).expect("non-empty");
    let cost = |s: Vec2, e: Vec2| {
        entry.map_or(0.0, |p| p.distance(s)) + exit.map_or(0.0, |p| p.distance(e))
    };
    if cost(end, start) < cost(start, end) {
        runs.reverse();
        for r in &mut runs {
            r.reverse();
        }
    }
    if let Some(p) = entry {
        let mut lead = travel(outline, p, runs[0][0], pitch);
        lead.pop();
        lead.extend(runs[0].iter().copied());
        runs[0] = lead;
    }
    if let Some(p) = exit {
        let last = runs.last_mut().expect("non-empty");
        let from = *last.last().expect("non-empty");
        last.extend(travel(outline, from, p, pitch).into_iter().skip(1));
    }
    runs
}

/// `p` moved onto the nearest closed outline (unchanged if there is none).
pub fn snap(outline: &[Contour], p: Vec2) -> Vec2 {
    outline
        .iter()
        .filter(|c| c.closed)
        .filter_map(|c| c.nearest(p))
        .min_by(|a, b| a.point.distance(p).total_cmp(&b.point.distance(p)))
        .map_or(p, |n| n.point)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Bezier, v2};

    #[test]
    fn nearest_on_lines_and_curves() {
        let r = Contour::rectangle(0.0, 0.0, 100.0, 50.0);
        let n = r.nearest(v2(30.0, -10.0)).expect("n");
        assert_eq!((n.segment, n.point), (0, v2(30.0, 0.0)));
        let n = r.nearest(v2(-5.0, 25.0)).expect("n");
        assert_eq!((n.segment, n.point), (3, v2(0.0, 25.0)));
        let mut c = Contour::new(vec![v2(0.0, 0.0), v2(100.0, 0.0)], false);
        c.set_curve(
            0,
            Some(Bezier {
                c1: v2(0.0, 100.0),
                c2: v2(100.0, 100.0),
            }),
        );
        let n = c.nearest(v2(50.0, 200.0)).expect("n");
        assert!((n.t - 0.5).abs() < 1e-6);
        assert!((n.point.y - 75.0).abs() < 1e-6);
    }

    #[test]
    fn starting_at_rotates_closed_and_reverses_open() {
        let r = Contour::rectangle(0.0, 0.0, 100.0, 50.0);
        let s = r.starting_at(v2(100.0, 20.0));
        assert_eq!(s.points[0], v2(100.0, 20.0));
        assert_eq!(s.points.len(), 5);
        assert_eq!(s.points[1], v2(100.0, 50.0));
        // At a vertex: no new point.
        let s = r.starting_at(v2(101.0, 51.0));
        assert_eq!(
            s.points,
            vec![v2(100.0, 50.0), v2(0.0, 50.0), v2(0.0, 0.0), v2(100.0, 0.0)]
        );
        let open = Contour::new(vec![v2(0.0, 0.0), v2(10.0, 0.0), v2(10.0, 10.0)], false);
        assert_eq!(open.starting_at(v2(9.0, 11.0)).points[0], v2(10.0, 10.0));
        assert_eq!(open.starting_at(v2(-1.0, 0.0)), open);
    }

    #[test]
    fn starting_at_keeps_curved_outlines_exact() {
        let circle = crate::shapes::arc(
            v2(0.0, 0.0),
            100.0,
            100.0,
            0.0,
            360.0,
            crate::shapes::ArcMode::Arc,
        );
        let s = circle.starting_at(v2(-200.0, -200.0));
        let expected = v2(-100.0, -100.0) * std::f64::consts::FRAC_1_SQRT_2;
        assert!(s.points[0].distance(expected) < 0.05, "{:?}", s.points[0]);
        assert_eq!(s.points.len(), 5);
        for p in s.path() {
            assert!((p.length() - 100.0).abs() < 0.1);
        }
    }

    #[test]
    fn route_starts_at_entry_and_ends_at_exit() {
        let outline = [Contour::rectangle(0.0, 0.0, 100.0, 100.0)];
        let runs = vec![vec![
            v2(0.0, 5.0),
            v2(100.0, 5.0),
            v2(100.0, 95.0),
            v2(0.0, 95.0),
        ]];
        // Entry near the bottom-left: the sequence is reversed and travels along the edge.
        let r = route_region(
            runs.clone(),
            &outline,
            Some(v2(-3.0, 80.0)),
            Some(v2(50.0, -2.0)),
            25.0,
        );
        assert_eq!(r[0][0], v2(0.0, 80.0));
        assert_eq!(*r[0].last().expect("p"), v2(50.0, 0.0));
        // Travel stays on the outline: from (0, 80) down the edge to (0, 95).
        assert_eq!(r[0][1], v2(0.0, 95.0));
        // ...and from the last row end (0, 5) round the corner to the exit.
        assert_eq!(
            r[0][r[0].len() - 3..],
            [v2(0.0, 0.0), v2(25.0, 0.0), v2(50.0, 0.0)]
        );
        // No entry/exit: unchanged.
        assert_eq!(route_region(runs.clone(), &outline, None, None, 25.0), runs);
    }

    #[test]
    fn travel_goes_the_short_way_round() {
        let outline = [Contour::rectangle(0.0, 0.0, 100.0, 100.0)];
        let t = travel(&outline, v2(10.0, 0.0), v2(0.0, 10.0), 100.0);
        assert_eq!(t, vec![v2(10.0, 0.0), v2(0.0, 0.0), v2(0.0, 10.0)]);
    }
}
