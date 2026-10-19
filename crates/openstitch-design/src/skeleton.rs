//! Variable-direction satin along a skeleton: a satin column whose stitches stay perpendicular
//! to a user-given centre line, with a width given at each centre-line point and interpolated
//! along it. Around a bend the direction turns smoothly over the column's half width. Own
//! algorithm. Points are 0.1 mm, widths mm.

use crate::generate::Run;
use crate::geometry::{Contour, Transform, Vec2, v2};
use serde::{Deserialize, Serialize};

/// Widest column, mm.
pub const MAX_WIDTH: f64 = 30.0;
/// Most centre-line points.
pub const MAX_POINTS: usize = 10_000;
/// Width used where a point has none, mm.
pub const DEFAULT_WIDTH: f64 = 3.0;
/// Most stitches across one column.
const MAX_RUNGS: usize = 200_000;

/// Centre line and the column width at each of its points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skeleton {
    /// Centre line, 0.1 mm.
    pub points: Vec<Vec2>,
    /// Full column width at each point, mm.
    pub widths: Vec<f64>,
}

/// Why a skeleton cannot make a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkeletonError {
    TooFew,
    TooMany,
    /// The number of widths differs from the number of points.
    WidthCount,
    /// Width `i` is not between 0 and [`MAX_WIDTH`] mm.
    Width(usize),
    /// Every width is (nearly) zero.
    Thin,
    /// The centre line is shorter than 0.1 mm or not finite.
    Short,
}

impl Skeleton {
    pub fn validate(&self) -> Result<(), SkeletonError> {
        if self.points.len() < 2 {
            return Err(SkeletonError::TooFew);
        }
        if self.points.len() > MAX_POINTS {
            return Err(SkeletonError::TooMany);
        }
        if self.widths.len() != self.points.len() {
            return Err(SkeletonError::WidthCount);
        }
        if let Some(i) = self
            .widths
            .iter()
            .position(|w| !(w.is_finite() && (0.0..=MAX_WIDTH).contains(w)))
        {
            return Err(SkeletonError::Width(i));
        }
        if self.widths.iter().all(|&w| w < 0.1) {
            return Err(SkeletonError::Thin);
        }
        if !self
            .points
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite())
            || self.length() < 1.0
        {
            return Err(SkeletonError::Short);
        }
        Ok(())
    }

    pub fn length(&self) -> f64 {
        self.points.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    /// Moved by `t`; widths scale with the transform's average scale.
    pub fn transformed(&self, t: &Transform) -> Skeleton {
        let k = (t.a * t.d - t.b * t.c).abs().sqrt();
        Skeleton {
            points: self.points.iter().map(|&p| t.apply(p)).collect(),
            widths: self.widths.iter().map(|w| (w * k).min(MAX_WIDTH)).collect(),
        }
    }
}

/// The skeleton cleaned for sewing: finite points, coincident neighbours merged, a width for
/// every point (0.1 mm, half width).
struct Centre {
    points: Vec<Vec2>,
    half: Vec<f64>,
    /// Arc length at each point.
    at: Vec<f64>,
}

impl Centre {
    fn new(s: &Skeleton) -> Option<Centre> {
        let mut points: Vec<Vec2> = Vec::new();
        let mut half: Vec<f64> = Vec::new();
        for (i, &p) in s.points.iter().enumerate() {
            if !(p.x.is_finite() && p.y.is_finite()) {
                continue;
            }
            let w = s
                .widths
                .get(i)
                .or(s.widths.last())
                .copied()
                .unwrap_or(DEFAULT_WIDTH);
            let w = if w.is_finite() {
                w.clamp(0.0, MAX_WIDTH)
            } else {
                0.0
            };
            if points.last().is_some_and(|q| q.distance(p) < 1e-6) {
                continue;
            }
            points.push(p);
            half.push(w * 5.0);
        }
        if points.len() < 2 {
            return None;
        }
        let mut at = vec![0.0];
        for w in points.windows(2) {
            at.push(at[at.len() - 1] + w[0].distance(w[1]));
        }
        (at[at.len() - 1] >= 1e-6 && at[at.len() - 1].is_finite()).then_some(Centre {
            points,
            half,
            at,
        })
    }

