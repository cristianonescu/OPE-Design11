//! Concentric Circle Stitch as PE-DESIGN 11.50 sews it (issue #138; references
//! `compat/reference/stitches/concentric-*.pes`). Circles about the centre point (by default
//! the centre of the region's bounding box), one row (1 / density) apart: radii d, 2d, 3d, …
//! out to the farthest outline point.
//!
//! - The circles that lie wholly inside the region are sewn first, from the largest inwards,
//!   each a closed loop counter-clockwise on screen that starts and ends on one radial seam;
//!   the needle steps one row inwards along the seam to the next circle.
//! - Then, after a trim, the circles the outline cuts are sewn from the smallest outwards as
//!   arcs, each region piece back and forth from its nearest end, with running stitches along
//!   the outline between pieces.
//! - The seam is where the smallest cut circle first meets the outline, walking
//!   counter-clockwise from the start point's direction (R1; the bottom by default): on a
//!   40 mm circle at 9 o'clock, on a 40 × 30 mm rectangle where the 15.5 mm circle leaves the
//!   bottom edge.
//! - Each circle has 4·2ⁿ needle points, the fewest whose chord fits the run pitch (PE-DESIGN
//!   halves a Bézier quarter until the chord fits), at least 16 unless that makes stitches
//!   under 0.6 mm (16 on PE's 2.0 and 2.5 mm circles, 8 on its 1.0 mm one); needle points sit
//!   on PE-DESIGN's 0.1 mm grid and a chord the rounding makes longer than the pitch is split
//!   in equal stitches.
//! - The second Start Point button sews the same stitches backwards.
//!
//! PE-DESIGN splits more of the chords inside a circle than the rounding explains (2.0, 1.0 +
//! 1.0 and 1.33 × 3 mm runs at a 2 mm pitch); OpenStitch's stitch counts are 3–5 % under
//! PE-DESIGN's for that reason (see the `concentric-*` specs).
//!
//! Lengths are 0.1 mm; output is deterministic.

use crate::fmath::Det;
use crate::geometry::{Contour, Vec2, contains, v2};
use crate::paths::{clip, order};
use std::f64::consts::TAU;

/// How far (0.1 mm) a cut circle may stray outside the outline and still be sewn whole.
const CLIP_TOLERANCE: f64 = 1.0;
/// Most circles per region.
const MAX_RINGS: usize = 20_000;
/// Most needle points per region (as for every other generator).
const MAX_POINTS: usize = 2_000_000;
/// Needle points on a small circle (PE-DESIGN 11.50: 16 on its 2.0 and 2.5 mm circles).
const MIN_CIRCLE_POINTS: usize = 16;
/// Shortest stitch on a circle before it gets fewer points, 0.1 mm.
const MIN_CIRCLE_STITCH: f64 = 6.0;

/// What the generator needs from the region's attributes.
#[derive(Clone, Copy, Debug)]
pub struct ConcentricParams {
    /// Centre of the circles, 0.1 mm.
    pub center: Vec2,
    /// Distance between circles, 0.1 mm (1 / density).
    pub spacing: f64,
    /// Running stitch length, 0.1 mm.
    pub pitch: f64,
    /// The second Start Point button: the same stitches backwards.
    pub start_inside: bool,
    /// The start point (R1): the seam is found walking from its direction.
    pub entry: Option<Vec2>,
}

/// The two parts PE-DESIGN sews with a trim between them: the whole circles as one path
/// (outermost first), then the arcs of the cut circles as pieces to join by travel. With
/// `start_inside` both are reversed and swapped.
pub struct Concentric {
    pub full: Vec<Vec2>,
    pub arcs: Vec<Vec<Vec2>>,
    /// Whether the arcs are sewn first (the second Start Point button, or a start point beyond
    /// the whole circles).
    pub arcs_first: bool,
    /// Whether the arcs and the whole circles are joined by travel, without a trim (a start
    /// point beyond the whole circles).
    pub joined: bool,
}

