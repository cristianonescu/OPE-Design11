//! Region sewing with its options: underlay styles, automatic direction, gradation, feathered
//! edges, split satin stitches and travel inside the region. Plain fill and satin without
//! options go straight to [`generate::fill`] / [`generate::satin`], so their output is
//! unchanged. Lengths are 0.1 mm.

use crate::direction;
use crate::generate::{self, RegionParams, RowSpan, Run};
use crate::geometry::{Contour, Rect, Span, Vec2, contains, subtract, v2};
use crate::model::{FeatherSide, RegionSewing, RegionType, UnderlayStyle};
use crate::paths::{Rng, TravelGraph, join};
use crate::patterns::Pattern;
use crate::regions;

/// Inset of underlay rows from the outline (0.1 mm).
const UNDERLAY_INSET: f64 = 5.0;

/// The layers of a region in sewing order (underlay first, then the top stitching); each layer
/// is a list of runs.
pub fn layers(closed: &[Contour], r: &RegionSewing, holes: &[Contour]) -> Vec<Vec<Run>> {
    layers_with(closed, r, holes, &[])
}

/// [`layers`] with the design's own (user) patterns available by id.
pub fn layers_with(
    closed: &[Contour],
    r: &RegionSewing,
    holes: &[Contour],
    user: &[Pattern],
) -> Vec<Vec<Run>> {
    // Skeleton: the top stitching is a satin column along the centre line.
    if let Some(skeleton) = &r.skeleton
        && r.kind == RegionType::Satin
    {
        let mut plain = r.clone();
        plain.skeleton = None;
        let mut out = layers_with(closed, &plain, holes, user);
        out.pop();
        let spacing = 10.0 / r.density.max(0.1);
        out.push(vec![crate::skeleton::satin(
            skeleton,
            spacing,
            r.pull_compensation * 10.0,
        )]);
        return out;
    }
    // Curved direction: sew the straightened region with horizontal rows, then bend back.
    if let Some(curve) = &r.direction_curve
        && direction::CURVE_TYPES.contains(&r.kind)
        && let Ok(w) = direction::Warp::new(curve)
    {
        let mut flat = r.clone();
        flat.direction_curve = None;
        flat.angle = 0.0;
        flat.auto_direction = false;
        flat.entry = r.entry.map(|p| w.forward(p));
        flat.reference = r.reference.map(|p| w.forward(p));
        let (c, h) = (w.forward_contours(closed), w.forward_contours(holes));
        return layers(&c, &flat, &h)
            .into_iter()
            .map(|l| w.back_runs(l))
            .collect();
    }
    // Reference point: the pattern grid starts there.
    if let Some(at) = r.reference
        && direction::REFERENCE_TYPES.contains(&r.kind)
    {
        let mut local = r.clone();
        local.reference = None;
        local.entry = r.entry.map(|p| p - at);
        local.center = r.center.map(|p| p - at);
        let back = |l: Vec<Run>| -> Vec<Run> {
            l.into_iter()
                .map(|run| run.into_iter().map(|p| p + at).collect())
                .collect()
        };
        return layers(
            &direction::shifted(closed, v2(-at.x, -at.y)),
            &local,
            &direction::shifted(holes, v2(-at.x, -at.y)),
        )
        .into_iter()
        .map(back)
        .collect();
    }
    // Several direction lines: the top stitching follows the column they make.
    if r.direction_lines.len() >= 2 && matches!(r.kind, RegionType::Satin | RegionType::Fill) {
        let mut plain = r.clone();
        plain.direction_lines.clear();
        let mut out = layers(closed, &plain, holes);
        out.pop();
        let spacing = 10.0 / r.density.max(0.1);
        let pull = r.pull_compensation * 10.0;
        let lines = if r.curved_lines {
            crate::column::curved_rungs(&r.direction_lines)
        } else {
            r.direction_lines.clone()
        };
        out.push(vec![match r.kind {
            RegionType::Satin => crate::column::satin(&lines, spacing, pull),
            _ => crate::column::fill(&lines, spacing, r.stitch_length * 10.0, pull),
        }]);
        return out;
    }
    let mut out = Vec::new();
    let angle = if r.auto_direction {
        auto_angle(closed)
    } else {
        r.angle
    };
    let params = RegionParams {
        angle,
        row_spacing: 10.0 / r.density.max(0.1),
        stitch_length: r.stitch_length * 10.0,
        pull_compensation: r.pull_compensation * 10.0,
    };
    let row_based = matches!(r.kind, RegionType::Fill | RegionType::Satin);
    if r.underlay {
        let edge = || -> Vec<Run> {
            closed
                .iter()
                .map(|c| match r.entry {
                    // The underlay begins (and ends) at the entry point.
                    Some(p) => generate::running(&c.starting_at(p), 25.0, 1),
                    None => generate::running(c, 25.0, 1),
                })
                .collect()
        };
        let rows = |a: f64| -> Vec<Run> {
            let p = RegionParams {
                angle: a,
                row_spacing: r.underlay_spacing_mm() * 10.0,
                stitch_length: params.stitch_length,
                pull_compensation: -UNDERLAY_INSET,
            };
            generate::satin(closed, &p, holes)
        };
        let across = if r.kind == RegionType::Satin {
            angle
        } else {
            angle + 90.0
        };
        match r.underlay_style {
            UnderlayStyle::Edge => out.push(edge()),
            UnderlayStyle::Zigzag => out.push(rows(across)),
            UnderlayStyle::EdgeZigzag => {
                out.push(edge());
                out.push(rows(across));
            }
            UnderlayStyle::Double => {
                out.push(rows(angle + 45.0));
                out.push(rows(angle - 45.0));
            }
        }
    }
    let shaped = r.gradation.is_some() || r.feather.is_some() || r.split_length.is_some();
    if !row_based {
        out.extend(regions::generate(closed, r, angle, holes, user));
        return out;
    }
    let mut runs = match r.kind {
        RegionType::Fill if !shaped => generate::fill(closed, &params, holes),
        RegionType::Satin if !shaped => generate::satin(closed, &params, holes),
        _ => shaped_rows(closed, r, &params, holes),
    };
    if r.travel {
        runs = reroute(runs, closed, holes);
    }
    out.push(runs);
    out
}

