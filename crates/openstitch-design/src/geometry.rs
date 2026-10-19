use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, RangeInclusive, Sub};

/// 2-D point in embroidery units (0.1 mm), +y down.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

pub const fn v2(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        v2(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        v2(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f64) -> Vec2 {
        v2(self.x * k, self.y * k)
    }
}

impl Vec2 {
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    pub fn distance(self, o: Vec2) -> f64 {
        (self - o).length()
    }

    /// Rotation by `radians` about the origin.
    pub fn rotated(self, radians: f64) -> Vec2 {
        let (s, c) = radians.sin_cos();
        v2(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    pub fn lerp(self, o: Vec2, t: f64) -> Vec2 {
        self + (o - self) * t
    }
}

/// Affine map `x' = a·x + c·y + tx`, `y' = b·x + d·y + ty`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    pub fn translation(dx: f64, dy: f64) -> Self {
        Transform {
            tx: dx,
            ty: dy,
            ..Self::IDENTITY
        }
    }

    pub fn scaling(sx: f64, sy: f64, about: Vec2) -> Self {
        Transform {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            tx: about.x - sx * about.x,
            ty: about.y - sy * about.y,
        }
    }

    /// Clockwise on screen (y down) for positive degrees.
    pub fn rotation(degrees: f64, about: Vec2) -> Self {
        let (sn, cs) = degrees.to_radians().sin_cos();
        Transform {
            a: cs,
            b: sn,
            c: -sn,
            d: cs,
            tx: about.x - cs * about.x + sn * about.y,
            ty: about.y - sn * about.x - cs * about.y,
        }
    }

    /// `self` followed by `next`.
    pub fn then(&self, next: &Transform) -> Transform {
        Transform {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            tx: next.a * self.tx + next.c * self.ty + next.tx,
            ty: next.b * self.tx + next.d * self.ty + next.ty,
        }
    }

    pub fn is_finite(&self) -> bool {
        [self.a, self.b, self.c, self.d, self.tx, self.ty]
            .iter()
            .all(|v| v.is_finite())
    }

    pub fn apply(&self, p: Vec2) -> Vec2 {
        v2(
            self.a * p.x + self.c * p.y + self.tx,
            self.b * p.x + self.d * p.y + self.ty,
        )
    }
}

/// Cubic Bézier control points of one contour segment (the segment's end points are the
/// contour vertices on either side).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bezier {
    pub c1: Vec2,
    pub c2: Vec2,
}

/// Maximum distance (0.1 mm units) between a flattened curve and the true curve: 0.05 mm.
pub const FLATNESS: f64 = 0.5;

/// Polyline or polygon whose segments may be cubic Béziers. Closed contours describe regions
/// (even-odd across a shape's contours, so inner contours are holes).
///
/// Segment `i` runs from `points[i]` to `points[i + 1]` (wrapping to `points[0]` for the closing
/// segment of a closed contour). `curves[i]`, when present and `Some`, makes that segment a
/// cubic Bézier; an empty `curves` means every segment is straight.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Contour {
    pub points: Vec<Vec2>,
    pub closed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curves: Vec<Option<Bezier>>,
}

impl Contour {
    pub fn new(points: Vec<Vec2>, closed: bool) -> Self {
        Contour {
            points,
            closed,
            curves: Vec::new(),
        }
    }

    pub fn rectangle(x: f64, y: f64, width: f64, height: f64) -> Self {
        Contour::new(
            vec![
                v2(x, y),
                v2(x + width, y),
                v2(x + width, y + height),
                v2(x, y + height),
            ],
            true,
        )
    }

    pub fn ellipse(center: Vec2, rx: f64, ry: f64, segments: usize) -> Self {
        let n = segments.max(8);
        let points = (0..n)
            .map(|i| {
                let t = i as f64 / n as f64 * std::f64::consts::TAU;
                v2(center.x + rx * t.cos(), center.y + ry * t.sin())
            })
            .collect();
        Contour::new(points, true)
    }

    /// Number of segments: one per vertex pair, plus the closing one when closed.
    pub fn segment_count(&self) -> usize {
        match (self.points.len(), self.closed) {
            (0 | 1, _) => 0,
            (n, true) => n,
            (n, false) => n - 1,
        }
    }

    /// End points of segment `i`.
    pub fn segment_ends(&self, i: usize) -> (Vec2, Vec2) {
        let n = self.points.len();
        (self.points[i], self.points[(i + 1) % n])
    }

    pub fn curve(&self, i: usize) -> Option<Bezier> {
        self.curves.get(i).copied().flatten()
    }

    pub fn has_curves(&self) -> bool {
        self.curves.iter().any(Option::is_some)
    }

    /// Sets (or with `None` clears) the curve of segment `i`, keeping `curves` empty when no
    /// segment is curved.
    pub fn set_curve(&mut self, i: usize, curve: Option<Bezier>) {
        if self.curves.len() < self.segment_count() {
            self.curves.resize(self.segment_count(), None);
        }
        self.curves.truncate(self.segment_count());
        if let Some(slot) = self.curves.get_mut(i) {
            *slot = curve;
        }
        if !self.has_curves() {
            self.curves.clear();
        }
    }

