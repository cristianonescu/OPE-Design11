//! Embossing, engraving and stamps (PE-DESIGN's "Embossing/Engraving" and "Applying and
//! editing stamps"): effects that change where the needle goes down inside a fill, satin or
//! programmable-fill region, so part of it looks raised or sunken. Independent algorithms on
//! the region's stitches:
//!
//! - engraved line: a penetration wherever a stitch crosses the effect outline, so the outline
//!   shows as a line of needle holes;
//! - engraved region: stitches inside the effect area are cut into short ones (sunken look);
//! - embossed region: needle points inside the effect area are dropped, so stitches there are
//!   long (raised look), up to [`EMBOSS_MAX`].
//!
//! Lengths are 0.1 mm.

use crate::generate::Run;
use crate::geometry::{Contour, Rect, Transform, Vec2, contains, v2};
use crate::model::RegionType;
use crate::shapes;
use serde::{Deserialize, Serialize};

/// Stitch length inside an engraved region.
pub const ENGRAVE_STEP: f64 = 10.0;
/// Longest stitch inside an embossed region.
pub const EMBOSS_MAX: f64 = 70.0;

/// Region types that take stamps and embossing.
pub const STAMP_TYPES: [RegionType; 3] = [
    RegionType::Fill,
    RegionType::Satin,
    RegionType::ProgrammableFill,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StampEffect {
    EngravedLine,
    EngravedRegion,
    EmbossedRegion,
}

impl StampEffect {
    pub fn id(self) -> &'static str {
        match self {
            StampEffect::EngravedLine => "engravedLine",
            StampEffect::EngravedRegion => "engravedRegion",
            StampEffect::EmbossedRegion => "embossedRegion",
        }
    }

    pub fn from_id(s: &str) -> Option<StampEffect> {
        [
            StampEffect::EngravedLine,
            StampEffect::EngravedRegion,
            StampEffect::EmbossedRegion,
        ]
        .into_iter()
        .find(|e| e.id() == s)
    }
}

/// One effect area on a region: closed outlines in model coordinates. `pattern` names the
/// stamp it came from (`None`: converted from a shape).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stamp {
    pub effect: StampEffect,
    pub contours: Vec<Contour>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

impl Stamp {
    pub fn bounds(&self) -> Option<Rect> {
        Rect::enclosing(self.contours.iter().flat_map(Contour::outline_points))
    }

    pub fn transformed(&self, t: &Transform) -> Stamp {
        Stamp {
            contours: self.contours.iter().map(|c| c.transformed(t)).collect(),
            ..self.clone()
        }
    }
}

/// Stamp patterns: OpenStitch's own designs, built from the preset shapes.
pub const PATTERNS: [&str; 7] = [
    "circle", "heart", "star", "diamond", "teardrop", "crescent", "cross",
];

/// Stamp `pattern` of `size` (0.1 mm, its larger side) centred at `at`, turned `rotation`
/// degrees.
pub fn stamp(
    pattern: &str,
    effect: StampEffect,
    at: Vec2,
    size: f64,
    rotation: f64,
) -> Option<Stamp> {
    let unit = match pattern {
        "circle" => shapes::arc(v2(0.0, 0.0), 1.0, 1.0, 0.0, 360.0, shapes::ArcMode::Arc),
        "heart" => shapes::heart(),
        "star" => shapes::star(5, 0.45),
        "diamond" => shapes::regular_polygon(4),
        "teardrop" => shapes::teardrop(),
        "crescent" => shapes::crescent(0.9),
        "cross" => shapes::cross(0.35),
        _ => return None,
    };
    let c = shapes::fit(&unit, at, size, size, rotation);
    Some(Stamp {
        effect,
        contours: vec![c],
        pattern: Some(pattern.to_string()),
    })
}

/// Straight edges of the closed contours.
fn edges(contours: &[Contour]) -> Vec<(Vec2, Vec2)> {
    contours
        .iter()
        .filter(|c| c.closed)
        .flat_map(Contour::edges)
        .collect()
}

