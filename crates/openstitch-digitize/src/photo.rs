//! Photo Stitch approximations: tonal images become density-modulated stitching.
//!
//! - [`PhotoStyle::Fill`] (after Photo Stitch 1): the image is sampled on a grid of fill
//!   stitches (row spacing × stitch length) and error-diffused (Floyd–Steinberg) onto the
//!   thread colours plus the fabric colour. Each thread sews its cells as staggered fill rows,
//!   the runs visited nearest first so a colour sews in few long paths (see `fill_layer`);
//!   fabric cells stay unsewn, so tone becomes stitch coverage.
//! - [`PhotoStyle::Lines`] (after Photo Stitch 2): rows of zigzag lines whose amplitude follows
//!   the tone (mono: darkness; colour: how much of each thread the dithered image holds
//!   locally). Light areas break the line.
//!
//! Mono mode works on integer Rec. 601 luminance (`(30 R + 59 G + 11 B) / 100`) and first
//! stretches the image's contrast over the full 0–255 range from its darkest to its lightest
//! grey (plain min/max auto-levels; left alone when the range is 1 level or less, or already
//! full), as PE-DESIGN's mono Photo Stitch does (#128).
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
    /// Fabric (page) colour: what unsewn areas show.
    pub fabric: Rgb,
    /// Fill style: sew the page colour too (PE-DESIGN 11.50's "Sew Page Color", ticked by
    /// default in Color mode), as a thread of its own that sews first, so every opaque cell is
    /// sewn and the image's light areas get the page thread instead of bare fabric. Mono and
    /// Lines ignore it (PE-DESIGN's Mono has no such option).
    pub sew_page: bool,
    /// Distance between rows, mm.
    pub row_spacing_mm: f64,
    /// Fill stitch length or zigzag pitch, mm.
    pub stitch_mm: f64,
}

impl PhotoStitch {
    pub fn new(style: PhotoStyle, mode: PhotoMode) -> Self {
        let (row_spacing_mm, stitch_mm) = match style {
            // Run Pitch 3.0 mm is PE-DESIGN 11.50's default; 0.25 mm rows give its density
            // (160-210 needle points per cm² in the tone bands of the round-4 capture
            // `photostitch1-default-plus-cross.pes`; 12 860 needle points against its 12 255).
            PhotoStyle::Fill => (FILL_ROW_SPACING_MM, RUN_PITCH_MM),
            PhotoStyle::Lines => (1.2, 0.8),
        };
        let sew_page = style == PhotoStyle::Fill && matches!(mode, PhotoMode::Color(_));
        PhotoStitch {
            style,
            mode,
            fabric: [255, 255, 255],
            // PE-DESIGN's Mono mode has no Sew Page Color and leaves light areas unsewn
            // (round-4 `photostitch1-mono-grey-tonal.pes`).
            sew_page,
            row_spacing_mm,
            stitch_mm,
        }
    }
}

/// Photo Stitch 1's default Run Pitch, mm (PE-DESIGN 11.50).
pub const RUN_PITCH_MM: f64 = 3.0;
/// Default fill row spacing, mm: with [`RUN_PITCH_MM`] it gives PE-DESIGN's needle density.
pub const FILL_ROW_SPACING_MM: f64 = 0.25;
/// Photo Stitch 1's default Max. Number of Colors (PE-DESIGN 11.50).
pub const COLORS: usize = 10;

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

