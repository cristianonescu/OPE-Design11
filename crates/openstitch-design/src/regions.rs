//! Region sew types beyond fill and satin: programmable fill, motif fill, piping, cross
//! stitch, concentric circle, radial, spiral, flexible spiral, stippling, net fill, zigzag net
//! fill and decorative fill. Independent algorithms specified from the public guide's
//! attribute lists. Lengths are 0.1 mm; output is deterministic (pseudo-random choices use a
//! fixed seed).

use crate::generate::{self, RegionParams, RowSpan, Run, subdivide};
use crate::geometry::{Contour, Rect, Span, Vec2, contains, v2};
use crate::model::{RegionSewing, RegionType};
use crate::paths::{Rng, TravelGraph, Walker, clip, join, order};
use crate::patterns::{self, NET_PATTERNS, Pattern, PatternKind};
use crate::region::{reroute, rotated_bounds, row_positions, spans_at};

/// Caps that keep generation bounded on huge regions or tiny spacings.
const MAX_ROWS: usize = 20_000;
const MAX_POINTS: usize = 2_000_000;

/// Layers (runs) of region type `r.kind` (not fill or satin), rows at `angle` degrees.
pub fn generate(
    closed: &[Contour],
    r: &RegionSewing,
    angle: f64,
    holes: &[Contour],
    user: &[Pattern],
) -> Vec<Vec<Run>> {
    let pitch = r.run_pitch_mm() * 10.0;
    let spacing = r.spacing_mm() * 10.0;
    let row_spacing = 10.0 / r.density.max(0.1);
    let (pw, ph) = r.pattern_size_mm();
    let (pw, ph) = (pw * 10.0, ph * 10.0);
    let region = Region {
        closed,
        holes,
        travel: r.travel,
        pitch,
    };
    match r.kind {
        RegionType::Fill | RegionType::Satin => Vec::new(),
        RegionType::ProgrammableFill => {
            let pattern = patterns::resolve(user, PatternKind::Fill, r.pattern.as_deref());
            let p = RegionParams {
                angle,
                row_spacing,
                stitch_length: r.stitch_length * 10.0,
                pull_compensation: r.pull_compensation * 10.0,
            };
            let tile = Tile {
                pattern,
                width: pw,
                height: ph,
                row_offset: r.row_offset.unwrap_or(0.0) / 100.0,
                column_offset: r.column_offset.unwrap_or(0.0) / 100.0,
            };
            let mut layers = Vec::new();
            if r.base_sewing {
                let base = RegionParams {
                    angle: angle + 90.0,
                    row_spacing: row_spacing * 2.0,
                    ..p
                };
                layers.push(region.rows(generate::fill(closed, &base, holes), base.stitch_length));
            }
            layers.push(region.rows(
                programmable(closed, holes, &p, &tile, r.frequency),
                p.stitch_length,
            ));
            layers
        }
        RegionType::Piping => {
            let p = RegionParams {
                angle,
                row_spacing,
                stitch_length: pitch,
                pull_compensation: 0.0,
            };
            vec![region.rows(piping(closed, holes, &p, r.frequency), pitch)]
        }
        RegionType::MotifFill => {
            let pattern = patterns::resolve(user, PatternKind::Motif, r.pattern.as_deref());
            let period = pw + spacing;
            let unit: Vec<Vec2> = pattern.paths[0]
                .iter()
                .map(|q| v2(q.x * pw, q.y * ph))
                .chain(std::iter::once(v2(period, 0.0)))
                .collect();
            let shift = r.row_offset.unwrap_or(50.0) / 100.0;
            let rows = tile_rows(closed, holes, angle, &unit, period, ph + spacing, shift);
            vec![region.sequence(rows)]
        }
        RegionType::DecorativeFill => {
            let pattern = patterns::resolve(user, PatternKind::Decorative, r.pattern.as_deref());
            let mut pieces = Vec::new();
            for path in &pattern.paths {
                let unit: Vec<Vec2> = path.iter().map(|q| v2(q.x * pw, q.y * ph)).collect();
                pieces.extend(tile_rows(
                    closed,
                    holes,
                    angle,
                    &unit,
                    pw,
                    ph,
                    pattern.row_shift,
                ));
            }
            let mut runs = region.sequence(pieces);
            if let Some(shift) = r.random_shift.filter(|&s| s > 0.0) {
                jitter(&mut runs, closed, holes, shift / 100.0 * pitch / 2.0);
            }
            vec![runs]
        }
        RegionType::CrossStitch => {
            vec![region.pieces_raw(cross_stitch(closed, holes, angle, spacing, r.triple))]
        }
        RegionType::ConcentricCircle => {
            let c = r.center.unwrap_or_else(|| centre(closed));
            let rings = concentric(closed, c, row_spacing, pitch);
            let mut pieces = Vec::new();
            for ring in rings {
                pieces.extend(clip(&ring, closed, holes));
            }
            if !r.start_inside {
                pieces.reverse();
            }
            vec![region.sequence(nearest_next(pieces))]
        }
        RegionType::Radial => {
            let c = r.center.unwrap_or_else(|| centre(closed));
            vec![region.sequence(radial(closed, holes, c, row_spacing, pitch))]
        }
        RegionType::Spiral => {
            let c = r.center.unwrap_or_else(|| centre(closed));
            let spiral = archimedean(c, reach(closed, c), row_spacing, pitch, r.counterclockwise);
            let mut pieces = clip(&spiral, closed, holes);
            if !r.start_inside {
                pieces.reverse();
                pieces.iter_mut().for_each(|p| p.reverse());
            }
            vec![region.sequence(nearest_next(pieces))]
        }
        RegionType::FlexibleSpiral => {
            let spiral = flexible_spiral(closed, spacing, pitch, r.counterclockwise);
            let mut pieces = clip(&spiral, closed, holes);
            if r.start_inside {
                pieces.reverse();
                pieces.iter_mut().for_each(|p| p.reverse());
            }
            vec![region.sequence(nearest_next(pieces))]
        }
        RegionType::Stippling => vec![region.loops(stipple(closed, holes, angle, spacing))],
        RegionType::NetFill | RegionType::ZigzagNetFill => {
            let id = r.pattern.as_deref().unwrap_or(NET_PATTERNS[0].id);
            let mut out = Vec::new();
            for family in net_families(closed, holes, angle, spacing, id) {
                let pieces = family;
                if r.kind == RegionType::NetFill {
                    out.extend(pieces.iter().map(|p| subdivide(p, pitch)));
                    continue;
                }
                let width = pw.min(spacing * 0.8);
                let step = 10.0 / r.density.max(0.1);
                for piece in pieces {
                    // Zigzag over the line, starting and ending on the line itself so
                    // travel between lines stays inside the region.
                    let line = Contour::new(piece.clone(), false);
                    let mut zz = generate::zigzag(&line, width, step);
                    let (first, last) = (piece[0], *piece.last().expect("2+ points"));
                    let mut run = Vec::new();
                    if r.underlay {
                        run.extend(subdivide(&piece, pitch));
                        zz.reverse();
                        run.extend(zz);
                        run.push(first);
                    } else {
                        run.push(first);
                        run.extend(zz);
                        run.push(last);
                    }
                    out.push(run);
                }
            }
            vec![region.sequence_raw(out)]
        }
    }
}

