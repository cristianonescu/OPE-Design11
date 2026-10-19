//! Curve provenance for the boolean operations: curved segments are flattened while remembering
//! which cubic (and which parameter) every sample came from. After the polygon engine has run,
//! output edges that still lie along an input curve are turned back into exact Bézier pieces of
//! that curve; every other edge stays straight. Offsets restore against offset copies of the
//! input curves. Own algorithm. Lengths are 0.1 mm.

use crate::geometry::{Bezier, Contour, FLATNESS, Vec2, contains, v2};
use std::collections::HashMap;

/// Output vertices closer than this to an input curve's flattening lie on it (boolean results
/// keep input vertices exactly, up to the engine's integer snapping).
pub(crate) const EXACT: f64 = 0.01;
/// The same for offsets, whose vertices only approximate the offset curve.
pub(crate) const APPROXIMATE: f64 = 2.0 * FLATNESS;

/// Largest jump in sample index between consecutive output vertices on one curve (the engine
/// may drop a sample that became collinear).
const MAX_STEP: f64 = 2.5;
/// Backward slack, in samples, tolerated within a run (offset joins jitter).
const BACKTRACK: f64 = 0.25;
/// Runs shorter than this, in samples, stay straight.
const MIN_RUN: f64 = 0.05;

#[derive(Clone, Copy)]
struct Cubic {
    a: Vec2,
    c1: Vec2,
    c2: Vec2,
    b: Vec2,
}

impl Cubic {
    fn at(&self, t: f64) -> Vec2 {
        let u = 1.0 - t;
        self.a * (u * u * u)
            + self.c1 * (3.0 * u * u * t)
            + self.c2 * (3.0 * u * t * t)
            + self.b * (t * t * t)
    }

    /// Unit tangent at `t`, falling back to the chord where the derivative vanishes.
    fn tangent(&self, t: f64) -> Option<Vec2> {
        let u = 1.0 - t;
        let d = (self.c1 - self.a) * (3.0 * u * u)
            + (self.c2 - self.c1) * (6.0 * u * t)
            + (self.b - self.c2) * (3.0 * t * t);
        let d = if d.length() > 1e-9 {
            d
        } else if t < 0.5 {
            self.c2 - self.a
        } else {
            self.b - self.c1
        };
        let d = if d.length() > 1e-9 {
            d
        } else {
            self.b - self.a
        };
        let len = d.length();
        (len > 1e-9 && len.is_finite()).then(|| d * (1.0 / len))
    }

    /// The piece between `t0` and `t1` (either order; reversed when `t1 < t0`).
    fn piece(&self, t0: f64, t1: f64) -> Cubic {
        if t1 < t0 {
            let c = self.piece(t1, t0);
            return Cubic {
                a: c.b,
                c1: c.c2,
                c2: c.c1,
                b: c.a,
            };
        }
        let left = self.split(t1).0;
        if t1 <= 1e-12 {
            return left;
        }
        left.split(t0 / t1).1
    }

    fn split(&self, t: f64) -> (Cubic, Cubic) {
        let (ab, bc, cd) = (
            self.a.lerp(self.c1, t),
            self.c1.lerp(self.c2, t),
            self.c2.lerp(self.b, t),
        );
        let (abc, bcd) = (ab.lerp(bc, t), bc.lerp(cd, t));
        let m = abc.lerp(bcd, t);
        (
            Cubic {
                a: self.a,
                c1: ab,
                c2: abc,
                b: m,
            },
            Cubic {
                a: m,
                c1: bcd,
                c2: cd,
                b: self.b,
            },
        )
    }

    fn handles(&self) -> Bezier {
        Bezier {
            c1: self.c1,
            c2: self.c2,
        }
    }
}

fn distance_to_line(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let len = ab.length();
    if len == 0.0 {
        return p.distance(a);
    }
    ((p.x - a.x) * ab.y - (p.y - a.y) * ab.x).abs() / len
}

/// Same subdivision as [`Contour::path`], recording the parameter of each pushed point.
fn flatten(c: &Cubic, t0: f64, t1: f64, depth: u32, out: &mut Vec<(Vec2, f64)>) {
    let flat = distance_to_line(c.c1, c.a, c.b).max(distance_to_line(c.c2, c.a, c.b));
    if depth >= 16 || flat <= FLATNESS {
        out.push((c.b, t1));
        return;
    }
    let (l, r) = c.split(0.5);
    let tm = (t0 + t1) / 2.0;
    flatten(&l, t0, tm, depth + 1, out);
    flatten(&r, tm, t1, depth + 1, out);
}

