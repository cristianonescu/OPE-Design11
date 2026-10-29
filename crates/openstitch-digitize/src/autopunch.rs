//! Auto Punch approximation: image → 3×3 modal colour census (at most 100 tracked colours)
//! → N PEC threads → regions (4-connected; those of at most the small-region floor removed)
//! → traced outlines → shape objects, satin where the region is a stroke no wider than the
//! satin width, fill elsewhere.
//!
//! Specified from PE-DESIGN 11's public guide (Auto Punch converts an image's colour areas
//! into embroidery regions with chosen colours omitted) and the Phase 6 gap analysis (#127:
//! the modal census, the `max(4, round(param · 100 / mmPerPx²))` small-region floor at
//! 4-connectivity and a centerline-thinness satin test); the algorithm is this project's own.

use crate::DigitizeError;
use crate::palette::{self, Census, Palette};
use crate::raster::{Frame, Raster};
use crate::regions::{self, NONE, TraceOptions};
use openstitch_core::Thread;
use openstitch_design::{DesignObject, RegionSewing, RegionType};

#[derive(Clone, Debug, PartialEq)]
pub struct AutoPunch {
    /// Thread colours to reduce to (1–64).
    pub colors: usize,
    /// Longest side of the working raster, pixels.
    pub resolution: u32,
    /// Small-region parameter, mm²: a 4-connected region of at most
    /// `max(4, round(min_area_mm2 / mmPerPx²))` working pixels is recoloured into a neighbour
    /// (#127's `max(4, round(param · 100 / mmPerPx²))` with `param` in cm²; see
    /// [`small_region_floor`]).
    pub min_area_mm2: f64,
    /// Extra 3×3 majority-filter passes on the thread labels after the modal colour census
    /// (0: the census's own 3×3 modal filter is the only smoothing).
    pub smooth: u32,
    pub trace: TraceOptions,
    /// Regions whose stroke width (twice the deepest inscribed distance) is at most this are
    /// satin across the stroke, mm; wider regions are fill along their long axis.
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
            smooth: 0,
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

/// Labels per working pixel from the census's modal colours: thread index into
/// `palette.threads`, or [`NONE`] where the pixel is transparent.
pub fn label(census: &Census, palette: &Palette) -> Vec<u16> {
    census
        .modal
        .iter()
        .map(|m| m.map_or(NONE, |c| palette.classify(c) as u16))
        .collect()
}

/// The small-region floor in pixels (#127): `max(4, round(param · 100 / mmPerPx²))` with the
/// parameter given in mm² (`param` in cm², so the `· 100` is the unit change) and `mm_per_px`
/// the working raster's pixel size, `max(width_mm / width_px, height_mm / height_px)`.
/// Regions of at most this many 4-connected pixels are removed.
pub fn small_region_floor(min_area_mm2: f64, mm_per_px: f64) -> usize {
    if mm_per_px <= 0.0 || !min_area_mm2.is_finite() {
        return 4;
    }
    let px = (min_area_mm2.max(0.0) / (mm_per_px * mm_per_px)).round();
    (px.min(1e9) as usize).max(4)
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
    let census = palette::census(&work);
    if census.colors.is_empty() {
        return Err(DigitizeError::Empty(
            "the image is fully transparent".into(),
        ));
    }
    let palette = Palette::from_census(&census, p.colors.clamp(1, 64));
    let mut labels = label(&census, &palette);
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
    // The frame scales x and y alike, so the working pixel's mm/px is both ratios.
    let floor = small_region_floor(p.min_area_mm2, frame.mm_per_px());
    regions::merge_small(&mut labels, w, h, floor + 1);
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
            // Centerline test: a region is a stroke (satin across it) when its width at its
            // deepest point is within the satin width; one that keeps area beyond it is fill.
            let width_mm = regions::thickness(&m, mw, mh) * frame.mm_per_px();
            let satin = width_mm <= p.satin_max_mm;
            let mut region = RegionSewing::new(
                if satin {
                    RegionType::Satin
                } else {
                    RegionType::Fill
                },
                thread.clone(),
            );
            // The axis is measured with the design's y pointing down; region angles are stored
            // counter-clockwise on screen, as PE-DESIGN gives them.
            region.angle = normalize(-if satin { across } else { axis }) + 0.0;
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
