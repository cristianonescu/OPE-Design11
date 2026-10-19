//! Design Center approximation, in PE-DESIGN's four stages:
//!
//! 1. **Original image → line image**: pixels darker than a threshold are line pixels.
//! 2. **Line image → figure lines**: the line pixels are thinned to a one-pixel skeleton
//!    (Zhang–Suen) and traced into polylines between end points and junctions (closed loops
//!    where there are none), short spurs pruned, simplified and optionally fitted with
//!    Béziers.
//! 3. **Figure regions**: areas enclosed by lines (not touching the image border) become
//!    regions, coloured by their mean image colour mapped to the PEC chart.
//! 4. **Sew settings**: lines get a line sew type and thread, regions a region sew type.
//!
//! Specified from PE-DESIGN 11's public guide (Design Center); the algorithms are this
//! project's own.

use crate::DigitizeError;
use crate::palette::nearest_pec;
use crate::raster::{Frame, Raster};
use crate::regions::{self, NONE, TraceOptions};
use crate::trace;
use openstitch_core::Thread;
use openstitch_design::{
    Contour, DesignObject, LineSewing, LineType, RegionSewing, RegionType, Vec2, v2,
};

#[derive(Clone, Debug, PartialEq)]
pub struct LineImageOptions {
    /// Longest side of the working raster, pixels.
    pub resolution: u32,
    /// Pixels with luminance (over white) below this are lines, 0–255.
    pub threshold: f64,
}

impl Default for LineImageOptions {
    fn default() -> Self {
        LineImageOptions {
            resolution: 600,
            threshold: 128.0,
        }
    }
}

/// Stage 1 result: the working raster and its line mask.
#[derive(Clone, Debug, PartialEq)]
pub struct LineImage {
    pub image: Raster,
    pub mask: Vec<bool>,
    pub width: usize,
    pub height: usize,
}

impl LineImage {
    pub fn line_pixels(&self) -> usize {
        self.mask.iter().filter(|&&b| b).count()
    }

