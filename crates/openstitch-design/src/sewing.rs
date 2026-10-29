//! Stitch Design Factory: sewing-stitch patterns for ordinary sewing machines (not embroidery),
//! in OpenStitch's own documented format `openstitch.sewing`.
//!
//! A sewing stitch is a short repeating needle sequence the machine sews while the fabric
//! feeds forward. Coordinates are mm: x runs along the feed (0 … length), y across it
//! (−width/2 … +width/2, +y towards the operator's right in the editor). Four kinds, after the
//! PE-DESIGN guide's "Sewing Stitch Patterns for Stitch Design Factory":
//!
//! - `decorative`: needle points placed one by one.
//! - `satin`: a column between a left and a right edge (a width profile along x), filled with
//!   zigzag at `density` stitches per mm.
//! - `decorativeSatin`: a closed outline filled with zigzag across the feed.
//! - `cross`: cells of a grid (`cellSize`, `rows` across) each sewn as an X.
//!
//! Brother's own sewing-machine formats (`.pmv`, `.pmp`, `.psf`) are undocumented. [`pmv`]
//! writes PMV the way the independent pyembroidery project describes it (experimental: its
//! lookup tables are not understood and no machine has accepted a file yet); PMP and PSF are
//! Blocked.

use crate::geometry::{Vec2, v2};
use serde::{Deserialize, Serialize};

/// Widest pattern accepted (mm); most machines sew 7 mm, some 9 mm.
pub const MAX_WIDTH: f64 = 9.0;
/// Longest pattern (mm).
pub const MAX_LENGTH: f64 = 200.0;
/// Most needle points a pattern may generate.
pub const MAX_NEEDLES: usize = 2000;
/// Longest move along the feed between two needle points (mm).
pub const MAX_FEED: f64 = 7.0;
/// Largest document accepted (bytes).
pub const MAX_DOCUMENT: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SewingKind {
    Decorative,
    Satin,
    DecorativeSatin,
    Cross,
}

impl SewingKind {
    pub const ALL: [SewingKind; 4] = [
        SewingKind::Decorative,
        SewingKind::Satin,
        SewingKind::DecorativeSatin,
        SewingKind::Cross,
    ];

    pub fn id(self) -> &'static str {
        match self {
            SewingKind::Decorative => "decorative",
            SewingKind::Satin => "satin",
            SewingKind::DecorativeSatin => "decorativeSatin",
            SewingKind::Cross => "cross",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SewingKind::Decorative => "Decorative stitch",
            SewingKind::Satin => "Satin stitch",
            SewingKind::DecorativeSatin => "Decorative satin stitch",
            SewingKind::Cross => "Cross stitch",
        }
    }
}

/// One sewing-stitch pattern. Serializes as an `openstitch.sewing` document and validates
/// when deserialized.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SewingFile", into = "SewingFile")]
pub struct SewingStitch {
    pub kind: SewingKind,
    pub id: String,
    pub name: String,
    /// Maximum width the pattern may use (mm, 1–9).
    pub width: f64,
    /// Decorative: needle points in sewing order.
    pub points: Vec<Vec2>,
    /// Satin: stations (x, left edge y, right edge y), x increasing.
    pub profile: Vec<[f64; 3]>,
    /// Decorative satin: closed outline.
    pub outline: Vec<Vec2>,
    /// Satin kinds: zigzag stitches per mm along the feed.
    pub density: f64,
    /// Cross: cell size (mm), rows across, and the cells (column, row) to sew.
    pub cell_size: f64,
    pub rows: u32,
    pub cells: Vec<[u32; 2]>,
    /// Cross: sew every diagonal three times.
    pub triple: bool,
    /// Start and end swapped: the needle points are sewn in reverse order.
    pub swapped: bool,
}

/// JSON shape of an `openstitch.sewing` document.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SewingFile {
    format: String,
    version: u32,
    kind: SewingKind,
    id: String,
    name: String,
    width: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    points: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    profile: Vec<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    outline: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    density: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cell_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rows: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cells: Vec<[u32; 2]>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    triple: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    swapped: bool,
    /// Generated needle points (mm), written on export for tools that only want to sew;
    /// ignored when read (they are always regenerated).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    needles: Vec<[f64; 2]>,
}

