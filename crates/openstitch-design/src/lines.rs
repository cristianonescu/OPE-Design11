//! Line sew types beyond running, triple and zigzag: motif, stem, candlewicking, E/V, chain,
//! cutting lines, and zigzag with half stitch. Independent algorithms specified from the
//! public guide's attribute lists; lengths are 0.1 mm and every generator is deterministic.

use crate::generate::{Run, subdivide};
use crate::geometry::{Vec2, v2};
use crate::model::{Arrange, ChainShape};
use crate::paths::{Walker, normal};
use crate::patterns::Pattern;

/// Side sign of element `k` under `arrange`.
fn side(arrange: Arrange, k: usize) -> f64 {
    match arrange {
        Arrange::Normal => 1.0,
        Arrange::Mirror => -1.0,
        Arrange::Alternate => {
            if k.is_multiple_of(2) {
                1.0
            } else {
                -1.0
            }
        }
    }
}

/// Appends `pts` to `run`, skipping a first point equal to the run's last.
fn extend(run: &mut Run, pts: impl IntoIterator<Item = Vec2>) {
    for p in pts {
        if run.last().is_none_or(|l| l.distance(p) > 1e-9) {
            run.push(p);
        }
    }
}

/// The side direction at arc length `s`: the normal there, or at a corner the bisector of the
/// two segments' normals, so a stroke at a corner points straight out of it (PE-DESIGN's E and
/// V at the corner of line-e-default.pes / line-v-default.pes).
fn side_at(w: &Walker, s: f64) -> Vec2 {
    const EPS: f64 = 1e-3;
    let (a, b) = (
        normal(w.tangent((s - EPS).max(0.0))),
        normal(w.tangent((s + EPS).min(w.length()))),
    );
    let sum = a + b;
    let len = (sum.x * sum.x + sum.y * sum.y).sqrt();
    if len < 1e-9 { a } else { sum * (1.0 / len) }
}

/// Running stitches along the path between arc lengths `a` and `b` (reversed when `b < a`).
fn run_between(w: &Walker, a: f64, b: f64, pitch: f64) -> Run {
    if a <= b {
        subdivide(&w.slice(a, b), pitch)
    } else {
        let mut r = subdivide(&w.slice(b, a), pitch);
        r.reverse();
        r
    }
}

pub struct MotifLine<'a> {
    pub pattern: &'a Pattern,
    pub width: f64,
    pub height: f64,
    pub gap: f64,
    pub pitch: f64,
    pub arrange: Arrange,
}

/// Motif line: copies of a library motif along the path, joined by running stitches over the
/// gaps. The copies (and gaps) are stretched along the path to fit a whole number of them
/// from end to end, on open and closed paths alike.
pub fn motif(path: &[Vec2], closed: bool, m: &MotifLine) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    let unit = &m.pattern.paths[0];
    let period = m.width + m.gap;
    if len <= 0.0 || period <= 0.0 || m.width > len {
        return subdivide(path, m.pitch);
    }
    // A whole number of motifs, scaled along the line so they fill it from end to end
    // (PE-DESIGN 11.50 sews 8 motifs of 4.94 mm on a 39.5 mm line at the 5 mm size:
    // compat/reference/stitches/line-motif-zigzag-3pt.pes).
    let (n, scale) = if closed {
        let n = (len / period).round().max(1.0);
        (n as usize, len / (n * period))
    } else {
        let n = ((len + m.gap) / period).round().max(1.0);
        (n as usize, (len + m.gap) / (n * period))
    };
    let (mw, mg) = (m.width * scale, m.gap * scale);
    let used = n as f64 * mw + (n as f64 - 1.0) * mg;
    let mut s = if closed { 0.0 } else { (len - used) / 2.0 };
    let mut run = run_between(&w, 0.0, s, m.pitch);
    for k in 0..n {
        let sign = side(m.arrange, k);
        let mapped: Vec<Vec2> = unit
            .iter()
            .map(|q| {
                let at = s + q.x * mw;
                w.at(at) + normal(w.tangent(at)) * (q.y * m.height * sign)
            })
            .collect();
        extend(&mut run, subdivide(&mapped, m.pitch));
        s += mw;
        if k + 1 < n {
            extend(&mut run, run_between(&w, s, s + mg, m.pitch));
            s += mg;
        }
    }
    extend(&mut run, run_between(&w, s, len, m.pitch));
    run
}

