//! Colour reduction: a 3×3 modal colour pre-filter and a colour census capped at
//! [`CENSUS_COLORS`] tracked colours (the prepass Auto Punch and Cross Stitch share, #127 #132),
//! median cut to N colours, each mapped to the nearest Brother PEC chart thread (equal-weight
//! squared-Euclidean distance, `openstitch-core`, #119), plus Floyd–Steinberg dithering.

use crate::raster::Raster;
use openstitch_core::{PEC_CHART, Thread, color_distance, nearest_index, pec_basic_chart};
use std::collections::HashMap;

pub type Rgb = [u8; 3];

/// Reduced palette: `means` are the median-cut box colours (what pixels are matched against),
/// `threads` the distinct PEC threads they map to, `thread_of[i]` the thread of mean `i`.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub means: Vec<Rgb>,
    pub thread_of: Vec<usize>,
    pub threads: Vec<Thread>,
    /// PEC chart number (1–64) of each thread.
    pub chart: Vec<usize>,
}

fn thread(c: Rgb) -> Thread {
    Thread::rgb(u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]))
}

/// PEC chart number and thread nearest to `c`.
pub fn nearest_pec(c: Rgb) -> (usize, Thread) {
    let i = nearest_index(&thread(c), pec_basic_chart()).unwrap_or(20);
    (i, PEC_CHART[i].clone().unwrap_or_else(|| thread(c)))
}

/// Most distinct colours the census tracks; later colours join their nearest tracked one.
pub const CENSUS_COLORS: usize = 100;

/// RGB distance within which a census colour counts under a more frequent tracked colour.
/// 32 matches PE-DESIGN's automatic colour count on the round-5 images best (g01 4, g13 10,
/// g14 2, g15 5, the photos 10); 16 keeps edge shades as colours, 48 merges distinct ones.
pub const MERGE_DIST: u32 = 32;

/// The colour census of an image: the 3×3 modal colour of every opaque pixel (alpha ≥ 128),
/// counted into at most [`CENSUS_COLORS`] tracked colours, in the order they were first met.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Census {
    /// Tracked colours with their pixel counts, first met first.
    pub colors: Vec<(Rgb, u64)>,
    /// The modal colour of every pixel, `None` where it is transparent.
    pub modal: Vec<Option<Rgb>>,
}

impl Census {
    /// Opaque pixels counted.
    pub fn pixels(&self) -> u64 {
        self.colors.iter().map(|c| c.1).sum()
    }

    /// The automatic colour count (PE-DESIGN's Auto Punch and Cross Stitch fill in "Max. Number
    /// of Colors" from the image: 2 for black bars on white, 5 for four flat shapes on white):
    /// the tracked colours holding at least [`AUTO_SHARE`] of the opaque pixels, at least 1 and
    /// at most [`AUTO_MAX`].
    pub fn auto_colors(&self) -> usize {
        let total = self.pixels() as f64;
        self.colors
            .iter()
            .filter(|c| c.1 as f64 >= total * AUTO_SHARE)
            .count()
            .clamp(1, AUTO_MAX)
    }
}

/// Smallest share of the opaque pixels a colour needs to count in [`Census::auto_colors`].
pub const AUTO_SHARE: f64 = 0.005;
/// Most colours [`Census::auto_colors`] picks: PE-DESIGN fills in at most 10 (round 5: 10 for
/// the twelve-colour mosaic `g13` and for every photo).
pub const AUTO_MAX: usize = 10;

/// Thread indices of `palette` in PE-DESIGN's colour order (round-4 captures
/// `autopunch-default.pes` and `autopunch-owner-shapes-moved.pes`, the same in Cross Stitch):
/// by the reduced colour's channel sum R + G + B, highest first (white 765, yellow 480, blue
/// 290, red 280, green 250 on the owner's shapes, wherever each colour sits and whatever its
/// area); ties to the lower index. A thread reached from several reduced colours takes the
/// brightest of them.
pub fn brightness_order(palette: &Palette, threads: impl IntoIterator<Item = usize>) -> Vec<usize> {
    let sum = |t: usize| -> u32 {
        palette
            .means
            .iter()
            .zip(&palette.thread_of)
            .filter(|(_, k)| **k == t)
            .map(|(m, _)| m.iter().map(|&v| u32::from(v)).sum::<u32>())
            .max()
            .unwrap_or(0)
    };
    let mut order: Vec<usize> = threads.into_iter().collect();
    order.sort_by_key(|&t| (std::cmp::Reverse(sum(t)), t));
    order
}