impl From<SewingStitch> for SewingFile {
    fn from(s: SewingStitch) -> Self {
        let satin = matches!(s.kind, SewingKind::Satin | SewingKind::DecorativeSatin);
        let cross = s.kind == SewingKind::Cross;
        SewingFile {
            format: FORMAT.into(),
            version: 1,
            kind: s.kind,
            id: s.id,
            name: s.name,
            width: s.width,
            points: s.points.iter().map(|p| [p.x, p.y]).collect(),
            profile: s.profile,
            outline: s.outline.iter().map(|p| [p.x, p.y]).collect(),
            density: satin.then_some(s.density),
            cell_size: cross.then_some(s.cell_size),
            rows: cross.then_some(s.rows),
            cells: s.cells,
            triple: s.triple,
            swapped: s.swapped,
            needles: Vec::new(),
        }
    }
}

impl TryFrom<SewingFile> for SewingStitch {
    type Error = String;
    fn try_from(f: SewingFile) -> Result<Self, String> {
        if f.needles.len() > MAX_NEEDLES {
            return Err("too many needle points".into());
        }
        if f.format != FORMAT || f.version != 1 {
            return Err("not an openstitch.sewing version 1 document".into());
        }
        let s = SewingStitch {
            kind: f.kind,
            id: f.id,
            name: f.name,
            width: f.width,
            points: f.points.iter().map(|&[x, y]| v2(x, y)).collect(),
            profile: f.profile,
            outline: f.outline.iter().map(|&[x, y]| v2(x, y)).collect(),
            density: f.density.unwrap_or(4.0),
            cell_size: f.cell_size.unwrap_or(2.0),
            rows: f.rows.unwrap_or(3),
            cells: f.cells,
            triple: f.triple,
            swapped: f.swapped,
        };
        s.validate()?;
        Ok(s)
    }
}

pub const FORMAT: &str = "openstitch.sewing";

/// Parses and validates one `openstitch.sewing` document.
pub fn parse(json: &str) -> Result<SewingStitch, String> {
    if json.len() > MAX_DOCUMENT {
        return Err("document larger than 1 MiB".into());
    }
    let f: SewingFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    SewingStitch::try_from(f)
}

fn finite(v: f64) -> bool {
    v.is_finite()
}

impl SewingStitch {
    /// A small valid pattern of `kind` to start editing from.
    pub fn new(kind: SewingKind) -> Self {
        let mut s = SewingStitch {
            kind,
            id: format!("my-{}", kind.id().to_lowercase()),
            name: kind.label().into(),
            width: 7.0,
            points: Vec::new(),
            profile: Vec::new(),
            outline: Vec::new(),
            density: 4.0,
            cell_size: 2.0,
            rows: 3,
            cells: Vec::new(),
            triple: false,
            swapped: false,
        };
        match kind {
            SewingKind::Decorative => {
                s.points = vec![
                    v2(0.0, 0.0),
                    v2(2.0, -3.0),
                    v2(4.0, 0.0),
                    v2(6.0, 3.0),
                    v2(8.0, 0.0),
                ];
            }
            SewingKind::Satin => {
                s.profile = vec![[0.0, -0.5, 0.5], [4.0, -3.5, 3.5], [8.0, -0.5, 0.5]];
            }
            SewingKind::DecorativeSatin => {
                s.outline = vec![v2(0.0, 0.0), v2(3.0, -3.0), v2(6.0, 0.0), v2(3.0, 3.0)];
            }
            SewingKind::Cross => {
                s.cells = vec![[0, 1], [1, 0], [1, 2], [2, 1]];
            }
        }
        s
    }