/// Stem stitch, as PE-DESIGN 11.50 sews it (line-stem-default.pes): on every straight leg,
/// stations `leg / round(leg / spacing)` apart, each a stitch of `width` across the line
/// leaning `angle` degrees counter-clockwise from it on screen (45: from the top right to the
/// bottom left), sewn top, bottom, top, bottom (the second pass 0.14 mm beside the first, so
/// the rope reads full), then on to the next station's top; `single` sews it once. The first
/// station is at the line's start, the last at its end, and a corner is sewn as a plain needle
/// point between the legs' stations. The line ends at its end point.
pub fn stem(path: &[Vec2], width: f64, spacing: f64, angle: f64, single: bool) -> Run {
    let Some(pieces) = stem_pieces(path, width, spacing, angle, single) else {
        return path.to_vec();
    };
    let mut run = vec![pieces[0].lead[0]];
    for p in &pieces {
        extend(&mut run, p.pts.iter().copied());
    }
    extend(&mut run, path.last().copied());
    run
}

/// The stations of [`stem`] in sewing order, each with its centre on the line as its lead; a
/// corner is a piece of its own (the corner point).
fn stem_pieces(
    path: &[Vec2],
    width: f64,
    spacing: f64,
    angle: f64,
    single: bool,
) -> Option<Vec<Piece>> {
    let legs: Vec<(Vec2, Vec2)> = path
        .windows(2)
        .filter(|q| q[0].distance(q[1]) > 1e-9)
        .map(|q| (q[0], q[1]))
        .collect();
    if legs.is_empty() || spacing <= 0.0 {
        return None;
    }
    let a = angle.to_radians();
    let along = width / 2.0 * (a.cos() / a.sin());
    let mut pieces = Vec::new();
    let mut s0 = 0.0;
    for (i, &(p, q)) in legs.iter().enumerate() {
        let l = p.distance(q);
        let t = (q - p) * (1.0 / l);
        let nrm = normal(t);
        let n = (l / spacing).round().max(1.0) as usize;
        let last = i + 1 == legs.len();
        if i > 0 {
            pieces.push(Piece {
                at: s0,
                lead: vec![p],
                pts: vec![p],
            });
        }
        for j in usize::from(i > 0)..n + usize::from(last) {
            let c = p + t * (l * j as f64 / n as f64);
            let top = c + t * along - nrm * (width / 2.0);
            let bottom = c - t * along + nrm * (width / 2.0);
            let u = top - bottom;
            let e = normal(u * (1.0 / u.x.hypot(u.y))) * 1.4;
            pieces.push(Piece {
                at: s0 + l * j as f64 / n as f64,
                lead: vec![c],
                pts: if single {
                    vec![top, bottom]
                } else {
                    vec![top - e, bottom - e, top + e, bottom + e]
                },
            });
        }
        s0 += l;
    }
    Some(pieces)
}

/// Candlewicking, as PE-DESIGN 11.50 sews it (line-candlewick-default.pes): a knot at both
/// ends of every straight leg and evenly between, `round(leg / (size + spacing))` apart per
/// leg. A knot is a star of `points` chords on a circle of `size` around the line: from the
/// point straight ahead it jumps `points / 2 - 1` points at a time until it is back there (a
/// corner knot faces along the leg it leads into). The first knot starts with a stitch to its
/// centre and back. Between knots, running stitch along the line from one knot's front point
/// to the next one's, every vertex kept, the short stitch first, then `pitch`.
pub fn candlewick(path: &[Vec2], size: f64, points: usize, spacing: f64, pitch: f64) -> Run {
    let Some(pieces) = candlewick_pieces(path, size, points, spacing, pitch, false) else {
        return path.to_vec();
    };
    let mut run = Vec::new();
    for p in &pieces {
        extend(&mut run, p.lead.iter().copied());
        extend(&mut run, p.pts.iter().copied());
    }
    run
}

