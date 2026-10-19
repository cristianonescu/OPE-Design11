//! Ornamental line patterns: the floral pattern grown inside a closed shape (PE-DESIGN's
//! "Creating a floral pattern") and decorative frames (an original stand-in for PE-DESIGN's
//! decorative pattern library, which is Brother's content). Independent designs: petal curves
//! are polar profiles scaled to the outline; frames are built from arcs, waves and teeth.
//! Lengths are 0.1 mm.

use crate::geometry::{Bezier, Contour, Rect, Vec2, v2};
use std::f64::consts::{PI, TAU};

/// Floral pattern variants: 9 petal counts × 4 petal profiles × 7 twists.
pub const FLORAL_PATTERNS: u32 = 252;

/// Densities (number of nested petal rings) 1–10.
pub const FLORAL_MAX_DENSITY: u32 = 10;

/// Needle-point spacing of generated lines.
const LINE_STEP: f64 = 20.0;

/// Area centroid of a closed polygon (bounding-box centre when degenerate).
fn centroid(p: &[Vec2]) -> Vec2 {
    let n = p.len();
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (u, w) = (p[i], p[(i + 1) % n]);
        let f = u.x * w.y - w.x * u.y;
        a += f;
        cx += (u.x + w.x) * f;
        cy += (u.y + w.y) * f;
    }
    if a.abs() < 1e-9 {
        return Rect::enclosing(p.iter().copied())
            .map(|b| b.center())
            .unwrap_or_default();
    }
    v2(cx / (3.0 * a), cy / (3.0 * a))
}

/// Distance from `c` along direction `d` (unit) to the nearest crossing of the closed
/// polygon `p`, if any.
fn ray_hit(p: &[Vec2], c: Vec2, d: Vec2) -> Option<f64> {
    let n = p.len();
    let mut best: Option<f64> = None;
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        let e = b - a;
        let den = d.x * e.y - d.y * e.x;
        if den.abs() < 1e-12 {
            continue;
        }
        let w = a - c;
        let t = (w.x * e.y - w.y * e.x) / den;
        let u = (w.x * d.y - w.y * d.x) / den;
        if t > 1e-9 && (0.0..=1.0).contains(&u) && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
}