/// The region being sewn and how pieces are joined.
struct Region<'a> {
    closed: &'a [Contour],
    holes: &'a [Contour],
    travel: bool,
    pitch: f64,
}

impl Region<'_> {
    fn graph(&self) -> Option<TravelGraph> {
        self.travel.then(|| {
            let mut g = TravelGraph::new(self.closed, self.holes);
            g.along_edges = true;
            g
        })
    }

    /// Row runs (fill-like): connections across gaps rerouted inside when travelling; no
    /// stitch longer than `limit`.
    fn rows(&self, runs: Vec<Run>, limit: f64) -> Vec<Run> {
        let runs = if self.travel {
            reroute(runs, self.closed, self.holes)
        } else {
            runs
        };
        runs.iter().map(|r| subdivide(r, limit)).collect()
    }

    /// Pieces already in sewing order: split to `pitch`, joined.
    fn sequence(&self, pieces: Vec<Vec<Vec2>>) -> Vec<Run> {
        let runs = pieces
            .into_iter()
            .map(|p| subdivide(&p, self.pitch))
            .collect();
        self.sequence_raw(runs)
    }

    /// Runs already split into stitches, in sewing order: joined.
    fn sequence_raw(&self, runs: Vec<Run>) -> Vec<Run> {
        let runs: Vec<Run> = runs.into_iter().filter(|r| r.len() >= 2).collect();
        let mut graph = self.graph();
        join(runs, graph.as_mut(), self.pitch.max(10.0))
    }

    /// Runs whose stitches are already final (cross stitch groups): ordered, joined.
    fn pieces_raw(&self, runs: Vec<Run>) -> Vec<Run> {
        let runs = order_runs(runs);
        self.sequence_raw(runs)
    }

    /// Closed loops: each starts at its point nearest the previous end.
    fn loops(&self, loops: Vec<Vec<Vec2>>) -> Vec<Run> {
        let mut pos: Option<Vec2> = None;
        let mut remaining = loops;
        let mut out = Vec::new();
        while !remaining.is_empty() {
            let here = pos.unwrap_or(remaining[0][0]);
            let (li, pi) = remaining
                .iter()
                .enumerate()
                .flat_map(|(li, l)| l.iter().enumerate().map(move |(pi, p)| (li, pi, *p)))
                .min_by(|a, b| here.distance(a.2).total_cmp(&here.distance(b.2)))
                .map(|(li, pi, _)| (li, pi))
                .expect("non-empty loops");
            let l = remaining.remove(li);
            let mut rotated: Vec<Vec2> = l[pi..].iter().chain(&l[..pi]).copied().collect();
            rotated.push(rotated[0]);
            pos = Some(rotated[0]);
            out.push(rotated);
        }
        self.sequence_raw(out)
    }
}