/// Concentric circles of `p.center` inside `closed` minus `holes`.
pub fn concentric(closed: &[Contour], holes: &[Contour], p: &ConcentricParams) -> Concentric {
    let mut out = Concentric {
        full: Vec::new(),
        arcs: Vec::new(),
        arcs_first: p.start_inside,
        joined: false,
    };
    if !(p.spacing > 0.0 && p.pitch > 0.0) {
        return out;
    }
    let c = p.center;
    let reach = closed
        .iter()
        .flat_map(|k| k.outline_points())
        .map(|q| q.distance(c))
        .fold(0.0, f64::max);
    let edges: Vec<(Vec2, Vec2)> = closed
        .iter()
        .chain(holes)
        .filter(|k| k.closed)
        .flat_map(Contour::edges)
        .collect();
    let inside = |q: Vec2| contains(q, closed) && !contains(q, holes);
    // Radii, and whether each circle lies wholly inside.
    let mut full_radii = Vec::new();
    let mut cut_radii = Vec::new();
    let mut k = 1;
    while (k as f64) * p.spacing <= reach + 1e-9 && k <= MAX_RINGS {
        let r = k as f64 * p.spacing;
        if crosses(c, r, &edges) {
            cut_radii.push(r);
        } else if inside(c + v2(r, 0.0)) {
            full_radii.push(r);
        }
        k += 1;
    }
    let entry_angle = p.entry.map_or(std::f64::consts::FRAC_PI_2, |e| {
        if e.distance(c) < 1e-9 {
            std::f64::consts::FRAC_PI_2
        } else {
            (e.y - c.y).datan2(e.x - c.x)
        }
    });
    // A start point farther out than the whole circles (a portrait rectangle's or ellipse's
    // bottom, a triangle's corner): PE-DESIGN 11.50 starts there on the cut circles, works
    // inwards through them and then through the whole circles to the middle, without a trim
    // (references concentric-rect30x40, concentric-ellipse30x40, concentric-triangle).
    let outermost_full = full_radii.last().copied().unwrap_or(0.0);
    if let Some(e) = p
        .entry
        .filter(|e| e.distance(c) > outermost_full + p.spacing + CLIP_TOLERANCE)
    {
        return outside_in(c, closed, holes, &edges, &full_radii, &cut_radii, e, p);
    }
    let seam = cut_radii
        .first()
        .and_then(|&r| first_crossing(c, r, entry_angle, &inside))
        .unwrap_or(entry_angle);
    // Whole circles, outermost first, each from the seam.
    let mut budget = 0usize;
    for &r in full_radii.iter().rev() {
        let ring = circle(c, r, seam, p.pitch);
        budget += ring.len();
        if budget > MAX_POINTS {
            break;
        }
        out.full.extend(ring);
    }
    out.full.dedup_by(|a, b| a.distance(*b) < 1e-9);
    // Cut circles, innermost first, as the pieces inside the region. A circle that only grazes
    // the outline or a hole's edge (nowhere more than CLIP_TOLERANCE outside: a 40 mm circle's
    // 20 mm circle on its flattened outline) stays one loop, as PE-DESIGN sews it, instead of
    // breaking into fragments where it crosses the flattened outline.
    let mut pieces = Vec::new();
    for &r in &cut_radii {
        let ring = circle(c, r, seam, p.pitch);
        budget += ring.len();
        if budget > MAX_POINTS {
            break;
        }
        let grazes = ring
            .iter()
            .all(|&q| inside(q) || distance_to_edges(q, &edges) <= CLIP_TOLERANCE);
        if grazes {
            pieces.push(ring);
        } else {
            pieces.extend(clip(&ring, closed, holes));
        }
    }
    let start = out.full.last().copied().or_else(|| {
        cut_radii
            .first()
            .map(|&r| c + v2(r * seam.dcos(), r * seam.dsin()))
    });
    out.arcs = order(pieces, start);
    if p.start_inside {
        out.full.reverse();
        out.arcs.reverse();
        out.arcs.iter_mut().for_each(|a| a.reverse());
    }
    out
}

/// The cut circles from the outermost in, ordered from the start point `entry`, then the
/// whole circles from the largest in, starting where the arcs end: one sequence, joined by
/// travel, ending in the middle.
#[allow(clippy::too_many_arguments)]
fn outside_in(
    c: Vec2,
    closed: &[Contour],
    holes: &[Contour],
    edges: &[(Vec2, Vec2)],
    full_radii: &[f64],
    cut_radii: &[f64],
    entry: Vec2,
    p: &ConcentricParams,
) -> Concentric {
    let inside = |q: Vec2| contains(q, closed) && !contains(q, holes);
    let entry_angle = (entry.y - c.y).datan2(entry.x - c.x);
    let mut pieces = Vec::new();
    let mut budget = 0usize;
    for &r in cut_radii.iter().rev() {
        let ring = circle(c, r, entry_angle, p.pitch);
        budget += ring.len();
        if budget > MAX_POINTS {
            break;
        }
        let grazes = ring
            .iter()
            .all(|&q| inside(q) || distance_to_edges(q, edges) <= CLIP_TOLERANCE);
        if grazes {
            pieces.push(ring);
        } else {
            pieces.extend(clip(&ring, closed, holes));
        }
    }
    let arcs = order(pieces, Some(entry));
    let end = arcs.last().and_then(|a| a.last().copied()).unwrap_or(entry);
    let seam = (end.y - c.y).datan2(end.x - c.x);
    let mut full = Vec::new();
    for &r in full_radii.iter().rev() {
        let ring = circle(c, r, seam, p.pitch);
        budget += ring.len();
        if budget > MAX_POINTS {
            break;
        }
        full.extend(ring);
    }
    full.dedup_by(|a, b| a.distance(*b) < 1e-9);
    let mut out = Concentric {
        full,
        arcs,
        arcs_first: true,
        joined: true,
    };
    if p.start_inside {
        out.full.reverse();
        out.arcs.reverse();
        out.arcs.iter_mut().for_each(|a| a.reverse());
        out.arcs_first = false;
    }
    out
}