    /// Segment `i` as a polyline from its start to its end vertex (inclusive); curved segments
    /// are flattened to within [`FLATNESS`].
    pub fn segment_polyline(&self, i: usize) -> Vec<Vec2> {
        let (a, b) = self.segment_ends(i);
        match self.curve(i) {
            None => vec![a, b],
            Some(c) => {
                let mut out = vec![a];
                flatten_cubic(a, c.c1, c.c2, b, 0, &mut out);
                out
            }
        }
    }

    /// Points in drawing order with curves flattened, repeating the first at the end when closed.
    pub fn path(&self) -> Vec<Vec2> {
        if !self.has_curves() {
            let mut p = self.points.clone();
            if self.closed && !p.is_empty() {
                p.push(p[0]);
            }
            return p;
        }
        let mut out: Vec<Vec2> = self.points.first().copied().into_iter().collect();
        for i in 0..self.segment_count() {
            out.extend(self.segment_polyline(i).into_iter().skip(1));
        }
        out
    }

    /// Flattened outline points without the closing repeat (for bounds).
    pub fn outline_points(&self) -> Vec<Vec2> {
        let mut p = self.path();
        if self.closed && p.len() > 1 {
            p.pop();
        }
        p
    }

    pub fn edges(&self) -> Vec<(Vec2, Vec2)> {
        self.path().windows(2).map(|w| (w[0], w[1])).collect()
    }

    pub fn transformed(&self, t: &Transform) -> Contour {
        Contour {
            points: self.points.iter().map(|&p| t.apply(p)).collect(),
            closed: self.closed,
            curves: self
                .curves
                .iter()
                .map(|c| {
                    c.map(|c| Bezier {
                        c1: t.apply(c.c1),
                        c2: t.apply(c.c2),
                    })
                })
                .collect(),
        }
    }

    /// Splits segment `i` at parameter `t` (0 < t < 1), inserting a vertex there. A curved
    /// segment is split exactly (de Casteljau) into two curves. Returns the new vertex index.
    pub fn insert_point(&mut self, i: usize, t: f64) -> usize {
        let (a, b) = self.segment_ends(i);
        let n = self.segment_count();
        let mut curves: Vec<Option<Bezier>> = (0..n).map(|k| self.curve(k)).collect();
        let (p, left, right) = match self.curve(i) {
            None => (a.lerp(b, t), None, None),
            Some(c) => {
                let (ab, bc, cd) = (a.lerp(c.c1, t), c.c1.lerp(c.c2, t), c.c2.lerp(b, t));
                let (abc, bcd) = (ab.lerp(bc, t), bc.lerp(cd, t));
                let p = abc.lerp(bcd, t);
                (
                    p,
                    Some(Bezier { c1: ab, c2: abc }),
                    Some(Bezier { c1: bcd, c2: cd }),
                )
            }
        };
        self.points.insert(i + 1, p);
        curves[i] = left;
        curves.insert(i + 1, right);
        self.curves = curves;
        if !self.has_curves() {
            self.curves.clear();
        }
        i + 1
    }

    /// Removes vertex `index`, joining its two neighbouring segments. The joined segment stays a
    /// curve (outer handles kept) if either neighbour was curved.
    pub fn delete_point(&mut self, index: usize) {
        let segs = self.segment_count();
        let mut curves: Vec<Option<Bezier>> = (0..segs).map(|k| self.curve(k)).collect();
        let before = if index > 0 {
            Some(index - 1)
        } else if self.closed {
            Some(segs - 1)
        } else {
            None
        };
        let after = if index < segs { Some(index) } else { None };
        match (before, after) {
            (Some(b), Some(a)) => {
                let (pa, _) = self.segment_ends(b);
                let (_, pb) = self.segment_ends(a);
                let joined = match (curves[b], curves[a]) {
                    (None, None) => None,
                    (cb, ca) => Some(Bezier {
                        c1: cb.map_or(pa, |c| c.c1),
                        c2: ca.map_or(pb, |c| c.c2),
                    }),
                };
                curves[b] = joined;
                curves.remove(a);
            }
            (Some(b), None) => {
                curves.remove(b);
            }
            (None, Some(a)) => {
                curves.remove(a);
            }
            (None, None) => {}
        }
        self.points.remove(index);
        self.curves = curves;
        self.curves.truncate(self.segment_count());
        if !self.has_curves() {
            self.curves.clear();
        }
    }
}

