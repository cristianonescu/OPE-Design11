//! Regions sewn along several direction lines (PE-DESIGN's Manual Punch shapes): consecutive
//! lines are the rungs of a column; stitches follow directions interpolated between them.
//! Satin zigzags across the column; fill sews rows parallel to the interpolated rungs. Own
//! algorithm. Lengths are 0.1 mm.

use crate::generate::{Run, subdivide};
use crate::geometry::{Bezier, Contour, Vec2};

/// The closed outline of a column: the left ends in order, then the right ends back.
pub fn outline(rungs: &[[Vec2; 2]]) -> Contour {
    let mut pts: Vec<Vec2> = rungs.iter().map(|r| r[0]).collect();
    pts.extend(rungs.iter().rev().map(|r| r[1]));
    Contour::new(pts, true)
}

/// Curved Block: each rail (the left ends, the right ends) as a smooth curve through its
/// points — Catmull-Rom segments written as cubic Béziers, the end tangents along the chord.
fn rail_curves(rail: &[Vec2]) -> Vec<Bezier> {
    let n = rail.len();
    (0..n.saturating_sub(1))
        .map(|i| {
            let prev = rail[i.saturating_sub(1)];
            let next = rail[(i + 2).min(n - 1)];
            Bezier {
                c1: rail[i] + (rail[i + 1] - prev) * (1.0 / 6.0),
                c2: rail[i + 1] - (next - rail[i]) * (1.0 / 6.0),
            }
        })
        .collect()
}

fn bezier_at(a: Vec2, c: Bezier, b: Vec2, t: f64) -> Vec2 {
    let s = 1.0 - t;
    a * (s * s * s) + c.c1 * (3.0 * s * s * t) + c.c2 * (3.0 * s * t * t) + b * (t * t * t)
}

/// The closed outline of a Curved Block column: both rails curved through the line ends, the
/// first and last lines straight.
pub fn curved_outline(rungs: &[[Vec2; 2]]) -> Contour {
    let mut c = outline(rungs);
    let n = rungs.len();
    if n < 3 {
        return c;
    }
    let left: Vec<Vec2> = rungs.iter().map(|r| r[0]).collect();
    let right: Vec<Vec2> = rungs.iter().map(|r| r[1]).collect();
    let mut curves: Vec<Option<Bezier>> = rail_curves(&left).into_iter().map(Some).collect();
    curves.push(None);
    // The right rail runs backwards in the outline.
    curves.extend(
        rail_curves(&right)
            .into_iter()
            .rev()
            .map(|z| Some(Bezier { c1: z.c2, c2: z.c1 })),
    );
    curves.push(None);
    c.curves = curves;
    c
}

/// Curved Block: the direction lines between the clicked ones, following the curved rails
/// (about one every 2 mm of rail), clicked lines included.
pub fn curved_rungs(rungs: &[[Vec2; 2]]) -> Vec<[Vec2; 2]> {
    if rungs.len() < 3 {
        return rungs.to_vec();
    }
    let left: Vec<Vec2> = rungs.iter().map(|r| r[0]).collect();
    let right: Vec<Vec2> = rungs.iter().map(|r| r[1]).collect();
    let (lc, rc) = (rail_curves(&left), rail_curves(&right));
    let mut out = vec![rungs[0]];
    for i in 0..rungs.len() - 1 {
        let len = left[i]
            .distance(left[i + 1])
            .max(right[i].distance(right[i + 1]));
        let steps = ((len / 20.0).ceil() as usize).clamp(1, 64);
        for k in 1..=steps {
            let t = k as f64 / steps as f64;
            out.push([
                bezier_at(left[i], lc[i], left[i + 1], t),
                bezier_at(right[i], rc[i], right[i + 1], t),
            ]);
        }
    }
    out
}

/// Whether segments a–b and c–d cross (touching ends count).
fn cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let o = |p: Vec2, q: Vec2, r: Vec2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    (d1 * d2 <= 0.0) && (d3 * d4 <= 0.0) && !(d1 == 0.0 && d2 == 0.0)
}

