//! Outlines of stitch coverage: the area sewn stitches cover, traced into contours. Used to
//! convert a stitch pattern back into shapes (PE-DESIGN's "Convert to Outline" and "Convert
//! to Blocks"; the algorithm is this project's own).
//!
//! Sewn segments are drawn as thread-wide strokes on a grid, gaps narrower than the closing
//! distance (the spacing between fill rows) are closed (dilate, then erode, by the chamfer
//! distance), and each 4-connected covered area is traced with marching squares. Each area
//! also reports the direction its stitches run (length-weighted doubled-angle mean), its
//! median stitch length and its thickness, so a caller can choose satin or fill.

use crate::raster::Frame;
use crate::regions::{self, NONE, TraceOptions};
use openstitch_design::{Contour, Rect, Vec2, v2};

#[derive(Clone, Debug, PartialEq)]
pub struct CoverageOptions {
    /// Grid cell, design units (0.1 mm).
    pub cell: f64,
    /// Drawn thread width, design units.
    pub thread_width: f64,
    /// Gaps up to about twice this are closed, design units.
    pub close: f64,
    /// Areas smaller than this are dropped, design units squared.
    pub min_area: f64,
    pub trace: TraceOptions,
}

impl Default for CoverageOptions {
    fn default() -> Self {
        CoverageOptions {
            cell: 2.0,
            thread_width: 4.0,
            close: 5.0,
            min_area: 100.0,
            trace: TraceOptions {
                simplify: 0.75,
                ..TraceOptions::default()
            },
        }
    }
}

/// Most grid cells one trace may use; coarser cells are used for larger patterns.
pub const MAX_CELLS: usize = 4 << 20;
/// Most stitch segments one trace may draw.
pub const MAX_SEGMENTS: usize = 2_000_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    /// Outer boundary and holes (even-odd), design units.
    pub contours: Vec<Contour>,
    /// Covered area, design units squared.
    pub area: f64,
    /// Main stitch direction, degrees in (-90, 90].
    pub angle: f64,
    /// Median length of the stitches inside, design units.
    pub stitch_length: f64,
    /// Thickness (largest inscribed width), design units.
    pub thickness: f64,
}

/// Traces the area covered by `runs` (each a polyline of needle points sewn one after the
/// other). Returns the areas in scan order of their top-left pixel; empty when nothing is
/// sewn.
pub fn trace(runs: &[Vec<Vec2>], o: &CoverageOptions) -> Vec<Area> {
    let segments: Vec<(Vec2, Vec2)> = runs
        .iter()
        .flat_map(|r| r.windows(2).map(|w| (w[0], w[1])))
        .filter(|(a, b)| a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite())
        .take(MAX_SEGMENTS)
        .collect();
    let points = segments.iter().flat_map(|(a, b)| [*a, *b]);
    let Some(bounds) = Rect::enclosing(points) else {
        return Vec::new();
    };
    let half = o.thread_width.max(0.0) / 2.0;
    let margin = half + o.close.max(0.0) * 2.0;
    let (w0, h0) = (
        bounds.width() + 2.0 * margin,
        bounds.height() + 2.0 * margin,
    );
    let mut cell = o.cell.max(0.5);
    while (w0 / cell).ceil() * (h0 / cell).ceil() > MAX_CELLS as f64 {
        cell *= 1.5;
    }
    let (w, h) = (
        (w0 / cell).ceil() as usize + 3,
        (h0 / cell).ceil() as usize + 3,
    );
    let origin = v2(bounds.min_x - margin - cell, bounds.min_y - margin - cell);
    let to_px = |p: Vec2| ((p.x - origin.x) / cell, (p.y - origin.y) / cell);
    let mut mask = vec![false; w * h];
    // At least 0.75 px, so diagonal strokes stay 4-connected.
    let r = (half / cell).max(0.75);
    for (a, b) in &segments {
        let (ax, ay) = to_px(*a);
        let (bx, by) = to_px(*b);
        let steps = ((bx - ax).hypot(by - ay) * 2.0).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            let (cx, cy) = (ax + (bx - ax) * t, ay + (by - ay) * t);
            let (x0, x1) = (
                (cx - r).floor().max(0.0) as usize,
                ((cx + r).ceil() as usize).min(w - 1),
            );
            let (y0, y1) = (
                (cy - r).floor().max(0.0) as usize,
                ((cy + r).ceil() as usize).min(h - 1),
            );
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                    if dx * dx + dy * dy <= r * r + 0.25 {
                        mask[y * w + x] = true;
                    }
                }
            }
        }
    }
    // Closing: dilate then erode by the closing radius (chamfer units: 3 per pixel).
    let k = (o.close.max(0.0) / cell * 3.0).round() as u32;
    if k > 0 {
        let outside: Vec<bool> = mask.iter().map(|&m| !m).collect();
        let grow = regions::distance_map(&outside, w, h);
        let dilated: Vec<bool> = (0..w * h).map(|i| mask[i] || grow[i] <= k).collect();
        let shrink = regions::distance_map(&dilated, w, h);
        mask = (0..w * h).map(|i| shrink[i] > k).collect();
    }
    let labels: Vec<u16> = mask.iter().map(|&m| if m { 0 } else { NONE }).collect();
    let (id, comps) = regions::components(&labels, w, h);
    let frame = Frame {
        origin,
        scale: cell,
    };
    let min_px = (o.min_area.max(0.0) / (cell * cell)).ceil() as usize;
    let mut out = Vec::new();
    for (c, k) in comps.iter().enumerate() {
        if k.area < min_px.max(1) {
            continue;
        }
        let (m, mw, mh) = regions::mask(&id, w, c as u32, k);
        let contours = regions::trace_mask(&m, mw, mh, k.x0, k.y0, &o.trace, &frame);
        if contours.is_empty() {
            continue;
        }
        // Stitches whose midpoint lies in this area.
        let (mut sc, mut ss) = (0.0, 0.0);
        let mut lengths = Vec::new();
        for (a, b) in &segments {
            let (mx, my) = to_px((*a + *b) * 0.5);
            let (px, py) = (mx.floor() as isize, my.floor() as isize);
            if px < 0 || py < 0 || px as usize >= w || py as usize >= h {
                continue;
            }
            if id[py as usize * w + px as usize] != c as u32 {
                continue;
            }
            let d = *b - *a;
            let len = d.length();
            if len <= 0.0 {
                continue;
            }
            let theta = d.y.atan2(d.x);
            sc += len * (2.0 * theta).cos();
            ss += len * (2.0 * theta).sin();
            lengths.push(len);
        }
        lengths.sort_by(f64::total_cmp);
        let mut angle = (0.5 * ss.atan2(sc)).to_degrees().round();
        if angle <= -90.0 {
            angle += 180.0;
        }
        out.push(Area {
            contours,
            area: k.area as f64 * cell * cell,
            angle: if angle == -0.0 { 0.0 } else { angle },
            stitch_length: lengths.get(lengths.len() / 2).copied().unwrap_or(0.0),
            thickness: regions::thickness(&m, mw, mh) * cell,
        });
    }
    out
}