/// Adaptive de Casteljau flattening; pushes points after `a` up to and including `d`.
fn flatten_cubic(a: Vec2, b: Vec2, c: Vec2, d: Vec2, depth: u32, out: &mut Vec<Vec2>) {
    let flat = distance_to_line(b, a, d).max(distance_to_line(c, a, d));
    if depth >= 16 || flat <= FLATNESS {
        out.push(d);
        return;
    }
    let (ab, bc, cd) = (a.lerp(b, 0.5), b.lerp(c, 0.5), c.lerp(d, 0.5));
    let (abc, bcd) = (ab.lerp(bc, 0.5), bc.lerp(cd, 0.5));
    let mid = abc.lerp(bcd, 0.5);
    flatten_cubic(a, ab, abc, mid, depth + 1, out);
    flatten_cubic(mid, bcd, cd, d, depth + 1, out);
}

fn distance_to_line(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let len = ab.length();
    if len == 0.0 {
        return p.distance(a);
    }
    ((p.x - a.x) * ab.y - (p.y - a.y) * ab.x).abs() / len
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Rect {
    pub fn enclosing(points: impl IntoIterator<Item = Vec2>) -> Option<Rect> {
        let mut it = points.into_iter();
        let f = it.next()?;
        Some(it.fold(
            Rect {
                min_x: f.x,
                min_y: f.y,
                max_x: f.x,
                max_y: f.y,
            },
            |r, p| Rect {
                min_x: r.min_x.min(p.x),
                min_y: r.min_y.min(p.y),
                max_x: r.max_x.max(p.x),
                max_y: r.max_y.max(p.y),
            },
        ))
    }
    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }
    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
    pub fn center(&self) -> Vec2 {
        v2(
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }
}

pub type Span = RangeInclusive<f64>;

/// Horizontal scanline through a region after rotating it by −angle.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanRow {
    pub index: usize,
    /// Position in rotated space.
    pub y: f64,
    /// Inside intervals, sorted, even-odd rule.
    pub spans: Vec<Span>,
}

/// Intersects closed `contours` with lines `spacing` apart at `angle_degrees` (0 = horizontal).
/// Rows are in rotated space (map back with [`from_rotated`]); the first sits half a spacing
/// inside the region. `excluding` contours are cut out of every row (hole sewing).
pub fn scan_rows(
    contours: &[Contour],
    angle_degrees: f64,
    spacing: f64,
    excluding: &[Contour],
) -> Vec<ScanRow> {
    if spacing.is_nan() || spacing <= 0.0 {
        return Vec::new();
    }
    let rad = -angle_degrees.to_radians();
    let rotate = |cs: &[Contour]| -> Vec<Contour> {
        cs.iter()
            .filter(|c| c.closed)
            .map(|c| {
                Contour::new(
                    c.outline_points().iter().map(|p| p.rotated(rad)).collect(),
                    true,
                )
            })
            .collect()
    };
    let rotated = rotate(contours);
    let Some(bbox) = Rect::enclosing(rotated.iter().flat_map(|c| c.points.iter().copied())) else {
        return Vec::new();
    };
    if bbox.height().is_nan() || bbox.height() <= 0.0 {
        return Vec::new();
    }
    let edges: Vec<_> = rotated.iter().flat_map(Contour::edges).collect();
    let hole_edges: Vec<_> = rotate(excluding).iter().flat_map(Contour::edges).collect();
    let mut rows = Vec::new();
    let mut y = bbox.min_y + spacing / 2.0;
    let mut index = 0;
    while y < bbox.max_y {
        let mut spans = spans_at(&edges, y);
        if !hole_edges.is_empty() {
            spans = subtract(&spans_at(&hole_edges, y), &spans);
        }
        rows.push(ScanRow { index, y, spans });
        y += spacing;
        index += 1;
    }
    rows
}

pub fn from_rotated(p: Vec2, angle_degrees: f64) -> Vec2 {
    p.rotated(angle_degrees.to_radians())
}

fn spans_at(edges: &[(Vec2, Vec2)], y: f64) -> Vec<Span> {
    let mut xs: Vec<f64> = edges
        .iter()
        .filter(|(p, q)| (p.y <= y && q.y > y) || (q.y <= y && p.y > y))
        .map(|(p, q)| p.x + (y - p.y) * (q.x - p.x) / (q.y - p.y))
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.as_chunks::<2>()
        .0
        .iter()
        .filter(|w| w[1] > w[0])
        .map(|w| w[0]..=w[1])
        .collect()
}

/// `spans` minus `holes` (both sorted, non-overlapping); zero-width leftovers dropped.
pub fn subtract(holes: &[Span], spans: &[Span]) -> Vec<Span> {
    let mut out = Vec::new();
    for s in spans {
        let mut start = *s.start();
        for h in holes {
            if *h.end() <= start || *h.start() >= *s.end() {
                continue;
            }
            if *h.start() > start {
                out.push(start..=*h.start());
            }
            start = start.max(*h.end());
        }
        if start < *s.end() {
            out.push(start..=*s.end());
        }
    }
    out
}

/// Even-odd point-in-region test.
pub fn contains(p: Vec2, contours: &[Contour]) -> bool {
    let mut inside = false;
    for c in contours.iter().filter(|c| c.closed) {
        for (a, b) in c.edges() {
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
                inside = !inside;
            }
        }
    }
    inside
}