/// The knots of [`candlewick`] in sewing order. A knot's lead is the running stitch to its
/// front point from the previous knot's (the first knot's: its front point and centre), its
/// points the star; it begins on the line one radius before its centre. `closed`: the path
/// ends where it starts, so the last knot (on the first one) is dropped and the first knot's
/// lead comes from the knot before it.
fn candlewick_pieces(
    path: &[Vec2],
    size: f64,
    points: usize,
    spacing: f64,
    pitch: f64,
    closed: bool,
) -> Option<Vec<Piece>> {
    let w = Walker::new(path);
    let len = w.length();
    if len <= 0.0 || size <= 0.0 || pitch <= 0.0 {
        return None;
    }
    let r = size / 2.0;
    let points = points.max(4);
    let step = (points / 2).saturating_sub(1).max(1);
    // Knot stations (arc length, facing direction).
    let mut stations: Vec<(f64, Vec2)> = Vec::new();
    let mut s0 = 0.0;
    for seg in path.windows(2) {
        let l = seg[0].distance(seg[1]);
        if l <= 1e-9 {
            continue;
        }
        let t = (seg[1] - seg[0]) * (1.0 / l);
        let n = (l / (size + spacing).max(1e-6)).round().max(1.0) as usize;
        for j in 0..=n {
            if j == 0 && !stations.is_empty() {
                stations.last_mut().expect("station").1 = t;
                continue;
            }
            stations.push((s0 + l * j as f64 / n as f64, t));
        }
        s0 += l;
    }
    // Where the loop closes, the knot there faces along the last leg (it ends the loop) and
    // the first leg's runs on from the corner without one (line-candlewick-startend-*.pes).
    let closed = closed && stations.len() > 2;
    if closed {
        stations.remove(0);
    }
    let front = |&(s, t): &(f64, Vec2)| w.at(s) + t * r;
    // Running stitch along the line from `prev`'s front point to arc `b`, then to `to`. After
    // the closing knot (facing away from the next leg) it runs on from the corner, and a short
    // first stitch is shared with the next (1.6 + 1.6 mm, not 0.7 + 2.5).
    let lead = |prev: &(f64, Vec2), b: f64, to: Vec2| {
        let closing = prev.0 >= len - 1e-9;
        let a = if closing { 0.0 } else { (prev.0 + r).min(len) };
        let mut pts = Vec::new();
        if b > a {
            let slice = w.slice(a, b);
            let total: f64 = slice.windows(2).map(|q| q[0].distance(q[1])).sum();
            let sw = Walker::new(&slice);
            let mut cuts: Vec<f64> = Vec::new();
            let mut d = total % pitch;
            if d < 1e-6 {
                d = pitch;
            }
            while d < total - 1e-6 {
                cuts.push(d);
                d += pitch;
            }
            let mut cum = 0.0;
            for q in slice.windows(2) {
                cum += q[0].distance(q[1]);
                cuts.push(cum);
            }
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
            if closing && cuts.len() > 1 && cuts[0] < pitch / 2.0 {
                cuts[0] = cuts[1] / 2.0;
            }
            pts.extend(cuts.iter().map(|&d| sw.at(d)));
        }
        pts.push(to);
        pts
    };
    let pieces = stations
        .iter()
        .enumerate()
        .map(|(k, st)| {
            let (c, t) = (w.at(st.0), st.1);
            let lead = match k {
                0 if closed => lead(&stations[stations.len() - 1], st.0, front(st)),
                0 => vec![front(st), c],
                _ => lead(&stations[k - 1], st.0, front(st)),
            };
            let pts = (0..=points)
                .map(|m| {
                    let j = (m * step) % points;
                    let ang = -std::f64::consts::TAU * j as f64 / points as f64;
                    c + t.rotated(ang) * r
                })
                .collect();
            Piece {
                at: st.0 - r,
                lead,
                pts,
            }
        })
        .collect();
    Some(pieces)
}