/// Orders runs greedily like [`order`], keeping their points.
fn order_runs(runs: Vec<Run>) -> Vec<Run> {
    order(runs.into_iter().filter(|r| !r.is_empty()).collect(), None)
}

/// Centre of the region's bounding box.
pub fn centre(closed: &[Contour]) -> Vec2 {
    Rect::enclosing(closed.iter().flat_map(Contour::outline_points))
        .map(|b| b.center())
        .unwrap_or_default()
}

/// Largest distance from `c` to the outline.
fn reach(closed: &[Contour], c: Vec2) -> f64 {
    closed
        .iter()
        .flat_map(Contour::outline_points)
        .map(|p| p.distance(c))
        .fold(0.0, f64::max)
}

// ------------------------------------------------------------------ row-based types

/// Spans of every row of the region at `p.angle`, `p.row_spacing` apart.
fn row_spans(closed: &[Contour], holes: &[Contour], p: &RegionParams) -> Vec<RowSpan> {
    let ys = row_positions(closed, p.angle, p.row_spacing, None);
    let mut out = Vec::new();
    for (row, &y) in ys.iter().enumerate().take(MAX_ROWS) {
        for s in spans_at(closed, holes, p.angle, y) {
            let (lo, hi) = (
                s.start() - p.pull_compensation,
                s.end() + p.pull_compensation,
            );
            if hi > lo {
                out.push(RowSpan {
                    row,
                    y,
                    range: lo..=hi,
                });
            }
        }
    }
    out
}

/// Needle positions on a span: every `l`, the grid shifted by `phase` (0…1 of `l`), with
/// points closer than a quarter stitch to an end dropped.
fn grid_points(span: &Span, l: f64, phase: f64) -> Vec<f64> {
    let (a, b) = (*span.start(), *span.end());
    let offset = phase * l;
    let mut xs = vec![a];
    let mut k = ((a - offset) / l).floor() + 1.0;
    while offset + k * l < b {
        let x = offset + k * l;
        if x - a > l * 0.25 && b - x > l * 0.25 {
            xs.push(x);
        }
        k += 1.0;
    }
    xs.push(b);
    xs
}

/// Piping: rows of running stitches at `p.stitch_length`, each row's needle points shifted
/// by `frequency` % of a stitch from the previous row.
fn piping(
    closed: &[Contour],
    holes: &[Contour],
    p: &RegionParams,
    frequency: Option<f64>,
) -> Vec<Run> {
    let f = frequency.unwrap_or(33.0) / 100.0;
    let spans = row_spans(closed, holes, p);
    generate::chain_spans(&spans, p, |span, row, forward| {
        let mut xs = grid_points(span, p.stitch_length, (row as f64 * f).fract());
        if !forward {
            xs.reverse();
        }
        xs
    })
}

/// A programmable-fill tile placement.
struct Tile<'a> {
    pattern: &'a Pattern,
    width: f64,
    height: f64,
    /// Shift of every other tile row, fraction of the width.
    row_offset: f64,
    /// Shift of every other tile column, fraction of the height.
    column_offset: f64,
}

/// Programmable fill: fill rows whose needle points fall on the tiled pattern's lines (so
/// the pattern shows as a texture) plus a regular grid where the pattern leaves gaps.
fn programmable(
    closed: &[Contour],
    holes: &[Contour],
    p: &RegionParams,
    tile: &Tile,
    frequency: Option<f64>,
) -> Vec<Run> {
    let f = frequency.unwrap_or(33.0) / 100.0;
    let l = p.stitch_length;
    let spans = row_spans(closed, holes, p);
    generate::chain_spans(&spans, p, |span, row, forward| {
        let (a, b) = (*span.start(), *span.end());
        let marks = tile_crossings(tile, span, row_y(&spans, row), p.row_spacing, l);
        let mut xs: Vec<f64> = grid_points(span, l, (row as f64 * f).fract())
            .into_iter()
            .filter(|&x| x == a || x == b || marks.iter().all(|m| (m - x).abs() > l * 0.3))
            .collect();
        xs.extend(
            marks
                .iter()
                .copied()
                .filter(|&m| m - a > 2.0 && b - m > 2.0),
        );
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|x, y| (*x - *y).abs() < 2.0);
        // No stitch longer than the stitch length.
        let mut out = vec![xs[0]];
        for w in xs.windows(2) {
            let n = ((w[1] - w[0]) / l).ceil().max(1.0) as usize;
            out.extend((1..=n).map(|k| w[0] + (w[1] - w[0]) * k as f64 / n as f64));
        }
        if !forward {
            out.reverse();
        }
        out
    })
}

