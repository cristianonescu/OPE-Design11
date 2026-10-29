//! Sewing-direction helpers: the curved direction ("round fill") and the region's reference
//! point. Both work by mapping the region into a space where the ordinary generators apply,
//! then mapping the stitches back. Lengths are 0.1 mm.
//!
//! Curved direction: in the frame of the curve's chord (x along the chord), the curve is the
//! graph of y = f(x). The warp (x, y) → (x, y − f(x)) straightens it, so rows that are
//! horizontal in warped space are copies of the curve shifted across the region; they never
//! cross each other.

use crate::generate::Run;
use crate::geometry::{Contour, Transform, Vec2, v2};
use crate::model::RegionType;

/// Region types that can follow a curved direction (PE-DESIGN: fill, programmable fill,
/// piping, motif fill).
pub const CURVE_TYPES: [RegionType; 4] = [
    RegionType::Fill,
    RegionType::ProgrammableFill,
    RegionType::Piping,
    RegionType::MotifFill,
];

/// Region types with a centre point.
pub const CENTER_TYPES: [RegionType; 3] = [
    RegionType::ConcentricCircle,
    RegionType::Radial,
    RegionType::Spiral,
];

/// Region types with a pattern reference point.
pub const REFERENCE_TYPES: [RegionType; 6] = [
    RegionType::ProgrammableFill,
    RegionType::MotifFill,
    RegionType::CrossStitch,
    RegionType::NetFill,
    RegionType::ZigzagNetFill,
    RegionType::DecorativeFill,
];

/// The straightening map of a direction curve.
#[derive(Clone, Debug)]
pub struct Warp {
    /// Chord frame: model → frame.
    to_frame: Transform,
    from_frame: Transform,
    /// The curve in frame coordinates, x strictly increasing.
    graph: Vec<Vec2>,
}

/// Why a curve cannot be a sewing direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveError {
    TooShort,
    TurnsBack,
}

impl Warp {
    pub fn new(curve: &Contour) -> Result<Warp, CurveError> {
        let path = curve.path();
        let (Some(&a), Some(&b)) = (path.first(), path.last()) else {
            return Err(CurveError::TooShort);
        };
        let chord = b - a;
        if chord.length() < 1.0 {
            return Err(CurveError::TooShort);
        }
        let angle = chord.y.atan2(chord.x).to_degrees();
        let to_frame =
            Transform::translation(-a.x, -a.y).then(&Transform::rotation(-angle, v2(0.0, 0.0)));
        let from_frame =
            Transform::rotation(angle, v2(0.0, 0.0)).then(&Transform::translation(a.x, a.y));
        let graph: Vec<Vec2> = path.iter().map(|&p| to_frame.apply(p)).collect();
        if graph.windows(2).any(|w| w[1].x <= w[0].x) {
            return Err(CurveError::TurnsBack);
        }
        Ok(Warp {
            to_frame,
            from_frame,
            graph,
        })
    }

    /// Height of the curve above frame x (constant beyond its ends).
    fn f(&self, x: f64) -> f64 {
        let g = &self.graph;
        let k = g.partition_point(|p| p.x < x);
        if k == 0 {
            return g[0].y;
        }
        if k >= g.len() {
            return g[g.len() - 1].y;
        }
        let (p, q) = (g[k - 1], g[k]);
        p.y + (q.y - p.y) * (x - p.x) / (q.x - p.x)
    }

    pub fn forward(&self, p: Vec2) -> Vec2 {
        let q = self.to_frame.apply(p);
        v2(q.x, q.y - self.f(q.x))
    }

    pub fn back(&self, p: Vec2) -> Vec2 {
        self.from_frame.apply(v2(p.x, p.y + self.f(p.x)))
    }