pub struct StrokeLine {
    /// Distance between strokes.
    pub interval: f64,
    pub stroke: f64,
    pub pitch: f64,
    /// Baseline passes (odd counts end at the far end; even counts get one more pass).
    pub run_times: u32,
    pub stroke_times: u32,
    pub arrange: Arrange,
}

/// Baseline between `a` and `b`, sewn back and forth an odd number of times ≥ `times`.
fn baseline(run: &mut Run, w: &Walker, a: f64, b: f64, pitch: f64, times: u32) {
    let passes = if times % 2 == 1 { times } else { times + 1 };
    for p in 0..passes {
        if p % 2 == 0 {
            extend(run, run_between(w, a, b, pitch));
        } else {
            extend(run, run_between(w, b, a, pitch));
        }
    }
}

/// E stitch (blanket stitch): a baseline with a stroke out to one side every `interval`.
pub fn e_stitch(path: &[Vec2], e: &StrokeLine) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    if len <= 0.0 || e.interval <= 0.0 {
        return path.to_vec();
    }
    let n = (len / e.interval).floor() as usize;
    let mut run = vec![w.at(0.0)];
    for k in 0..=n {
        let s = (k as f64 * e.interval).min(len);
        if k > 0 {
            baseline(&mut run, &w, s - e.interval, s, e.pitch, e.run_times);
        }
        let c = w.at(s);
        let tip = c + side_at(&w, s) * (e.stroke * side(e.arrange, k));
        for _ in 0..e.stroke_times.max(1) {
            run.push(tip);
            run.push(c);
        }
    }
    let last = n as f64 * e.interval;
    if last < len {
        baseline(&mut run, &w, last, len, e.pitch, e.run_times);
    }
    run
}

/// V stitch, as PE-DESIGN 11.50 sews it (line-v-default.pes): a V every `interval` along the
/// line, from the start to the end. Each arm runs from the line a fifth of the interval before
/// the V's position out to the apex `stroke` to the side and back to the line a fifth after it;
/// the line between Vs is running stitch, sewn `run_times`. The first and last V have one arm.
pub fn v_stitch(path: &[Vec2], e: &StrokeLine) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    if len <= 0.0 || e.interval <= 0.0 {
        return path.to_vec();
    }
    let arm = e.interval / 5.0;
    let n = (len / e.interval + 1e-9).floor() as usize;
    let mut stations: Vec<f64> = (0..=n).map(|k| k as f64 * e.interval).collect();
    if len - stations[n] > arm {
        stations.push(len);
    }
    let mut run = vec![w.at(0.0)];
    let mut at = 0.0;
    for (k, &s) in stations.iter().enumerate() {
        let before = (s - arm).max(at);
        if before > at {
            baseline(&mut run, &w, at, before, e.pitch, e.run_times);
        }
        let apex = w.at(s) + side_at(&w, s) * (e.stroke * side(e.arrange, k));
        let after = (s + arm).min(len);
        extend(&mut run, [apex, w.at(after)]);
        at = after;
    }
    if at < len {
        baseline(&mut run, &w, at, len, e.pitch, e.run_times);
    }
    run
}

