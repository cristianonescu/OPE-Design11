//! Photo Stitch approximations: tonal images become density-modulated stitching.
//!
//! - [`PhotoStyle::Fill`] (after Photo Stitch 1): the image is sampled on a grid of fill
//!   stitches (row spacing × stitch length) and error-diffused (Floyd–Steinberg) onto the
//!   thread colours plus the fabric colour. Each thread sews its cells as staggered fill rows;
//!   fabric cells stay unsewn, so tone becomes stitch coverage.
//! - [`PhotoStyle::Lines`] (after Photo Stitch 2): rows of zigzag lines whose amplitude follows
//!   the tone (mono: darkness; colour: how much of each thread the dithered image holds
//!   locally). Light areas break the line.
//!
//! Mono mode first stretches the image's contrast (2nd–98th luminance percentile to black–white).
//!
//! Specified from PE-DESIGN 11's public guide (Photo Stitch 1/2 create stitching from photos
//! in colour, sepia/grey or mono); the algorithms are this project's own.

use crate::DigitizeError;
use crate::palette::{self, Palette, Rgb, rgb_f};
use crate::raster::{Frame, Raster};
use openstitch_core::{Command, Stitch, Thread};
use openstitch_design::{DesignObject, StitchBlock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhotoStyle {
    Fill,
    Lines,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PhotoMode {
    /// Reduce to this many PEC threads.
    Color(usize),
    /// One thread.
    Mono(Thread),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhotoStitch {
    pub style: PhotoStyle,
    pub mode: PhotoMode,
    /// Fabric colour: what unsewn areas show.
    pub fabric: Rgb,
    /// Distance between rows, mm.
    pub row_spacing_mm: f64,
    /// Fill stitch length or zigzag pitch, mm.
    pub stitch_mm: f64,
}

impl PhotoStitch {
    pub fn new(style: PhotoStyle, mode: PhotoMode) -> Self {
        let (row_spacing_mm, stitch_mm) = match style {
            PhotoStyle::Fill => (0.4, 2.0),
            PhotoStyle::Lines => (1.2, 0.8),
        };
        PhotoStitch {
            style,
            mode,
            fabric: [255, 255, 255],
            row_spacing_mm,
            stitch_mm,
        }
    }
}

/// Most grid samples one conversion may use.
pub const MAX_SAMPLES: usize = 400_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Converted {
    /// One stitch object per thread, in sewing order (ids unassigned).
    pub objects: Vec<DesignObject>,
    pub threads: Vec<Thread>,
    /// Needle points per object.
    pub stitches: Vec<usize>,
}

fn luma(c: [f64; 3]) -> f64 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

pub fn convert(image: &Raster, frame: &Frame, p: &PhotoStitch) -> Result<Converted, DigitizeError> {
    let spacing = p.row_spacing_mm.clamp(0.2, 5.0) * 10.0;
    let pitch = p.stitch_mm.clamp(0.3, 7.0) * 10.0;
    let width = f64::from(image.width()) * frame.scale;
    let height = f64::from(image.height()) * frame.scale;
    let cols = (width / pitch).round().max(1.0);
    let rows = (height / spacing).round().max(1.0);
    if cols * rows > MAX_SAMPLES as f64 {
        return Err(DigitizeError::TooLarge(format!(
            "{cols}×{rows} samples (limit {MAX_SAMPLES}); use a smaller width or coarser spacing"
        )));
    }
    let (cols, rows) = (cols as usize, rows as usize);
    let (cw, ch) = (width / cols as f64, height / rows as f64);
    let grid = image.resample(cols as u32, rows as u32);
    let mut cells: Vec<Option<[f64; 3]>> = (0..rows as u32)
        .flat_map(|y| (0..cols as u32).map(move |x| (x, y)))
        .map(|(x, y)| {
            let [r, g, b, a] = grid.pixel(x, y);
            (a >= 128).then(|| [f64::from(r), f64::from(g), f64::from(b)])
        })
        .collect();
    if cells.iter().all(Option::is_none) {
        return Err(DigitizeError::Empty(
            "the image is fully transparent".into(),
        ));
    }
    let mono = matches!(p.mode, PhotoMode::Mono(_));
    if mono {
        stretch(&mut cells);
    }
    let fabric_pec = palette::nearest_pec(p.fabric).0;
    let threads: Vec<Thread> = match &p.mode {
        PhotoMode::Mono(t) => vec![t.clone()],
        PhotoMode::Color(n) => {
            let colors: Vec<Rgb> = cells.iter().flatten().map(|c| c.map(|v| v as u8)).collect();
            let pal = Palette::build(&colors, (*n).clamp(1, 64));
            // A thread matching the fabric is left out: the fabric shows instead.
            pal.chart
                .iter()
                .zip(&pal.threads)
                .filter(|(c, _)| **c != fabric_pec)
                .map(|(_, t)| t.clone())
                .collect()
        }
    };
    if threads.is_empty() {
        return Err(DigitizeError::Empty(
            "every colour matches the fabric".into(),
        ));
    }
    // Mono works on the stretched grey image: the thread is black, the fabric white.
    let fabric = if mono {
        [255.0; 3]
    } else {
        p.fabric.map(f64::from)
    };
    let mut targets: Vec<[f64; 3]> = if mono {
        vec![[0.0; 3]]
    } else {
        threads.iter().map(rgb_f).collect()
    };
    targets.push(fabric);
    let dithered = palette::dither(&cells, cols, rows, &targets);
    // Light threads first, so darker detail sews on top.
    let mut order: Vec<usize> = (0..threads.len()).collect();
    order.sort_by(|&a, &b| {
        luma(targets[b])
            .total_cmp(&luma(targets[a]))
            .then(a.cmp(&b))
    });
    let mut objects = Vec::new();
    let mut counts = Vec::new();
    let mut sewn_threads = Vec::new();
    let mut total = 0;
    for (layer, &t) in order.iter().enumerate() {
        let stitches = match p.style {
            PhotoStyle::Fill => fill_layer(&dithered, cols, rows, t, frame, cw, ch),
            PhotoStyle::Lines => {
                let weight: Vec<f64> = match &p.mode {
                    PhotoMode::Mono(_) => cells
                        .iter()
                        .map(|c| c.map_or(0.0, |c| 1.0 - c[0] / 255.0))
                        .collect(),
                    PhotoMode::Color(_) => share(&dithered, cols, rows, t),
                };
                let offset = (layer as f64 + 0.5) / order.len() as f64;
                lines_layer(&weight, cols, rows, frame, cw, ch, offset, order.len())
            }
        };
        if stitches.iter().all(|s| s.command != Command::Stitch) {
            continue;
        }
        total += stitches.len();
        if total > crate::MAX_STITCHES {
            return Err(DigitizeError::TooLarge(format!(
                "more than {} stitches; use a smaller width or coarser spacing",
                crate::MAX_STITCHES
            )));
        }
        counts.push(
            stitches
                .iter()
                .filter(|s| s.command == Command::Stitch)
                .count(),
        );
        let thread = threads[t].clone();
        let mut o = DesignObject::stitches(StitchBlock {
            stitches,
            threads: vec![thread.clone()],
        });
        o.name = Some(format!(
            "Photo Stitch {}",
            thread.name.as_deref().unwrap_or("thread")
        ));
        objects.push(o);
        sewn_threads.push(thread);
    }
    if objects.is_empty() {
        return Err(DigitizeError::Empty(
            "the image is the fabric colour throughout".into(),
        ));
    }
    Ok(Converted {
        objects,
        threads: sewn_threads,
        stitches: counts,
    })
}

/// Mono contrast stretch: every opaque cell becomes grey, its luminance mapped so the 2nd
/// percentile is black and the 98th white (left alone when that range is under 8 levels).
fn stretch(cells: &mut [Option<[f64; 3]>]) {
    let mut l: Vec<f64> = cells.iter().flatten().map(|c| luma(*c)).collect();
    l.sort_by(f64::total_cmp);
    let (lo, hi) = match (l.get(l.len() / 50), l.get(l.len() - 1 - l.len() / 50)) {
        (Some(&lo), Some(&hi)) if hi - lo >= 8.0 => (lo, hi),
        _ => (0.0, 255.0),
    };
    for c in cells.iter_mut().flatten() {
        let g = ((luma(*c) - lo) / (hi - lo) * 255.0).clamp(0.0, 255.0);
        *c = [g; 3];
    }
}

/// Fill rows for thread `t`: serpentine rows, runs of neighbouring cells sewn as one row
/// with needle points every cell width, staggered by thirds from row to row.
fn fill_layer(
    d: &[Option<usize>],
    cols: usize,
    rows: usize,
    t: usize,
    frame: &Frame,
    cw: f64,
    ch: f64,
) -> Vec<Stitch> {
    let mut out = Vec::new();
    for y in 0..rows {
        let row_y = frame.origin.y + (y as f64 + 0.5) * ch;
        let mut runs = Vec::new();
        let mut x = 0;
        while x < cols {
            if d[y * cols + x] != Some(t) {
                x += 1;
                continue;
            }
            let a = x;
            while x < cols && d[y * cols + x] == Some(t) {
                x += 1;
            }
            runs.push((a, x));
        }
        if y % 2 == 1 {
            runs.reverse();
        }
        let phase = (y % 3) as f64 / 3.0;
        for (a, b) in runs {
            let (x0, x1) = (a as f64 * cw, b as f64 * cw);
            let mut xs = vec![x0];
            let mut k = (x0 / cw - phase).floor() + 1.0;
            while (k + phase) * cw < x1 {
                let x = (k + phase) * cw;
                if x - x0 > cw * 0.25 && x1 - x > cw * 0.25 {
                    xs.push(x);
                }
                k += 1.0;
            }
            xs.push(x1);
            if y % 2 == 1 {
                xs.reverse();
            }
            out.push(Stitch::new(frame.origin.x + xs[0], row_y, Command::Jump));
            for x in xs {
                out.push(Stitch::new(frame.origin.x + x, row_y, Command::Stitch));
            }
        }
    }
    out
}

/// Share of thread `t` among the dithered cells of each cell's 3×3 neighbourhood.
fn share(d: &[Option<usize>], cols: usize, rows: usize, t: usize) -> Vec<f64> {
    let mut out = vec![0.0; cols * rows];
    for y in 0..rows {
        for x in 0..cols {
            let (mut hit, mut n) = (0.0, 0.0);
            for ny in y.saturating_sub(1)..(y + 2).min(rows) {
                for nx in x.saturating_sub(1)..(x + 2).min(cols) {
                    n += 1.0;
                    if d[ny * cols + nx] == Some(t) {
                        hit += 1.0;
                    }
                }
            }
            out[y * cols + x] = hit / n;
        }
    }
    out
}

/// Below this weight a line breaks.
const LINE_THRESHOLD: f64 = 0.08;

/// Zigzag lines along each row (offset by `offset` of a row within it, serpentine), one
/// needle point per sample, swinging ±weight × 45 % of the row spacing.
#[allow(clippy::too_many_arguments)]
fn lines_layer(
    weight: &[f64],
    cols: usize,
    rows: usize,
    frame: &Frame,
    cw: f64,
    ch: f64,
    offset: f64,
    layers: usize,
) -> Vec<Stitch> {
    let mut out = Vec::new();
    // Colour layers share each row, so each swings within its own band.
    let amplitude = ch * 0.45 / layers.max(1) as f64;
    for y in 0..rows {
        let row_y = frame.origin.y + (y as f64 + offset) * ch;
        let xs: Vec<usize> = if y % 2 == 0 {
            (0..cols).collect()
        } else {
            (0..cols).rev().collect()
        };
        let mut run: Vec<Stitch> = Vec::new();
        let flush = |run: &mut Vec<Stitch>, out: &mut Vec<Stitch>| {
            if run.len() >= 2 {
                out.push(Stitch::new(run[0].x, run[0].y, Command::Jump));
                out.append(run);
            }
            run.clear();
        };
        for (i, &x) in xs.iter().enumerate() {
            let w = weight[y * cols + x];
            if w < LINE_THRESHOLD {
                flush(&mut run, &mut out);
                continue;
            }
            let sign = if i % 2 == 0 { -1.0 } else { 1.0 };
            run.push(Stitch::new(
                frame.origin.x + (x as f64 + 0.5) * cw,
                row_y + sign * w * amplitude,
                Command::Stitch,
            ));
        }
        flush(&mut run, &mut out);
    }
    out
}