    /// Closed contours mapped into warped space. Every edge is split where it crosses a vertex
    /// of the curve, so the warped outline is exact (f is linear between those crossings).
    pub fn forward_contours(&self, contours: &[Contour]) -> Vec<Contour> {
        contours
            .iter()
            .map(|c| {
                let path: Vec<Vec2> = c.path().iter().map(|&p| self.to_frame.apply(p)).collect();
                let mut pts: Vec<Vec2> = Vec::new();
                for w in path.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    pts.push(a);
                    let (lo, hi) = (a.x.min(b.x), a.x.max(b.x));
                    let mut cuts: Vec<f64> = self
                        .graph
                        .iter()
                        .map(|g| g.x)
                        .filter(|&x| x > lo && x < hi)
                        .map(|x| (x - a.x) / (b.x - a.x))
                        .collect();
                    cuts.sort_by(f64::total_cmp);
                    pts.extend(cuts.into_iter().map(|t| a.lerp(b, t)));
                }
                if let Some(&last) = path.last() {
                    pts.push(last);
                }
                if c.closed && pts.len() > 1 {
                    pts.pop();
                }
                let warped = pts
                    .into_iter()
                    .map(|q| v2(q.x, q.y - self.f(q.x)))
                    .collect();
                Contour::new(warped, c.closed)
            })
            .collect()
    }

    /// Runs mapped back to model space. Needle points along rows are at most a stitch apart
    /// already, so they follow the curve; longer connections stay straight stitches.
    pub fn back_runs(&self, runs: Vec<Run>) -> Vec<Run> {
        runs.into_iter()
            .map(|r| r.into_iter().map(|p| self.back(p)).collect())
            .collect()
    }
}