/// Zigzag under sewing, as PE-DESIGN 11.50 sews it (line-zigzag-default.pes): a running
/// stitch along the line `offset` to the right (mitred at corners, starting and ending
/// `offset` in from the line's ends), then across and back the same way on the left side;
/// stitches split evenly at no more than `pitch`. Empty when the line is too short.
pub fn zigzag_underlay(path: &[Vec2], offset: f64, pitch: f64) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    if path.len() < 2 || len <= 2.0 * offset {
        return Vec::new();
    }
    let side = |d: f64| -> Vec<Vec2> {
        let mut pts = vec![w.at(offset) + normal(w.tangent(offset)) * d];
        let mut s = 0.0;
        for i in 1..path.len() - 1 {
            s += path[i - 1].distance(path[i]);
            if s <= offset || s >= len - offset {
                continue;
            }
            let (n1, n2) = (normal(w.tangent(s - 1e-3)), normal(w.tangent(s + 1e-3)));
            let k = 1.0 + (n1.x * n2.x + n1.y * n2.y);
            if k > 1e-6 {
                pts.push(path[i] + (n1 + n2) * (d / k));
            }
        }
        pts.push(w.at(len - offset) + normal(w.tangent(len - offset)) * d);
        subdivide(&pts, pitch)
    };
    let mut run = side(offset);
    let mut back = side(-offset);
    back.reverse();
    extend(&mut run, back);
    run
}

/// Chain stitch. Triangles, as PE-DESIGN 11.50 sews them (line-chain-default.pes): on every
/// straight leg, `n = floor(leg / (0.75 * size))` links `leg / n` apart, each a triangle from
/// its point on the line to a base `size` further on and `size / 3` to either side, sewn point,
/// left, right, point `times` times; then to the left corner, the base's middle and back to
/// the next link's point (links overlap; the last runs past the leg's end). Diamonds: links of
/// `size` along the line, upper halves forward, lower halves back, repeated `times`.
pub fn chain(path: &[Vec2], size: f64, shape: ChainShape, times: u32, arrange: Arrange) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    if len <= 0.0 || size <= 0.0 {
        return path.to_vec();
    }
    if shape == ChainShape::Triangle {
        let mut run = Vec::new();
        for p in chain_pieces(path, size, times, arrange) {
            extend(&mut run, p.lead);
            run.extend(p.pts);
        }
        return run;
    }
    let n = (len / size).round().max(1.0) as usize;
    let l = len / n as f64;
    let h = size / 2.0;
    let peak = |k: usize, sign: f64| {
        let s = (k as f64 + 0.5) * l;
        w.at(s) + normal(w.tangent(s)) * (h * sign * side(arrange, k))
    };
    let mut run = vec![w.at(0.0)];
    for _ in 0..times.max(1) {
        for k in 0..n {
            extend(&mut run, [peak(k, 1.0), w.at((k + 1) as f64 * l)]);
        }
        for k in (0..n).rev() {
            extend(&mut run, [peak(k, -1.0), w.at(k as f64 * l)]);
        }
    }
    run
}

/// The triangle links of [`chain`] in sewing order, each with its point on the line as its
/// lead.
fn chain_pieces(path: &[Vec2], size: f64, times: u32, arrange: Arrange) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut s0 = 0.0;
    for seg in path.windows(2) {
        let l = seg[0].distance(seg[1]);
        if l <= 1e-9 {
            continue;
        }
        let t = (seg[1] - seg[0]) * (1.0 / l);
        let n = (l / (0.75 * size) + 1e-9).floor().max(1.0) as usize;
        for j in 0..n {
            let nrm = normal(t) * (size / 3.0 * side(arrange, pieces.len()));
            let p = seg[0] + t * (l * j as f64 / n as f64);
            let base = p + t * size;
            let mut pts = Vec::new();
            for _ in 0..times.max(1) {
                pts.extend([base - nrm, base + nrm, p]);
            }
            pts.extend([base - nrm, base]);
            pieces.push(Piece {
                at: s0 + l * j as f64 / n as f64,
                lead: vec![p],
                pts,
            });
        }
        s0 += l;
    }
    pieces
}