/// Modal colour of the 3×3 window around `(x, y)`: the colour most of the window's opaque
/// pixels have. Ties keep the centre's own colour when it is among them, otherwise the
/// smallest packed colour. `None` for a transparent centre.
fn modal_at(image: &Raster, x: u32, y: u32) -> Option<Rgb> {
    let [r, g, b, a] = image.pixel(x, y);
    if a < 128 {
        return None;
    }
    let centre = [r, g, b];
    let mut counts: Vec<(Rgb, u8)> = Vec::with_capacity(9);
    for ny in y.saturating_sub(1)..(y + 2).min(image.height()) {
        for nx in x.saturating_sub(1)..(x + 2).min(image.width()) {
            let [r, g, b, a] = image.pixel(nx, ny);
            if a < 128 {
                continue;
            }
            let c = [r, g, b];
            match counts.iter_mut().find(|k| k.0 == c) {
                Some(k) => k.1 += 1,
                None => counts.push((c, 1)),
            }
        }
    }
    let most = counts.iter().map(|k| k.1).max().unwrap_or(0);
    let mut tied: Vec<Rgb> = counts.iter().filter(|k| k.1 == most).map(|k| k.0).collect();
    if tied.contains(&centre) {
        return Some(centre);
    }
    tied.sort_by_key(|&c| pack(c));
    tied.first().copied().or(Some(centre))
}

/// Squared Euclidean RGB distance (the census's own merge metric).
fn sq_dist(a: Rgb, b: Rgb) -> u32 {
    (0..3)
        .map(|k| {
            let d = i32::from(a[k]) - i32::from(b[k]);
            (d * d) as u32
        })
        .sum()
}

/// The colour census of `image` (#127): every opaque pixel takes its 3×3 modal colour, and the
/// modal colours are counted into at most [`CENSUS_COLORS`] tracked colours. Colours are taken
/// most frequent first (ties to the first met): one within [`MERGE_DIST`] of a tracked colour
/// counts under the nearest tracked one, any other is tracked while there is room and counted
/// under the nearest tracked one after that. Tracked colours are listed in the order first met.
///
/// Taking the colours by frequency, not in scan order, matters on anti-aliased images (round 5,
/// `examples/images/g01-emblem.png`): their many small edge shades filled all 100 slots by the
/// 32nd row, so the emblem's orange and yellow counted as grey. PE-DESIGN keeps all four
/// colours (4 automatic colours), as this census does. Deterministic for a given image.
pub fn census(image: &Raster) -> Census {
    let mut seen: HashMap<Rgb, usize> = HashMap::new();
    let mut all: Vec<(Rgb, u64)> = Vec::new();
    let mut modal = Vec::with_capacity(image.width() as usize * image.height() as usize);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let m = modal_at(image, x, y);
            modal.push(m);
            let Some(c) = m else { continue };
            let i = *seen.entry(c).or_insert_with(|| {
                all.push((c, 0));
                all.len() - 1
            });
            all[i].1 += 1;
        }
    }
    // Most frequent first: a colour within MERGE_DIST of a tracked colour counts under the
    // nearest one; otherwise it is tracked while there is room, else counted under the nearest.
    let mut rank: Vec<usize> = (0..all.len()).collect();
    rank.sort_by_key(|&i| (std::cmp::Reverse(all[i].1), i));
    // (first-met index, colour, count)
    let mut tracked: Vec<(usize, Rgb, u64)> = Vec::new();
    for &i in &rank {
        let (c, n) = all[i];
        let mut best = (u32::MAX, 0);
        for (j, k) in tracked.iter().enumerate() {
            let d = sq_dist(c, k.1);
            if d < best.0 {
                best = (d, j);
            }
        }
        if best.0 <= MERGE_DIST * MERGE_DIST || tracked.len() >= CENSUS_COLORS {
            tracked[best.1].2 += n;
        } else {
            tracked.push((i, c, n));
        }
    }
    tracked.sort_by_key(|k| k.0);
    let colors = tracked.into_iter().map(|(_, c, n)| (c, n)).collect();
    Census { colors, modal }
}

