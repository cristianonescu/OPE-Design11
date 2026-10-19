//! Regions sewn along several direction lines (PE-DESIGN's Manual Punch shapes): consecutive
//! lines are the rungs of a column; stitches follow directions interpolated between them.
//! Satin zigzags across the column; fill sews rows parallel to the interpolated rungs. Own
//! algorithm. Lengths are 0.1 mm.

use crate::generate::{Run, subdivide};
use crate::geometry::{Contour, Vec2};

/// The closed outline of a column: the left ends in order, then the right ends back.
pub fn outline(rungs: &[[Vec2; 2]]) -> Contour {
    let mut pts: Vec<Vec2> = rungs.iter().map(|r| r[0]).collect();
    pts.extend(rungs.iter().rev().map(|r| r[1]));
    Contour::new(pts, true)
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

/// Satin across the column: left, right, left… `spacing` apart, each stitch lengthened by
/// `pull` at both ends.
pub fn satin(rungs: &[[Vec2; 2]], spacing: f64, pull: f64) -> Run {
    rungs_at(rungs, spacing)
        .into_iter()
        .enumerate()
        .flat_map(|(k, [l, r])| {
            let len = l.distance(r).max(1e-9);
            let d = (r - l) * (pull / len);
            let (l, r) = (l - d, r + d);
            if k % 2 == 0 { [l, r] } else { [r, l] }
        })
        .collect()
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
        assert!(run.len() > 40);
        // Every other point on the left rail, the others on the right.
        let o = outline(&bend());
        for p in &run {
            let n = o.nearest(*p).expect("n");
            assert!(n.point.distance(*p) < 1e-6, "{p:?} is on the outline");
        }
        // Stitches are rung-like: the last one is the last rung.
        let k = run.len();
        let last = [run[k - 2], run[k - 1]];
        assert!(last.contains(&v2(140.0, 80.0)) && last.contains(&v2(100.0, 80.0)));
        // Pull compensation lengthens each stitch.
        let pulled = satin(&bend(), 4.0, 3.0);
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