    /// Checks every rule of the format.
    pub fn validate(&self) -> Result<(), String> {
        let id_ok = !self.id.is_empty()
            && self.id.len() <= 64
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !id_ok {
            return Err("id must be 1-64 characters a-z, 0-9 or -".into());
        }
        if self.name.is_empty() || self.name.len() > 128 {
            return Err("name must be 1-128 bytes".into());
        }
        if !finite(self.width) || !(1.0..=MAX_WIDTH).contains(&self.width) {
            return Err(format!("width must be 1...{MAX_WIDTH} mm"));
        }
        let half = self.width / 2.0 + 1e-9;
        let in_band = |p: Vec2| {
            finite(p.x) && finite(p.y) && (0.0..=MAX_LENGTH).contains(&p.x) && p.y.abs() <= half
        };
        match self.kind {
            SewingKind::Decorative => {
                if self.points.len() < 2 || self.points.len() > MAX_NEEDLES {
                    return Err(format!("needs 2-{MAX_NEEDLES} needle points"));
                }
                if !self.points.iter().all(|&p| in_band(p)) {
                    return Err("points must lie within the width and 0-200 mm along".into());
                }
                if self
                    .points
                    .windows(2)
                    .any(|w| (w[1].x - w[0].x).abs() > MAX_FEED + 1e-9)
                {
                    return Err(format!(
                        "a stitch may move at most {MAX_FEED} mm along the feed"
                    ));
                }
            }
            SewingKind::Satin => {
                if self.profile.len() < 2 || self.profile.len() > 200 {
                    return Err("a satin profile needs 2-200 stations".into());
                }
                let ok = self
                    .profile
                    .iter()
                    .all(|&[x, l, r]| in_band(v2(x, l)) && in_band(v2(x, r)) && l <= r);
                if !ok || self.profile.windows(2).any(|w| w[1][0] <= w[0][0]) {
                    return Err(
                        "stations need increasing x and left <= right within the width".into(),
                    );
                }
                self.check_density()?;
            }
            SewingKind::DecorativeSatin => {
                if self.outline.len() < 3 || self.outline.len() > 500 {
                    return Err("an outline needs 3-500 points".into());
                }
                if !self.outline.iter().all(|&p| in_band(p)) {
                    return Err("the outline must lie within the width and 0-200 mm".into());
                }
                if polygon_area(&self.outline).abs() < 0.01 {
                    return Err("the outline encloses no area".into());
                }
                self.check_density()?;
            }
            SewingKind::Cross => {
                if !finite(self.cell_size) || !(0.5..=5.0).contains(&self.cell_size) {
                    return Err("cell size must be 0.5-5 mm".into());
                }
                if self.rows == 0 || f64::from(self.rows) * self.cell_size > self.width + 1e-9 {
                    return Err("rows × cell size must fit the width".into());
                }
                if self.cells.is_empty() || self.cells.len() > 500 {
                    return Err("needs 1-500 cells".into());
                }
                let cols = (MAX_LENGTH / self.cell_size).floor() as u32;
                if self.cells.iter().any(|&[c, r]| r >= self.rows || c >= cols) {
                    return Err("a cell lies outside the grid".into());
                }
            }
        }
        let n = self.needles().len();
        if n > MAX_NEEDLES {
            return Err(format!(
                "generates {n} needle points (at most {MAX_NEEDLES})"
            ));
        }
        Ok(())
    }

    fn check_density(&self) -> Result<(), String> {
        if !finite(self.density) || !(1.0..=10.0).contains(&self.density) {
            return Err("density must be 1-10 stitches per mm".into());
        }
        Ok(())
    }

    /// Length along the feed (mm): the furthest x any needle point reaches.
    pub fn length(&self) -> f64 {
        self.needles().iter().map(|p| p.x).fold(0.0, f64::max)
    }

    /// The needle points the machine sews, in order (mm).
    pub fn needles(&self) -> Vec<Vec2> {
        let mut n = match self.kind {
            SewingKind::Decorative => self.points.clone(),
            SewingKind::Satin => satin(&self.profile, self.density),
            SewingKind::DecorativeSatin => decorative_satin(&self.outline, self.density),
            SewingKind::Cross => cross(self),
        };
        if self.swapped {
            n.reverse();
        }
        n
    }

