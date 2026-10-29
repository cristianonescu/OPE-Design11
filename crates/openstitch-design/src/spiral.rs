//! Spiral Stitch as PE-DESIGN 11.50 sews it (issue #138; references
//! `compat/reference/stitches/spiral-*.pes`): one continuous running-stitch path that starts
//! on the outline at the start point (R1), follows the outline round, and moves one row
//! (1 / density) inwards per turn until the middle. On a 40 × 30 mm rectangle the turns are
//! rectangles 0.5 mm apart on both axes (density 2.0): the turns are the outline offset
//! inward by a constant distance, not copies shrunk towards a centre.
//!
//! Turn `k` (k = 0, 1, …) is the region offset inward by `k·d`; turn 0 is the outline itself.
//! Holes (the object's own and paired hole sewing) grow by the same distance, so a ring gets a
//! spiral round its hole too. An inset can split (a U shape becomes two arms), so rings are
//! linked into *chains*: each ring continues the ring of the previous level that encloses it
//! (an outline ring continues an outline ring, a hole ring the nearest hole ring); where a
//! ring has several children the longest continues the chain and the others start their own.
//! Each chain is one spiral; chains are sewn one after another, nearest end first.
//!
//! A turn walks ring `k+1` from its point nearest the turn's start and stands off it towards
//! ring `k` by a share that falls linearly from 1 to 0: `S(s) = (1 − s)·n_k(q) + s·q` with
//! `q = ring_{k+1}(s)` and `n_k(q)` its nearest point on ring `k`. The rings are offsets a row
//! apart, so each turn stays in the band between its two rings and the turns never cross.
//!
//! Needle points per turn follow PE-DESIGN's sampling: a smooth turn (a circle) gets
//! 4·2ⁿ equally spaced points, the fewest whose chord fits the run pitch, as PE-DESIGN halves
//! a Bézier quarter (64 a turn on a 40 mm circle down to r ≈ 10 mm, then 32, 16, 8); a turn
//! with corners keeps its corners and splits each side into equal stitches. The second Start
//! Point button sews the same path backwards, from the middle out.
//!
//! Lengths are 0.1 mm; output is deterministic.

use crate::boolean::{self, BoolOp};
use crate::geometry::{Contour, Vec2, contains};
use crate::paths::Walker;

/// Most rings (levels of offset) per region.
const MAX_LEVELS: usize = 4_000;
/// Most needle points per region (as for every other generator).
const MAX_POINTS: usize = 2_000_000;
/// A turn sharper than this (degrees) is a corner the stitches keep.
const CORNER_DEGREES: f64 = 30.0;
/// How far (0.1 mm) a stitch may run over the pitch before it is split.
const SPLIT_SLACK: f64 = 0.5;
/// A spiral closes on its innermost ring only when that ring is at least this many stitches
/// round (on a ring of a few stitches the chords would cut across the turn before).
const MIN_CLOSING_STITCHES: f64 = 4.0;

/// What the generator needs from the region's attributes.
#[derive(Clone, Copy, Debug)]
pub struct SpiralParams {
    /// Distance between turns, 0.1 mm (1 / density).
    pub spacing: f64,
    /// Running stitch length, 0.1 mm.
    pub pitch: f64,
    /// Wind against the outline's own direction.
    pub reverse_winding: bool,
    /// The second Start Point button: the same path from the middle out.
    pub start_inside: bool,
    /// Where the needle comes in (R1); the spiral starts there.
    pub entry: Option<Vec2>,
}