/// One circle of radius `r` about `c` from angle `seam`, counter-clockwise on screen (y down:
/// decreasing angle), closed: 4·2ⁿ points whose chord fits `pitch`, rounded to 0.1 mm, a
/// rounded chord over the pitch split in equal stitches.
fn circle(c: Vec2, r: f64, seam: f64, pitch: f64) -> Vec<Vec2> {
    let chord = |n: usize| 2.0 * r * (std::f64::consts::PI / n as f64).dsin();
    let mut n = MIN_CIRCLE_POINTS;
    while chord(n) > pitch + 1e-9 && n < 1 << 20 {
        n *= 2;
    }
    // A small circle keeps 16 points unless its stitches would be shorter than
    // MIN_CIRCLE_STITCH (PE-DESIGN: 16 at 2.0 and 2.5 mm, 8 at 1.0 mm).
    while n > 4 && chord(n) < MIN_CIRCLE_STITCH {
        n /= 2;
    }
    let keys: Vec<Vec2> = (0..=n)
        .map(|j| {
            let t = seam - TAU * j as f64 / n as f64;
            let q = c + v2(r * t.dcos(), r * t.dsin());
            v2(q.x.round(), q.y.round())
        })
        .collect();
    crate::generate::subdivide(&keys, pitch)
}

/// Distance from `q` to the nearest of `edges`.
fn distance_to_edges(q: Vec2, edges: &[(Vec2, Vec2)]) -> f64 {
    edges
        .iter()
        .map(|&(a, b)| {
            let ab = b - a;
            let len2 = ab.x * ab.x + ab.y * ab.y;
            let t = if len2 > 0.0 {
                (((q.x - a.x) * ab.x + (q.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            a.lerp(b, t).distance(q)
        })
        .fold(f64::INFINITY, f64::min)
}

/// Whether the circle of radius `r` about `c` crosses any of `edges`.
fn crosses(c: Vec2, r: f64, edges: &[(Vec2, Vec2)]) -> bool {
    edges.iter().any(|&(a, b)| {
        let (da, db) = (a.distance(c), b.distance(c));
        if (da - r) * (db - r) <= 0.0 && (da - r).abs() + (db - r).abs() > 0.0 {
            return true;
        }
        if da < r && db < r {
            return false;
        }
        // Both ends outside: the segment may still dip inside the circle.
        let ab = b - a;
        let len2 = ab.x * ab.x + ab.y * ab.y;
        if len2 == 0.0 {
            return false;
        }
        let t = (((c.x - a.x) * ab.x + (c.y - a.y) * ab.y) / len2).clamp(0.0, 1.0);
        a.lerp(b, t).distance(c) < r
    })
}

/// Walking the circle of radius `r` about `c` counter-clockwise on screen from angle `from`,
/// the first angle where it enters or leaves the region (to 0.05°).
fn first_crossing(c: Vec2, r: f64, from: f64, inside: &impl Fn(Vec2) -> bool) -> Option<f64> {
    let at = |t: f64| c + v2(r * t.dcos(), r * t.dsin());
    let steps = 7200;
    let start = inside(at(from));
    let mut prev = from;
    for j in 1..=steps {
        let t = from - TAU * j as f64 / steps as f64;
        if inside(at(t)) != start {
            // Bisect between prev and t.
            let (mut lo, mut hi) = (prev, t);
            for _ in 0..30 {
                let mid = (lo + hi) / 2.0;
                if inside(at(mid)) == start {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some(hi);
        }
        prev = t;
    }
    None
}