    /// Swap: exchanges the start and end points, so the same needle points are sewn in
    /// reverse order (the guide's Home ▸ Stitch ▸ Swap).
    pub fn swap(&mut self) {
        self.swapped ^= true;
    }

    /// Duplicate: the pattern followed by a copy of itself placed where it ends along the feed
    /// (needle points, satin stations or cross cells). A decorative satin outline is one closed
    /// shape and cannot hold a copy. Fails, leaving `self` as it was, when the result breaks a
    /// rule of the format (length, needle count, feed).
    pub fn duplicate(&self) -> Result<SewingStitch, String> {
        let mut d = self.clone();
        match self.kind {
            SewingKind::Decorative => {
                let span = span(self.points.iter().map(|p| p.x));
                if span <= 0.0 {
                    return Err("the needle points have no length to repeat".into());
                }
                d.points
                    .extend(self.points.iter().map(|p| v2(p.x + span, p.y)));
            }
            SewingKind::Satin => {
                let span = span(self.profile.iter().map(|s| s[0]));
                d.profile.extend(
                    self.profile
                        .iter()
                        .skip(1)
                        .map(|&[x, l, r]| [x + span, l, r]),
                );
            }
            SewingKind::DecorativeSatin => {
                return Err(
                    "a decorative satin outline is one shape and cannot be duplicated".into(),
                );
            }
            SewingKind::Cross => {
                let lo = self.cells.iter().map(|c| c[0]).min().unwrap_or(0);
                let hi = self.cells.iter().map(|c| c[0]).max().unwrap_or(0);
                let cols = hi - lo + 1;
                d.cells
                    .extend(self.cells.iter().map(|&[c, r]| [c + cols, r]));
            }
        }
        d.validate()?;
        Ok(d)
    }

    /// Moves every point of the pattern by `delta` mm (cross cells by whole cells, rounded).
    /// Fails, leaving `self` as it was, when a point would leave the width or 0-200 mm.
    pub fn translate(&mut self, delta: Vec2) -> Result<(), String> {
        let mut d = self.clone();
        for p in d.points.iter_mut().chain(d.outline.iter_mut()) {
            *p = *p + delta;
        }
        for s in &mut d.profile {
            *s = [s[0] + delta.x, s[1] + delta.y, s[2] + delta.y];
        }
        let (dc, dr) = (
            (delta.x / d.cell_size).round() as i64,
            (delta.y / d.cell_size).round() as i64,
        );
        for c in &mut d.cells {
            let (col, row) = (i64::from(c[0]) + dc, i64::from(c[1]) + dr);
            if col < 0 || row < 0 {
                return Err("the pattern would leave the grid".into());
            }
            *c = [col as u32, row as u32];
        }
        d.validate()?;
        *self = d;
        Ok(())
    }

    /// The pattern as an `openstitch.sewing` document (with the generated needle points under
    /// `needles` when `with_needles`, for tools that only want to sew).
    pub fn to_json(&self, with_needles: bool) -> String {
        let mut v = serde_json::to_value(SewingFile::from(self.clone())).expect("serializes");
        if with_needles {
            v["needles"] = self
                .needles()
                .iter()
                .map(|p| serde_json::json!([round3(p.x), round3(p.y)]))
                .collect();
        }
        v.to_string()
    }
}

/// Largest minus smallest of `xs` (0 when empty).
fn span(xs: impl Iterator<Item = f64> + Clone) -> f64 {
    let hi = xs.clone().fold(f64::NEG_INFINITY, f64::max);
    let lo = xs.fold(f64::INFINITY, f64::min);
    if hi >= lo { hi - lo } else { 0.0 }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn polygon_area(p: &[Vec2]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| p[i].x * p[(i + 1) % n].y - p[(i + 1) % n].x * p[i].y)
        .sum::<f64>()
        / 2.0
}