/// One element of a stem, candlewick or chain line: where it begins on the line (`at`, arc
/// length), what is sewn to reach it (`lead`: the point on the line it starts from, or for a
/// candlewick knot the running stitch from the previous knot) and its own stitches.
struct Piece {
    at: f64,
    lead: Run,
    pts: Run,
}

/// A closed line [`start_loop`] can start at its entry point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoopLine {
    Stem {
        width: f64,
        spacing: f64,
        angle: f64,
        single: bool,
    },
    Candlewick {
        size: f64,
        points: usize,
        spacing: f64,
        pitch: f64,
    },
    /// Triangle links.
    Chain {
        size: f64,
        times: u32,
        arrange: Arrange,
    },
}

/// A closed stem, candlewick or triangle chain line with moved entry and exit points, as
/// PE-DESIGN 11.50 sews it (line-*-startend-*.pes): every element stays where it is without the
/// points; only where the sewing starts and ends changes. An element belongs to a point when it
/// is the last one to begin at or before it. The pattern starts with the element of the exit
/// point (arc `exit`) and goes round the loop to the one before it, so it also ends there; a
/// stem sews that station once more, as an unmoved closed stem does at its start, and a
/// candlewick sews that knot last. Feed (`run_from: None`) starts there. Run starts at the
/// element of the entry point (arc `run_from`) and sews only the leads (the elements' points on
/// the line; a candlewick's running stitch) to the exit's element, the shorter way round.
/// Returns the stitches and whether they start with leads rather than an element.
pub fn start_loop(
    path: &[Vec2],
    line: LoopLine,
    exit: f64,
    run_from: Option<f64>,
) -> Option<(Run, bool)> {
    let len = Walker::new(path).length();
    let (pieces, stem, defer) = match line {
        LoopLine::Stem {
            width,
            spacing,
            angle,
            single,
        } => {
            let mut p = stem_pieces(path, width, spacing, angle, single)?;
            // Where the loop closes PE-DESIGN sews the corner point and the last station's
            // bottom point instead of the whole station (the first station is there too).
            let last = p.last_mut().expect("station");
            let (v, n) = (last.lead[0], last.pts.len());
            let bottom = if n == 4 {
                (last.pts[1] + last.pts[3]) * 0.5
            } else {
                last.pts[n - 1]
            };
            last.pts = vec![v, bottom];
            (p, true, false)
        }
        LoopLine::Candlewick {
            size,
            points,
            spacing,
            pitch,
        } => (
            candlewick_pieces(path, size, points, spacing, pitch, true)?,
            false,
            true,
        ),
        LoopLine::Chain {
            size,
            times,
            arrange,
        } => (chain_pieces(path, size, times, arrange), false, false),
    };
    let n = pieces.len();
    if n < 2 || len <= 0.0 {
        return None;
    }
    let owner = |s: f64| {
        let behind = |k: usize| (s - pieces[k].at + 1e-6).rem_euclid(len);
        (0..n)
            .min_by(|&a, &b| behind(a).total_cmp(&behind(b)))
            .unwrap_or(0)
    };
    let k1 = owner(exit);
    let k2 = run_from.map_or(k1, owner);
    // The leads from the entry's element to the exit's, forwards or (when that is shorter)
    // backwards round the loop.
    let back = (k2 + n - k1) % n < (k1 + n - k2) % n;
    let mut run = Vec::new();
    let mut k = k2;
    loop {
        if back && k != k1 {
            extend(&mut run, pieces[k].lead.iter().rev().copied());
        } else {
            extend(&mut run, pieces[k].lead.iter().copied());
        }
        if k == k1 {
            break;
        }
        k = if back { (k + n - 1) % n } else { (k + 1) % n };
    }
    // A stem's stations follow on from one another without their centres (corners keep
    // their point).
    let sew = |p: &Piece, run: &mut Run| {
        if !stem || p.pts == p.lead {
            extend(run, p.lead.iter().copied());
        }
        extend(run, p.pts.iter().copied());
    };
    if defer {
        for i in 1..=n {
            sew(&pieces[(k1 + i) % n], &mut run);
        }
    } else {
        extend(&mut run, pieces[k1].pts.iter().copied());
        for i in 1..n {
            sew(&pieces[(k1 + i) % n], &mut run);
        }
        if stem {
            extend(&mut run, pieces[k1].pts.iter().copied());
        }
    }
    Some((run, k2 != k1 || defer))
}