/// Integer Rec. 601 luminance, 0–255: `(30 R + 59 G + 11 B) / 100`, truncated.
pub fn luma(c: [f64; 3]) -> f64 {
    f64::from(crate::raster::luma601(c[0] as u8, c[1] as u8, c[2] as u8))
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
    // Sew Page Color: the fabric target becomes a thread of its own (the nearest chart thread),
    // so the cells dithered to it are sewn too.
    let mut threads = threads;
    if p.sew_page && p.style == PhotoStyle::Fill && !mono {
        threads.push(palette::nearest_pec(p.fabric).1);
    }
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

/// Mono contrast stretch: every opaque cell becomes its grey `g = (30 R + 59 G + 11 B) / 100`;
/// when the greys span more than one level and less than the whole range, each is mapped
/// linearly so the darkest becomes 0 and the lightest 255: `g' = (g − min) · 255 / (max − min)`,
/// in integers. A flat image (range 0 or 1) and one already spanning 0–255 are left as they are.
fn stretch(cells: &mut [Option<[f64; 3]>]) {
    let mut greys: Vec<u8> = cells.iter().flatten().map(|c| luma(*c) as u8).collect();
    auto_levels(&mut greys);
    for (c, g) in cells.iter_mut().flatten().zip(greys) {
        *c = [f64::from(g); 3];
    }
}

/// Full-range auto-levels on 8-bit greys: when `max − min` is in `(1, 255)`, every grey becomes
/// `(g − min) · 255 / (max − min)` (integer division); otherwise the greys are left alone.
pub fn auto_levels(greys: &mut [u8]) {
    let (lo, hi) = greys
        .iter()
        .fold((255u8, 0u8), |(lo, hi), &g| (lo.min(g), hi.max(g)));
    let range = u32::from(hi.saturating_sub(lo));
    if !(1 < range && range < 255) {
        return;
    }
    for g in greys {
        *g = (u32::from(*g - lo) * 255 / range) as u8;
    }
}

/// Fill rows for thread `t`: serpentine rows, runs of neighbouring cells sewn as one row
/// Longest move between two runs of one thread that is sewn as a travel stitch instead of a
/// jump (and trim), in cell widths.
pub const JOIN_CELLS: f64 = 2.0;
/// A turn into the next run leaves out the previous run's last needle point when the stitch
/// from its second-to-last point stays within this many cell widths.
pub const TURN_CELLS: f64 = 1.25;

/// One run of neighbouring cells of a thread in a grid row, with its needle points (x in
/// design units, left to right) and the row's y.
struct Run {
    xs: Vec<f64>,
    y: f64,
    row: usize,
}

/// Fill rows for thread `t`. Each run of neighbouring cells in a row is a straight row of
/// needle points every cell width, staggered by thirds from row to row. The runs are sewn
/// nearest first, as PE-DESIGN's Photo Stitch 1 sews a colour in few long paths
/// (round-4 captures: 1–5 jumps a colour): from where the needle is, the closest unsewn run
/// end within [`JOIN_CELLS`] of a cell width in x and two rows in y is sewn next, entered from
/// that end, and reached with a travel stitch; when none is that close, the needle jumps to
/// the first unsewn run in row order.
fn fill_layer(
    d: &[Option<usize>],
    cols: usize,
    rows: usize,
    t: usize,
    frame: &Frame,
    cw: f64,
    ch: f64,
) -> Vec<Stitch> {
    // Runs, grouped by row; each row's runs left to right.
    let mut by_row: Vec<Vec<usize>> = vec![Vec::new(); rows];
    let mut runs: Vec<Run> = Vec::new();
    for y in 0..rows {
        let row_y = frame.origin.y + (y as f64 + 0.5) * ch;
        let phase = (y % 3) as f64 / 3.0;
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
            let (x0, x1) = (a as f64 * cw, x as f64 * cw);
            let mut xs = vec![frame.origin.x + x0];
            let mut k = (x0 / cw - phase).floor() + 1.0;
            while (k + phase) * cw < x1 {
                let px = (k + phase) * cw;
                if px - x0 > cw * 0.25 && x1 - px > cw * 0.25 {
                    xs.push(frame.origin.x + px);
                }
                k += 1.0;
            }
            xs.push(frame.origin.x + x1);
            by_row[y].push(runs.len());
            runs.push(Run {
                xs,
                y: row_y,
                row: y,
            });
        }
    }
    let mut done = vec![false; runs.len()];
    let mut next_scan = 0usize;
    let mut out: Vec<Stitch> = Vec::new();
    let reach_x = JOIN_CELLS * cw + 1e-6;
    let mut pen: Option<(f64, usize)> = None;
    let mut last_len = 0usize;
    for _ in 0..runs.len() {
        // The closest unsewn run end near the needle: (distance, run, enter from the right).
        let mut best: Option<(f64, usize, bool)> = None;
        if let Some((px, prow)) = pen {
            let first_row = prow.saturating_sub(2);
            let near = by_row.iter().enumerate().take(prow + 3).skip(first_row);
            for (row, in_row) in near {
                let dy = (row as f64 - prow as f64).abs() * ch;
                for &r in in_row {
                    if done[r] {
                        continue;
                    }
                    let (l, rgt) = (runs[r].xs[0], runs[r].xs[runs[r].xs.len() - 1]);
                    for (end, from_right) in [(l, false), (rgt, true)] {
                        let dx = (end - px).abs();
                        if dx > reach_x {
                            continue;
                        }
                        let dist = dx.hypot(dy);
                        if best.is_none_or(|b| dist < b.0 - 1e-9) {
                            best = Some((dist, r, from_right));
                        }
                    }
                }
            }
        }
        let (r, from_right, travel) = match best {
            Some((_, r, from_right)) => (r, from_right, true),
            None => {
                while done[next_scan] {
                    next_scan += 1;
                }
                // Serpentine as before: odd rows enter from the right.
                (next_scan, runs[next_scan].row % 2 == 1, false)
            }
        };
        done[r] = true;
        let run = &runs[r];
        let mut xs = run.xs.clone();
        if from_right {
            xs.reverse();
        }
        if !travel {
            out.push(Stitch::new(xs[0], run.y, Command::Jump));
        } else if last_len >= 2
            && let Some(prev) = out.len().checked_sub(2).map(|i| out[i])
            && (prev.x - xs[0]).hypot(prev.y - run.y) <= TURN_CELLS * cw + 1e-6
        {
            // A short hop to the next row: the previous run's last needle point is left out,
            // so the stitch turns from its second-to-last point straight into this run, as a
            // fill row turns, instead of a sub-millimetre travel stitch.
            out.pop();
        }
        for &x in &xs {
            out.push(Stitch::new(x, run.y, Command::Stitch));
        }
        last_len = xs.len();
        pen = Some((xs[xs.len() - 1], run.row));
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