/// `contours` moved by `d`.
pub fn shifted(contours: &[Contour], d: Vec2) -> Vec<Contour> {
    let t = Transform::translation(d.x, d.y);
    contours.iter().map(|c| c.transformed(&t)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Bezier;

    fn arch() -> Contour {
        let mut c = Contour::new(vec![v2(-100.0, 0.0), v2(300.0, 0.0)], false);
        c.set_curve(
            0,
            Some(Bezier {
                c1: v2(0.0, -150.0),
                c2: v2(200.0, -150.0),
            }),
        );
        c
    }

    #[test]
    fn warp_round_trips_and_straightens_the_curve() {
        let w = Warp::new(&arch()).expect("warp");
        for p in [
            v2(0.0, 0.0),
            v2(100.0, -50.0),
            v2(250.0, 80.0),
            v2(-200.0, 5.0),
        ] {
            assert!(w.back(w.forward(p)).distance(p) < 1e-9, "{p:?}");
        }
        for p in arch().path() {
            assert!(w.forward(p).y.abs() < 1e-9);
        }
    }

    #[test]
    fn rejected_curves() {
        let short = Contour::new(vec![v2(0.0, 0.0), v2(0.5, 0.0)], false);
        assert_eq!(Warp::new(&short).err(), Some(CurveError::TooShort));
        let back = Contour::new(vec![v2(0.0, 0.0), v2(100.0, 0.0), v2(50.0, 10.0)], false);
        assert_eq!(Warp::new(&back).err(), Some(CurveError::TurnsBack));
        // Any chord direction works.
        let diagonal = Contour::new(vec![v2(0.0, 0.0), v2(10.0, 30.0), v2(0.0, 100.0)], false);
        assert!(Warp::new(&diagonal).is_ok());
    }

    use crate::model::RegionSewing;
    use openstitch_core::Thread;

    fn layers(c: &Contour, r: &RegionSewing) -> Vec<Vec2> {
        crate::region::layers(std::slice::from_ref(c), r, &[])
            .into_iter()
            .flatten()
            .flatten()
            .collect()
    }

    fn sewing(kind: RegionType) -> RegionSewing {
        RegionSewing::new(kind, Thread::rgb(0xff0000))
    }

    #[test]
    fn concentric_rings_are_centred_on_the_centre_point() {
        let square = Contour::rectangle(0.0, 0.0, 400.0, 400.0);
        let mut r = sewing(RegionType::ConcentricCircle);
        let c = v2(120.0, 260.0);
        r.center = Some(c);
        let spacing = 10.0 / r.density;
        let points = layers(&square, &r);
        assert!(points.len() > 100);
        // Ring radii are multiples of the spacing (plus one offset): distances to the centre
        // fall into narrow bands.
        let phase = |p: &Vec2| (p.distance(c) / spacing).fract();
        let off = points
            .iter()
            .filter(|p| {
                let f = (phase(p) - phase(&points[points.len() / 2])).rem_euclid(1.0);
                f.min(1.0 - f) > 0.1
            })
            .count();
        assert!(off * 10 < points.len(), "{off} of {}", points.len());
    }

    #[test]
    fn radial_lines_meet_at_the_centre_point() {
        let square = Contour::rectangle(0.0, 0.0, 400.0, 400.0);
        let mut r = sewing(RegionType::Radial);
        let c = v2(100.0, 100.0);
        r.center = Some(c);
        let runs = crate::region::layers(std::slice::from_ref(&square), &r, &[])
            .pop()
            .expect("top");
        // Every straight piece of a run points at the centre.
        let mut checked = 0;
        for run in &runs {
            for w in run.windows(2) {
                let (a, b) = (w[0] - c, w[1] - c);
                let cross = a.x * b.y - a.y * b.x;
                if a.length() > 20.0
                    && b.length() > 20.0
                    && w[0].distance(w[1]) > 5.0
                    && (cross / (a.length() * b.length())).abs() < 0.02
                {
                    checked += 1;
                }
            }
        }
        assert!(checked > 200, "{checked}");
    }

    #[test]
    fn reference_point_moves_the_pattern_grid() {
        let square = Contour::rectangle(3.3, 7.1, 300.7, 290.3);
        let mut r0 = sewing(RegionType::ProgrammableFill);
        // The tile grid lines up with the region's axes at 0°. Without a reference point it
        // starts at the region's top-left corner (3.3, 7.1), as in PE-DESIGN.
        r0.angle = 0.0;
        r0.pattern = Some("diamonds".into());
        let (pw, ph) = r0.pattern_size_mm();
        let corner = v2(3.3, 7.1);
        let mut whole = r0.clone();
        whole.reference = Some(corner + v2(pw * 20.0, ph * 20.0));
        let mut half = r0.clone();
        half.reference = Some(corner + v2(pw * 5.0, 0.0));
        let base = layers(&square, &r0);
        let same = layers(&square, &whole);
        let moved = layers(&square, &half);
        // Share of `a`'s needle points that `b` also has.
        let shared = |a: &[Vec2], b: &[Vec2]| {
            a.iter()
                .filter(|p| b.iter().any(|q| q.distance(**p) < 1e-3))
                .count() as f64
                / a.len() as f64
        };
        // Whole tiles: the same pattern (up to a point flipping at the outline).
        assert!(shared(&base, &same) > 0.99, "{}", shared(&base, &same));
        // Half a tile: a different pattern.
        assert!(shared(&base, &moved) < 0.8, "{}", shared(&base, &moved));
    }

    #[test]
    fn curved_direction_rows_follow_the_curve() {
        // A band along the arch: straightened, it is a rectangle, so every needle point is
        // inside it (a straight top edge would leave notches the rows jump across).
        let arc = arch().path();
        let mut band: Vec<Vec2> = arc.iter().map(|p| v2(p.x, p.y - 60.0)).collect();
        band.extend(arc.iter().rev().map(|p| v2(p.x, p.y + 60.0)));
        let rect = Contour::new(band, true);
        let mut r = sewing(RegionType::Fill);
        r.direction_curve = Some(arch());
        // The top rows only: under sewing runs across them.
        r.underlay = false;
        let spacing = 10.0 / r.density;
        let w = Warp::new(&arch()).expect("warp");
        let points = layers(&rect, &r);
        assert!(points.len() > 200);
        // Straightened, every needle point lies on a row: one phase modulo the spacing.
        let phase = |p: &Vec2| (w.forward(*p).y / spacing).rem_euclid(1.0);
        let p0 = phase(&points[0]);
        assert!(
            points.iter().all(|p| {
                let f = (phase(p) - p0).rem_euclid(1.0);
                f.min(1.0 - f) < 0.02
            }),
            "points off the curved rows"
        );
        // All inside the band (pull compensation 0), to 0.03 mm.
        assert!(points.iter().all(|p| {
            let q = w.forward(*p);
            q.y.abs() <= 60.3 && q.x >= -0.3 && q.x <= 400.3
        }));
        // Different from the straight fill.
        let mut straight = r.clone();
        straight.direction_curve = None;
        assert_ne!(layers(&rect, &straight), points);
        // Types without curved direction ignore the curve.
        let mut satin = sewing(RegionType::Satin);
        satin.direction_curve = Some(arch());
        let mut plain = satin.clone();
        plain.direction_curve = None;
        assert_eq!(layers(&rect, &satin), layers(&rect, &plain));
    }
}