/// Satin column: one stitch across per 1/density mm, alternating edges.
fn satin(profile: &[[f64; 3]], density: f64) -> Vec<Vec2> {
    let (Some(first), Some(last)) = (profile.first(), profile.last()) else {
        return Vec::new();
    };
    let (x0, x1) = (first[0], last[0]);
    let step = 1.0 / density.max(0.1);
    let n = ((x1 - x0) / step).round().max(1.0) as usize;
    let edges = |x: f64| -> (f64, f64) {
        let i = profile
            .windows(2)
            .position(|w| x <= w[1][0])
            .unwrap_or(profile.len().saturating_sub(2));
        let (a, b) = (profile[i], profile[(i + 1).min(profile.len() - 1)]);
        let t = if b[0] > a[0] {
            ((x - a[0]) / (b[0] - a[0])).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)
    };
    (0..=n)
        .map(|k| {
            let x = x0 + (x1 - x0) * k as f64 / n as f64;
            let (l, r) = edges(x);
            v2(x, if k % 2 == 0 { l } else { r })
        })
        .collect()
}

/// Decorative satin: the outline cut by lines across the feed every 1/density mm; each cut
/// is one stitch, alternating ends. Where a cut crosses the outline more than twice the
/// outermost crossings are used.
fn decorative_satin(outline: &[Vec2], density: f64) -> Vec<Vec2> {
    let (lo, hi) = outline
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.x), hi.max(p.x))
        });
    let step = 1.0 / density.max(0.1);
    let mut out = Vec::new();
    let mut x = lo + step / 2.0;
    let n = outline.len();
    let mut k = 0;
    while x < hi && out.len() < MAX_NEEDLES * 2 {
        let mut ys: Vec<f64> = (0..n)
            .filter_map(|i| {
                let (a, b) = (outline[i], outline[(i + 1) % n]);
                ((a.x <= x && b.x > x) || (b.x <= x && a.x > x))
                    .then(|| a.y + (x - a.x) * (b.y - a.y) / (b.x - a.x))
            })
            .collect();
        ys.sort_by(f64::total_cmp);
        if let (Some(&top), Some(&bottom)) = (ys.first(), ys.last()) {
            out.push(v2(x, if k % 2 == 0 { top } else { bottom }));
            k += 1;
        }
        x += step;
    }
    out
}

/// Cross stitch cells, column by column along the feed: within a column the cells are sewn
/// in row order, each as "/" then "\" (corner to corner), so the needle never jumps back
/// against the feed by more than one cell.
fn cross(s: &SewingStitch) -> Vec<Vec2> {
    let c = s.cell_size;
    let top = -f64::from(s.rows) * c / 2.0;
    let mut cells = s.cells.clone();
    cells.sort_by_key(|&[col, row]| (col, if col % 2 == 0 { row } else { u32::MAX - row }));
    cells.dedup();
    let mut out = Vec::new();
    for [col, row] in cells {
        let (x, y) = (f64::from(col) * c, top + f64::from(row) * c);
        let (tl, tr, bl, br) = (v2(x, y), v2(x + c, y), v2(x, y + c), v2(x + c, y + c));
        let mut diag = |a: Vec2, b: Vec2| {
            out.push(a);
            out.push(b);
            if s.triple {
                out.push(a);
                out.push(b);
            }
        };
        diag(bl, tr);
        diag(tl, br);
    }
    out
}

/// An SVG drawing of the pattern repeated `repeats` times along the feed: thread in blue,
/// needle points as dots, the width band dashed. 10 px per mm.
pub fn svg(s: &SewingStitch, repeats: usize) -> String {
    let needles = s.needles();
    let len = s.length().max(1.0);
    let repeats = repeats.clamp(1, 20);
    let scale = 10.0;
    let w = len * repeats as f64 * scale + 40.0;
    let h = s.width * scale + 40.0;
    let map = |p: Vec2, r: usize| {
        (
            20.0 + (p.x + len * r as f64) * scale,
            20.0 + (p.y + s.width / 2.0) * scale,
        )
    };
    let mut d = String::new();
    let mut dots = String::new();
    for r in 0..repeats {
        for (i, p) in needles.iter().enumerate() {
            let (x, y) = map(*p, r);
            d.push_str(&format!(
                "{}{x:.2},{y:.2} ",
                if i == 0 && r == 0 { "M" } else { "L" }
            ));
            dots.push_str(&format!("<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"1.2\"/>"));
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.0} {h:.0}\">\
<rect x=\"20\" y=\"20\" width=\"{bw:.2}\" height=\"{bh:.2}\" fill=\"none\" stroke=\"#bbb\" stroke-dasharray=\"4 3\"/>\
<path d=\"{d}\" fill=\"none\" stroke=\"#0a55a3\" stroke-width=\"1.2\"/><g fill=\"#222\">{dots}</g></svg>\n",
        bw = len * repeats as f64 * scale,
        bh = s.width * scale,
    )
}