    fn length(&self) -> f64 {
        self.at[self.at.len() - 1]
    }

    fn direction(&self, j: usize) -> Vec2 {
        let d = self.points[j + 1] - self.points[j];
        d * (1.0 / d.length())
    }

    /// Centre point, unit tangent and half width at arc length `s`.
    fn station(&self, s: f64) -> (Vec2, Vec2, f64) {
        let last = self.points.len() - 2;
        let j = self.at[1..].partition_point(|&a| a < s).min(last);
        let len = self.at[j + 1] - self.at[j];
        let f = ((s - self.at[j]) / len).clamp(0.0, 1.0);
        let centre = self.points[j].lerp(self.points[j + 1], f);
        let half = self.half[j] + (self.half[j + 1] - self.half[j]) * f;
        let u = self.direction(j);
        // Turn smoothly through the vertex at either end of this segment.
        let blend = |v: usize, d: f64| -> Option<Vec2> {
            let (before, after) = (self.direction(v - 1), self.direction(v));
            let reach = self.half[v]
                .min((self.at[v] - self.at[v - 1]) / 2.0)
                .min((self.at[v + 1] - self.at[v]) / 2.0);
            if reach <= 1e-9 || d.abs() >= reach {
                return None;
            }
            let k = 0.5 + d / (2.0 * reach);
            let t = before.lerp(after, k);
            (t.length() > 1e-6).then(|| t * (1.0 / t.length()))
        };
        let tangent = if j > 0 && s - self.at[j] < len / 2.0 {
            blend(j, s - self.at[j])
        } else if j < last {
            blend(j + 1, s - self.at[j + 1])
        } else {
            None
        };
        (centre, tangent.unwrap_or(u), half)
    }

    /// Stitch ends across the column every `spacing` (or a little less) along the centre
    /// line, first and last ends included: [left, right], each lengthened by `pull`.
    fn rungs(&self, spacing: f64, pull: f64) -> Vec<[Vec2; 2]> {
        let length = self.length();
        let spacing = if spacing.is_finite() {
            spacing.max(0.1)
        } else {
            1.0
        };
        let pull = if pull.is_finite() { pull } else { 0.0 };
        let n = ((length / spacing).ceil() as usize).clamp(1, MAX_RUNGS);
        (0..=n)
            .map(|k| {
                let (c, t, half) = self.station(length * k as f64 / n as f64);
                let h = (half + pull).max(0.0);
                let left = v2(t.y, -t.x);
                [c + left * h, c - left * h]
            })
            .collect()
    }
}

/// Satin along the skeleton: the first rung sewn across, then one needle point per rung on
/// alternate sides ([`crate::column::zigzag`]), the rungs `spacing` apart along the centre
/// line (0.1 mm), each lengthened by `pull` at both ends. Empty for a degenerate skeleton.
pub fn satin(s: &Skeleton, spacing: f64, pull: f64) -> Run {
    let Some(c) = Centre::new(s) else {
        return Vec::new();
    };
    let mut run = crate::column::zigzag(c.rungs(spacing, pull));
    run.dedup_by(|a, b| a.distance(*b) < 1e-9);
    run
}

/// The rungs of [`satin`] filling the closed region `contours`, [left, right] in sewing
/// order: each rung reaches from the centre line to the region's outline along the rung (at
/// most 0.5 mm beyond the skeleton's half width), so the column covers the region where a
/// derived width is only approximate; then it is lengthened by `pull` at both ends. Empty for
/// a degenerate skeleton.
pub fn rungs_within(s: &Skeleton, contours: &[Contour], spacing: f64, pull: f64) -> Vec<[Vec2; 2]> {
    let Some(c) = Centre::new(s) else {
        return Vec::new();
    };
    let edges: Vec<(Vec2, Vec2)> = contours
        .iter()
        .filter(|c| c.closed)
        .flat_map(Contour::edges)
        .collect();
    let pull = if pull.is_finite() { pull } else { 0.0 };
    c.rungs(spacing, 0.0)
        .into_iter()
        .map(|[l, r]| {
            let centre = l.lerp(r, 0.5);
            let half = l.distance(centre);
            if half < 1e-6 || !crate::geometry::contains(centre, contours) {
                return [l, r];
            }
            let reach = |end: Vec2| {
                let dir = (end - centre) * (1.0 / half);
                let d = ray(centre, dir, &edges).map_or(half, |d| d.min(half + 5.0));
                centre + dir * (d + pull).max(0.0)
            };
            [reach(l), reach(r)]
        })
        .collect()
}