impl Palette {
    /// Median cut of `colors` (a pixel list; order does not matter) into at most `n` boxes,
    /// then PEC matching. Deterministic: ties go to the lowest box / smallest packed colour.
    pub fn build(colors: &[Rgb], n: usize) -> Palette {
        Self::from_histogram(&histogram(colors), n)
    }

    /// The palette of a colour census: its tracked colours, weighted by their counts, median
    /// cut into at most `n` boxes, then PEC matching (#127).
    pub fn from_census(census: &Census, n: usize) -> Palette {
        let mut hist = census.colors.clone();
        hist.sort_by_key(|c| pack(c.0));
        Self::from_histogram(&hist, n)
    }

    /// Median cut of a sorted colour histogram, then PEC matching.
    fn from_histogram(hist: &[(Rgb, u64)], n: usize) -> Palette {
        let means = median_cut_histogram(hist, n.max(1));
        let mut threads: Vec<Thread> = Vec::new();
        let mut chart: Vec<usize> = Vec::new();
        let mut thread_of = Vec::with_capacity(means.len());
        for &m in &means {
            let (i, t) = nearest_pec(m);
            let k = chart.iter().position(|&c| c == i).unwrap_or_else(|| {
                chart.push(i);
                threads.push(t);
                chart.len() - 1
            });
            thread_of.push(k);
        }
        Palette {
            means,
            thread_of,
            threads,
            chart,
        }
    }

    /// Thread index of the box mean nearest to `c` (ties to the lower index).
    pub fn classify(&self, c: Rgb) -> usize {
        let t = thread(c);
        let mut best = (i64::MAX, 0);
        for (i, &m) in self.means.iter().enumerate() {
            let d = color_distance(&t, &thread(m));
            if d < best.0 {
                best = (d, i);
            }
        }
        self.thread_of.get(best.1).copied().unwrap_or(0)
    }
}

fn pack(c: Rgb) -> u32 {
    u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2])
}

/// Distinct colours of a pixel list with their counts, sorted by packed value.
fn histogram(colors: &[Rgb]) -> Vec<(Rgb, u64)> {
    let mut packed: Vec<u32> = colors.iter().map(|&c| pack(c)).collect();
    packed.sort_unstable();
    let mut hist: Vec<(Rgb, u64)> = Vec::new();
    for p in packed {
        let c = [(p >> 16) as u8, (p >> 8) as u8, p as u8];
        match hist.last_mut() {
            Some((last, count)) if *last == c => *count += 1,
            _ => hist.push((c, 1)),
        }
    }
    hist
}

/// Weighted median cut (variance-driven: the box with the largest squared error splits at
/// its mean). Returns up to `n` mean colours (fewer if the image has fewer distinct colours).
pub fn median_cut(colors: &[Rgb], n: usize) -> Vec<Rgb> {
    median_cut_histogram(&histogram(colors), n)
}