/// Row connections that cut across a gap (a hole, a notch) are split off, then every gap is
/// bridged by a running path inside the region.
pub fn reroute(runs: Vec<Run>, closed: &[Contour], holes: &[Contour]) -> Vec<Run> {
    let pieces: Vec<Run> = runs
        .into_iter()
        .flat_map(|run| split_outside(run, closed, holes))
        .collect();
    let mut graph = TravelGraph::new(closed, holes);
    join(pieces, Some(&mut graph), 25.0)
}

/// Splits `run` wherever a stitch leaves the region (its midpoint is outside and more than
/// 0.05 mm from the outline).
fn split_outside(run: Run, closed: &[Contour], holes: &[Contour]) -> Vec<Run> {
    let edges: Vec<(Vec2, Vec2)> = closed
        .iter()
        .chain(holes)
        .flat_map(Contour::edges)
        .collect();
    let near = |p: Vec2| {
        edges.iter().any(|&(a, b)| {
            let d = b - a;
            let l2 = d.x * d.x + d.y * d.y;
            let t = if l2 > 0.0 {
                (((p - a).x * d.x + (p - a).y * d.y) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            p.distance(a + d * t) <= 0.5
        })
    };
    let inside = |p: Vec2| contains(p, closed) && !contains(p, holes);
    let mut out = Vec::new();
    let mut current: Run = Vec::new();
    for p in run {
        if let Some(&last) = current.last() {
            let m = last.lerp(p, 0.5);
            if !inside(m) && !near(m) {
                out.push(std::mem::take(&mut current));
            }
        }
        current.push(p);
    }
    out.push(current);
    out.retain(|r| !r.is_empty());
    out
}

/// Direction (degrees) whose rows run across the region's narrow side: perpendicular to the
/// principal axis of the outline (second moments of the edges, weighted by length).
pub fn auto_angle(contours: &[Contour]) -> f64 {
    let mut w = 0.0;
    let (mut mx, mut my) = (0.0, 0.0);
    let edges: Vec<(Vec2, f64)> = contours
        .iter()
        .flat_map(Contour::edges)
        .map(|(a, b)| (a.lerp(b, 0.5), a.distance(b)))
        .collect();
    for &(m, l) in &edges {
        w += l;
        mx += m.x * l;
        my += m.y * l;
    }
    if w <= 0.0 {
        return 0.0;
    }
    let (cx, cy) = (mx / w, my / w);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for &(m, l) in &edges {
        let (dx, dy) = (m.x - cx, m.y - cy);
        sxx += dx * dx * l;
        syy += dy * dy * l;
        sxy += dx * dy * l;
    }
    let major = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let deg = major.to_degrees() + 90.0;
    // Normalise to (-90, 90] and round to 0.01° so output does not depend on noise.
    let mut a = deg.rem_euclid(180.0);
    if a > 90.0 {
        a -= 180.0;
    }
    (a * 100.0).round() / 100.0
}

/// Rows with gradation, feathered ends or split satin stitches.
fn shaped_rows(
    closed: &[Contour],
    r: &RegionSewing,
    p: &RegionParams,
    holes: &[Contour],
) -> Vec<Run> {
    let ys = row_positions(closed, p.angle, p.row_spacing, r.gradation);
    let mut spans = Vec::new();
    for (row, &y) in ys.iter().enumerate() {
        for s in spans_at(closed, holes, p.angle, y) {
            let lo = s.start() - p.pull_compensation;
            let hi = s.end() + p.pull_compensation;
            if hi > lo {
                spans.push(RowSpan {
                    row,
                    y,
                    range: lo..=hi,
                });
            }
        }
    }
    let l = p.stitch_length;
    let split = r.split_length.map(|s| s * 10.0);
    let satin = r.kind == RegionType::Satin;
    let feather = r.feather;
    // Rows are ordered on their full spans; feathering then shortens the ends, so the
    // sewing order does not depend on the random ends.
    generate::chain_spans(&spans, p, |span, row, forward| {
        let (mut lo, mut hi) = (*span.start(), *span.end());
        if let Some(f) = feather {
            let mut rng =
                Rng::new(0x5EED_F00D ^ (row as u64).wrapping_mul(0x9E37_79B9) ^ lo.to_bits());
            let w = (f.width * 10.0).min((hi - lo) * 0.45);
            if matches!(f.side, FeatherSide::Start | FeatherSide::Both) {
                lo += rng.next_f64() * w;
            }
            if matches!(f.side, FeatherSide::End | FeatherSide::Both) {
                hi -= rng.next_f64() * w;
            }
        }
        let span = lo..=hi;
        let mut xs = if satin {
            satin_points(&span, row, split)
        } else {
            tatami_points(&span, row, l)
        };
        if !forward {
            xs.reverse();
        }
        xs
    })
}

/// Satin row: one stitch, or with `split` equal stitches no longer than `split`, staggered by
/// half a stitch on odd rows so the split points do not line up.
fn satin_points(span: &Span, row: usize, split: Option<f64>) -> Vec<f64> {
    let (a, b) = (*span.start(), *span.end());
    let len = b - a;
    let Some(split) = split.filter(|&s| s > 0.0 && len > s) else {
        return vec![a, b];
    };
    let n = (len / split).ceil();
    let step = len / n;
    let mut xs = vec![a];
    if row % 2 == 1 {
        xs.extend((0..n as usize).map(|k| a + (k as f64 + 0.5) * step));
    } else {
        xs.extend((1..n as usize).map(|k| a + k as f64 * step));
    }
    xs.push(b);
    xs
}

/// Tatami needle positions, as [`generate::fill`] places them.
fn tatami_points(span: &Span, row: usize, l: f64) -> Vec<f64> {
    let phase = (row % 3) as f64 / 3.0 * l;
    let mut xs = vec![*span.start()];
    let mut k = ((span.start() - phase) / l).floor() + 1.0;
    while phase + k * l < *span.end() {
        let x = phase + k * l;
        if x - span.start() > l * 0.25 && span.end() - x > l * 0.25 {
            xs.push(x);
        }
        k += 1.0;
    }
    xs.push(*span.end());
    xs
}

/// Bounds of `contours` rotated by −`angle`.
pub fn rotated_bounds(contours: &[Contour], angle: f64) -> Option<Rect> {
    let rad = -angle.to_radians();
    Rect::enclosing(
        contours
            .iter()
            .flat_map(|c| c.outline_points())
            .map(|p| p.rotated(rad)),
    )
}

/// Row positions (rotated space): `spacing` apart, or with gradation the spacing grows (or
/// shrinks) so the density falls linearly to `gradation` % at the far side.
pub fn row_positions(
    contours: &[Contour],
    angle: f64,
    spacing: f64,
    gradation: Option<f64>,
) -> Vec<f64> {
    let Some(b) = rotated_bounds(contours, angle) else {
        return Vec::new();
    };
    if spacing.is_nan() || spacing <= 0.0 || b.height().is_nan() || b.height() <= 0.0 {
        return Vec::new();
    }
    let end = gradation.map_or(1.0, |g| (g / 100.0).clamp(0.05, 1.0));
    let mut ys = Vec::new();
    let mut y = b.min_y + spacing / 2.0;
    while y < b.max_y && ys.len() < 100_000 {
        ys.push(y);
        let f = (y - b.min_y) / b.height();
        y += spacing / (1.0 + (end - 1.0) * f);
    }
    ys
}

/// Inside intervals of the row at rotated `y` (even-odd), minus the holes.
pub fn spans_at(contours: &[Contour], holes: &[Contour], angle: f64, y: f64) -> Vec<Span> {
    let rad = -angle.to_radians();
    let xs_at = |cs: &[Contour]| -> Vec<Span> {
        let mut xs: Vec<f64> = cs
            .iter()
            .filter(|c| c.closed)
            .flat_map(Contour::edges)
            .map(|(p, q)| (p.rotated(rad), q.rotated(rad)))
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
    };
    let spans = xs_at(contours);
    if holes.is_empty() {
        spans
    } else {
        subtract(&xs_at(holes), &spans)
    }
}

/// Maps a rotated-space point back to design space.
pub fn unrotate(x: f64, y: f64, angle: f64) -> Vec2 {
    v2(x, y).rotated(angle.to_radians())
}