/// The column's outline: the left side along the centre line, then the right side back.
pub fn outline(s: &Skeleton) -> Option<Contour> {
    let c = Centre::new(s)?;
    let rungs = c.rungs(5.0, 0.0);
    let mut pts: Vec<Vec2> = rungs.iter().map(|r| r[0]).collect();
    pts.extend(rungs.iter().rev().map(|r| r[1]));
    pts.dedup_by(|a, b| a.distance(*b) < 1e-6);
    (pts.len() >= 3).then(|| Contour::new(pts, true))
}

/// Grid cells along the longer side of the region when a skeleton is derived.
const DERIVE_CELLS: f64 = 160.0;
/// Most region edges a derivation looks at.
const DERIVE_MAX_EDGES: usize = 5_000;
/// Clearance growth along the line (per unit length) above which an end runs into a corner
/// (a corner of about 25° or more).
const CORNER_RATE: f64 = 0.2;

/// A grid over the region: which cells are inside and how far each inside cell's centre is
/// from the outline (0.1 mm).
struct Grid {
    origin: Vec2,
    step: f64,
    nx: usize,
    ny: usize,
    /// Distance to the outline; negative outside.
    clearance: Vec<f64>,
}

impl Grid {
    fn new(edges: &[(Vec2, Vec2)], b: &crate::geometry::Rect) -> Option<Grid> {
        let step = (b.width().max(b.height()) / DERIVE_CELLS).max(0.25);
        let nx = (b.width() / step).ceil() as usize + 1;
        let ny = (b.height() / step).ceil() as usize + 1;
        if nx < 2 || ny < 2 || nx * ny > 200_000 {
            return None;
        }
        let origin = v2(b.min_x, b.min_y);
        let mut clearance = vec![-1.0; nx * ny];
        let mut crossings = Vec::new();
        for j in 0..ny {
            let y = origin.y + (j as f64 + 0.5) * step;
            // Even-odd: the cells between alternate crossings of this row are inside.
            crossings.clear();
            for &(a, c) in edges {
                if (a.y > y) != (c.y > y) {
                    crossings.push(a.x + (y - a.y) * (c.x - a.x) / (c.y - a.y));
                }
            }
            crossings.sort_by(f64::total_cmp);
            for pair in crossings.as_chunks::<2>().0 {
                for i in 0..nx {
                    let x = origin.x + (i as f64 + 0.5) * step;
                    if x > pair[0] && x < pair[1] {
                        let p = v2(x, y);
                        clearance[j * nx + i] = edges
                            .iter()
                            .map(|&(a, c)| segment_distance(p, a, c))
                            .fold(f64::INFINITY, f64::min);
                    }
                }
            }
        }
        Some(Grid {
            origin,
            step,
            nx,
            ny,
            clearance,
        })
    }

    fn centre(&self, k: usize) -> Vec2 {
        v2(
            self.origin.x + ((k % self.nx) as f64 + 0.5) * self.step,
            self.origin.y + ((k / self.nx) as f64 + 0.5) * self.step,
        )
    }