fn row_y(spans: &[RowSpan], row: usize) -> f64 {
    spans
        .iter()
        .find(|s| s.row == row)
        .map(|s| s.y)
        .unwrap_or_default()
}

/// Positions along the row at rotated `y` where the tiled pattern's lines cross it; lines
/// nearly parallel to the row (within one row spacing) add points every third of a stitch.
fn tile_crossings(tile: &Tile, span: &Span, y: f64, row_spacing: f64, l: f64) -> Vec<f64> {
    let (w, h) = (tile.width, tile.height);
    let (a, b) = (*span.start(), *span.end());
    let mut out = Vec::new();
    let first = (a / w).floor() as i64 - 1;
    let last = (b / w).ceil() as i64 + 1;
    for i in first..=last {
        let yo = if i.rem_euclid(2) == 1 {
            tile.column_offset * h
        } else {
            0.0
        };
        let j = ((y - yo) / h).floor();
        let xo = if (j as i64).rem_euclid(2) == 1 {
            tile.row_offset * w
        } else {
            0.0
        };
        let x0 = i as f64 * w + xo;
        let v = (y - yo - j * h) / h;
        for path in &tile.pattern.paths {
            for seg in path.windows(2) {
                let (p, q) = (seg[0], seg[1]);
                if ((p.y - q.y) * h).abs() < row_spacing {
                    let my = (p.y + q.y) / 2.0;
                    if ((my - v) * h).abs() <= row_spacing / 2.0 {
                        let (lo, hi) = (p.x.min(q.x) * w, p.x.max(q.x) * w);
                        let mut x = lo;
                        while x <= hi {
                            out.push(x0 + x);
                            x += l / 3.0;
                        }
                    }
                    continue;
                }
                if (p.y - v) * (q.y - v) <= 0.0 {
                    let u = p.x + (v - p.y) / (q.y - p.y) * (q.x - p.x);
                    out.push(x0 + u * w);
                }
            }
        }
    }
    out.retain(|&x| x > a && x < b);
    out.sort_by(f64::total_cmp);
    out
}

// ------------------------------------------------------------------ tiled rows

/// Rows of a periodic polyline (`unit`: one period from x = 0 to x = `period`, in rotated
/// space units) every `row_step`, each row shifted by `shift` periods more than the last;
/// mapped back to design space and clipped to the region minus `holes`. Returns the clipped
/// pieces in sewing order (see [`order_rows`]).
fn tile_rows(
    closed: &[Contour],
    holes: &[Contour],
    angle: f64,
    unit: &[Vec2],
    period: f64,
    row_step: f64,
    shift: f64,
) -> Vec<Vec<Vec2>> {
    let Some(b) = rotated_bounds(closed, angle) else {
        return Vec::new();
    };
    if period <= 0.0 || row_step <= 0.0 || unit.len() < 2 {
        return Vec::new();
    }
    let (ymin, ymax) = unit
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
    let first = ((b.min_y - ymax) / row_step).floor() as i64;
    let last = ((b.max_y - ymin) / row_step).ceil() as i64;
    let rows = (last - first).clamp(0, MAX_ROWS as i64);
    let periods = ((b.width() / period).ceil() as usize + 3).min(MAX_POINTS / unit.len().max(1));
    let mut out: Vec<Vec<Vec<Vec2>>> = Vec::new();
    let mut budget = MAX_POINTS;
    for k in first..first + rows + 1 {
        let y0 = k as f64 * row_step;
        let sx = (k as f64 * shift).rem_euclid(1.0) * period;
        let start = ((b.min_x - sx) / period).floor() - 1.0;
        let mut row: Vec<Vec2> = Vec::with_capacity(periods * unit.len());
        for n in 0..periods {
            let x0 = sx + (start + n as f64) * period;
            for (i, q) in unit.iter().enumerate() {
                if n > 0 && i == 0 {
                    continue;
                }
                row.push(v2(x0 + q.x, y0 + q.y).rotated(angle.to_radians()));
            }
        }
        budget = budget.saturating_sub(row.len());
        if budget == 0 {
            break;
        }
        out.push(clip(&row, closed, holes));
    }
    order_rows(out, angle)
}

