//! Colour reduction: median cut to N colours, each mapped to the nearest Brother PEC chart
//! thread (equal-weight squared-Euclidean distance, `openstitch-core`), plus Floyd–Steinberg
//! dithering.

use openstitch_core::{PEC_CHART, Thread, color_distance, nearest_index, pec_basic_chart};

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

impl Palette {
    /// Median cut of `colors` (a pixel list; order does not matter) into at most `n` boxes,
    /// then PEC matching. Deterministic: ties go to the lowest box / smallest packed colour.
    pub fn build(colors: &[Rgb], n: usize) -> Palette {
        let means = median_cut(colors, n.max(1));
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

/// Weighted median cut (variance-driven: the box with the largest squared error splits at
/// its mean). Returns up to `n` mean colours (fewer if the image has fewer distinct colours).
pub fn median_cut(colors: &[Rgb], n: usize) -> Vec<Rgb> {
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
    if hist.is_empty() {
        return Vec::new();
    }
    let mut boxes: Vec<Vec<(Rgb, u64)>> = vec![hist];
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
