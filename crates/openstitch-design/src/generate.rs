//! Independent, deterministic stitch-generation algorithms. Lengths are 0.1 mm.

use crate::geometry::{Contour, Span, Vec2, from_rotated, scan_rows, v2};

/// Needle points sewn one after another; moving between runs is travel.
pub type Run = Vec<Vec2>;

/// Keeps every vertex and splits each edge into equal parts no longer than `max_len`.
pub fn subdivide(path: &[Vec2], max_len: f64) -> Vec<Vec2> {
    let Some(&first) = path.first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    for w in path.windows(2) {
        let n = ((w[0].distance(w[1]) / max_len.max(1e-6)).ceil() as usize).max(1);
        out.extend((1..=n).map(|k| w[0].lerp(w[1], k as f64 / n as f64)));
    }
    out
}

/// Needle points along a contour: every vertex kept, straight segments split into equal
/// stitches no longer than `pitch`, curved segments split at equal arc length.
pub fn along(contour: &Contour, pitch: f64) -> Vec<Vec2> {
    if !contour.has_curves() {
        return subdivide(&contour.path(), pitch);
    }
    let mut out: Vec<Vec2> = contour.points.first().copied().into_iter().collect();
    for i in 0..contour.segment_count() {
        match contour.curve(i) {
            None => {
                let (a, b) = contour.segment_ends(i);
                out.extend(subdivide(&[a, b], pitch).into_iter().skip(1));
            }
            Some(_) => out.extend(
                resample(&contour.segment_polyline(i), pitch)
                    .into_iter()
                    .skip(1),
            ),
        }
    }
    out
}

/// Points at equal arc-length steps no longer than `max_len` along `poly`, ends included.
fn resample(poly: &[Vec2], max_len: f64) -> Vec<Vec2> {
    let lengths: Vec<f64> = poly.windows(2).map(|w| w[0].distance(w[1])).collect();
    let total: f64 = lengths.iter().sum();
    let (Some(&first), Some(&last)) = (poly.first(), poly.last()) else {
        return Vec::new();
    };
    let n = ((total / max_len.max(1e-6)).ceil() as usize).max(1);
    let step = total / n as f64;
    let mut out = vec![first];
    let (mut seg, mut walked) = (0, 0.0);
    for k in 1..n {
        let target = step * k as f64;
        while seg < lengths.len() - 1 && walked + lengths[seg] < target {
            walked += lengths[seg];
            seg += 1;
        }
        let t = if lengths[seg] > 0.0 {
            (target - walked) / lengths[seg]
        } else {
            0.0
        };
        out.push(poly[seg].lerp(poly[seg + 1], t.clamp(0.0, 1.0)));
    }
    out.push(last);
    out
}

/// Running stitch: vertices kept, segments split to `pitch`; `passes` > 1 sews back and forth.
pub fn running(contour: &Contour, pitch: f64, passes: u32) -> Run {
    let forward = along(contour, pitch);
    if forward.len() < 2 {
        return forward;
    }
    let mut run = forward.clone();
    for pass in 1..passes.max(1) {
        if pass % 2 == 1 {
            run.extend(forward.iter().rev().skip(1).copied());
        } else {
            run.extend(forward.iter().skip(1).copied());
        }
    }
    run
}

/// Triple stitch: every stitch forward, back, forward.
pub fn triple(contour: &Contour, pitch: f64) -> Run {
    let pts = along(contour, pitch);
    let Some(&first) = pts.first() else {
        return Vec::new();
    };
    let mut run = vec![first];
    for w in pts.windows(2) {
        run.extend([w[1], w[0], w[1]]);
    }
    run
}