/// Orders row pieces like a digitizer sews a fill: up or down through pieces of neighbouring
/// rows that overlap along the row (so each lobe of a shape is finished in one sweep), and
/// only when a sweep ends, on to the nearest unsewn piece. Each piece is entered from the end
/// nearer the needle.
fn order_rows(rows: Vec<Vec<Vec<Vec2>>>, angle: f64) -> Vec<Vec<Vec2>> {
    let u = v2(1.0, 0.0).rotated(angle.to_radians());
    struct Piece {
        row: usize,
        lo: f64,
        hi: f64,
        pts: Vec<Vec2>,
    }
    let mut pieces: Vec<Piece> = Vec::new();
    for (row, list) in rows.into_iter().enumerate() {
        for pts in list {
            let (lo, hi) = pts
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                    let x = p.x * u.x + p.y * u.y;
                    (lo.min(x), hi.max(x))
                });
            pieces.push(Piece { row, lo, hi, pts });
        }
    }
    let n = pieces.len();
    let mut sewn = vec![false; n];
    let mut out = Vec::with_capacity(n);
    if n == 0 {
        return out;
    }
    let end_distance = |p: &Piece, pos: Vec2| {
        pos.distance(p.pts[0])
            .min(pos.distance(*p.pts.last().expect("non-empty")))
    };
    let mut cur = 0;
    let mut pos: Option<Vec2> = None;
    let mut dir: i64 = 1;
    loop {
        sewn[cur] = true;
        let mut pts = std::mem::take(&mut pieces[cur].pts);
        if let Some(at) = pos
            && at.distance(*pts.last().expect("non-empty")) < at.distance(pts[0])
        {
            pts.reverse();
        }
        let here = *pts.last().expect("non-empty");
        pieces[cur].pts = vec![pts[0], here];
        out.push(pts);
        pos = Some(here);
        let (row, lo, hi) = (pieces[cur].row as i64, pieces[cur].lo, pieces[cur].hi);
        let neighbour = |d: i64| {
            (0..n)
                .filter(|&i| {
                    !sewn[i]
                        && pieces[i].row as i64 == row + d
                        && pieces[i].lo < hi
                        && pieces[i].hi > lo
                })
                .min_by(|&a, &b| {
                    end_distance(&pieces[a], here).total_cmp(&end_distance(&pieces[b], here))
                })
        };
        if let Some(next) = neighbour(dir) {
            cur = next;
        } else if let Some(next) = neighbour(-dir) {
            dir = -dir;
            cur = next;
        } else if let Some(next) = (0..n).filter(|&i| !sewn[i]).min_by(|&a, &b| {
            end_distance(&pieces[a], here).total_cmp(&end_distance(&pieces[b], here))
        }) {
            dir = if pieces[next].row as i64 >= row {
                1
            } else {
                -1
            };
            cur = next;
        } else {
            break;
        }
    }
    out
}

/// Pieces in nearest-next order starting with the first piece (as given).
fn nearest_next(pieces: Vec<Vec<Vec2>>) -> Vec<Vec<Vec2>> {
    let start = pieces.first().map(|p| p[0]);
    order(pieces, start)
}

// ------------------------------------------------------------------ cross stitch

/// Cross stitch: an X in every grid cell (`size` square, rotated by `angle`) that lies wholly
/// inside the region. Each row of neighbouring cells is one run: "/" diagonals forward, "\"
/// diagonals back; `triple` sews every diagonal three times.
fn cross_stitch(
    closed: &[Contour],
    holes: &[Contour],
    angle: f64,
    size: f64,
    triple: bool,
) -> Vec<Run> {
    let Some(b) = rotated_bounds(closed, angle) else {
        return Vec::new();
    };
    if size <= 0.0 {
        return Vec::new();
    }
    let rad = angle.to_radians();
    let at = |x: f64, y: f64| v2(x, y).rotated(rad);
    let inside = |p: Vec2| contains(p, closed) && !contains(p, holes);
    let x0 = (b.min_x / size).floor() * size;
    let y0 = (b.min_y / size).floor() * size;
    let cols = ((b.max_x - x0) / size).ceil() as usize;
    let rows = ((b.max_y - y0) / size).ceil() as usize;
    if cols.saturating_mul(rows) > MAX_POINTS {
        return Vec::new();
    }
    let shrink = 0.02 * size;
    let cell_in = |cx: usize, cy: usize| {
        let (x, y) = (x0 + cx as f64 * size, y0 + cy as f64 * size);
        [
            (x + shrink, y + shrink),
            (x + size - shrink, y + shrink),
            (x + shrink, y + size - shrink),
            (x + size - shrink, y + size - shrink),
            (x + size / 2.0, y + size / 2.0),
        ]
        .iter()
        .all(|&(px, py)| inside(at(px, py)))
    };
    let diag = |run: &mut Run, from: Vec2, to: Vec2| {
        if triple {
            run.extend([to, from, to]);
        } else {
            run.push(to);
        }
        let _ = from;
    };
    let mut out = Vec::new();
    for cy in 0..rows {
        let mut cx = 0;
        while cx < cols {
            if !cell_in(cx, cy) {
                cx += 1;
                continue;
            }
            let start = cx;
            while cx < cols && cell_in(cx, cy) {
                cx += 1;
            }
            let (top, bottom) = (y0 + cy as f64 * size, y0 + (cy + 1) as f64 * size);
            let xs = |c: usize| x0 + c as f64 * size;
            let mut run = vec![at(xs(start), bottom)];
            for c in start..cx {
                let (bl, tr, br) = (at(xs(c), bottom), at(xs(c + 1), top), at(xs(c + 1), bottom));
                diag(&mut run, bl, tr);
                run.push(br);
            }
            for c in (start..cx).rev() {
                let (br, tl, bl) = (at(xs(c + 1), bottom), at(xs(c), top), at(xs(c), bottom));
                diag(&mut run, br, tl);
                run.push(bl);
            }
            out.push(run);
        }
    }
    out
}