/// Closed polyline resampled to points about `step` apart (the first point kept).
fn resample_closed(p: &[Vec2], step: f64) -> Vec<Vec2> {
    let n = p.len();
    if n < 3 {
        return p.to_vec();
    }
    let total: f64 = (0..n).map(|i| p[i].distance(p[(i + 1) % n])).sum();
    let count = ((total / step).round() as usize).max(8);
    let seg = total / count as f64;
    let mut out = Vec::with_capacity(count);
    let (mut i, mut walked) = (0, 0.0);
    for k in 0..count {
        let target = seg * k as f64;
        loop {
            let len = p[i].distance(p[(i + 1) % n]);
            if walked + len >= target || i == n - 1 {
                let t = if len > 0.0 {
                    ((target - walked) / len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                out.push(p[i].lerp(p[(i + 1) % n], t));
                break;
            }
            walked += len;
            i += 1;
        }
    }
    out
}

/// Petal profile in 0–1 for angle parameter `u` (one petal per π).
fn petal(style: u32, u: f64) -> f64 {
    let c = u.cos().abs();
    match style {
        0 => c,
        1 => c.sqrt(),
        2 => (c + 0.35 * (3.0 * u).cos().abs()) / 1.35,
        _ => c * c,
    }
}

/// The floral pattern for a closed `outline`: `density` nested rings of petals whose tips
/// touch the outline (outermost ring) and whose count, shape and twist come from `pattern`
/// (0..[`FLORAL_PATTERNS`]). Rays from the centroid that miss the outline use the nearest
/// outline distance. Returns closed contours, outermost first.
pub fn floral(outline: &Contour, density: u32, pattern: u32) -> Vec<Contour> {
    let poly = outline.outline_points();
    if poly.len() < 3 {
        return Vec::new();
    }
    let density = density.clamp(1, FLORAL_MAX_DENSITY);
    let pattern = pattern % FLORAL_PATTERNS;
    let petals = 3 + pattern % 9;
    let style = (pattern / 9) % 4;
    let twist = f64::from((pattern / 36) % 7) / 7.0;
    let c = centroid(&poly);
    let fallback = poly
        .iter()
        .map(|p| p.distance(c))
        .fold(f64::INFINITY, f64::min);
    const SAMPLES: usize = 1440;
    let reach: Vec<f64> = (0..SAMPLES)
        .map(|k| {
            let a = k as f64 / SAMPLES as f64 * TAU;
            ray_hit(&poly, c, v2(a.cos(), a.sin())).unwrap_or(fallback)
        })
        .collect();
    const INNER: f64 = 0.3;
    (0..density)
        .map(|j| {
            let scale = 1.0 - f64::from(j) / f64::from(density + 1) * 0.85;
            let phase = f64::from(j) * twist * PI / f64::from(petals);
            // Start in a petal valley (u = π/2), so the stitches joining one ring to the next
            // are short and sit near the centre.
            let valley = ((PI / 2.0 - phase) * 2.0 / f64::from(petals)).rem_euclid(TAU);
            let k0 = (valley / TAU * SAMPLES as f64).round() as usize;
            let pts: Vec<Vec2> = (k0..k0 + SAMPLES)
                .map(|k| k % SAMPLES)
                .map(|k| {
                    let a = k as f64 / SAMPLES as f64 * TAU;
                    let u = a * f64::from(petals) / 2.0 + phase;
                    let rho = scale * (INNER + (1.0 - INNER) * petal(style, u));
                    c + v2(a.cos(), a.sin()) * (reach[k] * rho)
                })
                .collect();
            Contour::new(resample_closed(&pts, LINE_STEP), true)
        })
        .collect()
}

/// Decorative frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// Rectangle edged with outward semicircles.
    Scallop,
    /// Rectangle whose edges are waves.
    Wave,
    /// Rectangle edged with triangular teeth.
    Zigzag,
    /// Ellipse edged with round bumps.
    ScallopOval,
    /// Four corner scrolls (open curves).
    Corners,
}

impl Frame {
    pub const ALL: [Frame; 5] = [
        Frame::Scallop,
        Frame::Wave,
        Frame::Zigzag,
        Frame::ScallopOval,
        Frame::Corners,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Frame::Scallop => "scallop",
            Frame::Wave => "wave",
            Frame::Zigzag => "zigzag",
            Frame::ScallopOval => "scallopOval",
            Frame::Corners => "corners",
        }
    }

    pub fn from_id(s: &str) -> Option<Frame> {
        Frame::ALL.into_iter().find(|f| f.id() == s)
    }
}

/// `n` equal steps along the rectangle's perimeter, clockwise from the top-left corner; each
/// step is (start, end, outward normal).
fn edge_steps(b: &Rect, size: f64) -> Vec<(Vec2, Vec2, Vec2)> {
    let corners = [
        (v2(b.min_x, b.min_y), v2(b.max_x, b.min_y), v2(0.0, -1.0)),
        (v2(b.max_x, b.min_y), v2(b.max_x, b.max_y), v2(1.0, 0.0)),
        (v2(b.max_x, b.max_y), v2(b.min_x, b.max_y), v2(0.0, 1.0)),
        (v2(b.min_x, b.max_y), v2(b.min_x, b.min_y), v2(-1.0, 0.0)),
    ];
    let mut out = Vec::new();
    for (a, z, n) in corners {
        let k = ((a.distance(z) / size).round() as usize).max(1);
        for i in 0..k {
            out.push((
                a.lerp(z, i as f64 / k as f64),
                a.lerp(z, (i + 1) as f64 / k as f64),
                n,
            ));
        }
    }
    out
}