/// PMV (Brother sewing-machine stitch file), written exactly as the independent pyembroidery
/// project's `PmvWriter` does (its MIT-licensed code is a public description of the format).
/// Experimental: pyembroidery notes that the trailing length table is not understood, and no
/// machine has been tested. At most 100 needle points; widths over 7 mm are scaled to fit.
pub fn pmv(s: &SewingStitch) -> Result<Vec<u8>, String> {
    // pyembroidery units are 0.1 mm with y up in its stitch list; our y grows downwards on
    // screen, so its `y = -stitch[1]` sees our y negated twice: pass y unchanged as -y.
    let needles = s.needles();
    if needles.len() > PMV_MAX_NEEDLES {
        return Err(format!(
            "PMV holds at most {PMV_MAX_NEEDLES} needle points (this pattern has {})",
            needles.len()
        ));
    }
    let pts: Vec<(f64, f64)> = needles.iter().map(|p| (p.x * 10.0, -p.y * 10.0)).collect();
    Ok(pmv_bytes(&pts))
}

/// Most needle points PMV export accepts (pyembroidery's limit).
pub const PMV_MAX_NEEDLES: usize = 100;

/// PMV bytes for pyembroidery-style stitches (x, y in 0.1 mm, all sewn).
pub fn pmv_bytes(stitches: &[(f64, f64)]) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut count = 0usize;
    for &(_, sy) in stitches {
        let y = -sy;
        count += 1;
        min_y = min_y.min(y);
        max_y = max_y.max(y);
        if count >= PMV_MAX_NEEDLES {
            break;
        }
    }
    let center_y = (min_y + max_y) / 2.0;
    let normal_max_y = max_y - center_y;
    let scale_y = if normal_max_y > 35.0 {
        14.0 / normal_max_y
    } else {
        1.0 / 2.5
    };
    let scale_x = 1.0 / 2.5;
    out.extend_from_slice(b"#PMV0001");
    out.extend_from_slice(&[b'.'; 36]);
    out.extend_from_slice(
        b"\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\
\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\
\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00\
\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00",
    );
    let le16 = |out: &mut Vec<u8>, v: i64| out.extend_from_slice(&(v as u16).to_le_bytes());
    le16(&mut out, count as i64);
    le16(&mut out, count as i64 * 2);
    let (mut min_x, mut max_x) = (i64::MAX, i64::MIN);
    let (mut min_yy, mut max_yy) = (i64::MAX, i64::MIN);
    let mut xx: i64 = 0;
    for &(sx, sy) in stitches.iter().take(count) {
        let x = sx * scale_x;
        let y = (((-sy) - center_y) * scale_y).round_ties_even() as i64;
        let dx = (x - xx as f64).round_ties_even() as i64;
        xx += dx;
        min_x = min_x.min(xx);
        max_x = max_x.max(xx);
        min_yy = min_yy.min(y);
        max_yy = max_yy.max(y);
        let bx = if dx < 0 { dx + 64 } else { dx };
        let by = if y < 0 { y + 32 } else { y };
        out.push(bx as u8);
        out.push(by as u8);
    }
    le16(&mut out, 0);
    le16(&mut out, 256);
    out.extend_from_slice(b"\x00\x00\x00\x00\x05\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00");
    le16(&mut out, 256);
    out.push(0);
    out.push(0);
    // Length table: pyembroidery's fixed values ("not solved").
    const LENGTHS: [(i64, i64); 18] = [
        (0, 0),
        (10, 71),
        (20, 143),
        (40, 214),
        (60, 286),
        (80, 357),
        (100, 429),
        (120, 500),
        (140, 571),
        (160, 714),
        (180, 786),
        (200, 857),
        (250, 1000),
        (300, 1286),
        (350, 1429),
        (400, 1571),
        (450, 1786),
        (500, 2000),
    ];
    out.push((LENGTHS.len() - 1) as u8);
    out.push(LENGTHS.len() as u8);
    for (a, b) in LENGTHS {
        le16(&mut out, a);
        le16(&mut out, b);
    }
    let width_range = if count == 0 { 0 } else { max_yy - min_yy };
    if width_range == 0 {
        out.push(0);
        out.push(1);
        le16(&mut out, 8192);
        le16(&mut out, 1000);
    } else {
        let steps = 15i64;
        let second_max = 28000.0 / width_range as f64;
        let second_step = second_max / (steps - 1) as f64;
        out.push((steps - 1) as u8);
        out.push(steps as u8);
        for i in 0..steps {
            le16(&mut out, 50 * i);
            le16(&mut out, (second_step * i as f64).round_ties_even() as i64);
        }
    }
    let _ = (min_x, max_x);
    le16(&mut out, 0x12);
    out.extend_from_slice(&[0u8; 16]);
    out
}