/// One input curve and its flattening: `samples[k]` lies at parameter `ts[k]`.
struct Source {
    cubic: Cubic,
    samples: Vec<Vec2>,
    ts: Vec<f64>,
}

impl Source {
    fn new(cubic: Cubic) -> Self {
        let mut out = vec![(cubic.a, 0.0)];
        flatten(&cubic, 0.0, 1.0, 0, &mut out);
        let (samples, ts) = out.into_iter().unzip();
        Source { cubic, samples, ts }
    }

    fn last(&self) -> f64 {
        (self.samples.len() - 1) as f64
    }

    /// Curve parameter at sample position `s` (sample index plus fraction along the chord).
    fn t(&self, s: f64) -> f64 {
        let s = s.clamp(0.0, self.last());
        let k = (s.floor() as usize).min(self.samples.len() - 2);
        let f = s - k as f64;
        self.ts[k] + (self.ts[k + 1] - self.ts[k]) * f
    }
}

/// Input curves of a boolean operation, indexed for "which curve is this point on" queries.
pub(crate) struct Provenance {
    sources: Vec<Source>,
    tolerance: f64,
    cell: f64,
    grid: HashMap<(i64, i64), Vec<(u32, u32)>>,
}

pub(crate) type Path = Vec<[f64; 2]>;

impl Provenance {
    /// Flattens the closed contours for the polygon engine, recording their curves.
    pub(crate) fn flatten(contours: &[Contour]) -> (Vec<Path>, Provenance) {
        let mut sources = Vec::new();
        let mut paths = Vec::new();
        for c in contours.iter().filter(|c| c.closed && !c.points.is_empty()) {
            let mut path: Vec<Vec2> = vec![c.points[0]];
            for i in 0..c.segment_count() {
                let (a, b) = c.segment_ends(i);
                match c.curve(i) {
                    None => path.push(b),
                    Some(h) => {
                        let s = Source::new(Cubic {
                            a,
                            c1: h.c1,
                            c2: h.c2,
                            b,
                        });
                        path.extend(s.samples[1..].iter().copied());
                        sources.push(s);
                    }
                }
            }
            if path.len() > 1 {
                path.pop();
            }
            if path.len() >= 3 {
                paths.push(path.iter().map(|p| [p.x, p.y]).collect());
            }
        }
        (paths, Provenance::index(sources, EXACT))
    }

