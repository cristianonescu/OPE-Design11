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
/// stitches no longer than `pitch`, curved segments split by halving the curve's parameter
/// until no chord is longer than `pitch` (see [`halve_curve`]).
pub fn along(contour: &Contour, pitch: f64) -> Vec<Vec2> {
    if !contour.has_curves() {
        return subdivide(&contour.path(), pitch);
    }
    let mut out: Vec<Vec2> = contour.points.first().copied().into_iter().collect();
    for i in 0..contour.segment_count() {
        let (a, b) = contour.segment_ends(i);
        match contour.curve(i) {
            None => out.extend(subdivide(&[a, b], pitch).into_iter().skip(1)),
            Some(c) => out.extend(halve_curve(a, c.c1, c.c2, b, pitch).into_iter().skip(1)),
        }
    }
    out
}

/// Most halvings of a curve's parameter when sampling it for stitches (2^16 chords): a
/// safety stop far beyond any curve a design page holds (an 800 mm circle's quarter arc at a
/// 2 mm pitch takes 9 halvings), so a chord longer than the pitch never survives on a long or
/// tight curve. Until 2026-10-29 the stop was 2^8 chords, which left 2.6 mm stitches on that
/// circle (#121); no PE-DESIGN reference reaches that far, its page being at most 360 mm.
const MAX_CURVE_HALVINGS: u32 = 16;

/// Needle points on the cubic Bézier `a`, `c1`, `c2`, `b` at equal parameter steps, the
/// parameter halved until every chord is no longer than `pitch`: PE-DESIGN 11.50 sews a
/// 60 mm circle's running stitch (pitch 2 mm) as 4 × 32 chords of 1.44-1.60 mm whose needle
/// points lie at the quarter arcs' parameters k/32 (compat text-fit-default), not at equal
/// arc length and not on a flatness-tolerance polyline resampled at the pitch (#121: that
/// would move every needle point off PE's). Outlines, by contrast, are flattened adaptively to
/// [`crate::geometry::FLATNESS`], where no reference pins the sampling.
pub fn halve_curve(a: Vec2, c1: Vec2, c2: Vec2, b: Vec2, pitch: f64) -> Vec<Vec2> {
    let at = |t: f64| {
        let u = 1.0 - t;
        a * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + b * (t * t * t)
    };
    let pitch = pitch.max(1e-6);
    let mut halvings = 0;
    loop {
        let n = 1usize << halvings;
        let pts: Vec<Vec2> = (0..=n).map(|k| at(k as f64 / n as f64)).collect();
        if halvings >= MAX_CURVE_HALVINGS
            || pts.windows(2).all(|w| w[0].distance(w[1]) <= pitch + 1e-9)
        {
            return pts;
        }
        halvings += 1;
    }
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
    fill_shifted(contours, p, holes, Stagger::Third)
}

/// How the needle grid of a tatami fill moves from row to row (the stagger that keeps the
/// penetrations from lining up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stagger {
    /// A third of a stitch per row, anchored at the origin: OpenStitch's placement before
    /// Fill Frequency existed, kept for `.osd` files without a Frequency.
    Third,
    /// PE-DESIGN's Frequency: the grid moves this fraction of a stitch per row; row k's grid
    /// lies (k + 1) times it from the first row's start (regular brick).
    Regular(f64),
    /// OpenStitch's own random stagger: row k's grid lies at a pseudo-random fraction of a
    /// stitch, drawn from this seed and k, so the output is reproducible and no two rows of
    /// a region line up by rule. PE-DESIGN offers nothing like it (its Fill pane has the
    /// Frequency field only).
    Random(u64),
}

impl Stagger {
    /// Offset of row `row`'s needle grid, fraction of a stitch, from the page origin.
    pub fn phase(self, row: usize) -> f64 {
        match self {
            Stagger::Third => (row % 3) as f64 / 3.0,
            Stagger::Regular(s) => (row as f64 * s).fract(),
            Stagger::Random(seed) => random_fraction(seed, row),
        }
    }

    /// Offset of row `k`'s grid from the first row's start (PE-DESIGN's anchor, see
    /// [`grid_anchor`]), fraction of a stitch; `None` for the origin-anchored [`Stagger::Third`].
    pub fn anchored(self, k: usize) -> Option<f64> {
        match self {
            Stagger::Third => None,
            Stagger::Regular(f) => Some(((k as f64 + 1.0) * f).fract()),
            Stagger::Random(seed) => Some(random_fraction(seed, k)),
        }
    }
}

