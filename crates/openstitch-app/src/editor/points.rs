//! Point-level geometry for the Point and Pen tools: vertex / handle / segment hit-testing and
//! the contour edits the engine commands make, mirrored here for live previews.

use openstitch_design::{Bezier, Contour, DesignObject, Vec2};

/// Which control point of a segment's cubic Bézier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// `c1`, attached to the segment's start vertex.
    Start,
    /// `c2`, attached to the segment's end vertex.
    End,
}

/// A vertex of one contour of a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointRef {
    pub id: u64,
    pub contour: usize,
    pub index: usize,
}

/// What the pointer is over in point-editing mode, nearest first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointHit {
    Vertex {
        contour: usize,
        index: usize,
    },
    Handle {
        contour: usize,
        segment: usize,
        control: Control,
    },
    Segment {
        contour: usize,
        segment: usize,
        t: f64,
    },
}

/// One visible curve handle: the line from `anchor` (a vertex) to `control`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLine {
    pub segment: usize,
    pub control: Control,
    pub anchor: Vec2,
    pub at: Vec2,
}

/// Point on the cubic Bézier `a, b, c, d` at parameter `t`.
pub fn cubic_at(a: Vec2, b: Vec2, c: Vec2, d: Vec2, t: f64) -> Vec2 {
    let u = 1.0 - t;
    a * (u * u * u) + b * (3.0 * u * u * t) + c * (3.0 * u * t * t) + d * (t * t * t)
}

/// Point on segment `i` at parameter `t` (the Bézier parameter for curves, as
/// `object.insertPoint` takes it).
pub fn segment_at(c: &Contour, i: usize, t: f64) -> Vec2 {
    let (a, b) = c.segment_ends(i);
    match c.curve(i) {
        None => a.lerp(b, t),
        Some(z) => cubic_at(a, z.c1, z.c2, b, t),
    }
}

/// Parameter and distance of the point of segment `i` nearest to `p`.
pub fn nearest_on_segment(c: &Contour, i: usize, p: Vec2) -> (f64, f64) {
    let (a, b) = c.segment_ends(i);
    if c.curve(i).is_none() {
        let ab = b - a;
        let len2 = ab.x * ab.x + ab.y * ab.y;
        let t = if len2 <= 0.0 {
            0.0
        } else {
            (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
        };
        return (t, p.distance(a.lerp(b, t)));
    }
    // Coarse samples, then golden-section search around the best one.
    const SAMPLES: usize = 48;
    let d = |t: f64| p.distance(segment_at(c, i, t));
    let best = (0..=SAMPLES)
        .map(|k| k as f64 / SAMPLES as f64)
        .min_by(|x, y| d(*x).total_cmp(&d(*y)))
        .unwrap_or(0.0);
    let step = 1.0 / SAMPLES as f64;
    let (mut lo, mut hi) = ((best - step).max(0.0), (best + step).min(1.0));
    let g = 0.618_033_988_75;
    for _ in 0..40 {
        let m1 = hi - g * (hi - lo);
        let m2 = lo + g * (hi - lo);
        if d(m1) <= d(m2) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    let t = (lo + hi) / 2.0;
    (t, d(t))
}

/// Nearest segment of `c` to `p`: `(segment, t, distance)`.
pub fn nearest_segment(c: &Contour, p: Vec2) -> Option<(usize, f64, f64)> {
    (0..c.segment_count())
        .map(|i| {
            let (t, d) = nearest_on_segment(c, i, p);
            (i, t, d)
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
}

/// The curve handles of `c`, two per curved segment.
pub fn handle_lines(c: &Contour) -> Vec<HandleLine> {
    let mut out = Vec::new();
    for i in 0..c.segment_count() {
        if let Some(z) = c.curve(i) {
            let (a, b) = c.segment_ends(i);
            out.push(HandleLine {
                segment: i,
                control: Control::Start,
                anchor: a,
                at: z.c1,
            });
            out.push(HandleLine {
                segment: i,
                control: Control::End,
                anchor: b,
                at: z.c2,
            });
        }
    }
    out
}

/// What of `o` lies under `p`: a vertex or handle within `radius`, the nearer winning (a vertex
/// on a tie), else a segment within `tolerance`.
pub fn point_at(o: &DesignObject, p: Vec2, radius: f64, tolerance: f64) -> Option<PointHit> {
    let mut best: Option<(f64, PointHit)> = None;
    let mut offer = |d: f64, hit: PointHit| {
        if d <= radius && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, hit));
        }
    };
    for (ci, c) in o.contours.iter().enumerate() {
        for (index, &v) in c.points.iter().enumerate() {
            offer(v.distance(p), PointHit::Vertex { contour: ci, index });
        }
    }
    for (ci, c) in o.contours.iter().enumerate() {
        for h in handle_lines(c) {
            offer(
                h.at.distance(p),
                PointHit::Handle {
                    contour: ci,
                    segment: h.segment,
                    control: h.control,
                },
            );
        }
    }
    if let Some((_, hit)) = best {
        return Some(hit);
    }
    o.contours
        .iter()
        .enumerate()
        .filter_map(|(ci, c)| nearest_segment(c, p).map(|(s, t, d)| (ci, s, t, d)))
        .filter(|&(.., d)| d <= tolerance)
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(contour, segment, t, _)| PointHit::Segment {
            contour,
            segment,
            t,
        })
}

/// `c` with vertex `index` moved to `to`, its handles following (as `object.setPoint` does).
pub fn with_vertex(c: &Contour, index: usize, to: Vec2) -> Contour {
    let mut c = c.clone();
    let delta = to - c.points[index];
    c.points[index] = to;
    let segs = c.segment_count();
    let incoming = if index > 0 {
        Some(index - 1)
    } else if c.closed && segs > 0 {
        Some(segs - 1)
    } else {
        None
    };
    if let Some(mut z) = (index < segs).then(|| c.curve(index)).flatten() {
        z.c1 = z.c1 + delta;
        c.set_curve(index, Some(z));
    }
    if let Some(i) = incoming
        && let Some(mut z) = c.curve(i)
    {
        z.c2 = z.c2 + delta;
        c.set_curve(i, Some(z));
    }
    c
}

/// `c` with one control point of curved segment `segment` moved to `to`.
pub fn with_control(c: &Contour, segment: usize, control: Control, to: Vec2) -> Contour {
    let mut c = c.clone();
    if let Some(mut z) = c.curve(segment) {
        match control {
            Control::Start => z.c1 = to,
            Control::End => z.c2 = to,
        }
        c.set_curve(segment, Some(z));
    }
    c
}

/// Handles that turn straight segment `i` into a smooth curve: Catmull-Rom tangents through
/// the neighbouring vertices (the chord at an open path's ends).
pub fn smooth_curve(c: &Contour, i: usize) -> Bezier {
    let n = c.points.len();
    let (a, b) = c.segment_ends(i);
    let j = (i + 1) % n;
    let prev = if i > 0 || c.closed {
        Some(c.points[(i + n - 1) % n])
    } else {
        None
    };
    let next = if j + 1 < n || c.closed {
        Some(c.points[(j + 1) % n])
    } else {
        None
    };
    // Tangents are only borrowed from a neighbour that is not the segment's other end.
    let ta = prev.filter(|_| n > 2).map_or(b - a, |p| (b - p) * 0.5);
    let tb = next.filter(|_| n > 2).map_or(b - a, |q| (q - a) * 0.5);
    Bezier {
        c1: a + ta * (1.0 / 3.0),
        c2: b - tb * (1.0 / 3.0),
    }
}