    fn index(sources: Vec<Source>, tolerance: f64) -> Provenance {
        let all = || sources.iter().flat_map(|s| s.samples.iter());
        let (mut lo, mut hi) = (v2(f64::MAX, f64::MAX), v2(f64::MIN, f64::MIN));
        for p in all() {
            lo = v2(lo.x.min(p.x), lo.y.min(p.y));
            hi = v2(hi.x.max(p.x), hi.y.max(p.y));
        }
        let extent = (hi.x - lo.x).max(hi.y - lo.y);
        let cell = if extent.is_finite() {
            (extent / 128.0).max(1.0)
        } else {
            1.0
        };
        let mut p = Provenance {
            sources,
            tolerance,
            cell,
            grid: HashMap::new(),
        };
        for (si, s) in p.sources.iter().enumerate() {
            for (k, w) in s.samples.windows(2).enumerate() {
                let (x0, y0) =
                    p.key(v2(w[0].x.min(w[1].x), w[0].y.min(w[1].y)) - v2(tolerance, tolerance));
                let (x1, y1) =
                    p.key(v2(w[0].x.max(w[1].x), w[0].y.max(w[1].y)) + v2(tolerance, tolerance));
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        p.grid
                            .entry((x, y))
                            .or_default()
                            .push((si as u32, k as u32));
                    }
                }
            }
        }
        p
    }

    /// The curves of both.
    pub(crate) fn join(mut self, other: Provenance) -> Provenance {
        self.sources.extend(other.sources);
        Provenance::index(self.sources, self.tolerance.max(other.tolerance))
    }

    fn key(&self, p: Vec2) -> (i64, i64) {
        (
            (p.x / self.cell).floor() as i64,
            (p.y / self.cell).floor() as i64,
        )
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// The input curves moved `distance` away from the region `contours` bound (negative =
    /// into it), each approximated by cubics within [`FLATNESS`]; where that fails (cusps of
    /// inward offsets) the piece is left out and its edges stay straight.
    pub(crate) fn offset(&self, contours: &[Contour], distance: f64) -> Provenance {
        let mut out = Vec::new();
        for s in &self.sources {
            let c = &s.cubic;
            let (Some(tan), p) = (c.tangent(0.5), c.at(0.5)) else {
                continue;
            };
            let left = v2(-tan.y, tan.x);
            let probe = 2.0;
            let (l, r) = (
                contains(p + left * probe, contours),
                contains(p - left * probe, contours),
            );
            let outward = match (l, r) {
                (true, false) => -1.0,
                (false, true) => 1.0,
                _ => continue,
            };
            offset_pieces(c, 0.0, 1.0, distance * outward, 0, &mut out);
        }
        Provenance::index(out.into_iter().map(Source::new).collect(), APPROXIMATE)
    }

    /// For each input curve near `p`: (curve, sample position).
    fn on(&self, p: Vec2) -> Vec<(usize, f64)> {
        let mut best: Vec<(usize, f64, f64)> = Vec::new();
        let Some(chords) = self.grid.get(&self.key(p)) else {
            return Vec::new();
        };
        for &(si, k) in chords {
            let s = &self.sources[si as usize];
            let (a, b) = (s.samples[k as usize], s.samples[k as usize + 1]);
            let ab = b - a;
            let len2 = ab.x * ab.x + ab.y * ab.y;
            let f = if len2 > 0.0 {
                (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = p.distance(a.lerp(b, f));
            if d > self.tolerance {
                continue;
            }
            let pos = k as f64 + f;
            match best.iter_mut().find(|e| e.0 == si as usize) {
                Some(e) if d < e.2 => *e = (si as usize, pos, d),
                Some(_) => {}
                None => best.push((si as usize, pos, d)),
            }
        }
        best.into_iter().map(|(c, s, _)| (c, s)).collect()
    }

    fn position(&self, p: Vec2, curve: usize) -> Option<f64> {
        self.on(p).into_iter().find(|e| e.0 == curve).map(|e| e.1)
    }

    /// The closed output polygon `points` with edges along input curves turned back into
    /// curves.
    pub(crate) fn restore(&self, points: Vec<Vec2>) -> Contour {
        let n = points.len();
        if self.is_empty() || n < 3 {
            return Contour::new(points, true);
        }
        let on: Vec<Vec<(usize, f64)>> = points.iter().map(|&p| self.on(p)).collect();
        if on.iter().all(Vec::is_empty) {
            return Contour::new(points, true);
        }
        // Start where no run can pass through: off every curve, else at a curve's end.
        let at_end = |i: usize| {
            on[i]
                .iter()
                .any(|&(c, s)| s < MIN_RUN || s > self.sources[c].last() - MIN_RUN)
        };
        let start = (0..n)
            .find(|&i| on[i].is_empty())
            .or_else(|| (0..n).find(|&i| at_end(i)))
            .unwrap_or(0);
        let pts: Vec<Vec2> = (0..n).map(|k| points[(start + k) % n]).collect();
        let on: Vec<&Vec<(usize, f64)>> = (0..n).map(|k| &on[(start + k) % n]).collect();

        let mut out = Vec::with_capacity(n);
        let mut curves = Vec::with_capacity(n);
        let mut i = 0;
        while i < n {
            let mut best: Option<(usize, usize, f64, f64)> = None;
            for &(c, s0) in on[i] {
                let (j, s1) = self.run(c, s0, i, &pts, &on);
                if j > i && (s1 - s0).abs() >= MIN_RUN && best.is_none_or(|b| j > b.0) {
                    best = Some((j, c, s0, s1));
                }
            }
            out.push(pts[i]);
            match best {
                Some((j, c, s0, s1)) => {
                    let src = &self.sources[c];
                    curves.push(Some(src.cubic.piece(src.t(s0), src.t(s1)).handles()));
                    i = j;
                }
                None => {
                    curves.push(None);
                    i += 1;
                }
            }
        }
        let mut contour = Contour::new(out, true);
        if curves.iter().any(Option::is_some) {
            contour.curves = curves;
        }
        contour
    }

    /// How far a run along curve `c` starting at vertex `i` (sample position `s0`) extends:
    /// the last vertex index (`n` = back at vertex 0) and its sample position.
    fn run(
        &self,
        c: usize,
        s0: f64,
        i: usize,
        pts: &[Vec2],
        on: &[&Vec<(usize, f64)>],
    ) -> (usize, f64) {
        let n = pts.len();
        let (mut k, mut s, mut dir) = (i, s0, 0.0);
        while k < n {
            let next = (k + 1) % n;
            let Some(sn) = on[next].iter().find(|e| e.0 == c).map(|e| e.1) else {
                break;
            };
            let d = sn - s;
            if d.abs() > MAX_STEP {
                break;
            }
            if dir == 0.0 && d.abs() > MIN_RUN {
                dir = d.signum();
            }
            if d * dir < -BACKTRACK {
                break;
            }
            // The edge itself must follow the curve, not cut across it.
            if self.position(pts[k].lerp(pts[next], 0.5), c).is_none() {
                break;
            }
            k += 1;
            s = sn;
        }
        (k, s)
    }
}

/// Approximates the offset of `c` over `[t0, t1]` by `d` along its left normal, splitting
/// until each piece is within [`FLATNESS`].
fn offset_pieces(c: &Cubic, t0: f64, t1: f64, d: f64, depth: u32, out: &mut Vec<Cubic>) {
    let q = |t: f64| c.tangent(t).map(|u| c.at(t) + v2(-u.y, u.x) * d);
    let piece = c.piece(t0, t1);
    let fitted = (|| {
        let (a, b) = (q(t0)?, q(t1)?);
        // Handles scaled by how much longer the offset is than the curve: exact for circles.
        let (mut along, mut off) = (0.0, 0.0);
        let mut prev = (piece.a, a);
        for k in 1..=8 {
            let t = t0 + (t1 - t0) * k as f64 / 8.0;
            let now = (c.at(t), q(t)?);
            along += now.0.distance(prev.0);
            off += now.1.distance(prev.1);
            prev = now;
        }
        if along < 1e-9 {
            return None;
        }
        let r = off / along;
        let f = Cubic {
            a,
            c1: a + (piece.c1 - piece.a) * r,
            c2: b + (piece.c2 - piece.b) * r,
            b,
        };
        let worst = (1..8)
            .map(|k| {
                let u = k as f64 / 8.0;
                q(t0 + (t1 - t0) * u).map_or(f64::INFINITY, |p| p.distance(f.at(u)))
            })
            .fold(0.0, f64::max);
        (worst.is_finite() && worst <= FLATNESS).then_some(f)
    })();
    match fitted {
        Some(f) => out.push(f),
        None if depth < 6 => {
            let tm = (t0 + t1) / 2.0;
            offset_pieces(c, t0, tm, d, depth + 1, out);
            offset_pieces(c, tm, t1, d, depth + 1, out);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_join_up_to_the_curve() {
        let c = Cubic {
            a: v2(0.0, 0.0),
            c1: v2(30.0, 60.0),
            c2: v2(70.0, -60.0),
            b: v2(100.0, 0.0),
        };
        let p = c.piece(0.25, 0.75);
        assert!(p.a.distance(c.at(0.25)) < 1e-9 && p.b.distance(c.at(0.75)) < 1e-9);
        assert!(p.at(0.5).distance(c.at(0.5)) < 1e-9);
        let r = c.piece(0.75, 0.25);
        assert!(r.a.distance(c.at(0.75)) < 1e-9 && r.at(0.5).distance(c.at(0.5)) < 1e-9);
        let whole = c.piece(0.0, 1.0);
        assert!(whole.c1.distance(c.c1) < 1e-9 && whole.c2.distance(c.c2) < 1e-9);
    }

    #[test]
    fn untouched_polygon_comes_back_unchanged() {
        let (paths, prov) = Provenance::flatten(&[Contour::rectangle(0.0, 0.0, 10.0, 10.0)]);
        assert_eq!(paths.len(), 1);
        assert!(prov.is_empty());
        let pts: Vec<Vec2> = paths[0].iter().map(|p| v2(p[0], p[1])).collect();
        assert_eq!(prov.restore(pts.clone()).points, pts);
    }
}