/// A pseudo-random fraction in [0, 1) for row `row` of a fill seeded with `seed`: one
/// SplitMix64 step (see [`crate::paths::Rng`]) of the seed mixed with the row number, so
/// every row draws its own value and the same seed always gives the same rows.
fn random_fraction(seed: u64, row: usize) -> f64 {
    let mixed = seed ^ (row as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    crate::paths::Rng::new(mixed).next_f64()
}

/// Offset of row `row`'s needle grid, fraction of a stitch (see [`Stagger::phase`]).
pub fn row_phase(row: usize, stagger: Stagger) -> f64 {
    stagger.phase(row)
}

/// [`fill`] whose needle grid moves from row to row as `stagger` says (see [`Stagger`]).
pub fn fill_shifted(
    contours: &[Contour],
    p: &RegionParams,
    holes: &[Contour],
    stagger: Stagger,
) -> Vec<Run> {
    let l = p.stitch_length;
    sew_rows(contours, p, holes, |span, row, forward| {
        let mut xs = tatami_xs(span, stagger.phase(row) * l, l);
        if !forward {
            xs.reverse();
        }
        xs
    })
}

/// Shortest row span [`fill_edge_to_edge`] sews, 0.1 mm.
const MIN_EDGE_ROW_SPAN: f64 = 3.0;

/// [`fill_shifted`] with PE-DESIGN's row placement: the first row lies on one edge of the
/// region and the last on the other (a 40 mm square at 4.5 rows/mm has 181 rows), instead of
/// starting half a row spacing in. Rows on an edge are sampled just inside it. With an
/// `entry` point (design space) the rows chain from the row end nearest it, so the fill starts
/// at the entry instead of travelling along the outline to the first row.
pub fn fill_edge_to_edge(
    contours: &[Contour],
    p: &RegionParams,
    holes: &[Contour],
    stagger: Stagger,
    entry: Option<Vec2>,
    sloped: bool,
) -> Vec<Run> {
    let l = p.stitch_length;
    let start = entry.map(|e| e.rotated(-p.angle.to_radians()));
    let anchor = grid_anchor(contours, p, holes);
    if let (Some(s), Some(a)) = (start, anchor)
        && let Some(run) = pe_halves(
            &edge_row_spans(contours, p, holes, true),
            p,
            stagger,
            s,
            a,
            sloped,
        )
    {
        return vec![run];
    }
    // Regions with several spans per row keep the older order; with a Frequency their rows
    // take PE-DESIGN's spacing (stretched to the far edge, as in its pentagon) and grid (row k
    // at (k + 1)·shift from the rotated bounds' minimum, points down to 0.3 mm from the ends).
    let spans = edge_row_spans(contours, p, holes, stagger.anchored(0).is_some());
    let min_x = anchor;
    // The needle grid moves on in sewing order, as in PE-DESIGN 11: a fill sewn from its last
    // row (an entry at the bottom of a rectangle) counts the rows from there.
    let last = spans.iter().map(|s| s.row).max().unwrap_or(0);
    let from_last = match (start, spans.first(), spans.last()) {
        (Some(st), Some(f), Some(e)) => (st.y - f.y).abs() > (st.y - e.y).abs(),
        _ => false,
    };
    chain(
        &spans,
        p,
        |span, row, forward| {
            let mut xs = match (stagger.anchored(row), min_x) {
                (Some(f), Some(min_x)) => {
                    let phase = min_x + f * l;
                    let mut xs = vec![*span.start()];
                    xs.extend(pe_grid(span, phase, l));
                    xs.push(*span.end());
                    xs
                }
                _ => {
                    let row = if from_last { last - row } else { row };
                    tatami_xs(span, stagger.phase(row) * l, l)
                }
            };
            if !forward {
                xs.reverse();
            }
            xs
        },
        Chain::Brick,
        start,
    )
}

/// Longest travel stitch of PE-DESIGN's fill (its entry, exit and between-half runs), 0.1 mm.
const FILL_TRAVEL: f64 = 20.0;
/// PE-DESIGN leaves out a needle grid point closer than this to its row's end, 0.1 mm (it
/// keeps 0.4 mm, drops 0.2 mm).
const PE_END_GAP: f64 = 3.0;

/// Where PE-DESIGN's needle grid starts (rotated space): the start of the first row, on the
/// outline's extreme point across the rows (a square's corner, a pentagon's top vertex).
fn grid_anchor(contours: &[Contour], p: &RegionParams, holes: &[Contour]) -> Option<f64> {
    let b = crate::region::rotated_bounds(contours, p.angle)?;
    let y = b.min_y + 1e-3_f64.min(b.height() / 4.0);
    Some(
        crate::region::spans_at(contours, holes, p.angle, y)
            .first()
            .map_or(b.min_x, |s| *s.start()),
    )
}

/// PE-DESIGN's needle grid inside `span` (its ends left out): every `l` from `phase`, without
/// points closer than [`PE_END_GAP`] to an end.
fn pe_grid(span: &Span, phase: f64, l: f64) -> Vec<f64> {
    let (lo, hi) = (*span.start(), *span.end());
    let mut j = ((lo - phase) / l).floor();
    let mut xs = Vec::new();
    loop {
        let x = phase + j * l;
        if x >= hi - PE_END_GAP {
            break;
        }
        if x > lo + PE_END_GAP {
            xs.push(x);
        }
        j += 1.0;
    }
    xs
}

/// PE-DESIGN 11's fill order for a region whose every row is one span (a convex region),
/// starting and ending at `s` (rotated space). The rows on each side of `s` are a half, sewn
/// from its far edge in towards `s`; the row through `s` belongs to both, and a half holding
/// nothing but that row is left out. The half on the rotated minimum side goes first. Each
/// half's first row is sewn along the row direction (+x) and its last row back (−x), so the
/// first row is sewn out and back when the half has an odd number of rows. Rows sewn +x keep
/// both edge points; rows sewn −x have none (the stitches go straight from row to row), except
/// a half's last row, and except when rows are more than [`FILL_TRAVEL`] apart: then every row
/// keeps its ends and the needle walks from row to row in stitches up to that length. Straight
/// runs of stitches up to [`FILL_TRAVEL`] go from `s` to each half's first row and from the last
/// row back to `s`. Row k's needle grid lies `(k + 1)·shift` of a stitch from `min_x` (see
/// [`grid_anchor`]), k counted from the minimum side (PE's Frequency, or the seeded random
/// stagger); with [`Stagger::Third`] the old ⅓ stagger stays.
/// `None` when a row has more than one span or there are no rows.
fn pe_halves(
    spans: &[RowSpan],
    p: &RegionParams,
    stagger: Stagger,
    s: Vec2,
    min_x: f64,
    sloped: bool,
) -> Option<Run> {
    let (first, last) = (spans.first()?, spans.last()?);
    let rows = last.row.checked_sub(first.row)? + 1;
    if spans.len() != rows || spans.windows(2).any(|w| w[1].row != w[0].row + 1) {
        return None;
    }
    let l = p.stitch_length;
    let gap = if spans.len() > 1 {
        (last.y - first.y) / (spans.len() - 1) as f64
    } else {
        p.row_spacing
    };
    // The row through s: the nearest row within half a spacing.
    let through = spans
        .iter()
        .position(|r| (r.y - s.y).abs() <= gap / 2.0 + 1e-9);
    let low: Vec<&RowSpan> = spans
        .iter()
        .enumerate()
        .filter(|(i, r)| through.map_or(r.y < s.y, |t| *i <= t))
        .map(|(_, r)| r)
        .collect();
    let mut high: Vec<&RowSpan> = spans
        .iter()
        .enumerate()
        .filter(|(i, r)| through.map_or(r.y > s.y, |t| *i >= t))
        .map(|(_, r)| r)
        .collect();
    high.reverse();
    let mut halves = vec![low, high];
    let shared_only = |h: &Vec<&RowSpan>| h.len() == 1 && through.is_some();
    if halves.iter().any(|h| h.len() > 1) {
        halves.retain(|h| !h.is_empty() && !shared_only(h));
    }
    let walk = gap > FILL_TRAVEL;
    let pull = p.pull_compensation.max(0.0);
    let grid = |r: &RowSpan| -> Vec<f64> {
        let k = (r.row - first.row) as f64;
        let phase = match stagger.anchored(k as usize) {
            Some(f) => min_x + f * l,
            None => stagger.phase(r.row) * l,
        };
        pe_grid(&r.range, phase, l)
    };
    let home = from_rotated(s, p.angle);
    let mut run: Run = vec![home];
    // Straight travel in equal stitches up to FILL_TRAVEL; a needle point within 0.1 mm of
    // the target (a row end sampled just inside the outline) is moved onto it.
    let go = |run: &mut Run, to: Vec2| {
        let from = *run.last().expect("starts at s");
        let d = from.distance(to);
        if d < 1.0 {
            *run.last_mut().expect("starts at s") = to;
            return;
        }
        let n = ((d / FILL_TRAVEL) - 1e-3).ceil().max(1.0) as usize;
        run.extend((1..=n).map(|k| from.lerp(to, k as f64 / n as f64)));
    };
    for half in halves.iter().filter(|h| !h.is_empty()) {
        if sloped && half.len() > 1 {
            sloped_half(half, &mut run, &go, pull, p, &|b: &RowSpan| {
                // Grid step k · f · L for the pass ending on row k (anchored(k - 1) is k · f).
                let k = b.row - first.row;
                let f = match k.checked_sub(1) {
                    Some(k) => stagger.anchored(k),
                    None => stagger.anchored(0).map(|_| 0.0),
                };
                match f {
                    Some(f) => min_x + f * l,
                    None => stagger.phase(b.row.saturating_sub(1)) * l,
                }
            });
            go(&mut run, home);
            continue;
        }
        let odd = half.len() % 2 == 1;
        let mut passes = vec![(half[0], true)];
        if odd {
            passes.push((half[0], false));
        }
        for (i, r) in half.iter().enumerate().skip(1) {
            passes.push((r, (i % 2 == 1) == odd));
        }
        let n = passes.len();
        for (i, (r, forward)) in passes.into_iter().enumerate() {
            let (lo, hi) = (*r.range.start(), *r.range.end());
            let mut xs = grid(r);
            let ends = forward || walk || i + 1 == n;
            if ends {
                xs.insert(0, lo);
                xs.push(hi);
            }
            if !forward {
                xs.reverse();
            }
            let pts: Vec<Vec2> = xs
                .iter()
                .map(|&x| from_rotated(v2(x, r.y), p.angle))
                .collect();
            let Some(&head) = pts.first() else { continue };
            if i == 0 {
                // Travel reaches the outline; the row end lies the pull compensation beyond.
                go(&mut run, from_rotated(v2(lo + pull, r.y), p.angle));
                run.push(head);
            } else if walk {
                go(&mut run, head);
            } else {
                run.push(head);
            }
            run.extend(pts.into_iter().skip(1));
            if i + 1 == n {
                go(&mut run, from_rotated(v2(lo + pull, r.y), p.angle));
            }
        }
        go(&mut run, home);
    }
    run.dedup_by(|a, b| a.distance(*b) < 1e-9);
    Some(run)
}

/// One half of PE-DESIGN's Stitch Type 2 fill (its second Stitch Type button): sloped rows.
/// Pass i runs from row i's edge to row i + 1's far edge, rising one row spacing over its
/// length, so consecutive passes meet at the edges in one continuous zigzag with no step at the
/// row ends (PE-DESIGN 11.50, round-5 captures tatami-40-st2-dir0/30/90 and round 4's
/// tatami-40-stitch-type-2: every pass rises 0.20-0.22 mm, Type 1's are flat). The needle grid
/// along each pass ([`pe_grid`], the Frequency rule) is the one of the row before its end row in
/// row order: the start row in the half sewn upward, two rows past the end in the half sewn
/// downward (tatami-40-st2-dir90's second half), each point at the height interpolated along
/// the pass. The half's first pass is sewn along the row direction
/// (+x) from its edge point, after the travel from the start point; the half ends at its last
/// row's start side, as Type 1 does.
fn sloped_half(
    half: &[&RowSpan],
    run: &mut Run,
    go: &dyn Fn(&mut Run, Vec2),
    pull: f64,
    p: &RegionParams,
    phase: &dyn Fn(&RowSpan) -> f64,
) {
    let l = p.stitch_length;
    let mut forward = true;
    for (i, w) in half.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let (from, to) = if forward {
            (*a.range.start(), *b.range.end())
        } else {
            (*a.range.end(), *b.range.start())
        };
        let (lo, hi) = (from.min(to), from.max(to));
        let mut xs = pe_grid(&(lo..=hi), phase(b), l);
        if !forward {
            xs.reverse();
        }
        let along = (to - from).abs().max(1e-9);
        let at = |x: f64| {
            let t = ((x - from).abs() / along).clamp(0.0, 1.0);
            from_rotated(v2(x, a.y + (b.y - a.y) * t), p.angle)
        };
        if i == 0 {
            // Travel reaches the outline; the row end lies the pull compensation beyond.
            let edge = if forward { from + pull } else { from - pull };
            go(run, from_rotated(v2(edge, a.y), p.angle));
            run.push(at(from));
        }
        run.extend(xs.into_iter().map(at));
        run.push(at(to));
        forward = !forward;
    }
    if let Some(last) = half.last() {
        go(
            run,
            from_rotated(v2(*last.range.start() + pull, last.y), p.angle),
        );
    }
}