/// Zigzag line of total `width`, alternating sides every `spacing` along the path.
pub fn zigzag(contour: &Contour, width: f64, spacing: f64) -> Run {
    let path = contour.path();
    if path.len() < 2 || spacing.is_nan() || spacing <= 0.0 {
        return path;
    }
    let mut run = Vec::new();
    let mut side = 1.0;
    let mut carried = 0.0;
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.distance(b);
        if len <= 0.0 {
            continue;
        }
        let dir = (b - a) * (1.0 / len);
        let normal = v2(-dir.y, dir.x);
        let mut t = carried;
        while t <= len {
            run.push(a + dir * t + normal * (side * width / 2.0));
            side = -side;
            t += spacing;
        }
        carried = t - len;
    }
    run
}

#[derive(Clone, Copy, Debug)]
pub struct RegionParams {
    pub angle: f64,
    pub row_spacing: f64,
    pub stitch_length: f64,
    pub pull_compensation: f64,
}

/// Tatami fill: rows at `angle`, needle points on a `stitch_length` grid shifted a third of a
/// stitch per row so penetrations do not line up.
pub fn fill(contours: &[Contour], p: &RegionParams, holes: &[Contour]) -> Vec<Run> {
    let l = p.stitch_length;
    sew_rows(contours, p, holes, |span, row, forward| {
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
        if !forward {
            xs.reverse();
        }
        xs
    })
}

/// Satin region: one stitch across the region per row, alternating direction.
pub fn satin(contours: &[Contour], p: &RegionParams, holes: &[Contour]) -> Vec<Run> {
    sew_rows(contours, p, holes, |span, _, forward| {
        if forward {
            vec![*span.start(), *span.end()]
        } else {
            vec![*span.end(), *span.start()]
        }
    })
}

/// One inside interval of a scan row, in rotated space.
pub struct RowSpan {
    pub row: usize,
    pub y: f64,
    pub range: Span,
}

/// Scanline rows → runs. Spans chain greedily (nearest unsewn span end), entering from the
/// nearer end; a new run starts when the next span is beyond three row spacings plus a stitch.
fn sew_rows(
    contours: &[Contour],
    p: &RegionParams,
    holes: &[Contour],
    points: impl Fn(&Span, usize, bool) -> Vec<f64>,
) -> Vec<Run> {
    let spans: Vec<RowSpan> = scan_rows(contours, p.angle, p.row_spacing, holes)
        .into_iter()
        .flat_map(|r| {
            let (row, y) = (r.index, r.y);
            r.spans.into_iter().map(move |s| {
                let lo = s.start() - p.pull_compensation;
                let hi = s.end() + p.pull_compensation;
                RowSpan {
                    row,
                    y,
                    range: lo..=hi.max(lo),
                }
            })
        })
        .collect();
    chain_spans(&spans, p, points)
}

/// Sews prepared row spans (see [`sew_rows`]): greedy nearest-end chaining, needle positions
/// along each span from `points(span, row, forward)`, mapped back from rotated space.
pub fn chain_spans(
    spans: &[RowSpan],
    p: &RegionParams,
    points: impl Fn(&Span, usize, bool) -> Vec<f64>,
) -> Vec<Run> {
    let Some(first) = spans.first() else {
        return Vec::new();
    };
    let mut used = vec![false; spans.len()];
    let mut runs: Vec<Run> = Vec::new();
    let mut current: Run = Vec::new();
    let mut position = v2(*first.range.start(), first.y);
    let join_limit = p.row_spacing * 3.0 + p.stitch_length;
    for _ in 0..spans.len() {
        let (mut best, mut forward, mut best_d) = (0, true, f64::INFINITY);
        for (i, s) in spans.iter().enumerate().filter(|(i, _)| !used[*i]) {
            let dl = position.distance(v2(*s.range.start(), s.y));
            let du = position.distance(v2(*s.range.end(), s.y));
            if dl < best_d {
                (best, forward, best_d) = (i, true, dl);
            }
            if du < best_d {
                (best, forward, best_d) = (i, false, du);
            }
        }
        used[best] = true;
        let s = &spans[best];
        if best_d > join_limit && !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
        let xs = points(&s.range, s.row, forward);
        current.extend(xs.iter().map(|&x| from_rotated(v2(x, s.y), p.angle)));
        if let Some(&x) = xs.last() {
            position = v2(x, s.y);
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}