// ------------------------------------------------------------------ circles and spirals

/// Concentric circles about `c`, `spacing` apart, out to the farthest outline point.
fn concentric(closed: &[Contour], c: Vec2, spacing: f64, pitch: f64) -> Vec<Vec<Vec2>> {
    let r_max = reach(closed, c);
    let mut out = Vec::new();
    let mut r = spacing / 2.0;
    while r <= r_max && out.len() < MAX_ROWS {
        let n = ((std::f64::consts::TAU * r / pitch.max(1.0)).ceil() as usize).max(8);
        out.push(
            (0..=n)
                .map(|k| c + v2(r, 0.0).rotated(std::f64::consts::TAU * k as f64 / n as f64))
                .collect(),
        );
        r += spacing;
    }
    out
}

/// Radial lines from the centre, `spacing` apart at the outer edge. Lines are thinned towards
/// the centre: a line of level L (the number of trailing zero bits of its index) starts where
/// the lines of higher levels are `spacing` apart, so the centre is not overloaded. Lines run
/// out and in alternately, in angular order.
fn radial(
    closed: &[Contour],
    holes: &[Contour],
    c: Vec2,
    spacing: f64,
    pitch: f64,
) -> Vec<Vec<Vec2>> {
    let r_max = reach(closed, c) + 1.0;
    let n_min = (std::f64::consts::TAU * r_max / spacing).ceil().max(8.0) as usize;
    let n = n_min.next_power_of_two().min(1 << 16);
    let mut out = Vec::new();
    for i in 0..n {
        let level = if i == 0 { 16 } else { i.trailing_zeros() };
        let gap = std::f64::consts::TAU * 2f64.powi(level as i32) / n as f64;
        let start = if level >= 16 {
            0.0
        } else {
            spacing / (2.0 * gap)
        };
        if start >= r_max {
            continue;
        }
        let dir = v2(1.0, 0.0).rotated(std::f64::consts::TAU * i as f64 / n as f64);
        let ray = subdivide(&[c + dir * start, c + dir * r_max], pitch);
        let mut pieces = clip(&ray, closed, holes);
        if out.len() % 2 == 1 {
            pieces.reverse();
            pieces.iter_mut().for_each(|p| p.reverse());
        }
        out.extend(pieces);
    }
    out
}

/// Archimedean spiral about `c` (radius grows `spacing` per turn) out to `r_max`, points about
/// `pitch` apart; clockwise on screen unless `ccw`.
fn archimedean(c: Vec2, r_max: f64, spacing: f64, pitch: f64, ccw: bool) -> Vec<Vec2> {
    let sign = if ccw { -1.0 } else { 1.0 };
    let mut out = vec![c];
    let mut theta: f64 = 0.0;
    while out.len() < MAX_POINTS {
        let r = spacing * theta / std::f64::consts::TAU;
        theta += pitch / r.max(spacing);
        let r = spacing * theta / std::f64::consts::TAU;
        out.push(c + v2(r, 0.0).rotated(sign * theta));
        if r > r_max + spacing {
            break;
        }
    }
    out
}

/// Flexible spiral: the region's outer outline shrunk towards its centre, turn by turn, so the
/// spiral follows the shape. Each turn shrinks by `spacing` relative to the mean distance of
/// the outline from the centre. Starts outside.
fn flexible_spiral(closed: &[Contour], spacing: f64, pitch: f64, ccw: bool) -> Vec<Vec2> {
    let Some(outer) = closed
        .iter()
        .max_by(|a, b| area(a).abs().total_cmp(&area(b).abs()))
    else {
        return Vec::new();
    };
    let mut ring = outer.path();
    // Clockwise on screen (y down) is positive signed area in this convention.
    if (area(outer) < 0.0) != ccw {
        ring.reverse();
    }
    let c = centroid(outer);
    let w = Walker::new(&ring);
    let len = w.length();
    if len <= 0.0 || spacing <= 0.0 {
        return Vec::new();
    }
    let mean = ring.iter().map(|p| p.distance(c)).sum::<f64>() / ring.len() as f64;
    let shrink = spacing / mean.max(spacing);
    let m = ((len / pitch.max(1.0)).ceil() as usize).max(8);
    let mut out = Vec::new();
    let mut k = 0usize;
    loop {
        let s = (k % m) as f64 / m as f64;
        let scale = 1.0 - (k as f64 / m as f64) * shrink;
        if scale <= 0.0 || out.len() >= MAX_POINTS {
            break;
        }
        let p = w.at(s * len);
        out.push(c + (p - c) * scale);
        k += 1;
    }
    out.push(c);
    out
}