/// Rows from the rotated bounds' minimum to their maximum, `row_spacing` apart, lengthened by
/// the pull compensation. With `stretch` the spacing grows so that a whole number of gaps
/// fills the height and the last row lies on the far edge (PE-DESIGN: 149 rows over 33 mm at
/// 4.5 rows/mm, 33/148 mm apart). Spans shorter than
/// [`MIN_EDGE_ROW_SPAN`] are left out: an end row that only touches a curved outline at its
/// tip would otherwise be a stitch a fraction of a millimetre long.
fn edge_row_spans(
    contours: &[Contour],
    p: &RegionParams,
    holes: &[Contour],
    stretch: bool,
) -> Vec<RowSpan> {
    let Some(b) = crate::region::rotated_bounds(contours, p.angle) else {
        return Vec::new();
    };
    let (h, s) = (b.height(), p.row_spacing);
    if !(h > 0.0 && s > 0.0) {
        return Vec::new();
    }
    let n = ((h / s) + 1e-6).floor().clamp(1.0, 100_000.0) as usize;
    let s = if stretch { h / n as f64 } else { s };
    let inside = 1e-3_f64.min(h / 4.0);
    (0..=n)
        .flat_map(|row| {
            // Sampled just past the row's line, so a row on a horizontal edge inside the region
            // (a notch's end) does not run along that edge across the gap.
            let y = (b.min_y + row as f64 * s + inside).clamp(b.min_y + inside, b.max_y - inside);
            crate::region::spans_at(contours, holes, p.angle, y)
                .into_iter()
                .filter_map(move |sp| {
                    let lo = sp.start() - p.pull_compensation;
                    let hi = sp.end() + p.pull_compensation;
                    (hi - lo >= MIN_EDGE_ROW_SPAN).then_some(RowSpan {
                        row,
                        y,
                        range: lo..=hi,
                    })
                })
        })
        .collect()
}