/// [`median_cut`] of a colour histogram sorted by packed value.
fn median_cut_histogram(hist: &[(Rgb, u64)], n: usize) -> Vec<Rgb> {
    if hist.is_empty() {
        return Vec::new();
    }
    let mut boxes: Vec<Vec<(Rgb, u64)>> = vec![hist.to_vec()];
    while boxes.len() < n {
        // Split the box with the largest squared error, along its highest-variance channel,
        // at the weighted mean; ties go to the lower box and channel.
        let mut pick: Option<(usize, usize, f64, f64)> = None;
        for (b, cs) in boxes.iter().enumerate() {
            if cs.len() < 2 {
                continue;
            }
            let w: f64 = cs.iter().map(|c| c.1 as f64).sum();
            let mut sse = 0.0;
            let mut best_ch = (0, -1.0, 0.0);
            for ch in 0..3 {
                let mean = cs
                    .iter()
                    .map(|c| f64::from(c.0[ch]) * c.1 as f64)
                    .sum::<f64>()
                    / w;
                let var: f64 = cs
                    .iter()
                    .map(|c| (f64::from(c.0[ch]) - mean).powi(2) * c.1 as f64)
                    .sum();
                sse += var;
                if var > best_ch.1 {
                    best_ch = (ch, var, mean);
                }
            }
            if pick.is_none_or(|(_, _, e, _)| sse > e) {
                pick = Some((b, best_ch.0, sse, best_ch.2));
            }
        }
        let Some((b, ch, sse, mean)) = pick else {
            break;
        };
        if sse <= 0.0 {
            break;
        }
        let mut cs = std::mem::take(&mut boxes[b]);
        cs.sort_by_key(|c| (c.0[ch], pack(c.0)));
        let cut = cs
            .iter()
            .position(|c| f64::from(c.0[ch]) > mean)
            .unwrap_or(cs.len() - 1)
            .clamp(1, cs.len() - 1);
        let upper = cs.split_off(cut);
        boxes[b] = cs;
        boxes.push(upper);
    }
    boxes
        .iter()
        .map(|cs| {
            let w: u64 = cs.iter().map(|c| c.1).sum();
            let mean = |ch: usize| {
                let s: u64 = cs.iter().map(|c| u64::from(c.0[ch]) * c.1).sum();
                ((s + w / 2) / w) as u8
            };
            [mean(0), mean(1), mean(2)]
        })
        .collect()
}

/// Floyd–Steinberg error diffusion (serpentine) of a `w`×`h` grid onto `targets`. Cells that
/// are `None` (transparent) are skipped and receive no error. Returns the target index per
/// cell, `None` for skipped cells. Ties go to the lower target index.
pub fn dither(
    cells: &[Option<[f64; 3]>],
    w: usize,
    h: usize,
    targets: &[[f64; 3]],
) -> Vec<Option<usize>> {
    let mut work: Vec<[f64; 3]> = cells.iter().map(|c| c.unwrap_or([0.0; 3])).collect();
    let mut out = vec![None; cells.len()];
    if targets.is_empty() {
        return out;
    }
    for y in 0..h {
        let forward = y % 2 == 0;
        for k in 0..w {
            let x = if forward { k } else { w - 1 - k };
            let i = y * w + x;
            if cells[i].is_none() {
                continue;
            }
            let c = work[i].map(|v| v.clamp(0.0, 255.0));
            let mut best = (f64::INFINITY, 0);
            for (t, tc) in targets.iter().enumerate() {
                let d = weighted(c, *tc);
                if d < best.0 {
                    best = (d, t);
                }
            }
            out[i] = Some(best.1);
            let e = [0, 1, 2].map(|ch| c[ch] - targets[best.1][ch]);
            let dir: isize = if forward { 1 } else { -1 };
            let mut spread = |dx: isize, dy: usize, f: f64| {
                let nx = x as isize + dx;
                let ny = y + dy;
                if nx < 0 || nx >= w as isize || ny >= h {
                    return;
                }
                let j = ny * w + nx as usize;
                if cells[j].is_some() {
                    for ch in 0..3 {
                        work[j][ch] += e[ch] * f;
                    }
                }
            };
            spread(dir, 0, 7.0 / 16.0);
            spread(-dir, 1, 3.0 / 16.0);
            spread(0, 1, 5.0 / 16.0);
            spread(dir, 1, 1.0 / 16.0);
        }
    }
    out
}

/// Equal-weight squared Euclidean distance on floating-point colours (the dithering error is
/// fractional), the same metric as `openstitch_core::color_distance`.
fn weighted(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (r, g, bl) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    r * r + g * g + bl * bl
}

pub fn rgb_f(t: &Thread) -> [f64; 3] {
    [f64::from(t.red), f64::from(t.green), f64::from(t.blue)]
}