/// The spiral fill of `closed` minus `holes`: one needle-point path per chain, in sewing
/// order, no stitch longer than `p.pitch`. The caller joins them (travel inside the region,
/// or jumps).
pub fn spiral(closed: &[Contour], holes: &[Contour], p: &SpiralParams) -> Vec<Vec<Vec2>> {
    if !(p.spacing > 0.0 && p.pitch > 0.0) {
        return Vec::new();
    }
    let Some(outer) = closed.iter().filter(|c| c.closed).max_by(|a, b| {
        signed_area(&a.outline_points())
            .abs()
            .total_cmp(&signed_area(&b.outline_points()).abs())
    }) else {
        return Vec::new();
    };
    // The outline's own direction (positive signed area: clockwise on screen, y down).
    let clockwise = (signed_area(&outer.outline_points()) > 0.0) != p.reverse_winding;
    let base: Vec<Contour> = if holes.is_empty() {
        closed.to_vec()
    } else {
        boolean::contours(&boolean::combine(closed, holes, BoolOp::Difference))
    };
    let rings = Rings::build(&base, p, clockwise);
    let chains = rings.chains();
    let mut pieces = sew(&rings, &chains, p);
    if p.start_inside {
        pieces.reverse();
        pieces.iter_mut().for_each(|piece| piece.reverse());
    }
    pieces
}

/// One ring: a closed loop (no repeated end point), oriented the way the spiral turns.
struct Ring {
    level: usize,
    hole: bool,
    points: Vec<Vec2>,
    length: f64,
}

/// A region of one level: its contours (for containment tests) and its rings.
type LevelRegion = (Vec<Contour>, Vec<usize>);

/// Every ring, level by level, and the link from each ring to the ring it continues.
struct Rings {
    rings: Vec<Ring>,
    parent: Vec<Option<usize>>,
}

impl Rings {
    fn build(base: &[Contour], p: &SpiralParams, clockwise: bool) -> Rings {
        let mut rings: Vec<Ring> = Vec::new();
        let mut parent: Vec<Option<usize>> = Vec::new();
        let mut previous: Vec<LevelRegion> = Vec::new();
        let mut budget = 0.0;
        for level in 0..MAX_LEVELS {
            let regions = if level == 0 {
                // The outline itself, cleaned (self-overlaps resolved, holes as holes).
                boolean::combine(base, &[], BoolOp::Union)
            } else {
                boolean::offset(base, -(level as f64) * p.spacing, true)
            };
            if regions.is_empty() {
                break;
            }
            let mut current = Vec::new();
            for region in &regions {
                let mut members = Vec::new();
                let mut outlines = Vec::new();
                for (ci, c) in region.iter().enumerate() {
                    let mut points = c.outline_points();
                    let length = closed_length(&points);
                    outlines.push(Contour::new(points.clone(), true));
                    // A loop shorter than a stitch is a sliver at the middle of the shape.
                    if points.len() < 3 || length < p.pitch {
                        continue;
                    }
                    if (signed_area(&points) > 0.0) != clockwise {
                        points.reverse();
                    }
                    let hole = ci > 0;
                    let up = parent_ring(&rings, &previous, &points, hole);
                    members.push(rings.len());
                    parent.push(up);
                    rings.push(Ring {
                        level,
                        hole,
                        points,
                        length,
                    });
                    budget += length / p.pitch;
                }
                current.push((outlines, members));
            }
            previous = current;
            if budget > MAX_POINTS as f64 {
                break;
            }
        }
        Rings { rings, parent }
    }

    /// Chains of rings from the outline (or a hole) towards the middle: a ring continues its
    /// parent's chain when it is the parent's longest child, else starts a new chain.
    fn chains(&self) -> Vec<Vec<usize>> {
        let n = self.rings.len();
        let mut heir: Vec<Option<usize>> = vec![None; n];
        for (i, up) in self.parent.iter().enumerate() {
            if let Some(u) = *up
                && heir[u].is_none_or(|h| self.rings[h].length < self.rings[i].length)
            {
                heir[u] = Some(i);
            }
        }
        let mut chain_of: Vec<Option<usize>> = vec![None; n];
        let mut chains: Vec<Vec<usize>> = Vec::new();
        // Rings are stored level by level, so a parent always comes first.
        for i in 0..n {
            let continues = self.parent[i].filter(|&u| heir[u] == Some(i));
            match continues.and_then(|u| chain_of[u]) {
                Some(c) => {
                    chains[c].push(i);
                    chain_of[i] = Some(c);
                }
                None => {
                    chain_of[i] = Some(chains.len());
                    chains.push(vec![i]);
                }
            }
        }
        chains
    }
}