/// A PMV file read back: the needle points and the two lookup tables, as the layout
/// [`pmv_bytes`] writes (pyembroidery's description). Needle points are 0.1 mm in the writer's
/// input orientation (x along the feed, y across, +y as `pmv_bytes` takes it), recovered from
/// the 0.25 mm packed pairs; the y centre and any width scaling the writer applied are not in
/// the file, so y is centred on the packed zero.
#[derive(Clone, Debug, PartialEq)]
pub struct PmvRead {
    /// The 36 banner bytes after the magic (`.` × 36 in pyembroidery's files).
    pub banner: [u8; 36],
    /// Needle points, 0.1 mm.
    pub needles: Vec<(f64, f64)>,
    /// The length table pairs, as stored.
    pub lengths: Vec<(u16, u16)>,
    /// The width table pairs, as stored.
    pub widths: Vec<(u16, u16)>,
}

/// PMV magic bytes.
pub const PMV_MAGIC: &[u8; 8] = b"#PMV0001";
/// Fixed bytes between the banner and the needle count (pyembroidery's layout).
pub const PMV_PREAMBLE: &[u8; 56] = b"                                                    ";
/// Most needle points a PMV file read may hold (the writer stops at [`PMV_MAX_NEEDLES`];
/// files from other writers are accepted up to this bound).
pub const PMV_READ_MAX_NEEDLES: usize = 4096;
/// Largest PMV file read.
pub const PMV_MAX_BYTES: usize = 1 << 16;