/// Parameters (0–1, exclusive) where segment a→b crosses `edges`, sorted.
fn crossings(a: Vec2, b: Vec2, edges: &[(Vec2, Vec2)]) -> Vec<f64> {
    let d = b - a;
    let mut ts: Vec<f64> = edges
        .iter()
        .filter_map(|&(p, q)| {
            let e = q - p;
            let den = d.x * e.y - d.y * e.x;
            if den.abs() < 1e-12 {
                return None;
            }
            let w = p - a;
            let t = (w.x * e.y - w.y * e.x) / den;
            let u = (w.x * d.y - w.y * d.x) / den;
            (t > 1e-9 && t < 1.0 - 1e-9 && (0.0..=1.0).contains(&u)).then_some(t)
        })
        .collect();
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    ts
}

/// Points along a→b (excluding a, including b) with crossings of `edges` inserted, each piece
/// tagged with whether its midpoint is inside `area`.
fn pieces(a: Vec2, b: Vec2, edges: &[(Vec2, Vec2)], area: &[Contour]) -> Vec<(Vec2, bool)> {
    let mut ts = crossings(a, b, edges);
    ts.push(1.0);
    let mut out = Vec::new();
    let mut from = 0.0;
    for t in ts {
        let mid = a.lerp(b, (from + t) / 2.0);
        out.push((a.lerp(b, t), contains(mid, area)));
        from = t;
    }
    out
}

fn apply_one(runs: Vec<Run>, s: &Stamp) -> Vec<Run> {
    let e = edges(&s.contours);
    if e.is_empty() {
        return runs;
    }
    let area = &s.contours;
    runs.into_iter()
        .map(|run| {
            let Some(&first) = run.first() else {
                return run;
            };
            let mut out = vec![first];
            // Whether the piece ending at each output point lies inside the area.
            let mut inside_before = vec![false];
            for w in run.windows(2) {
                let (a, b) = (w[0], w[1]);
                let mut from = a;
                for (p, inside) in pieces(a, b, &e, area) {
                    match s.effect {
                        StampEffect::EngravedRegion if inside => {
                            let n = (from.distance(p) / ENGRAVE_STEP).ceil().max(1.0) as usize;
                            out.extend((1..=n).map(|i| from.lerp(p, i as f64 / n as f64)));
                        }
                        _ => {
                            out.push(p);
                            inside_before.push(inside);
                        }
                    }
                    from = p;
                }
            }
            if s.effect == StampEffect::EmbossedRegion {
                out = emboss(&out, &inside_before);
            }
            out
        })
        .collect()
}

/// Drops needle points with the area on both sides (run ends and outline crossings stay), then
/// splits stitches longer than [`EMBOSS_MAX`] evenly.
fn emboss(run: &[Vec2], inside_before: &[bool]) -> Run {
    let n = run.len();
    let kept: Vec<Vec2> = (0..n)
        .filter(|&i| i == 0 || i + 1 == n || !(inside_before[i] && inside_before[i + 1]))
        .map(|i| run[i])
        .collect();
    let mut out = vec![kept[0]];
    for w in kept.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.distance(b);
        if len > EMBOSS_MAX {
            // Split points staggered like tatami: the phase steps with the row's offset across
            // the stitch, so neighbouring rows do not split in line (which would draw stripes
            // across the raised area). Pieces stay at most EMBOSS_MAX.
            let k = (len / EMBOSS_MAX).ceil() as usize;
            let d = (b - a) * (1.0 / len);
            let across = (-a.x * d.y + a.y * d.x).abs();
            let phase = 0.25 + 0.5 * (across / 10.0).rem_euclid(1.0);
            out.extend((0..k).map(|i| a.lerp(b, (i as f64 + phase) / k as f64)));
        }
        out.push(b);
    }
    out
}