/// Why rungs cannot make a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RungError {
    TooFew,
    /// A rung shorter than 0.1 mm.
    Degenerate(usize),
    /// Rungs `i` and `i + 1`, or the rails between them, cross.
    Crossing(usize),
}

pub fn validate(rungs: &[[Vec2; 2]]) -> Result<(), RungError> {
    if rungs.len() < 2 {
        return Err(RungError::TooFew);
    }
    if let Some(i) = rungs.iter().position(|r| r[0].distance(r[1]) < 1.0) {
        return Err(RungError::Degenerate(i));
    }
    for (i, w) in rungs.windows(2).enumerate() {
        let ([a, b], [c, d]) = (w[0], w[1]);
        // Neighbouring rungs must not cross, nor may the left and right rails.
        if cross(a, b, c, d) || cross(a, c, b, d) {
            return Err(RungError::Crossing(i));
        }
    }
    Ok(())
}

/// Points along the column, `spacing` apart on the longer rail of each section: one
/// interpolated rung per step, first and last rungs included.
fn rungs_at(rungs: &[[Vec2; 2]], spacing: f64) -> Vec<[Vec2; 2]> {
    let mut out = vec![rungs[0]];
    for w in rungs.windows(2) {
        let ([a, b], [c, d]) = (w[0], w[1]);
        let len = a.distance(c).max(b.distance(d));
        let n = ((len / spacing.max(0.1)).ceil() as usize).clamp(1, 100_000);
        for k in 1..=n {
            let t = k as f64 / n as f64;
            out.push([a.lerp(c, t), b.lerp(d, t)]);
        }
    }
    out
}

/// Satin across the column: one needle point per rung, `spacing` apart, on alternate sides
/// (see [`zigzag`]), each rung lengthened by `pull` at both ends.
pub fn satin(rungs: &[[Vec2; 2]], spacing: f64, pull: f64) -> Run {
    zigzag(rungs_at(rungs, spacing).into_iter().map(|[l, r]| {
        let len = l.distance(r).max(1e-9);
        let d = (r - l) * (pull / len);
        [l - d, r + d]
    }))
}

/// Satin needle points from rungs [left, right] in sewing order: the first rung is sewn
/// across (left, right), then each later rung gives one needle point, left on odd rungs and
/// right on even ones. Consecutive points are on opposite sides, so every stitch crosses the
/// column and none runs along its edge.
pub fn zigzag(rungs: impl IntoIterator<Item = [Vec2; 2]>) -> Run {
    let mut run = Vec::new();
    for (k, [l, r]) in rungs.into_iter().enumerate() {
        match k {
            0 => run.extend([l, r]),
            _ if k % 2 == 1 => run.push(l),
            _ => run.push(r),
        }
    }
    run
}