/// Reads a PMV file in the layout [`pmv_bytes`] writes. Refuses other magics, a preamble
/// that differs from pyembroidery's, truncated bodies and inconsistent tables, and never
/// panics on hostile input.
pub fn pmv_read(bytes: &[u8]) -> Result<PmvRead, String> {
    if bytes.len() > PMV_MAX_BYTES {
        return Err(format!("PMV file larger than {PMV_MAX_BYTES} bytes"));
    }
    let mut at = 0usize;
    let take = |at: &mut usize, n: usize, what: &str| -> Result<&[u8], String> {
        let end = at.checked_add(n).filter(|&e| e <= bytes.len());
        match end {
            Some(end) => {
                let s = &bytes[*at..end];
                *at = end;
                Ok(s)
            }
            None => Err(format!("truncated PMV: {what} at byte {at}")),
        }
    };
    let u16_at = |at: &mut usize, what: &str| -> Result<u16, String> {
        let s = take(at, 2, what)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    };
    if take(&mut at, 8, "magic")? != PMV_MAGIC {
        return Err("not a PMV file: the magic is not #PMV0001".into());
    }
    let mut banner = [0u8; 36];
    banner.copy_from_slice(take(&mut at, 36, "banner")?);
    if take(&mut at, 56, "preamble")? != PMV_PREAMBLE {
        return Err("PMV preamble differs from the documented layout".into());
    }
    let count = u16_at(&mut at, "needle count")? as usize;
    let twice = u16_at(&mut at, "needle byte count")? as usize;
    if twice != count * 2 {
        return Err(format!(
            "PMV needle byte count {twice} is not twice the needle count {count}"
        ));
    }
    if count > PMV_READ_MAX_NEEDLES {
        return Err(format!(
            "PMV holds {count} needle points, more than {PMV_READ_MAX_NEEDLES}"
        ));
    }
    let packed = take(&mut at, count * 2, "needle points")?;
    let mut needles = Vec::with_capacity(count);
    let mut x: i64 = 0;
    for &[bx, by] in packed.as_chunks::<2>().0 {
        if bx >= 64 || by >= 32 {
            return Err(format!(
                "PMV needle byte pair {bx:#x} {by:#x} is out of range"
            ));
        }
        let dx = if bx >= 32 { bx as i64 - 64 } else { bx as i64 };
        let y = if by >= 16 { by as i64 - 32 } else { by as i64 };
        x += dx;
        // 0.25 mm units → 0.1 mm; y was negated by the writer.
        needles.push((x as f64 * 2.5, -(y as f64) * 2.5));
    }
    if u16_at(&mut at, "body end")? != 0 || u16_at(&mut at, "body end")? != 256 {
        return Err("PMV body end marker differs from the documented layout".into());
    }
    let fixed = take(&mut at, 16, "fixed block")?;
    if fixed != b"\x00\x00\x00\x00\x05\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00" {
        return Err("PMV fixed block differs from the documented layout".into());
    }
    if u16_at(&mut at, "table lead")? != 256 || u16_at(&mut at, "table lead")? != 0 {
        return Err("PMV table lead differs from the documented layout".into());
    }
    let read_table = |at: &mut usize, what: &str| -> Result<Vec<(u16, u16)>, String> {
        let head = take(at, 2, what)?;
        let (last, n) = (head[0] as usize, head[1] as usize);
        if n == 0 || last + 1 != n {
            return Err(format!(
                "PMV {what} table header {last} {n} is inconsistent"
            ));
        }
        let mut t = Vec::with_capacity(n);
        for _ in 0..n {
            let a = u16_at(at, what)?;
            let b = u16_at(at, what)?;
            t.push((a, b));
        }
        Ok(t)
    };
    let lengths = read_table(&mut at, "length")?;
    let widths = read_table(&mut at, "width")?;
    if u16_at(&mut at, "trailer")? != 0x12 || take(&mut at, 16, "trailer")? != [0u8; 16] {
        return Err("PMV trailer differs from the documented layout".into());
    }
    if at != bytes.len() {
        return Err(format!(
            "PMV has {} unexpected trailing bytes",
            bytes.len() - at
        ));
    }
    Ok(PmvRead {
        banner,
        needles,
        lengths,
        widths,
    })
}

/// A decorative sewing stitch from a PMV file's needle points (mm, back in the editor's
/// orientation that [`pmv`] exports from), `id`/`name` given. The width is the needle points'
/// span across the feed, at least 1 mm; points are shifted so the first needle point sits at
/// x = 0 and the pattern is centred across the feed.
pub fn pmv_import(bytes: &[u8], id: &str, name: &str) -> Result<SewingStitch, String> {
    let read = pmv_read(bytes)?;
    if read.needles.len() < 2 {
        return Err("PMV holds fewer than 2 needle points".into());
    }
    let x0 = read.needles[0].0;
    let (min_y, max_y) = read
        .needles
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &(_, y)| {
            (lo.min(y), hi.max(y))
        });
    let mid = (min_y + max_y) / 2.0;
    let mut s = SewingStitch::new(SewingKind::Decorative);
    s.id = id.to_string();
    s.name = name.to_string();
    s.width = ((max_y - min_y) / 10.0).clamp(1.0, MAX_WIDTH);
    s.points = read
        .needles
        .iter()
        .map(|&(x, y)| v2((x - x0) / 10.0, -(y - mid) / 10.0))
        .collect();
    s.validate()?;
    Ok(s)
}