    /// The line image as black-on-white RGBA, for previews.
    pub fn to_raster(&self) -> Raster {
        let w = self.width;
        Raster::from_fn(self.width as u32, self.height as u32, |x, y| {
            if self.mask[y as usize * w + x as usize] {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        })
        .unwrap_or_else(|_| self.image.clone())
    }
}

/// Stage 1.
pub fn line_image(image: &Raster, o: &LineImageOptions) -> LineImage {
    let work = image.fit(o.resolution.clamp(16, 2048));
    let (w, h) = (work.width() as usize, work.height() as usize);
    let mut mask = Vec::with_capacity(w * h);
    for y in 0..work.height() {
        for x in 0..work.width() {
            mask.push(work.luma_over_white(x, y) < o.threshold);
        }
    }
    LineImage {
        image: work,
        mask,
        width: w,
        height: h,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FigureOptions {
    /// Open lines shorter than this are dropped (spurs, specks), mm.
    pub min_line_mm: f64,
    /// Enclosed areas smaller than this are not regions, mm².
    pub min_region_mm2: f64,
    pub trace: TraceOptions,
}

impl Default for FigureOptions {
    fn default() -> Self {
        FigureOptions {
            min_line_mm: 1.0,
            min_region_mm2: 4.0,
            trace: TraceOptions {
                simplify: 1.0,
                ..TraceOptions::default()
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FigureRegion {
    pub contours: Vec<Contour>,
    /// Mean image colour of the region.
    pub color: [u8; 3],
    /// Nearest PEC thread.
    pub thread: Thread,
    pub area_px: usize,
}

/// Stages 2–3 result, in design units (0.1 mm).
#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    /// Open polylines and closed loops.
    pub lines: Vec<Contour>,
    pub regions: Vec<FigureRegion>,
    /// Median stroke width of the line image, mm.
    pub line_width_mm: f64,
}

/// Most lines plus regions a figure may hold.
pub const MAX_FIGURE_PARTS: usize = 20_000;

/// Zhang–Suen thinning of `mask` to an 8-connected one-pixel skeleton.
pub fn thin(mask: &[bool], w: usize, h: usize) -> Vec<bool> {
    let (pw, ph) = (w + 2, h + 2);
    let mut m = vec![false; pw * ph];
    for y in 0..h {
        for x in 0..w {
            m[(y + 1) * pw + x + 1] = mask[y * w + x];
        }
    }
    let mut remove = Vec::new();
    for _ in 0..(w.max(h) + 2) {
        let mut changed = false;
        for step in 0..2 {
            remove.clear();
            for y in 1..ph - 1 {
                for x in 1..pw - 1 {
                    let i = y * pw + x;
                    if !m[i] {
                        continue;
                    }
                    // P2..P9 clockwise from north.
                    let p = [
                        m[i - pw],
                        m[i - pw + 1],
                        m[i + 1],
                        m[i + pw + 1],
                        m[i + pw],
                        m[i + pw - 1],
                        m[i - 1],
                        m[i - pw - 1],
                    ];
                    let b = p.iter().filter(|&&v| v).count();
                    let a = (0..8).filter(|&k| !p[k] && p[(k + 1) % 8]).count();
                    let (p2, p4, p6, p8) = (p[0], p[2], p[4], p[6]);
                    let ok = if step == 0 {
                        !(p2 && p4 && p6) && !(p4 && p6 && p8)
                    } else {
                        !(p2 && p4 && p8) && !(p2 && p6 && p8)
                    };
                    if (2..=6).contains(&b) && a == 1 && ok {
                        remove.push(i);
                    }
                }
            }
            for &i in &remove {
                m[i] = false;
            }
            changed |= !remove.is_empty();
        }
        if !changed {
            break;
        }
    }
    let mut out = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = m[(y + 1) * pw + x + 1];
        }
    }
    out
}

const DIRS: [(isize, isize); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Skeleton polylines in pixel-centre coordinates: `(points, closed)`. A pixel's branches
/// are the contiguous groups of set pixels around it (crossing number), so diagonal steps do
/// not look like junctions. Paths run between nodes (end points: one branch; junctions: three
/// or more), leaving each pixel through the branch it did not enter from; what is left are
/// closed loops. Paths directly between two neighbouring nodes (inside junction clusters) are
/// dropped.
pub fn skeleton_paths(s: &[bool], w: usize, h: usize) -> Vec<(Vec<Vec2>, bool)> {
    let at = |i: usize, k: usize| -> Option<usize> {
        let (dx, dy) = DIRS[k];
        let (nx, ny) = ((i % w) as isize + dx, (i / w) as isize + dy);
        (nx >= 0 && ny >= 0 && nx < w as isize && ny < h as isize)
            .then(|| ny as usize * w + nx as usize)
            .filter(|&j| s[j])
    };
    // Branches: groups of neighbouring set pixels, cyclically contiguous; each group lists
    // its pixels with the 4-neighbours (even directions) first.
    let branches = |i: usize| -> Vec<Vec<usize>> {
        let set: Vec<Option<usize>> = (0..8).map(|k| at(i, k)).collect();
        let Some(gap) = (0..8).find(|&k| set[k].is_none()) else {
            return vec![set.iter().flatten().copied().collect()];
        };
        let mut groups: Vec<Vec<(usize, usize)>> = Vec::new();
        let mut open = false;
        for step in 1..=8 {
            let k = (gap + step) % 8;
            match set[k] {
                Some(j) => {
                    if !open {
                        groups.push(Vec::new());
                        open = true;
                    }
                    if let Some(g) = groups.last_mut() {
                        g.push((k, j));
                    }
                }
                None => open = false,
            }
        }
        groups
            .into_iter()
            .map(|mut g| {
                g.sort_by_key(|&(k, _)| (k % 2, k));
                g.into_iter().map(|(_, j)| j).collect()
            })
            .collect()
    };
    let centre = |i: usize| v2((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
    let branch_count: Vec<usize> = (0..s.len())
        .map(|i| if s[i] { branches(i).len() } else { 0 })
        .collect();
    let is_node = |i: usize| s[i] && branch_count[i] != 2;
    let mut visited = vec![false; s.len()];
    // Leaves `cur` through the branch that does not hold `prev`.
    let step = |cur: usize, prev: usize, visited: &[bool]| -> Option<usize> {
        branches(cur)
            .into_iter()
            .filter(|g| !g.contains(&prev))
            .flatten()
            .find(|&j| is_node(j) || !visited[j])
    };
    let mut paths = Vec::new();
    for n in (0..s.len()).filter(|&i| is_node(i)) {
        for g in branches(n) {
            let Some(&q) = g.iter().find(|&&j| is_node(j) || !visited[j]) else {
                continue;
            };
            if is_node(q) {
                continue;
            }
            let mut pts = vec![centre(n)];
            let (mut prev, mut cur) = (n, q);
            loop {
                pts.push(centre(cur));
                if is_node(cur) {
                    break;
                }
                visited[cur] = true;
                let Some(next) = step(cur, prev, &visited) else {
                    break;
                };
                (prev, cur) = (cur, next);
            }
            paths.push((pts, false));
        }
    }
    // Loops: every pixel left has two branches.
    for start in 0..s.len() {
        if !s[start] || visited[start] || is_node(start) {
            continue;
        }
        let mut pts = Vec::new();
        let first = branches(start);
        let mut prev = first
            .first()
            .and_then(|g| g.first().copied())
            .unwrap_or(start);
        let mut cur = start;
        loop {
            visited[cur] = true;
            pts.push(centre(cur));
            match step(cur, prev, &visited) {
                Some(next) if !visited[next] => (prev, cur) = (cur, next),
                _ => break,
            }
        }
        if pts.len() >= 3 {
            paths.push((pts, true));
        }
    }
    paths
}

fn length(pts: &[Vec2]) -> f64 {
    pts.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// Stages 2–3. `frame` places the original image; the line image is its resampled copy.
pub fn figure(
    original: &Raster,
    li: &LineImage,
    frame: &Frame,
    o: &FigureOptions,
) -> Result<Figure, DigitizeError> {
    let frame = frame.rescaled(original.width(), li.width as u32);
    let (w, h) = (li.width, li.height);
    let mm = frame.mm_per_px();
    let skeleton = thin(&li.mask, w, h);
    let min_len = if mm > 0.0 {
        o.min_line_mm.max(0.0) / mm
    } else {
        0.0
    };
    let mut lines = Vec::new();
    for (pts, closed) in skeleton_paths(&skeleton, w, h) {
        if !closed && length(&pts) < min_len {
            continue;
        }
        let kept = if closed {
            trace::simplify_closed(&pts, o.trace.simplify)
        } else {
            trace::simplify_open(&pts, o.trace.simplify)
        };
        if kept.len() < 2 {
            continue;
        }
        lines.push(trace::contour(
            &pts,
            &kept,
            closed,
            o.trace.curves,
            o.trace.simplify,
            o.trace.corner_degrees,
            &frame,
        ));
        if lines.len() > MAX_FIGURE_PARTS {
            return Err(DigitizeError::TooLarge(format!(
                "more than {MAX_FIGURE_PARTS} lines; raise the threshold or lower the resolution"
            )));
        }
    }
    // Median stroke width at the skeleton.
    let dist = regions::distance_map(&li.mask, w, h);
    let mut at: Vec<u32> = (0..w * h)
        .filter(|&i| skeleton[i])
        .map(|i| dist[i])
        .collect();
    at.sort_unstable();
    let line_width_mm = at
        .get(at.len() / 2)
        .map_or(0.0, |&d| regions::width_for(d) * mm);
    // Regions: areas between the lines that do not reach the border.
    let labels: Vec<u16> = li.mask.iter().map(|&l| if l { NONE } else { 0 }).collect();
    let (id, comps) = regions::components(&labels, w, h);
    let min_area = if mm > 0.0 {
        (o.min_region_mm2.max(0.0) / (mm * mm)).ceil().min(1e9) as usize
    } else {
        0
    };
    let mut found = Vec::new();
    for (c, k) in comps.iter().enumerate() {
        if k.x0 == 0 || k.y0 == 0 || k.x1 == w || k.y1 == h || k.area < min_area.max(1) {
            continue;
        }
        let (m, mw, mh) = regions::mask(&id, w, c as u32, k);
        let contours = regions::trace_mask(&m, mw, mh, k.x0, k.y0, &o.trace, &frame);
        if contours.is_empty() {
            continue;
        }
        let mut sum = [0u64; 3];
        for y in 0..mh {
            for x in 0..mw {
                if m[y * mw + x] {
                    let p = li.image.pixel((x + k.x0) as u32, (y + k.y0) as u32);
                    for ch in 0..3 {
                        sum[ch] += u64::from(p[ch]);
                    }
                }
            }
        }
        let n = k.area as u64;
        let color = sum.map(|v| ((v + n / 2) / n) as u8);
        found.push(FigureRegion {
            contours,
            color,
            thread: nearest_pec(color).1,
            area_px: k.area,
        });
        if lines.len() + found.len() > MAX_FIGURE_PARTS {
            return Err(DigitizeError::TooLarge(format!(
                "more than {MAX_FIGURE_PARTS} figure parts; raise the minimum region area"
            )));
        }
    }
    if lines.is_empty() && found.is_empty() {
        return Err(DigitizeError::Empty(
            "no lines found; adjust the threshold".into(),
        ));
    }
    // Largest regions first.
    found.sort_by_key(|r| std::cmp::Reverse(r.area_px));
    Ok(Figure {
        lines,
        regions: found,
        line_width_mm,
    })
}

/// How regions are sewn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionSew {
    None,
    Fill,
    Satin,
    /// Fill every region whose colour is not near white (the paper).
    Auto,
}

/// Stage 4 settings.
#[derive(Clone, Debug, PartialEq)]
pub struct SewSettings {
    /// `None` leaves lines unsewn.
    pub line: Option<LineType>,
    pub line_thread: Thread,
    /// Zigzag width, mm; `None` uses the measured stroke width (1–6 mm).
    pub line_width_mm: Option<f64>,
    pub region: RegionSew,
    /// `Some` sews every region in this thread instead of its image colour.
    pub region_thread: Option<Thread>,
}

impl Default for SewSettings {
    fn default() -> Self {
        SewSettings {
            line: Some(LineType::Running),
            line_thread: Thread::rgb(0),
            line_width_mm: None,
            region: RegionSew::Auto,
            region_thread: None,
        }
    }
}

/// Stage 4: region objects (largest first) then one line object holding every line.
pub fn sew(f: &Figure, s: &SewSettings) -> Vec<DesignObject> {
    let mut out = Vec::new();
    for (i, r) in f.regions.iter().enumerate() {
        let luma = 0.299 * f64::from(r.color[0])
            + 0.587 * f64::from(r.color[1])
            + 0.114 * f64::from(r.color[2]);
        let kind = match s.region {
            RegionSew::None => continue,
            RegionSew::Fill => RegionType::Fill,
            RegionSew::Satin => RegionType::Satin,
            RegionSew::Auto if luma >= 230.0 => continue,
            RegionSew::Auto => RegionType::Fill,
        };
        let thread = s.region_thread.clone().unwrap_or_else(|| r.thread.clone());
        let mut o = DesignObject::shape(
            r.contours.clone(),
            Some(RegionSewing::new(kind, thread)),
            None,
        );
        o.name = Some(format!("Design Center region {}", i + 1));
        out.push(o);
    }
    if let (Some(kind), false) = (s.line, f.lines.is_empty()) {
        let mut line = LineSewing::new(kind, s.line_thread.clone());
        line.width = s.line_width_mm.unwrap_or(f.line_width_mm).clamp(1.0, 6.0);
        let mut o = DesignObject::shape(f.lines.clone(), None, Some(line));
        o.name = Some("Design Center lines".into());
        out.push(o);
    }
    out
}