    /// Inside neighbours of cell `k` and the step to each (in cells).
    fn neighbours(&self, k: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        let (i, j) = ((k % self.nx) as i64, (k / self.nx) as i64);
        [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ]
        .into_iter()
        .filter_map(move |(di, dj)| {
            let (a, b) = (i + di, j + dj);
            if a < 0 || b < 0 || a >= self.nx as i64 || b >= self.ny as i64 {
                return None;
            }
            let n = b as usize * self.nx + a as usize;
            (self.clearance[n] >= 0.0).then_some((
                n,
                if di != 0 && dj != 0 {
                    std::f64::consts::SQRT_2
                } else {
                    1.0
                },
            ))
        })
    }

    /// Shortest paths from `from` with the step cost `cost(cell entered, step)`: the cost to
    /// each cell and the cell before it.
    fn paths(&self, from: usize, cost: impl Fn(usize, f64) -> f64) -> (Vec<f64>, Vec<usize>) {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        #[derive(PartialEq)]
        struct Cost(f64);
        impl Eq for Cost {}
        impl PartialOrd for Cost {
            fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(o))
            }
        }
        impl Ord for Cost {
            fn cmp(&self, o: &Self) -> std::cmp::Ordering {
                self.0.total_cmp(&o.0)
            }
        }
        let mut best = vec![f64::INFINITY; self.clearance.len()];
        let mut prev = vec![usize::MAX; self.clearance.len()];
        let mut heap = BinaryHeap::new();
        best[from] = 0.0;
        heap.push(Reverse((Cost(0.0), from)));
        while let Some(Reverse((Cost(c), k))) = heap.pop() {
            if c > best[k] {
                continue;
            }
            for (n, step) in self.neighbours(k) {
                let nc = c + cost(n, step);
                if nc < best[n] {
                    best[n] = nc;
                    prev[n] = k;
                    heap.push(Reverse((Cost(nc), n)));
                }
            }
        }
        (best, prev)
    }
}

fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let len2 = d.x * d.x + d.y * d.y;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * d.x + (p.y - a.y) * d.y) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + d * t)
}