/// Tatami needle positions along `span`: its ends, and the `l` grid shifted by `phase` in
/// between, leaving out grid points within a quarter stitch of an end.
fn tatami_xs(span: &Span, phase: f64, l: f64) -> Vec<f64> {
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

/// Satin region: one needle point per row on alternate edges, so every stitch crosses the
/// region (see [`chain_satin_spans`]).
pub fn satin(contours: &[Contour], p: &RegionParams, holes: &[Contour]) -> Vec<Run> {
    chain_satin_spans(&row_spans(contours, p, holes), p, |span, _, forward| {
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
    chain_spans(&row_spans(contours, p, holes), p, points)
}

/// Inside intervals of every scan row, lengthened by the pull compensation at both ends. A
/// negative pull compensation (an inset) leaves out spans shorter than twice the inset: they
/// would collapse to a needle point outside the region.
fn row_spans(contours: &[Contour], p: &RegionParams, holes: &[Contour]) -> Vec<RowSpan> {
    scan_rows(contours, p.angle, p.row_spacing, holes)
        .into_iter()
        .flat_map(|r| {
            let (row, y) = (r.index, r.y);
            r.spans.into_iter().filter_map(move |s| {
                let lo = s.start() - p.pull_compensation;
                let hi = s.end() + p.pull_compensation;
                (hi >= lo).then_some(RowSpan {
                    row,
                    y,
                    range: lo..=hi,
                })
            })
        })
        .collect()
}

/// Sews prepared row spans (see [`sew_rows`]): greedy nearest-end chaining, needle positions
/// along each span from `points(span, row, forward)`, mapped back from rotated space.
pub fn chain_spans(
    spans: &[RowSpan],
    p: &RegionParams,
    points: impl Fn(&Span, usize, bool) -> Vec<f64>,
) -> Vec<Run> {
    chain(spans, p, points, Chain::Fill, None)
}

/// [`chain_spans`] for satin: when a row follows the previous one along the same edge (the
/// next row, entered within three row distances of the last needle point), its first needle
/// point is left out. The stitch then goes from one edge straight to the far edge of the next
/// row, so needle points alternate between the edges and no short stitch runs along an edge.
/// The first row of each run is sewn across in full.
pub fn chain_satin_spans(
    spans: &[RowSpan],
    p: &RegionParams,
    points: impl Fn(&Span, usize, bool) -> Vec<f64>,
) -> Vec<Run> {
    chain(spans, p, points, Chain::Satin, None)
}

/// [`chain_spans`] for PE-DESIGN's satin sawtooth: when a row follows the previous one along
/// the same edge, it is entered from its far end, so every row is sewn whole in the same
/// direction and the stitch between rows slants back across the column. Rows are then two
/// crossings apart (twice the satin's line spacing).
pub fn chain_sawtooth_spans(
    spans: &[RowSpan],
    p: &RegionParams,
    points: impl Fn(&Span, usize, bool) -> Vec<f64>,
) -> Vec<Run> {
    chain(spans, p, points, Chain::Sawtooth, None)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Chain {
    Fill,
    /// [`Chain::Fill`] with PE-DESIGN's brick row ends (the top fill).
    Brick,
    Satin,
    Sawtooth,
}

fn chain(
    spans: &[RowSpan],
    p: &RegionParams,
    points: impl Fn(&Span, usize, bool) -> Vec<f64>,
    mode: Chain,
    start: Option<Vec2>,
) -> Vec<Run> {
    let Some(first) = spans.first() else {
        return Vec::new();
    };
    let mut used = vec![false; spans.len()];
    let mut runs: Vec<Run> = Vec::new();
    let mut current: Run = Vec::new();
    let mut position = start.unwrap_or(v2(*first.range.start(), first.y));
    let join_limit = p.row_spacing * 3.0 + p.stitch_length;
    let mut last_row: Option<(usize, f64)> = None;
    let mut trim_end = false;
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
        let along_edge = !current.is_empty()
            && last_row.is_some_and(|(row, y)| {
                row.abs_diff(s.row) == 1 && best_d <= 3.0 * (s.y - y).abs() + 1e-9
            });
        let forward = if mode == Chain::Sawtooth && along_edge {
            !forward
        } else {
            forward
        };
        let xs = points(&s.range, s.row, forward);
        // Brick: PE-DESIGN's row ends. A row sewn backwards straight after the row next
        // to it has no edge points: the stitches run diagonally from the previous row's end to
        // its first grid point, and from its last grid point to the next row's start.
        // Rows shorter than a stitch keep their ends (the narrow rows at a pentagon's vertices,
        // hole-sewing-pentagon-star). PE-DESIGN drops them on a letter's 3.5 mm strokes
        // (text-size-h); the rule between the two is not settled.
        let brick = mode == Chain::Brick
            && !forward
            && along_edge
            && xs.len() > 2
            && s.range.end() - s.range.start() >= p.stitch_length;
        if mode == Chain::Brick && along_edge && trim_end && current.len() > 1 {
            current.pop();
        }
        trim_end = brick;
        let skip = usize::from((mode == Chain::Satin && xs.len() >= 2 && along_edge) || brick);
        current.extend(
            xs.iter()
                .skip(skip)
                .map(|&x| from_rotated(v2(x, s.y), p.angle)),
        );
        if let Some(&x) = xs.last() {
            position = v2(x, s.y);
        }
        last_row = Some((s.row, s.y));
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}