/// The region's top-layer `runs` with every stamp applied in order.
pub fn apply(mut runs: Vec<Run>, stamps: &[Stamp]) -> Vec<Run> {
    for s in stamps {
        runs = apply_one(runs, s);
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Run> {
        // Three horizontal rows across 0..400 with a needle point every 40.
        (0..3)
            .map(|r| {
                let y = 150.0 + 17.0 * f64::from(r);
                (0..=10).map(|k| v2(40.0 * f64::from(k), y)).collect()
            })
            .collect()
    }

    fn square_stamp(effect: StampEffect) -> Stamp {
        Stamp {
            effect,
            contours: vec![Contour::rectangle(105.0, 100.0, 190.0, 200.0)],
            pattern: None,
        }
    }

    fn inside(p: &Vec2) -> bool {
        p.x > 105.0 && p.x < 295.0
    }

    #[test]
    fn engraved_line_adds_penetrations_on_the_outline() {
        let out = apply(rows(), &[square_stamp(StampEffect::EngravedLine)]);
        for run in &out {
            assert!(run.contains(&v2(105.0, run[0].y)));
            assert!(run.contains(&v2(295.0, run[0].y)));
            assert_eq!(run.len(), 13);
        }
    }

    #[test]
    fn engraved_region_shortens_stitches_inside() {
        let out = apply(rows(), &[square_stamp(StampEffect::EngravedRegion)]);
        for run in &out {
            for w in run.windows(2) {
                let mid = w[0].lerp(w[1], 0.5);
                if inside(&mid) {
                    assert!(w[0].distance(w[1]) <= ENGRAVE_STEP + 1e-9);
                } else {
                    assert!(w[0].distance(w[1]) <= 40.0 + 1e-9);
                }
            }
            assert!(run.len() > 25);
        }
    }

    #[test]
    fn embossed_region_drops_inside_points() {
        let out = apply(rows(), &[square_stamp(StampEffect::EmbossedRegion)]);
        // Rows split at different places.
        let splits: Vec<f64> = out
            .iter()
            .map(|r| {
                r.iter()
                    .find(|p| p.x > 105.0 && p.x < 295.0)
                    .expect("split")
                    .x
            })
            .collect();
        assert!(
            splits[0] != splits[1] || splits[1] != splits[2],
            "{splits:?}"
        );
        for run in &out {
            assert!(
                run.windows(2)
                    .all(|w| w[0].distance(w[1]) <= EMBOSS_MAX + 1e-9)
            );
            // The 19 mm inside is one long stitch split in staggered pieces: fewer needle points
            // than the plain row's five.
            let inner = run.iter().filter(|p| inside(p)).count();
            assert!(inner < 5, "{inner}");
            assert!(run.contains(&v2(105.0, run[0].y)) && run.contains(&v2(295.0, run[0].y)));
            assert_eq!(run.first(), Some(&v2(0.0, run[0].y)));
            assert_eq!(run.last(), Some(&v2(400.0, run[0].y)));
        }
    }

    #[test]
    fn stamps_from_patterns() {
        for p in PATTERNS {
            let s = stamp(
                p,
                StampEffect::EngravedRegion,
                v2(500.0, 500.0),
                100.0,
                30.0,
            )
            .expect("pattern");
            let b = s.bounds().expect("b");
            assert!(b.center().distance(v2(500.0, 500.0)) < 40.0, "{p}");
            assert!(b.width() <= 150.0 && b.height() <= 150.0, "{p}");
        }
        assert!(stamp("dragon", StampEffect::EngravedLine, v2(0.0, 0.0), 10.0, 0.0).is_none());
        assert_eq!(
            StampEffect::from_id("embossedRegion"),
            Some(StampEffect::EmbossedRegion)
        );
        assert!(StampEffect::from_id("x").is_none());
        // No stamps: unchanged.
        assert_eq!(apply(rows(), &[]), rows());
    }
}
