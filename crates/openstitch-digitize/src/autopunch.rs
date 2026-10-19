//! Auto Punch approximation: image → colour reduction to N PEC threads → regions → traced
//! outlines → shape objects, satin where the region is narrow enough, fill elsewhere.
//!
//! Specified from PE-DESIGN 11's public guide (Auto Punch converts an image's colour areas
//! into embroidery regions with chosen colours omitted); the algorithm is this project's own.

use crate::DigitizeError;
use crate::palette::Palette;
use crate::raster::{Frame, Raster};
use crate::regions::{self, NONE, TraceOptions};
use openstitch_core::Thread;
use openstitch_design::geometry::scan_rows;
use openstitch_design::{DesignObject, RegionSewing, RegionType};

#[derive(Clone, Debug, PartialEq)]
pub struct AutoPunch {
    /// Thread colours to reduce to (1–64).
    pub colors: usize,
    /// Longest side of the working raster, pixels.
    pub resolution: u32,
    /// Regions smaller than this merge into a neighbour, mm².
    pub min_area_mm2: f64,
    /// 3×3 majority-filter passes before segmentation.
    pub smooth: u32,
    pub trace: TraceOptions,
    /// Regions whose satin stitches would be at most this long are satin, mm.
    pub satin_max_mm: f64,
    /// Leave out the colour covering most of the image border (the background).
    pub skip_background: bool,
}

impl Default for AutoPunch {
    fn default() -> Self {
        AutoPunch {
            colors: 8,
            resolution: 400,
            min_area_mm2: 1.0,
            smooth: 1,
            trace: TraceOptions::default(),
            satin_max_mm: 6.0,
            skip_background: true,
        }
    }
}

/// Most regions one conversion may produce.
pub const MAX_REGIONS: usize = 5000;

#[derive(Clone, Debug, PartialEq)]
pub struct Converted {
    /// Shape objects in sewing order (ids unassigned).
    pub objects: Vec<DesignObject>,
    /// Threads in sewing order.
    pub threads: Vec<Thread>,
    /// The background thread left out, if any.
    pub background: Option<Thread>,
}

/// Labels per working pixel: thread index into `palette.threads`, or [`NONE`].
pub fn label(work: &Raster, palette: &Palette) -> Vec<u16> {
    let (w, h) = (work.width(), work.height());
    let mut labels = Vec::with_capacity(w as usize * h as usize);
    for y in 0..h {
        for x in 0..w {
            let [r, g, b, a] = work.pixel(x, y);
            labels.push(if a >= 128 {
                palette.classify([r, g, b]) as u16
            } else {
                NONE
            });
        }
    }
    labels
}

/// The label covering more than half of the border pixels, if any.
pub fn border_majority(labels: &[u16], w: usize, h: usize) -> Option<u16> {
    let mut counts: Vec<(u16, usize)> = Vec::new();
    let mut total = 0;
    for y in 0..h {
        for x in 0..w {
            if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
                total += 1;
                let l = labels[y * w + x];
                match counts.iter_mut().find(|c| c.0 == l) {
                    Some(c) => c.1 += 1,
                    None => counts.push((l, 1)),
                }
            }
        }
    }
    counts
        .into_iter()
        .filter(|c| c.0 != NONE && c.1 * 2 > total)
        .map(|c| c.0)
        .next()
}

pub fn convert(image: &Raster, frame: &Frame, p: &AutoPunch) -> Result<Converted, DigitizeError> {
    let work = image.fit(p.resolution.clamp(16, 2048));
    let frame = frame.rescaled(image.width(), work.width());
    let (w, h) = (work.width() as usize, work.height() as usize);
    let mut opaque = Vec::new();
    for y in 0..work.height() {
        for x in 0..work.width() {
            let [r, g, b, a] = work.pixel(x, y);
            if a >= 128 {
                opaque.push([r, g, b]);
            }
        }
    }
    if opaque.is_empty() {
        return Err(DigitizeError::Empty(
            "the image is fully transparent".into(),
        ));
    }
    let palette = Palette::build(&opaque, p.colors.clamp(1, 64));
    let mut labels = label(&work, &palette);
    let background = if p.skip_background {
        border_majority(&labels, w, h)
    } else {
        None
    };
    if let Some(bg) = background {
        for l in &mut labels {
            if *l == bg {
                *l = NONE;
            }
        }
    }
    regions::majority(&mut labels, w, h, p.smooth.min(5));
    let px_area = frame.mm_per_px() * frame.mm_per_px();
    let min_area = if px_area > 0.0 {
        (p.min_area_mm2.max(0.0) / px_area).ceil().min(1e9) as usize
    } else {
        0
    };
    regions::merge_small(&mut labels, w, h, min_area);
    let (id, comps) = regions::components(&labels, w, h);
    if comps.is_empty() {
        return Err(DigitizeError::Empty(
            "no regions left after removing the background".into(),
        ));
    }
    if comps.len() > MAX_REGIONS {
        return Err(DigitizeError::TooLarge(format!(
            "{} regions (limit {MAX_REGIONS}); raise the minimum area or lower the resolution",
            comps.len()
        )));
    }
    // Colours by total area, largest first (backgrounds sew first); ties to the lower thread.
    let mut area = vec![0usize; palette.threads.len()];
    for c in &comps {
        area[c.label as usize] += c.area;
    }
    let mut order: Vec<usize> = (0..palette.threads.len())
        .filter(|&t| area[t] > 0)
        .collect();
    order.sort_by_key(|&t| (std::cmp::Reverse(area[t]), t));
    let mut objects = Vec::new();
    for &t in &order {
        let thread = palette.threads[t].clone();
        for (c, k) in comps
            .iter()
            .enumerate()
            .filter(|(_, k)| k.label as usize == t)
        {
            let (m, mw, mh) = regions::mask(&id, w, c as u32, k);
            let contours = regions::trace_mask(&m, mw, mh, k.x0, k.y0, &p.trace, &frame);
            if contours.is_empty() {
                continue;
            }
            let axis = regions::principal_axis(&m, mw, mh);
            let across = normalize(axis + 90.0);
            let longest = scan_rows(&contours, across, 2.0, &[])
                .iter()
                .flat_map(|r| r.spans.iter().map(|s| s.end() - s.start()))
                .fold(0.0, f64::max);
            let satin = longest <= p.satin_max_mm * 10.0;
            let mut region = RegionSewing::new(
                if satin {
                    RegionType::Satin
                } else {
                    RegionType::Fill
                },
                thread.clone(),
            );
            region.angle = if satin { across } else { normalize(axis) };
            let mut o = DesignObject::shape(contours, Some(region), None);
            o.name = Some(format!(
                "Auto Punch {} {}",
                objects.len() + 1,
                thread.name.as_deref().unwrap_or("thread")
            ));
            objects.push(o);
        }
    }
    if objects.is_empty() {
        return Err(DigitizeError::Empty(
            "every region was too small to trace".into(),
        ));
    }
    Ok(Converted {
        objects,
        threads: order.iter().map(|&t| palette.threads[t].clone()).collect(),
        background: background.map(|b| palette.threads[b as usize].clone()),
    })
}

/// Angle in (-90, 90].
fn normalize(deg: f64) -> f64 {
    let mut d = deg % 180.0;
    if d <= -90.0 {
        d += 180.0;
    }
    if d > 90.0 {
        d -= 180.0;
    }
    d
}