/// Distance from `p` along `dir` (unit) to the nearest edge, if one is hit.
fn ray(p: Vec2, dir: Vec2, edges: &[(Vec2, Vec2)]) -> Option<f64> {
    edges
        .iter()
        .filter_map(|&(a, b)| {
            let e = b - a;
            let den = dir.x * e.y - dir.y * e.x;
            if den.abs() < 1e-12 {
                return None;
            }
            let w = a - p;
            let t = (w.x * e.y - w.y * e.x) / den;
            let u = (w.x * dir.y - w.y * dir.x) / den;
            (t > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some(t)
        })
        .min_by(f64::total_cmp)
}

/// Douglas–Peucker: the indexes of `pts` to keep so the line stays within `tolerance`.
fn simplify(pts: &[Vec2], tolerance: f64) -> Vec<usize> {
    fn walk(pts: &[Vec2], lo: usize, hi: usize, tolerance: f64, keep: &mut Vec<usize>) {
        let (far, d) = (lo + 1..hi)
            .map(|i| (i, segment_distance(pts[i], pts[lo], pts[hi])))
            .fold((lo, 0.0), |a, b| if b.1 > a.1 { b } else { a });
        if d > tolerance {
            walk(pts, lo, far, tolerance, keep);
            walk(pts, far, hi, tolerance, keep);
        } else {
            keep.push(hi);
        }
    }
    let mut keep = vec![0];
    if pts.len() > 1 {
        walk(pts, 0, pts.len() - 1, tolerance, &mut keep);
    }
    keep
}

/// A skeleton for the closed region `contours` (0.1 mm, even-odd, holes allowed): an
/// approximation of its medial axis, not an exact one. The region is sampled on a grid of
/// about 160 cells along its longer side; the centre line runs between the two cells farthest
/// apart through the region, along the path that keeps furthest from the outline. Where an
/// end runs into a corner (the clearance grows quickly along it) it is trimmed back, at most a
/// quarter of the line, then reaches out again most of its clearance towards the end of the
/// stroke. The line is smoothed and simplified, and the width at each point is the chord
/// across the region perpendicular to the line there (twice the clearance where that chord
/// runs far along the region at a bend). Widths are capped at
/// [`MAX_WIDTH`]. Branches are not followed: a Y gives its longest stroke. `None` when the
/// region is empty, too small, or no line can be found.
pub fn derive(contours: &[Contour]) -> Option<Skeleton> {
    let edges: Vec<(Vec2, Vec2)> = contours
        .iter()
        .filter(|c| c.closed)
        .flat_map(Contour::edges)
        .filter(|(a, b)| a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite())
        .collect();
    if edges.len() < 3 || edges.len() > DERIVE_MAX_EDGES {
        return None;
    }
    let b = crate::geometry::Rect::enclosing(edges.iter().map(|e| e.0))?;
    if b.width().max(b.height()) < 2.0 {
        return None;
    }
    let grid = Grid::new(&edges, &b)?;
    // Start in the widest part, so a region in pieces gives its main piece.
    let seed = (0..grid.clearance.len())
        .max_by(|&a, &c| grid.clearance[a].total_cmp(&grid.clearance[c]))
        .filter(|&k| grid.clearance[k] > 0.0)?;
    let farthest = |best: &[f64]| {
        (0..best.len())
            .filter(|&k| best[k].is_finite())
            .max_by(|&a, &c| best[a].total_cmp(&best[c]))
    };
    let length = |_: usize, step: f64| step;
    let a = farthest(&grid.paths(seed, length).0)?;
    let end = farthest(&grid.paths(a, length).0)?;
    if a == end {
        return None;
    }
    // Along the ridge: steps close to the outline cost much more.
    let widest = grid.clearance[seed];
    let (_, prev) = grid.paths(a, |n, step| {
        let k = widest / (grid.clearance[n] + grid.step * 0.5);
        step * k * k
    });
    let mut cells = vec![end];
    while let Some(&k) = cells.last() {
        if k == a {
            break;
        }
        let p = prev[k];
        if p == usize::MAX || cells.len() > grid.clearance.len() {
            return None;
        }
        cells.push(p);
    }
    cells.reverse();
    let pts: Vec<Vec2> = cells.iter().map(|&k| grid.centre(k)).collect();
    let clear: Vec<f64> = cells.iter().map(|&k| grid.clearance[k]).collect();
    // Trim the ends that run into corners: there the clearance grows quickly along the
    // line, while along the middle of a stroke it stays about the same.
    let at: Vec<f64> = std::iter::once(0.0)
        .chain(pts.windows(2).scan(0.0, |s, w| {
            *s += w[0].distance(w[1]);
            Some(*s)
        }))
        .collect();
    let total = at[at.len() - 1];
    let window = 3.0 * grid.step;
    let mut lo = 0;
    while lo + 2 < pts.len() && at[lo] <= total / 4.0 {
        let Some(j) = (lo + 1..pts.len()).find(|&j| at[j] - at[lo] >= window) else {
            break;
        };
        if clear[j] - clear[lo] <= CORNER_RATE * (at[j] - at[lo]) {
            break;
        }
        lo += 1;
    }
    let mut hi = pts.len() - 1;
    while hi > lo + 2 && total - at[hi] <= total / 4.0 {
        let Some(j) = (0..hi).rev().find(|&j| at[hi] - at[j] >= window) else {
            break;
        };
        if clear[j] - clear[hi] <= CORNER_RATE * (at[hi] - at[j]) {
            break;
        }
        hi -= 1;
    }
    // Then reach back towards the end of the stroke, most of the clearance further.
    let reach = |end: usize, inner: usize| -> Option<Vec2> {
        let d = pts[end] - pts[inner];
        (d.length() > 1e-9 && clear[end] > grid.step)
            .then(|| pts[end] + d * (0.8 * clear[end] / d.length()))
    };
    let inner_of = |end: usize, toward: i64| {
        let mut k = end as i64;
        while (k + toward) >= lo as i64
            && (k + toward) <= hi as i64
            && (at[k as usize] - at[end]).abs() < (3.0 * grid.step).max(2.0 * clear[end])
        {
            k += toward;
        }
        k as usize
    };
    let head = reach(lo, inner_of(lo, 1));
    let tail = reach(hi, inner_of(hi, -1));
    let mut pts: Vec<Vec2> = head
        .into_iter()
        .chain(pts[lo..=hi].iter().copied())
        .chain(tail)
        .collect();
    // Smooth the grid staircase (ends stay).
    for _ in 0..3 {
        let old = pts.clone();
        for i in 1..pts.len().saturating_sub(1) {
            pts[i] = (old[i - 1] + old[i] * 2.0 + old[i + 1]) * 0.25;
        }
    }
    let keep = simplify(&pts, grid.step * 0.75);
    let line: Vec<Vec2> = keep.iter().map(|&i| pts[i]).collect();
    if line.len() < 2 {
        return None;
    }
    let mut points = Vec::with_capacity(line.len());
    let mut widths = Vec::with_capacity(line.len());
    let last = line.len() - 1;
    let mut chords = Vec::with_capacity(line.len());
    for i in 0..line.len() {
        let t = line[(i + 1).min(last)] - line[i.saturating_sub(1)];
        let p = line[i];
        let near = edges
            .iter()
            .map(|&(a, c)| segment_distance(p, a, c))
            .fold(f64::INFINITY, f64::min);
        let chord = (t.length() > 1e-9).then(|| {
            let t = t * (1.0 / t.length());
            let left = v2(t.y, -t.x);
            (left, ray(p, left, &edges), ray(p, left * -1.0, &edges))
        });
        let (centre, width) = match chord {
            Some((left, Some(l), Some(r))) if l + r <= 6.0 * near + 2.0 * grid.step => {
                (p + left * ((l - r) / 2.0), l + r)
            }
            _ => (p, 2.0 * near),
        };
        chords.push(chord);
        points.push(centre);
        widths.push(width);
    }
    // The reached-out ends are close to the outline: their chord, no wider than next door.
    for (end, next, reached) in [(0, 1, head.is_some()), (last, last - 1, tail.is_some())] {
        if !reached || end == next {
            continue;
        }
        let w = match chords[end] {
            Some((left, Some(l), Some(r))) if l + r <= widths[next] * 1.1 => {
                points[end] = line[end] + left * ((l - r) / 2.0);
                (l + r).min(widths[next])
            }
            _ => widths[next],
        };
        widths[end] = w;
    }
    let widths = widths
        .into_iter()
        .map(|w| (w / 10.0).clamp(0.0, MAX_WIDTH))
        .collect();
    let s = Skeleton { points, widths };
    s.validate().is_ok().then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skeleton(points: &[(f64, f64)], widths: &[f64]) -> Skeleton {
        Skeleton {
            points: points.iter().map(|&(x, y)| v2(x, y)).collect(),
            widths: widths.to_vec(),
        }
    }

    #[test]
    fn stitches_are_perpendicular_to_the_centre_line() {
        // An L: right 200, then down 200; 4 mm wide.
        let s = skeleton(&[(0.0, 0.0), (200.0, 0.0), (200.0, 200.0)], &[4.0; 3]);
        assert_eq!(s.validate(), Ok(()));
        let run = satin(&s, 3.0, 0.0);
        let c = Centre::new(&s).expect("centre");
        for [l, r] in c.rungs(3.0, 0.0) {
            let mid = l.lerp(r, 0.5);
            let across = r - l;
            assert!((across.length() - 40.0).abs() < 1e-6, "full width");
            if mid.x < 170.0 {
                // First leg: centre on y = 0, stitch vertical.
                assert!(mid.y.abs() < 1e-9 && across.x.abs() < 1e-9, "{l:?} {r:?}");
            } else if mid.y > 30.0 {
                assert!((mid.x - 200.0).abs() < 1e-9 && across.y.abs() < 1e-9);
            }
        }
        // Through the bend the direction turns gradually.
        let turning = c.rungs(3.0, 0.0).iter().any(|[l, r]| {
            let d = *r - *l;
            d.x.abs() > 5.0 && d.y.abs() > 5.0
        });
        assert!(turning);
        assert!(run.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
    }

    #[test]
    fn density_and_width_are_respected() {
        let s = skeleton(&[(0.0, 0.0), (300.0, 0.0)], &[2.0, 6.0]);
        let spacing = 10.0 / 4.0; // 4 lines per mm
        let run = satin(&s, spacing, 0.0);
        // One needle point per spacing step, both ends included, plus the first rung's other
        // end.
        assert_eq!(run.len(), 122);
        // Every stitch crosses the centre line (y = 0): needle points alternate sides.
        for w in run.windows(2) {
            assert!(w[0].y * w[1].y < 0.0, "{w:?} runs along an edge");
        }
        let centres: Vec<Vec2> = c_rungs(&s, spacing)
            .iter()
            .map(|[l, r]| l.lerp(*r, 0.5))
            .collect();
        for w in centres.windows(2) {
            assert!(w[0].distance(w[1]) <= spacing + 1e-9);
        }
        // Width tapers from 2 mm to 6 mm.
        assert!((run[0].distance(run[1]) - 20.0).abs() < 1e-6);
        let last = run[run.len() - 1];
        assert!((last.x - 300.0).abs() < 1e-6 && (last.y.abs() - 30.0).abs() < 1e-6);
        // Pull compensation lengthens each stitch at both ends.
        let pulled = satin(&s, spacing, 3.0);
        assert!((pulled[0].distance(pulled[1]) - 26.0).abs() < 1e-6);
    }

    fn c_rungs(s: &Skeleton, spacing: f64) -> Vec<[Vec2; 2]> {
        Centre::new(s).expect("centre").rungs(spacing, 0.0)
    }

    #[test]
    fn degenerate_input_gives_no_nan() {
        let cases = [
            skeleton(&[], &[]),
            skeleton(&[(5.0, 5.0)], &[3.0]),
            skeleton(&[(5.0, 5.0), (5.0, 5.0), (5.0, 5.0)], &[3.0; 3]),
            skeleton(
                &[(0.0, 0.0), (f64::NAN, 1.0), (10.0, 0.0)],
                &[f64::NAN, 3.0],
            ),
            skeleton(&[(0.0, 0.0), (100.0, 0.0), (0.0, 0.0)], &[]),
            skeleton(&[(0.0, 0.0), (100.0, 0.0)], &[f64::INFINITY, -2.0]),
        ];
        for s in &cases {
            for (sp, pull) in [(3.0, 0.0), (0.0, 0.0), (f64::NAN, f64::NAN), (1e-12, -50.0)] {
                let run = satin(s, sp, pull);
                assert!(
                    run.iter().all(|p| p.x.is_finite() && p.y.is_finite()),
                    "{s:?}"
                );
            }
            if let Some(o) = outline(s) {
                assert!(o.points.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
            }
        }
        assert!(satin(&cases[0], 3.0, 0.0).is_empty());
        assert!(satin(&cases[2], 3.0, 0.0).is_empty());
        // A centre line that turns straight back still sews.
        assert!(!satin(&cases[4], 3.0, 0.0).is_empty());
    }

    #[test]
    fn validation() {
        let ok = skeleton(&[(0.0, 0.0), (100.0, 0.0)], &[3.0, 0.0]);
        assert_eq!(ok.validate(), Ok(()));
        assert_eq!(
            skeleton(&[(0.0, 0.0)], &[3.0]).validate(),
            Err(SkeletonError::TooFew)
        );
        assert_eq!(
            skeleton(&[(0.0, 0.0), (10.0, 0.0)], &[3.0]).validate(),
            Err(SkeletonError::WidthCount)
        );
        assert_eq!(
            skeleton(&[(0.0, 0.0), (10.0, 0.0)], &[3.0, 31.0]).validate(),
            Err(SkeletonError::Width(1))
        );
        assert_eq!(
            skeleton(&[(0.0, 0.0), (10.0, 0.0)], &[0.0, 0.0]).validate(),
            Err(SkeletonError::Thin)
        );
        assert_eq!(
            skeleton(&[(0.0, 0.0), (0.5, 0.0)], &[3.0, 3.0]).validate(),
            Err(SkeletonError::Short)
        );
        let o = outline(&ok).expect("outline");
        assert!(crate::geometry::contains(v2(50.0, 0.0), &[o]));
        let t = Transform::scaling(2.0, 2.0, v2(0.0, 0.0));
        assert_eq!(ok.transformed(&t).widths, vec![6.0, 0.0]);
    }

    #[test]
    fn derived_skeleton_of_a_bar_runs_down_its_middle() {
        // 100 mm × 8 mm.
        let bar = Contour::rectangle(0.0, 0.0, 1000.0, 80.0);
        let s = derive(std::slice::from_ref(&bar)).expect("skeleton");
        assert_eq!(s.validate(), Ok(()));
        assert!(s.length() > 800.0, "length {}", s.length());
        for (p, w) in s.points.iter().zip(&s.widths) {
            assert!((p.y - 40.0).abs() < 4.0, "{p:?} off the middle");
            if p.x > 100.0 && p.x < 900.0 {
                assert!((w - 8.0).abs() < 0.5, "width {w} at {p:?}");
            }
        }
        // A straight bar simplifies to a few points.
        assert!(s.points.len() <= 12, "{} points", s.points.len());
        let run = satin(&s, 3.0, 0.0);
        assert!(run.iter().all(
            |p| crate::geometry::contains(*p, std::slice::from_ref(&bar))
                || p.y.abs() < 1.0
                || (p.y - 80.0).abs() < 1.0
        ));
    }

    #[test]
    fn derived_skeleton_follows_a_bend() {
        // Half a ring: radii 200 and 260 (6 mm band) around (0, 0), the upper half.
        let n = 48;
        let mut pts: Vec<Vec2> = (0..=n)
            .map(|k| {
                let a = std::f64::consts::PI * k as f64 / n as f64;
                v2(260.0 * a.cos(), -260.0 * a.sin())
            })
            .collect();
        pts.extend((0..=n).rev().map(|k| {
            let a = std::f64::consts::PI * k as f64 / n as f64;
            v2(200.0 * a.cos(), -200.0 * a.sin())
        }));
        let band = Contour::new(pts, true);
        let s = derive(std::slice::from_ref(&band)).expect("skeleton");
        assert!(s.length() > 400.0, "length {}", s.length());
        let last = s.points.len() - 1;
        for (i, (p, w)) in s.points.iter().zip(&s.widths).enumerate() {
            // Within 0.8 mm of the band's middle (the reached-out ends 1.5 mm).
            let r = p.length();
            let off = if i == 0 || i == last { 15.0 } else { 8.0 };
            assert!((r - 230.0).abs() < off, "{p:?} radius {r}");
            if p.y < -60.0 {
                assert!((w - 6.0).abs() < 0.8, "width {w} at {p:?}");
            }
        }
        // The column stays perpendicular to the band: across, not along.
        let o = outline(&s).expect("outline");
        assert!(o.points.iter().all(|p| (p.length() - 230.0).abs() < 40.0));
    }

    #[test]
    fn deriving_from_nothing_gives_none() {
        assert_eq!(derive(&[]), None);
        let open = Contour::new(vec![v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 50.0)], false);
        assert_eq!(derive(&[open]), None);
        assert_eq!(derive(&[Contour::rectangle(0.0, 0.0, 1.0, 1.0)]), None);
        let nan = Contour::new(
            vec![
                v2(0.0, 0.0),
                v2(f64::NAN, 0.0),
                v2(100.0, 50.0),
                v2(0.0, 50.0),
            ],
            true,
        );
        if let Some(s) = derive(&[nan]) {
            assert_eq!(s.validate(), Ok(()));
        }
        // A wide disc still gives a sewable line, widths capped.
        let disc = Contour::ellipse(v2(0.0, 0.0), 500.0, 500.0, 72);
        let s = derive(&[disc]).expect("disc");
        assert!(s.widths.iter().all(|w| *w <= MAX_WIDTH));
        assert_eq!(s.validate(), Ok(()));
    }
}