/// Fill: rows along the interpolated rungs, `spacing` apart, needle points at most
/// `stitch_length` apart with a third-of-a-stitch stagger per row, sewn back and forth.
pub fn fill(rungs: &[[Vec2; 2]], spacing: f64, stitch_length: f64, pull: f64) -> Run {
    let mut run = Vec::new();
    for (k, [l, r]) in rungs_at(rungs, spacing).into_iter().enumerate() {
        let len = l.distance(r).max(1e-9);
        let d = (r - l) * (pull / len);
        let (l, r) = (l - d, r + d);
        let (from, to) = if k % 2 == 0 { (l, r) } else { (r, l) };
        let total = from.distance(to);
        let phase = (k % 3) as f64 / 3.0 * stitch_length;
        let mut row = vec![from];
        let mut s = phase.max(stitch_length * 0.25);
        while s < total - stitch_length * 0.25 {
            row.push(from.lerp(to, s / total));
            s += stitch_length;
        }
        row.push(to);
        run.extend(subdivide(&row, stitch_length));
    }
    run.dedup_by(|a, b| a.distance(*b) < 1e-9);
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{contains, v2};

    fn bend() -> Vec<[Vec2; 2]> {
        vec![
            [v2(0.0, 0.0), v2(0.0, 40.0)],
            [v2(100.0, 0.0), v2(80.0, 40.0)],
            [v2(140.0, 80.0), v2(100.0, 80.0)],
        ]
    }

    #[test]
    fn curved_block_rails_pass_through_the_line_ends() {
        let o = curved_outline(&bend());
        assert_eq!(o.points.len(), 6);
        assert_eq!(o.curves.len(), 6);
        // Rails curved; the first and last lines straight.
        assert!(o.curve(0).is_some() && o.curve(1).is_some() && o.curve(2).is_none());
        assert!(o.curve(3).is_some() && o.curve(4).is_some() && o.curve(5).is_none());
        let dense = curved_rungs(&bend());
        assert!(dense.len() > 6);
        assert_eq!(dense[0], bend()[0]);
        assert_eq!(*dense.last().expect("last"), bend()[2]);
        assert!(dense.contains(&bend()[1]), "clicked lines are kept");
        assert_eq!(validate(&dense), Ok(()));
        // Two lines: nothing to curve.
        assert_eq!(curved_rungs(&bend()[..2]), bend()[..2].to_vec());
        assert!(!curved_outline(&bend()[..2]).has_curves());
    }

    #[test]
    fn outline_and_validation() {
        let o = outline(&bend());
        assert_eq!(o.points.len(), 6);
        assert!(contains(v2(50.0, 20.0), std::slice::from_ref(&o)));
        assert_eq!(validate(&bend()), Ok(()));
        assert_eq!(validate(&bend()[..1]), Err(RungError::TooFew));
        let mut flat = bend();
        flat[1] = [v2(50.0, 5.0), v2(50.0, 5.05)];
        assert_eq!(validate(&flat), Err(RungError::Degenerate(1)));
        // Second rung crossing the first.
        let crossing = vec![
            [v2(0.0, 0.0), v2(0.0, 40.0)],
            [v2(-20.0, 20.0), v2(20.0, 20.0)],
        ];
        assert_eq!(validate(&crossing), Err(RungError::Crossing(0)));
        // Twisted: left and right swap sides.
        let twisted = vec![
            [v2(0.0, 0.0), v2(0.0, 40.0)],
            [v2(50.0, 40.0), v2(50.0, 0.0)],
        ];
        assert_eq!(validate(&twisted), Err(RungError::Crossing(0)));
    }

    #[test]
    fn satin_alternates_rails_and_follows_the_turn() {
        let run = satin(&bend(), 4.0, 0.0);
        assert!(run.len() > 20);
        // The first rung is sewn across, then one needle point per rung on alternate rails:
        // every stitch crosses the column, none runs along a rail.
        let on = |side: usize, p: Vec2| {
            let rail = Contour::new(bend().iter().map(|r| r[side]).collect(), false);
            rail.nearest(p).expect("n").point.distance(p) < 1e-6
        };
        let rail = |p: Vec2| {
            assert!(on(0, p) != on(1, p), "{p:?} is on exactly one rail");
            usize::from(on(1, p))
        };
        assert_eq!(run[0], bend()[0][0]);
        assert_eq!(run[1], bend()[0][1]);
        for w in run.windows(2).skip(1) {
            assert_ne!(rail(w[0]), rail(w[1]), "{:?} runs along a rail", w);
        }
        // The last needle point is on the last rung.
        let last = run[run.len() - 1];
        assert!(last == v2(140.0, 80.0) || last == v2(100.0, 80.0));
        // Pull compensation lengthens each rung at both ends.
        let pulled = satin(&bend(), 4.0, 3.0);
        assert_eq!(pulled.len(), run.len());
        assert!(pulled[0].distance(pulled[1]) > run[0].distance(run[1]) + 5.0);
    }

    #[test]
    fn fill_rows_stay_inside_and_are_short() {
        let run = fill(&bend(), 4.0, 30.0, 0.0);
        let o = [outline(&bend())];
        for w in run.windows(2) {
            assert!(w[0].distance(w[1]) <= 30.0 + 1e-6);
            let mid = w[0].lerp(w[1], 0.5);
            assert!(contains(mid, &o) || o[0].nearest(mid).expect("n").point.distance(mid) < 1.0);
        }
    }
}