/// A decorative frame in box `b` (0.1 mm) whose ornaments are about `size` long.
pub fn frame(kind: Frame, b: &Rect, size: f64) -> Vec<Contour> {
    let size = size.max(10.0);
    let k = 4.0 / 3.0;
    match kind {
        Frame::Scallop | Frame::Wave | Frame::Zigzag => {
            let steps = edge_steps(b, size);
            let mut points = Vec::new();
            let mut curves = Vec::new();
            for (i, &(a, z, n)) in steps.iter().enumerate() {
                let h = a.distance(z) / 2.0;
                match kind {
                    Frame::Scallop => {
                        points.push(a);
                        curves.push(Some(Bezier {
                            c1: a + n * (h * k),
                            c2: z + n * (h * k),
                        }));
                    }
                    Frame::Wave => {
                        // One wave per step, alternating out and in.
                        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                        points.push(a);
                        curves.push(Some(Bezier {
                            c1: a.lerp(z, 1.0 / 3.0) + n * (side * h * 0.8),
                            c2: a.lerp(z, 2.0 / 3.0) + n * (side * h * 0.8),
                        }));
                    }
                    _ => {
                        points.push(a);
                        curves.push(None);
                        points.push(a.lerp(z, 0.5) + n * h);
                        curves.push(None);
                    }
                }
            }
            let mut c = Contour::new(points, true);
            for (i, curve) in curves.into_iter().enumerate() {
                c.set_curve(i, curve);
            }
            vec![c]
        }
        Frame::ScallopOval => {
            let (rx, ry) = (b.width() / 2.0, b.height() / 2.0);
            let perimeter = PI * (3.0 * (rx + ry) - ((3.0 * rx + ry) * (rx + 3.0 * ry)).sqrt());
            let n = ((perimeter / size).round() as usize).max(6);
            let centre = b.center();
            let on = |t: f64| centre + v2(rx * t.cos(), ry * t.sin());
            let outward = |t: f64| {
                let v = v2(ry * t.cos(), rx * t.sin());
                v * (1.0 / v.length().max(1e-9))
            };
            let mut c = Contour::new(
                (0..n).map(|i| on(i as f64 / n as f64 * TAU)).collect(),
                true,
            );
            for i in 0..n {
                let (t0, t1) = (i as f64 / n as f64 * TAU, (i + 1) as f64 / n as f64 * TAU);
                let h = on(t0).distance(on(t1)) / 2.0;
                c.set_curve(
                    i,
                    Some(Bezier {
                        c1: on(t0) + outward(t0) * (h * k),
                        c2: on(t1) + outward(t1) * (h * k),
                    }),
                );
            }
            vec![c]
        }
        Frame::Corners => {
            let s = size.min(b.width() / 2.0).min(b.height() / 2.0);
            let corners = [
                (v2(b.min_x, b.min_y), v2(1.0, 1.0)),
                (v2(b.max_x, b.min_y), v2(-1.0, 1.0)),
                (v2(b.max_x, b.max_y), v2(-1.0, -1.0)),
                (v2(b.min_x, b.max_y), v2(1.0, -1.0)),
            ];
            corners
                .into_iter()
                .map(|(p, d)| {
                    // An L along both edges ending in an inward curl.
                    let along_x = p + v2(d.x * s * 2.0, 0.0);
                    let along_y = p + v2(0.0, d.y * s * 2.0);
                    let curl = p + v2(d.x * s * 0.6, d.y * s * 0.6);
                    let mut c = Contour::new(vec![along_x, p, along_y, curl], false);
                    c.set_curve(
                        2,
                        Some(Bezier {
                            c1: along_y + v2(d.x * s * 0.9, 0.0),
                            c2: curl + v2(0.0, d.y * s * 0.6),
                        }),
                    );
                    c
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::contains;

    fn bounds(cs: &[Contour]) -> Rect {
        Rect::enclosing(cs.iter().flat_map(|c| c.outline_points())).expect("bounds")
    }

    #[test]
    fn floral_rings_stay_inside_and_touch_the_outline() {
        let square = Contour::rectangle(0.0, 0.0, 400.0, 400.0);
        let rings = floral(&square, 3, 0);
        assert_eq!(rings.len(), 3);
        let region = [square.clone()];
        for ring in &rings {
            assert!(ring.closed);
            assert!(
                ring.points
                    .iter()
                    .all(|p| { p.x >= -0.01 && p.x <= 400.01 && p.y >= -0.01 && p.y <= 400.01 })
            );
            assert!(ring.points.windows(2).all(|w| w[0].distance(w[1]) < 25.0));
        }
        // Outer petal tips reach the outline; inner rings are smaller.
        let b0 = bounds(&rings[..1]);
        // Three petals: the tips reach three sides of the square.
        assert!(b0.width() > 350.0 && b0.height() > 390.0, "{b0:?}");
        assert!(bounds(&rings[2..]).width() < b0.width() * 0.8);
        assert!(contains(v2(200.0, 200.0), &region));
        // Rings start in a valley, close to each other.
        for w in rings.windows(2) {
            assert!(w[0].points[0].distance(w[1].points[0]) < 40.0);
        }
    }

    #[test]
    fn floral_patterns_differ_and_wrap() {
        let circle = crate::shapes::arc(
            v2(0.0, 0.0),
            300.0,
            300.0,
            0.0,
            360.0,
            crate::shapes::ArcMode::Arc,
        );
        let a = floral(&circle, 1, 0);
        let b = floral(&circle, 1, 1);
        assert_ne!(a, b);
        assert_eq!(floral(&circle, 1, FLORAL_PATTERNS), a);
        assert_eq!(floral(&circle, 0, 0).len(), 1);
        assert_eq!(floral(&circle, 99, 0).len(), FLORAL_MAX_DENSITY as usize);
        // Five petals (pattern 2): five local maxima of the radius around the centre.
        let five = floral(&circle, 1, 2);
        let r: Vec<f64> = five[0].points.iter().map(|p| p.length()).collect();
        let n = r.len();
        let peaks = (0..n)
            .filter(|&i| r[i] > r[(i + n - 1) % n] && r[i] >= r[(i + 1) % n] && r[i] > 250.0)
            .count();
        assert_eq!(peaks, 5);
        assert!(floral(&Contour::new(vec![v2(0.0, 0.0)], true), 2, 0).is_empty());
    }

    #[test]
    fn frames_fit_their_box() {
        let b = Rect {
            min_x: 100.0,
            min_y: 100.0,
            max_x: 500.0,
            max_y: 300.0,
        };
        for kind in Frame::ALL {
            let f = frame(kind, &b, 40.0);
            assert!(!f.is_empty(), "{kind:?}");
            let fb = bounds(&f);
            // Ornaments stay within one ornament size of the box.
            assert!(
                fb.min_x >= 100.0 - 41.0 && fb.max_x <= 500.0 + 41.0,
                "{kind:?} {fb:?}"
            );
            assert!(
                fb.min_y >= 100.0 - 41.0 && fb.max_y <= 300.0 + 41.0,
                "{kind:?} {fb:?}"
            );
            assert_eq!(Frame::from_id(kind.id()), Some(kind));
        }
        // Closed frames enclose the box centre.
        for kind in [
            Frame::Scallop,
            Frame::Wave,
            Frame::Zigzag,
            Frame::ScallopOval,
        ] {
            assert!(contains(b.center(), &frame(kind, &b, 40.0)), "{kind:?}");
        }
        assert_eq!(frame(Frame::Corners, &b, 40.0).len(), 4);
        assert!(Frame::from_id("laurel").is_none());
    }
}
