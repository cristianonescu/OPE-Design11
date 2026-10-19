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

/// Satin along the skeleton: left, right, left… with the stitches `spacing` apart along the
/// centre line (0.1 mm), each lengthened by `pull` at both ends. Empty for a degenerate
/// skeleton.
pub fn satin(s: &Skeleton, spacing: f64, pull: f64) -> Run {
    let Some(c) = Centre::new(s) else {
        return Vec::new();
    };
    let mut run: Run = c
        .rungs(spacing, pull)
        .into_iter()
        .enumerate()
        .flat_map(|(k, [l, r])| if k % 2 == 0 { [l, r] } else { [r, l] })
        .collect();
    run.dedup_by(|a, b| a.distance(*b) < 1e-9);
    run
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

#[cfg(test)]
mod tests {
    use super::*;

    fn skeleton(points: &[(f64, f64)], widths: &[f64]) -> Skeleton {
        Skeleton {
            points: points.iter().map(|&(x, y)| v2(x, y)).collect(),
            widths: widths.to_vec(),
        }
    }

    /// The stitches of a satin run: consecutive pairs.
    fn stitches(run: &Run) -> Vec<(Vec2, Vec2)> {
        run.windows(2).step_by(2).map(|w| (w[0], w[1])).collect()
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
        let st = stitches(&run);
        // One stitch per spacing step, both ends included.
        assert_eq!(run.len(), 242);
        assert_eq!(st.len(), 121);
        let centres: Vec<Vec2> = c_rungs(&s, spacing)
            .iter()
            .map(|[l, r]| l.lerp(*r, 0.5))
            .collect();
        for w in centres.windows(2) {
            assert!(w[0].distance(w[1]) <= spacing + 1e-9);
        }
        // Width tapers from 2 mm to 6 mm.
        assert!((st[0].0.distance(st[0].1) - 20.0).abs() < 1e-6);
        let (a, b) = (run[run.len() - 2], run[run.len() - 1]);
        assert!((a.distance(b) - 60.0).abs() < 1e-6);
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
}