/// Signed area (shoelace) of a closed contour's outline.
fn area(c: &Contour) -> f64 {
    let p = c.outline_points();
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

/// Area centroid of a closed contour (bounding-box centre for degenerate ones).
fn centroid(c: &Contour) -> Vec2 {
    let p = c.outline_points();
    let n = p.len();
    let a = area(c);
    if a.abs() < 1e-9 {
        return centre(std::slice::from_ref(c));
    }
    let (mut cx, mut cy) = (0.0, 0.0);
    for i in 0..n {
        let (u, v) = (p[i], p[(i + 1) % n]);
        let f = u.x * v.y - v.x * u.y;
        cx += (u.x + v.x) * f;
        cy += (u.y + v.y) * f;
    }
    v2(cx / (6.0 * a), cy / (6.0 * a))
}

// ------------------------------------------------------------------ stippling

/// Stippling: a meandering closed path that never crosses itself. Cells of twice the spacing
/// that lie wholly inside the region get a random spanning tree (fixed seed); the path walks
/// around the tree at half a cell, through every quarter cell (a Hamiltonian cycle of the
/// doubled grid), then is rounded by three corner-cutting passes. One loop per connected part.
fn stipple(closed: &[Contour], holes: &[Contour], angle: f64, spacing: f64) -> Vec<Vec<Vec2>> {
    let Some(b) = rotated_bounds(closed, angle) else {
        return Vec::new();
    };
    let cell = spacing * 2.0;
    if cell <= 0.0 {
        return Vec::new();
    }
    let rad = angle.to_radians();
    let at = |x: f64, y: f64| v2(x, y).rotated(rad);
    let inside = |p: Vec2| contains(p, closed) && !contains(p, holes);
    let cols = ((b.width() / cell).floor() as usize).max(1);
    let rows = ((b.height() / cell).floor() as usize).max(1);
    if cols.saturating_mul(rows) > MAX_POINTS / 8 {
        return Vec::new();
    }
    let ox = b.min_x + (b.width() - cols as f64 * cell) / 2.0;
    let oy = b.min_y + (b.height() - rows as f64 * cell) / 2.0;
    let on: Vec<bool> = (0..rows * cols)
        .map(|i| {
            let (cx, cy) = (i % cols, i / cols);
            let (x, y) = (ox + cx as f64 * cell, oy + cy as f64 * cell);
            [0.0, 0.5, 1.0].iter().all(|&fx| {
                [0.0, 0.5, 1.0]
                    .iter()
                    .all(|&fy| inside(at(x + fx * cell, y + fy * cell)))
            })
        })
        .collect();
    let mut rng = Rng::new(0x57_1991E);
    let mut visited = vec![false; rows * cols];
    // Tree edges: right[i] links cell i to i + 1, down[i] links i to i + cols.
    let mut right = vec![false; rows * cols];
    let mut down = vec![false; rows * cols];
    let mut loops = Vec::new();
    for root in 0..rows * cols {
        if !on[root] || visited[root] {
            continue;
        }
        let mut stack = vec![root];
        visited[root] = true;
        let mut members = vec![root];
        while let Some(&cur) = stack.last() {
            let (cx, cy) = (cur % cols, cur / cols);
            let mut options: Vec<usize> = Vec::with_capacity(4);
            if cx + 1 < cols {
                options.push(cur + 1);
            }
            if cx > 0 {
                options.push(cur - 1);
            }
            if cy + 1 < rows {
                options.push(cur + cols);
            }
            if cy > 0 {
                options.push(cur - cols);
            }
            options.retain(|&n| on[n] && !visited[n]);
            if options.is_empty() {
                stack.pop();
                continue;
            }
            let next = options[(rng.next_u64() % options.len() as u64) as usize];
            visited[next] = true;
            members.push(next);
            match next {
                n if n == cur + 1 => right[cur] = true,
                n if n + 1 == cur => right[n] = true,
                n if n == cur + cols => down[cur] = true,
                n => down[n] = true,
            }
            stack.push(next);
        }
        loops.push(walk_tree(&members, cols, &right, &down, |fx, fy| {
            at(
                ox + (fx as f64 + 0.5) * spacing,
                oy + (fy as f64 + 0.5) * spacing,
            )
        }));
    }
    loops
        .into_iter()
        .map(|l| chaikin(&chaikin(&chaikin(&l))))
        .collect()
}

/// The Hamiltonian cycle of the doubled grid around a spanning tree: each tree cell is a ring
/// of four quarter cells; a tree edge swaps the two ring edges facing it for two edges joining
/// the rings. Returns the cycle (without the closing repeat) mapped by `at(fine x, fine y)`.
fn walk_tree(
    members: &[usize],
    cols: usize,
    right: &[bool],
    down: &[bool],
    at: impl Fn(usize, usize) -> Vec2,
) -> Vec<Vec2> {
    use std::collections::HashMap;
    type Node = (usize, usize);
    let mut adj: HashMap<Node, Vec<Node>> = HashMap::new();
    let link = |a: Node, b: Node, adj: &mut HashMap<Node, Vec<Node>>| {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    };
    for &i in members {
        let (x, y) = (2 * (i % cols), 2 * (i / cols));
        let (tl, tr, bl, br) = ((x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1));
        // Top and left ring edges always exist unless a tree edge from the neighbour above /
        // to the left replaces them; right and bottom unless this cell's own edge does.
        let from_left = i % cols > 0 && right[i - 1];
        let from_above = i >= cols && down[i - cols];
        if !from_above {
            link(tl, tr, &mut adj);
        }
        if !from_left {
            link(tl, bl, &mut adj);
        }
        if right[i] {
            link(tr, (x + 2, y), &mut adj);
            link(br, (x + 2, y + 1), &mut adj);
        } else {
            link(tr, br, &mut adj);
        }
        if down[i] {
            link(bl, (x, y + 2), &mut adj);
            link(br, (x + 1, y + 2), &mut adj);
        } else {
            link(bl, br, &mut adj);
        }
    }
    let Some(&start) = members.first() else {
        return Vec::new();
    };
    let start = (2 * (start % cols), 2 * (start / cols));
    let mut path = vec![start];
    let mut prev = start;
    let mut cur = adj
        .get(&start)
        .and_then(|n| n.first())
        .copied()
        .unwrap_or(start);
    while cur != start && path.len() <= adj.len() {
        path.push(cur);
        let next = adj[&cur]
            .iter()
            .copied()
            .find(|&n| n != prev)
            .unwrap_or(prev);
        prev = cur;
        cur = next;
    }
    path.into_iter().map(|(x, y)| at(x, y)).collect()
}

/// One pass of Chaikin corner cutting on a closed loop (no closing repeat).
fn chaikin(l: &[Vec2]) -> Vec<Vec2> {
    let n = l.len();
    if n < 3 {
        return l.to_vec();
    }
    let mut out = Vec::with_capacity(2 * n);
    for i in 0..n {
        let (a, b) = (l[i], l[(i + 1) % n]);
        out.push(a.lerp(b, 0.25));
        out.push(a.lerp(b, 0.75));
    }
    out
}

// ------------------------------------------------------------------ nets

/// The line families of net pattern `id`: per family, clipped pieces in row order.
fn net_families(
    closed: &[Contour],
    holes: &[Contour],
    angle: f64,
    spacing: f64,
    id: &str,
) -> Vec<Vec<Vec<Vec2>>> {
    let line = |a: f64, step: f64| {
        let unit = [v2(0.0, 0.0), v2(step, 0.0)];
        tile_rows(closed, holes, angle + a, &unit, step, step, 0.0)
    };
    let s = spacing;
    match id {
        "diamond" => vec![line(45.0, s), line(-45.0, s)],
        "triangle" => {
            let h = s * 3f64.sqrt() / 2.0;
            vec![line(0.0, h), line(60.0, h), line(120.0, h)]
        }
        "honeycomb" => {
            let a = s / 3f64.sqrt();
            let h = a * 3f64.sqrt() / 2.0;
            let unit = [
                v2(0.0, 0.0),
                v2(a, 0.0),
                v2(1.5 * a, h),
                v2(2.5 * a, h),
                v2(3.0 * a, 0.0),
            ];
            vec![tile_rows(closed, holes, angle, &unit, 3.0 * a, h, 0.5)]
        }
        "star" => vec![
            line(0.0, s),
            line(90.0, s),
            line(45.0, s * std::f64::consts::SQRT_2),
            line(-45.0, s * std::f64::consts::SQRT_2),
        ],
        "waves" => {
            let unit: Vec<Vec2> = (0..=16)
                .map(|k| {
                    let t = k as f64 / 16.0;
                    v2(t * 2.0 * s, (t * std::f64::consts::TAU).sin() * s / 4.0)
                })
                .collect();
            vec![
                tile_rows(closed, holes, angle, &unit, 2.0 * s, s, 0.0),
                tile_rows(closed, holes, angle + 90.0, &unit, 2.0 * s, s, 0.0),
            ]
        }
        _ => vec![line(0.0, s), line(90.0, s)],
    }
}

/// Moves needle points by up to `amount` (deterministic), keeping each inside the region.
fn jitter(runs: &mut [Run], closed: &[Contour], holes: &[Contour], amount: f64) {
    let mut rng = Rng::new(0xDEC0_5EED);
    for run in runs.iter_mut() {
        for p in run.iter_mut() {
            let d = v2(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0) * amount;
            let q = *p + d;
            if contains(q, closed) && !contains(q, holes) {
                *p = q;
            }
        }
    }
}