/// Arc length along `path` of the point on it nearest `p`.
pub fn arc_of(path: &[Vec2], p: Vec2) -> f64 {
    let (mut best, mut at, mut s0) = (f64::INFINITY, 0.0, 0.0);
    for q in path.windows(2) {
        let d = q[1] - q[0];
        let l2 = d.x * d.x + d.y * d.y;
        let t = if l2 > 0.0 {
            (((p - q[0]).x * d.x + (p - q[0]).y * d.y) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dist = (q[0] + d * t).distance(p);
        if dist < best {
            (best, at) = (dist, s0 + l2.sqrt() * t);
        }
        s0 += l2.sqrt();
    }
    at
}

/// Cutwork needle penetration spacing (0.1 mm).
pub const CUT_PITCH: f64 = 10.0;
/// Length of an uncut bridge (0.1 mm).
pub const CUT_BRIDGE: f64 = 20.0;

/// Cutting line for the four cutwork needles (blades at 0°, 45°, 90°, 135°): penetrations every
/// [`CUT_PITCH`], each stretch going to the needle whose blade best matches its direction.
/// Returns runs per needle. `leave_uncut` keeps short bridges (four on a closed line, two on
/// an open one).
pub fn cutting(path: &[Vec2], closed: bool, leave_uncut: bool) -> [Vec<Run>; 4] {
    let mut out: [Vec<Run>; 4] = Default::default();
    let w = Walker::new(path);
    let len = w.length();
    if len <= 0.0 {
        return out;
    }
    let bridges: Vec<f64> = if leave_uncut {
        let count = if closed { 4 } else { 2 };
        (0..count)
            .map(|k| len * (k as f64 + 0.5) / count as f64)
            .collect()
    } else {
        Vec::new()
    };
    let pts = subdivide(path, CUT_PITCH);
    let mut s = 0.0;
    let mut current: Option<(usize, Run)> = None;
    for seg in pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let d = a.distance(b);
        let mid = s + d / 2.0;
        s += d;
        if d <= 1e-9 {
            continue;
        }
        let bridged = bridges.iter().any(|&c| (mid - c).abs() < CUT_BRIDGE / 2.0);
        let deg = (b.y - a.y).atan2(b.x - a.x).to_degrees().rem_euclid(180.0);
        let needle = ((deg / 45.0).round() as usize) % 4;
        match &mut current {
            Some((n, run)) if !bridged && *n == needle && run.last() == Some(&a) => run.push(b),
            _ => {
                if let Some((n, run)) = current.take() {
                    out[n].push(run);
                }
                if !bridged {
                    current = Some((needle, vec![a, b]));
                }
            }
        }
    }
    if let Some((n, run)) = current {
        out[n].push(run);
    }
    out
}

/// Zigzag like [`crate::generate::zigzag`], with half stitch: on the inner side of a bend,
/// a point closer than one spacing (half the straight-line distance) to the previous point on
/// that side is pulled to the
/// centre line, so stitches do not pile up.
pub fn zigzag_half(path: &[Vec2], width: f64, spacing: f64) -> Run {
    if path.len() < 2 || spacing.is_nan() || spacing <= 0.0 {
        return path.to_vec();
    }
    let mut run: Run = Vec::new();
    let mut centres: Vec<Vec2> = Vec::new();
    let mut side_sign = 1.0;
    let mut carried = 0.0;
    for seg in path.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let len = a.distance(b);
        if len <= 0.0 {
            continue;
        }
        let dir = (b - a) * (1.0 / len);
        let nrm = v2(-dir.y, dir.x);
        let mut t = carried;
        while t <= len {
            let c = a + dir * t;
            run.push(c + nrm * (side_sign * width / 2.0));
            centres.push(c);
            side_sign = -side_sign;
            t += spacing;
        }
        carried = t - len;
    }
    let mut last_kept: [Option<Vec2>; 2] = [None, None];
    for i in 0..run.len() {
        let slot = i % 2;
        if let Some(prev) = last_kept[slot]
            && prev.distance(run[i]) < spacing
        {
            run[i] = centres[i];
            continue;
        }
        last_kept[slot] = Some(run[i]);
    }
    run
}