/// The ring of the previous level that a new ring continues: the enclosing region's outline
/// for an outline ring, its nearest hole ring for a hole ring.
fn parent_ring(
    rings: &[Ring],
    previous: &[LevelRegion],
    points: &[Vec2],
    hole: bool,
) -> Option<usize> {
    let probe = points[0];
    let (_, members) = previous
        .iter()
        .find(|(contours, _)| contains(probe, contours))
        .or_else(|| {
            // A hole ring lies inside its region's grown hole, outside the region's area:
            // look for the region whose outline encloses it instead.
            previous
                .iter()
                .find(|(contours, _)| contains(probe, &contours[..1.min(contours.len())]))
        })?;
    let mut candidates = members.iter().copied().filter(|&m| rings[m].hole == hole);
    if !hole {
        return candidates.next();
    }
    candidates.min_by(|&a, &b| {
        distance_to(&rings[a].points, probe).total_cmp(&distance_to(&rings[b].points, probe))
    })
}

/// Sews the chains: the first from the start point (or the longest outline), then the
/// unsewn chain with an end nearest the needle, from that end.
fn sew(rings: &Rings, chains: &[Vec<usize>], p: &SpiralParams) -> Vec<Vec<Vec2>> {
    let mut pieces = Vec::new();
    if chains.is_empty() {
        return pieces;
    }
    let ring = |c: usize, last: bool| {
        let chain = &chains[c];
        &rings.rings[if last {
            chain[chain.len() - 1]
        } else {
            chain[0]
        }]
    };
    let first = match p.entry {
        Some(e) => (0..chains.len())
            .min_by(|&a, &b| {
                let d = |c: usize| distance_to(&ring(c, false).points, e);
                d(a).total_cmp(&d(b))
            })
            .unwrap_or(0),
        None => (0..chains.len())
            .filter(|&c| ring(c, false).level == 0)
            .max_by(|&a, &b| ring(a, false).length.total_cmp(&ring(b, false).length))
            .unwrap_or(0),
    };
    let mut pos = p
        .entry
        .unwrap_or_else(|| lowest(&ring(first, false).points));
    let mut done = vec![false; chains.len()];
    let mut next = Some((first, false));
    while let Some((c, reversed)) = next {
        done[c] = true;
        let mut order: Vec<&Ring> = chains[c].iter().map(|&i| &rings.rings[i]).collect();
        if reversed {
            order.reverse();
        }
        let path = chain_path(&order, pos, p.pitch);
        if let Some(&end) = path.last() {
            pos = end;
        }
        if path.len() >= 2 {
            pieces.push(path);
        }
        next = (0..chains.len())
            .filter(|&k| !done[k])
            .flat_map(|k| {
                [
                    (k, false, distance_to(&ring(k, false).points, pos)),
                    (k, true, distance_to(&ring(k, true).points, pos)),
                ]
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(k, rev, _)| (k, rev));
    }
    pieces
}

/// One chain as a spiral: each turn blends ring `k` into ring `k+1`; the last ring closes
/// the path when it is long enough. No stitch is longer than `pitch`.
fn chain_path(order: &[&Ring], pos: Vec2, pitch: f64) -> Vec<Vec2> {
    let Some(first) = order.first() else {
        return Vec::new();
    };
    let closed = closed_polyline(&first.points);
    let mut a = rotate(&Walker::new(&closed), nearest_arc(&closed, pos));
    let mut path: Vec<Vec2> = vec![a[0]];
    for ring in &order[1..] {
        let closed = closed_polyline(&ring.points);
        let b = rotate(&Walker::new(&closed), nearest_arc(&closed, a[0]));
        let (ca, cb) = (corners(&a), corners(&b));
        if !ca.is_empty() && ca.len() == cb.len() {
            path.extend(corner_turn(&a, &b, &ca, &cb, pitch));
        } else {
            let lb = Walker::new(&b).length();
            let count = turn_points(Walker::new(&a).length(), pitch);
            for (s, q) in samples(&b, pitch, count) {
                if s > 0.0 {
                    path.push(nearest_point(&a, q).lerp(q, s / lb));
                }
            }
        }
        a = b;
    }
    if Walker::new(&a).length() >= MIN_CLOSING_STITCHES * pitch {
        let count = turn_points(Walker::new(&a).length(), pitch);
        path.extend(
            samples(&a, pitch, count)
                .into_iter()
                .skip(1)
                .map(|(_, q)| q),
        );
    }
    split_long(&path, pitch)
}

/// A turn between two rings with the same corners (a rectangle and its inset): side by side,
/// corner to corner, each side in equal stitches, standing off ring `b` towards ring `a` by a
/// share that falls from 1 to 0 over the turn. Corners stay corners.
fn corner_turn(a: &[Vec2], b: &[Vec2], ca: &[f64], cb: &[f64], pitch: f64) -> Vec<Vec2> {
    let (wa, wb) = (Walker::new(a), Walker::new(b));
    let (la, lb) = (wa.length(), wb.length());
    let bounds = |c: &[f64], len: f64| {
        let mut v = vec![0.0];
        v.extend_from_slice(c);
        v.push(len);
        v
    };
    let (sa, sb) = (bounds(ca, la), bounds(cb, lb));
    let mut out = Vec::new();
    for j in 0..sa.len() - 1 {
        let (a0, a1, b0, b1) = (sa[j], sa[j + 1], sb[j], sb[j + 1]);
        let m = (((a1 - a0).max(b1 - b0) / pitch).ceil() as usize).max(1);
        for k in 1..=m {
            let t = k as f64 / m as f64;
            let sbk = b0 + (b1 - b0) * t;
            out.push(wa.at(a0 + (a1 - a0) * t).lerp(wb.at(sbk), sbk / lb));
        }
    }
    out
}

/// Arc lengths of the corners of the closed polyline `poly` (start and end excluded).
fn corners(poly: &[Vec2]) -> Vec<f64> {
    let limit = CORNER_DEGREES.to_radians().cos();
    let mut out = Vec::new();
    let mut s = 0.0;
    for i in 1..poly.len().saturating_sub(1) {
        s += poly[i - 1].distance(poly[i]);
        let (u, v) = (poly[i] - poly[i - 1], poly[i + 1] - poly[i]);
        let (lu, lv) = (u.length(), v.length());
        if lu > 1e-9 && lv > 1e-9 && (u.x * v.x + u.y * v.y) / (lu * lv) < limit {
            out.push(s);
        }
    }
    out
}

/// `path` with every stitch more than [`SPLIT_SLACK`] over `pitch` split into equal stitches
/// no longer than `pitch`: a blended turn may come out a hair over the pitch (PE-DESIGN's own
/// longest spiral stitch at a 2 mm pitch is 2.04 mm), and halving such a stitch would double
/// the needle points.
fn split_long(path: &[Vec2], pitch: f64) -> Vec<Vec2> {
    let Some(&first) = path.first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    for w in path.windows(2) {
        let len = w[0].distance(w[1]);
        let n = if len > pitch + SPLIT_SLACK {
            (len / pitch).ceil() as usize
        } else {
            1
        };
        out.extend((1..=n).map(|k| w[0].lerp(w[1], k as f64 / n as f64)));
    }
    out
}

/// Needle points on a smooth turn of length `len`: 4·2ⁿ, the fewest whose chord fits the
/// pitch (PE-DESIGN halves a Bézier quarter until the chord fits).
fn turn_points(len: f64, pitch: f64) -> usize {
    let mut n = 4usize;
    while len / n as f64 > pitch && n < 1 << 20 {
        n *= 2;
    }
    n
}

/// Needle points along the closed polyline `poly` (which starts and ends at the same point),
/// with their arc lengths: a polyline without corners in `count` equal steps; one with
/// corners keeps them and splits each side into equal stitches no longer than `pitch`.
fn samples(poly: &[Vec2], pitch: f64, count: usize) -> Vec<(f64, Vec2)> {
    let w = Walker::new(poly);
    let len = w.length();
    let limit = CORNER_DEGREES.to_radians().cos();
    let n = poly.len();
    let corner = |i: usize| {
        // The turn at vertex i (the closing vertex wraps to the start).
        let prev = if i == 0 { poly[n - 2] } else { poly[i - 1] };
        let next = if i + 1 >= n { poly[1] } else { poly[i + 1] };
        let (u, v) = (poly[i] - prev, next - poly[i]);
        let (lu, lv) = (u.length(), v.length());
        lu > 1e-9 && lv > 1e-9 && (u.x * v.x + u.y * v.y) / (lu * lv) < limit
    };
    let mut cum = vec![0.0];
    for win in poly.windows(2) {
        cum.push(cum[cum.len() - 1] + win[0].distance(win[1]));
    }
    let mut stops: Vec<f64> = (1..n - 1).filter(|&i| corner(i)).map(|i| cum[i]).collect();
    if stops.is_empty() {
        return (0..=count)
            .map(|k| {
                let s = len * k as f64 / count as f64;
                (s, w.at(s))
            })
            .collect();
    }
    stops.push(len);
    let mut out = vec![(0.0, poly[0])];
    let mut from = 0.0;
    for to in stops {
        let m = (((to - from) / pitch).ceil() as usize).max(1);
        for k in 1..=m {
            let s = from + (to - from) * k as f64 / m as f64;
            out.push((s, w.at(s)));
        }
        from = to;
    }
    out
}

/// `points` closed: the first point repeated at the end.
fn closed_polyline(points: &[Vec2]) -> Vec<Vec2> {
    let mut c = points.to_vec();
    if let Some(&f) = points.first() {
        c.push(f);
    }
    c
}

/// The closed polyline walked by `w`, starting and ending at arc length `s`.
fn rotate(w: &Walker, s: f64) -> Vec<Vec2> {
    let mut out = w.slice(s, w.length());
    out.extend(w.slice(0.0, s).into_iter().skip(1));
    out
}

/// Arc length along `poly` of its point nearest `p`.
fn nearest_arc(poly: &[Vec2], p: Vec2) -> f64 {
    let (mut best, mut best_s, mut s) = (f64::INFINITY, 0.0, 0.0);
    for w in poly.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.distance(b);
        let t = if len > 0.0 {
            let ab = b - a;
            (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / (len * len)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d = a.lerp(b, t).distance(p);
        if d < best {
            (best, best_s) = (d, s + t * len);
        }
        s += len;
    }
    best_s
}

/// The point of the polyline `poly` nearest `p`.
fn nearest_point(poly: &[Vec2], p: Vec2) -> Vec2 {
    let mut best = (f64::INFINITY, p);
    for w in poly.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ab = b - a;
        let len2 = ab.x * ab.x + ab.y * ab.y;
        let t = if len2 > 0.0 {
            (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = a.lerp(b, t);
        let d = q.distance(p);
        if d < best.0 {
            best = (d, q);
        }
    }
    best.1
}

/// Smallest distance from `p` to the vertices of `points` (rings are densely sampled).
fn distance_to(points: &[Vec2], p: Vec2) -> f64 {
    points
        .iter()
        .map(|q| q.distance(p))
        .fold(f64::INFINITY, f64::min)
}

/// The lowest point on screen (largest y), leftmost among equals: where the spiral starts
/// when no start point is set, as the shape tools put R1 at the bottom.
fn lowest(points: &[Vec2]) -> Vec2 {
    points
        .iter()
        .copied()
        .max_by(|a, b| a.y.total_cmp(&b.y).then(b.x.total_cmp(&a.x)))
        .unwrap_or_default()
}

fn closed_length(points: &[Vec2]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| points[i].distance(points[(i + 1) % n]))
        .sum()
}

/// Shoelace signed area of a closed loop (positive: clockwise on screen, y down).
fn signed_area(points: &[Vec2]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}
