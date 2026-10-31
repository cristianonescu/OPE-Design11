//! Label maps: majority smoothing, 4-connected components, small-region merging, tracing a
//! component to contours, and shape measures (thickness, principal axis).

use crate::raster::Frame;
use crate::trace::{self, signed_area};
use openstitch_design::Contour;
use openstitch_design::fmath::Det;

/// Pixels that are not sewn.
pub const NONE: u16 = u16::MAX;

#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub label: u16,
    pub area: usize,
    /// Bounding box `[x0, x1) × [y0, y1)`.
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
}

/// 4-connected components of equal, non-[`NONE`] labels, numbered in scan order of their
/// first pixel. Returns the component id per pixel (`u32::MAX` for `NONE`) and the components.
pub fn components(labels: &[u16], w: usize, h: usize) -> (Vec<u32>, Vec<Component>) {
    let mut id = vec![u32::MAX; labels.len()];
    let mut comps = Vec::new();
    let mut stack = Vec::new();
    for start in 0..labels.len() {
        if labels[start] == NONE || id[start] != u32::MAX {
            continue;
        }
        let label = labels[start];
        let c = comps.len() as u32;
        let mut comp = Component {
            label,
            area: 0,
            x0: usize::MAX,
            y0: usize::MAX,
            x1: 0,
            y1: 0,
        };
        id[start] = c;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            comp.area += 1;
            comp.x0 = comp.x0.min(x);
            comp.y0 = comp.y0.min(y);
            comp.x1 = comp.x1.max(x + 1);
            comp.y1 = comp.y1.max(y + 1);
            let mut visit = |j: usize| {
                if labels[j] == label && id[j] == u32::MAX {
                    id[j] = c;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
        comps.push(comp);
    }
    (id, comps)
}

/// `passes` rounds of a 3×3 majority filter: a pixel takes a label held by at least 5 of
/// the 9 pixels in its window (edges count only pixels inside the image, majority > half).
pub fn majority(labels: &mut [u16], w: usize, h: usize, passes: u32) {
    for _ in 0..passes {
        let old = labels.to_vec();
        let mut counts: Vec<(u16, u8)> = Vec::with_capacity(9);
        for y in 0..h {
            for x in 0..w {
                counts.clear();
                let mut total = 0u8;
                for ny in y.saturating_sub(1)..(y + 2).min(h) {
                    for nx in x.saturating_sub(1)..(x + 2).min(w) {
                        let l = old[ny * w + nx];
                        total += 1;
                        match counts.iter_mut().find(|c| c.0 == l) {
                            Some(c) => c.1 += 1,
                            None => counts.push((l, 1)),
                        }
                    }
                }
                if let Some(&(l, n)) = counts.iter().max_by_key(|c| c.1)
                    && n * 2 > total
                {
                    labels[y * w + x] = l;
                }
            }
        }
    }
}

/// Merges every component smaller than `min_area` pixels into the neighbouring label it
/// shares the longest border with (ties to the lower label); components surrounded only by
/// unsewn pixels become unsewn. Smallest first; repeated until none is left (at most 8
/// rounds).
pub fn merge_small(labels: &mut [u16], w: usize, h: usize, min_area: usize) {
    merge_small_except(labels, w, h, min_area, |_| false);
}

/// [`merge_small`], keeping the small components `keep` accepts (OpenStitch mode keeps the
/// long, thin pieces of line art and merges only compact specks).
pub fn merge_small_except(
    labels: &mut [u16],
    w: usize,
    h: usize,
    min_area: usize,
    keep: impl Fn(&Component) -> bool,
) {
    if min_area <= 1 {
        return;
    }
    for _ in 0..8 {
        let (id, comps) = components(labels, w, h);
        let mut small: Vec<usize> = (0..comps.len())
            .filter(|&c| comps[c].area < min_area && !keep(&comps[c]))
            .collect();
        if small.is_empty() {
            return;
        }
        small.sort_by_key(|&c| (comps[c].area, c));
        for c in small {
            let k = &comps[c];
            let mut neighbours: Vec<(u16, usize)> = Vec::new();
            let mut pixels = Vec::new();
            for y in k.y0..k.y1 {
                for x in k.x0..k.x1 {
                    let i = y * w + x;
                    if id[i] != c as u32 || labels[i] != k.label {
                        continue;
                    }
                    pixels.push(i);
                    let mut look = |j: usize| {
                        if id[j] != c as u32 {
                            let l = labels[j];
                            match neighbours.iter_mut().find(|n| n.0 == l) {
                                Some(n) => n.1 += 1,
                                None => neighbours.push((l, 1)),
                            }
                        }
                    };
                    if x > 0 {
                        look(i - 1);
                    }
                    if x + 1 < w {
                        look(i + 1);
                    }
                    if y > 0 {
                        look(i - w);
                    }
                    if y + 1 < h {
                        look(i + w);
                    }
                }
            }
            if neighbours.iter().any(|n| n.0 == k.label) {
                // An earlier merge joined it to a region of its own colour.
                continue;
            }
            neighbours.sort_by_key(|n| (std::cmp::Reverse(n.1), n.0));
            let target = neighbours
                .iter()
                .find(|n| n.0 != NONE)
                .map_or(NONE, |n| n.0);
            for i in pixels {
                labels[i] = target;
            }
        }
    }
}

/// Mask of component `c` cropped to its bounding box.
pub fn mask(id: &[u32], w: usize, c: u32, k: &Component) -> (Vec<bool>, usize, usize) {
    let (mw, mh) = (k.x1 - k.x0, k.y1 - k.y0);
    let mut m = vec![false; mw * mh];
    for y in 0..mh {
        for x in 0..mw {
            m[y * mw + x] = id[(y + k.y0) * w + x + k.x0] == c;
        }
    }
    (m, mw, mh)
}

/// Tracing options shared by the conversions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceOptions {
    /// Douglas–Peucker tolerance, pixels.
    pub simplify: f64,
    /// Fit cubic Béziers to smooth stretches.
    pub curves: bool,
    /// Turning angle above which a vertex stays a corner, degrees.
    pub corner_degrees: f64,
}

impl Default for TraceOptions {
    fn default() -> Self {
        TraceOptions {
            simplify: 0.7,
            curves: true,
            corner_degrees: 55.0,
        }
    }
}

/// Closed design contours of a cropped mask whose top-left pixel is (`ox`, `oy`): outer
/// boundaries and holes (even-odd), loops smaller than a quarter pixel dropped.
pub fn trace_mask(
    m: &[bool],
    mw: usize,
    mh: usize,
    ox: usize,
    oy: usize,
    o: &TraceOptions,
    frame: &Frame,
) -> Vec<Contour> {
    let shifted = Frame {
        origin: frame.point(ox as f64, oy as f64),
        scale: frame.scale,
    };
    trace::marching_squares(m, mw, mh)
        .into_iter()
        .filter(|l| signed_area(l).abs() >= 0.25)
        .map(|l| {
            let kept = trace::simplify_closed(&l, o.simplify);
            trace::contour(
                &l,
                &kept,
                true,
                o.curves,
                o.simplify,
                o.corner_degrees,
                &shifted,
            )
        })
        .collect()
}

/// Chamfer (3-4) distance of each mask pixel to the nearest outside pixel (pixels beyond the
/// mask edge count as outside); 3 per pixel step, 0 outside the mask.
pub fn distance_map(m: &[bool], mw: usize, mh: usize) -> Vec<u32> {
    let (pw, ph) = (mw + 2, mh + 2);
    let mut d = vec![0u32; pw * ph];
    for y in 0..mh {
        for x in 0..mw {
            if m[y * mw + x] {
                d[(y + 1) * pw + x + 1] = u32::MAX / 2;
            }
        }
    }
    for y in 1..ph - 1 {
        for x in 1..pw - 1 {
            let i = y * pw + x;
            if d[i] != 0 {
                let v = [
                    d[i - 1] + 3,
                    d[i - pw] + 3,
                    d[i - pw - 1] + 4,
                    d[i - pw + 1] + 4,
                ];
                d[i] = d[i].min(v.into_iter().min().unwrap_or(u32::MAX));
            }
        }
    }
    for y in (1..ph - 1).rev() {
        for x in (1..pw - 1).rev() {
            let i = y * pw + x;
            if d[i] != 0 {
                let v = [
                    d[i + 1] + 3,
                    d[i + pw] + 3,
                    d[i + pw + 1] + 4,
                    d[i + pw - 1] + 4,
                ];
                d[i] = d[i].min(v.into_iter().min().unwrap_or(u32::MAX));
            }
        }
    }
    (0..mh)
        .flat_map(|y| (0..mw).map(move |x| (x, y)))
        .map(|(x, y)| d[(y + 1) * pw + x + 1])
        .collect()
}

/// Stroke width in pixels for a chamfer distance: a 5-pixel-wide bar's centre measures 5.
pub fn width_for(distance: u32) -> f64 {
    (2.0 * f64::from(distance) / 3.0 - 1.0).max(1.0)
}

/// Thickness of a mask in pixels: the width at its deepest point.
///
/// The chamfer width reads a stroke an odd number of pixels wide exactly but one pixel short
/// when the width is even: its centre then falls between two pixel rows, and the deepest
/// pixels form a ridge two pixels thick. So when a deepest pixel's neighbour on the side away
/// from its nearest edge is as deep as it is, one pixel is added (#127: Auto Punch's satin
/// bounds then hold at every working resolution, not only at some).
pub fn thickness(m: &[bool], mw: usize, mh: usize) -> f64 {
    let d = distance_map(m, mw, mh);
    let Some(&max) = d.iter().max() else {
        return width_for(0);
    };
    let at = |x: isize, y: isize| -> u32 {
        if x < 0 || y < 0 || x >= mw as isize || y >= mh as isize {
            0
        } else {
            d[y as usize * mw + x as usize]
        }
    };
    let steps: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let two_thick = (0..mh).any(|y| {
        (0..mw).any(|x| {
            if d[y * mw + x] != max {
                return false;
            }
            let (x, y) = (x as isize, y as isize);
            // The steps towards the nearest edge are those to the shallowest neighbours; the
            // ridge is two pixels thick when the pixel opposite each of them is as deep.
            // (At a stroke's end the end itself is one of those steps, and the pixel beyond
            // it along the stroke does not count.)
            let shallowest = steps
                .iter()
                .map(|(dx, dy)| at(x + dx, y + dy))
                .min()
                .unwrap_or(0);
            steps
                .iter()
                .filter(|(dx, dy)| at(x + dx, y + dy) == shallowest)
                .all(|(dx, dy)| at(x - dx, y - dy) == max)
        })
    });
    width_for(max) + if two_thick { 1.0 } else { 0.0 }
}

/// Direction of a mask's long axis from its second moments, degrees in (-90, 90], rounded to
/// whole degrees; 0 for round or square blobs.
pub fn principal_axis(m: &[bool], mw: usize, mh: usize) -> f64 {
    let (mut n, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for y in 0..mh {
        for x in 0..mw {
            if m[y * mw + x] {
                n += 1.0;
                sx += x as f64;
                sy += y as f64;
            }
        }
    }
    if n == 0.0 {
        return 0.0;
    }
    let (cx, cy) = (sx / n, sy / n);
    let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
    for y in 0..mh {
        for x in 0..mw {
            if m[y * mw + x] {
                let (dx, dy) = (x as f64 - cx, y as f64 - cy);
                a += dx * dx;
                b += dx * dy;
                c += dy * dy;
            }
        }
    }
    if (a - c).abs() < 1e-9 * n * n && b.abs() < 1e-9 * n * n {
        return 0.0;
    }
    let deg = (0.5 * (2.0 * b).datan2(a - c)).to_degrees().round();
    if deg <= -90.0 { deg + 180.0 } else { deg }
}
