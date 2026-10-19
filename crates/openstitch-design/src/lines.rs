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

/// Motif line: copies of a library motif along the path, joined by running stitches. On a
/// closed path the copies are stretched to fit a whole number; on an open path they are
/// centred and the remainder is running stitch.
pub fn motif(path: &[Vec2], closed: bool, m: &MotifLine) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    let unit = &m.pattern.paths[0];
    let period = m.width + m.gap;
    if len <= 0.0 || period <= 0.0 || m.width > len {
        return subdivide(path, m.pitch);
    }
    let (n, scale) = if closed {
        let n = (len / period).round().max(1.0);
        (n as usize, len / (n * period))
    } else {
        let n = ((len + m.gap) / period).floor().max(1.0);
        (n as usize, 1.0)
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

/// Stem stitch: slanted stitches of `width` across the line at `angle` degrees to it, every
/// `spacing`, each overlapping the next like a twisted rope; sewn three times unless `single`.
pub fn stem(path: &[Vec2], width: f64, spacing: f64, angle: f64, single: bool) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    if len <= 0.0 || spacing <= 0.0 {
        return path.to_vec();
    }
    let a = angle.to_radians();
    let along = width / 2.0 * (a.cos() / a.sin());
    let n = (len / spacing).floor() as usize + 1;
    let mut run = Vec::new();
    for k in 0..n {
        let s = (k as f64 * spacing).min(len);
        let (c, t) = (w.at(s), w.tangent(s));
        let nrm = normal(t);
        let from = c - t * along - nrm * (width / 2.0);
        let to = c + t * along + nrm * (width / 2.0);
        if single {
            extend(&mut run, [from, to]);
        } else {
            extend(&mut run, [from, to, from, to]);
        }
    }
    run
}

/// Candlewicking: knots of `size` (a star of `spokes` stitches out from the centre and back)
/// every `spacing`, joined by running stitches.
pub fn candlewick(path: &[Vec2], size: f64, spokes: usize, spacing: f64, pitch: f64) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    if len <= 0.0 || spacing <= 0.0 {
        return path.to_vec();
    }
    let n = (len / spacing).floor() as usize;
    let mut run = Vec::new();
    let r = size / 2.0;
    for k in 0..=n {
        let s = (k as f64 * spacing).min(len);
        if k > 0 {
            extend(&mut run, run_between(&w, (s - spacing).max(0.0), s, pitch));
        }
        let c = w.at(s);
        let t = w.tangent(s);
        extend(&mut run, [c]);
        for j in 0..spokes {
            let d = t.rotated(std::f64::consts::TAU * j as f64 / spokes as f64);
            run.push(c + d * r);
            run.push(c);
        }
    }
    let last = n as f64 * spacing;
    if last < len {
        extend(&mut run, run_between(&w, last, len, pitch));
    }
    run
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
        let tip = c + normal(w.tangent(s)) * (e.stroke * side(e.arrange, k));
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

/// V stitch: a baseline with a V of two strokes every `interval`, the line and the strokes
/// sewn `run_times`.
pub fn v_stitch(path: &[Vec2], e: &StrokeLine) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    if len <= 0.0 || e.interval <= 0.0 {
        return path.to_vec();
    }
    let n = (len / e.interval).floor() as usize;
    let mut run = vec![w.at(0.0)];
    let mut s = 0.0;
    for k in 0..n {
        let mid = (k as f64 + 0.5) * e.interval;
        baseline(&mut run, &w, s, mid, e.pitch, e.run_times);
        s = mid;
        let c = w.at(mid);
        let sign = side(e.arrange, k);
        for apex_at in [mid - e.interval / 4.0, mid + e.interval / 4.0] {
            let apex = w.at(apex_at) + normal(w.tangent(apex_at)) * (e.stroke * sign);
            for _ in 0..e.run_times.max(1) {
                run.push(apex);
                run.push(c);
            }
        }
    }
    baseline(&mut run, &w, s, len, e.pitch, e.run_times);
    run
}

/// Chain stitch: links of `size` along the line. Triangles: peaks sewn forward, the baseline
/// back; diamonds: upper halves forward, lower halves back. Repeated `times` (there and back).
pub fn chain(
    path: &[Vec2],
    size: f64,
    shape: ChainShape,
    times: u32,
    arrange: Arrange,
    pitch: f64,
) -> Run {
    let w = Walker::new(path);
    let len = w.length();
    if len <= 0.0 || size <= 0.0 {
        return path.to_vec();
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
            match shape {
                ChainShape::Triangle => extend(
                    &mut run,
                    run_between(&w, (k + 1) as f64 * l, k as f64 * l, pitch),
                ),
                ChainShape::Diamond => extend(&mut run, [peak(k, -1.0), w.at(k as f64 * l)]),
            }
        }
    }
    run
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