/// Zigzag between two rails, as PE-DESIGN 11.50 sews it (line-zigzag-default.pes and the
/// density minimum and maximum files): the rails are the line offset `width / 2` to each side,
/// mitred at corners. Along each straight leg a stitch joins the two rails at the same fraction
/// of the leg, so stitches lean more and more towards a mitre; consecutive stitches stay
/// `spacing` apart measured across the leg divided by the lean, which keeps the thread density
/// the same. It starts on the left rail at the line's start and ends with a stitch exactly at
/// the line's end.
pub fn zigzag_rails(path: &[Vec2], width: f64, spacing: f64) -> Run {
    let pts: Vec<Vec2> = {
        let mut v: Vec<Vec2> = Vec::new();
        for &p in path {
            if v.last().is_none_or(|l| l.distance(p) > 1e-9) {
                v.push(p);
            }
        }
        v
    };
    if pts.len() < 2 || spacing.is_nan() || spacing <= 0.0 {
        return pts;
    }
    let h = width / 2.0;
    let dir = |i: usize| {
        let d = pts[i + 1] - pts[i];
        d * (1.0 / pts[i].distance(pts[i + 1]))
    };
    // Rail point at vertex `i` on side `sign` (+1 right, −1 left).
    let rail = |i: usize, sign: f64| -> Vec2 {
        let n = if i == 0 {
            normal(dir(0))
        } else if i == pts.len() - 1 {
            normal(dir(i - 1))
        } else {
            let (a, b) = (normal(dir(i - 1)), normal(dir(i)));
            let k = 1.0 + (a.x * b.x + a.y * b.y);
            if k < 1e-6 {
                return pts[i] + a * (sign * h);
            }
            (a + b) * (1.0 / k)
        };
        pts[i] + n * (sign * h)
    };
    let mut run = Vec::new();
    let mut side = -1.0;
    let mut carry = 0.0;
    let legs = pts.len() - 1;
    for i in 0..legs {
        let len = pts[i].distance(pts[i + 1]);
        let d = dir(i);
        let (l0, l1, r0, r1) = (
            rail(i, -1.0),
            rail(i + 1, -1.0),
            rail(i, 1.0),
            rail(i + 1, 1.0),
        );
        let at = |f: f64, s: f64| {
            if s < 0.0 {
                l0.lerp(l1, f)
            } else {
                r0.lerp(r1, f)
            }
        };
        let mut f = carry / len;
        while f <= 1.0 + 1e-9 {
            run.push(at(f.min(1.0), side));
            side = -side;
            // The stitch across here leans by the rails' offset along the leg.
            let rung = at(f.min(1.0), 1.0) - at(f.min(1.0), -1.0);
            let across = (rung.x * d.y - rung.y * d.x).abs().max(1e-9);
            let lean = rung.distance(Vec2::default()) / across;
            f += spacing * lean / len;
        }
        carry = (f - 1.0) * len;
        if i + 1 == legs && carry > 1e-6 {
            // A last stitch across exactly at the line's end, after the last regular one.
            let last = run.last().copied();
            let end = at(1.0, side);
            if last.is_none_or(|p| p.distance(end) > 1e-9) {
                run.push(end);
            }
            run.push(at(1.0, -side));
        }
    }
    run
}
