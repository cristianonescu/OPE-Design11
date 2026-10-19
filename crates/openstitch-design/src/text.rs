//! Lettering: bundled open-licence fonts and the layout of a [`TextSpec`] into glyph contours.
//!
//! Glyph outlines come from the font's `glyf`/`CFF` data through `ttf-parser`; TrueType
//! quadratic segments are converted exactly to cubic Béziers, so text contours use the same
//! [`Contour`] model as drawn shapes. Inner contours (the counter of an "o") are holes under the
//! even-odd rule the region generators use. Each glyph is filled on its own, so glyphs that touch
//! or overlap (script joins) never cut holes into each other.

use crate::fonts::{
    DesignFont, EmbroideryFont, FontData, MAX_GLYPH_SEGMENTS, MAX_LAYOUT_SEGMENTS, parse_face,
    shaping_note,
};
use crate::geometry::{Bezier, Contour, Rect, Transform, Vec2, v2};
use crate::model::StitchBlock;
use openstitch_core::{Command, Stitch};
use serde::{Deserialize, Serialize};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

/// A font bundled with OpenStitch. Only SIL OFL fonts are bundled, unmodified (see
/// THIRD_PARTY_NOTICES.md and `crates/openstitch-design/fonts/`).
#[derive(Debug)]
pub struct FontInfo {
    /// Stable identifier stored in `.osd` files.
    pub id: &'static str,
    pub family: &'static str,
    pub style: &'static str,
    pub license: &'static str,
    pub data: &'static [u8],
}

pub static FONTS: [FontInfo; 2] = [
    FontInfo {
        id: "lato-bold",
        family: "Lato",
        style: "Bold",
        license: "OFL-1.1",
        data: include_bytes!("../fonts/Lato-Bold.ttf"),
    },
    FontInfo {
        id: "pacifico",
        family: "Pacifico",
        style: "Regular",
        license: "OFL-1.1",
        data: include_bytes!("../fonts/Pacifico-Regular.ttf"),
    },
];

pub const DEFAULT_FONT: &str = "lato-bold";

/// The font with this id, or whose "family" / "family style" matches, ignoring case.
pub fn font(name: &str) -> Option<&'static FontInfo> {
    let name = name.trim();
    FONTS.iter().find(|f| {
        f.id.eq_ignore_ascii_case(name)
            || f.family.eq_ignore_ascii_case(name)
            || format!("{} {}", f.family, f.style).eq_ignore_ascii_case(name)
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// What a text object says and how it is set. Its contours (on the owning
/// [`DesignObject`](crate::DesignObject)) are derived from this by [`layout`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSpec {
    pub text: String,
    /// [`FontInfo::id`].
    pub font: String,
    /// Height of capital letters, mm.
    pub size: f64,
    /// Extra space between letters, mm (may be negative).
    #[serde(default)]
    pub letter_spacing: f64,
    /// Extra space between lines, mm (may be negative).
    #[serde(default)]
    pub line_spacing: f64,
    #[serde(default)]
    pub align: TextAlign,
    /// Shape object whose first contour the text follows (text on a path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<u64>,
    /// Placement after layout. Straight text is laid out with the first baseline on y = 0
    /// and the alignment point at x = 0; text on a path is laid out in page coordinates.
    #[serde(default = "identity")]
    pub transform: Transform,
    /// Number of contours of each outlined glyph, in order: compiling fills glyph by glyph.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub glyphs: Vec<u32>,
    /// Name Drop attribute: the text is fitted into a fixed box, so a replacement keeps the
    /// layout (Name Drop lists, template slots).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_drop: Option<NameDrop>,
}

/// Vertical placement in a Name Drop box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

/// PE-DESIGN's Name Drop attribute: whatever the text says, it is fitted into `box` (laid-out
/// coordinates, before [`TextSpec::transform`], 0.1 mm), uniformly when `maintain_aspect`
/// (then placed by `align` and `valign`), else stretched to fill it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameDrop {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default = "yes")]
    pub maintain_aspect: bool,
    #[serde(default = "center")]
    pub align: TextAlign,
    #[serde(default)]
    pub valign: VAlign,
    /// Optional label (template slot name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

fn yes() -> bool {
    true
}

fn center() -> TextAlign {
    TextAlign::Center
}

impl NameDrop {
    /// The transform fitting laid-out bounds `b` into the box.
    pub fn fit(&self, b: &Rect) -> Transform {
        let (w, h) = (b.width().max(1e-9), b.height().max(1e-9));
        let (sx, sy) = if self.maintain_aspect {
            let k = (self.width / w).min(self.height / h);
            (k, k)
        } else {
            (self.width / w, self.height / h)
        };
        let (fw, fh) = (w * sx, h * sy);
        let x = match self.align {
            TextAlign::Left => self.x,
            TextAlign::Center => self.x + (self.width - fw) / 2.0,
            TextAlign::Right => self.x + self.width - fw,
        };
        let y = match self.valign {
            VAlign::Top => self.y,
            VAlign::Middle => self.y + (self.height - fh) / 2.0,
            VAlign::Bottom => self.y + self.height - fh,
        };
        Transform::translation(-b.min_x, -b.min_y)
            .then(&Transform::scaling(sx, sy, v2(0.0, 0.0)))
            .then(&Transform::translation(x, y))
    }
}

fn identity() -> Transform {
    Transform::IDENTITY
}

impl TextSpec {
    pub fn new(text: impl Into<String>, font: &str, size: f64) -> Self {
        TextSpec {
            text: text.into(),
            font: font.into(),
            size,
            letter_spacing: 0.0,
            line_spacing: 0.0,
            align: TextAlign::Left,
            path: None,
            transform: Transform::IDENTITY,
            glyphs: Vec::new(),
            name_drop: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TextError {
    UnknownFont(String),
    BadFont(String),
    /// The laid-out outlines exceed [`MAX_LAYOUT_SEGMENTS`].
    TooComplex,
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextError::UnknownFont(n) => write!(f, "unknown font '{n}'"),
            TextError::BadFont(n) => write!(f, "font '{n}' cannot be read"),
            TextError::TooComplex => write!(
                f,
                "the text's outlines are too complex (more than {MAX_LAYOUT_SEGMENTS} segments)"
            ),
        }
    }
}

impl std::error::Error for TextError {}

/// One placed glyph of a layout: outlines, stitch data (user-mapped text), or both.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub ch: char,
    pub contours: Vec<Contour>,
    pub stitches: Option<StitchBlock>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// Glyphs with an outline or stitches (spaces and missing characters have none), placed in
    /// 0.1 mm.
    pub glyphs: Vec<Glyph>,
    /// Characters the font has no glyph for (each once, in order of appearance). Glyphs over
    /// [`MAX_GLYPH_SEGMENTS`] count as missing.
    pub missing: Vec<char>,
    /// Scripts in the text that need shaping this layout does not do ([`shaping_note`]).
    pub shaping: Vec<&'static str>,
}

impl Layout {
    pub fn contours(&self) -> Vec<Contour> {
        self.glyphs
            .iter()
            .flat_map(|g| g.contours.iter().cloned())
            .collect()
    }

    /// Number of contours of each glyph that has an outline.
    pub fn glyph_counts(&self) -> Vec<u32> {
        self.outlined().map(|g| g.contours.len() as u32).collect()
    }

    /// Glyphs with an outline, in order.
    pub fn outlined(&self) -> impl Iterator<Item = &Glyph> {
        self.glyphs.iter().filter(|g| !g.contours.is_empty())
    }

    /// The stitch glyphs joined into one block, in text order: each glyph starts after a trim,
    /// and its thread list follows the previous glyph's.
    pub fn stitch_block(&self) -> Option<StitchBlock> {
        let mut out = StitchBlock {
            stitches: Vec::new(),
            threads: Vec::new(),
        };
        for b in self.glyphs.iter().filter_map(|g| g.stitches.as_ref()) {
            if let Some(last) = out.stitches.last().copied() {
                out.stitches
                    .push(Stitch::new(last.x, last.y, Command::Trim));
                out.stitches
                    .push(Stitch::new(last.x, last.y, Command::ColorChange));
            }
            out.stitches.extend(
                b.stitches
                    .iter()
                    .filter(|s| !matches!(s.command, Command::End | Command::Stop)),
            );
            out.threads.extend(b.threads.iter().cloned());
        }
        (!out.stitches.is_empty()).then_some(out)
    }
}

/// Collects a glyph outline as contours in font units (y up).
#[derive(Default)]
struct Builder {
    contours: Vec<Contour>,
    points: Vec<Vec2>,
    curves: Vec<Option<Bezier>>,
    /// Segments seen; past [`MAX_GLYPH_SEGMENTS`] the glyph is abandoned.
    segments: usize,
}

impl Builder {
    fn last(&self) -> Vec2 {
        self.points.last().copied().unwrap_or_default()
    }

    /// Counts one segment; false once the glyph is over the limit (the rest is ignored).
    fn take(&mut self) -> bool {
        self.segments += 1;
        self.segments <= MAX_GLYPH_SEGMENTS
    }

    fn too_complex(&self) -> bool {
        self.segments > MAX_GLYPH_SEGMENTS
    }

    fn finish(&mut self) {
        let mut points = std::mem::take(&mut self.points);
        let mut curves = std::mem::take(&mut self.curves);
        if points.len() > 1 && points.last() == points.first() {
            points.pop();
        } else if !points.is_empty() {
            curves.push(None);
        }
        let mut contour = Contour::new(points, true);
        if curves.iter().any(Option::is_some) {
            curves.truncate(contour.segment_count());
            contour.curves = curves;
        }
        let degenerate =
            contour.points.len() < 2 || (contour.points.len() < 3 && !contour.has_curves());
        if !degenerate {
            self.contours.push(contour);
        }
    }
}

impl OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        if !self.take() {
            return;
        }
        if !self.points.is_empty() {
            self.finish();
        }
        self.points.push(v2(x.into(), y.into()));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if !self.take() {
            return;
        }
        self.curves.push(None);
        self.points.push(v2(x.into(), y.into()));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        if !self.take() {
            return;
        }
        // Exact degree elevation: the cubic handles sit 2/3 of the way to the quadratic one.
        let (p0, q, p) = (
            self.last(),
            v2(x1.into(), y1.into()),
            v2(x.into(), y.into()),
        );
        self.curves.push(Some(Bezier {
            c1: p0.lerp(q, 2.0 / 3.0),
            c2: p.lerp(q, 2.0 / 3.0),
        }));
        self.points.push(p);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        if !self.take() {
            return;
        }
        self.curves.push(Some(Bezier {
            c1: v2(x1.into(), y1.into()),
            c2: v2(x2.into(), y2.into()),
        }));
        self.points.push(v2(x.into(), y.into()));
    }

    fn close(&mut self) {
        if !self.too_complex() && !self.points.is_empty() {
            self.finish();
        }
    }
}

/// A bundled font or a font stored in the design.
#[derive(Clone, Copy, Debug)]
pub enum FontRef<'a> {
    Bundled(&'static FontInfo),
    Design(&'a DesignFont),
}

impl FontRef<'_> {
    pub fn id(&self) -> &str {
        match self {
            FontRef::Bundled(f) => f.id,
            FontRef::Design(f) => &f.id,
        }
    }
}

/// The font `name` refers to: a bundled font ([`font`]) first, then a design font by id or
/// family, ignoring case.
pub fn resolve<'a>(name: &str, fonts: &'a [DesignFont]) -> Option<FontRef<'a>> {
    if let Some(f) = font(name) {
        return Some(FontRef::Bundled(f));
    }
    let name = name.trim();
    fonts
        .iter()
        .find(|f| f.id.eq_ignore_ascii_case(name))
        .or_else(|| {
            fonts.iter().find(|f| {
                f.family.eq_ignore_ascii_case(name)
                    || format!("{} {}", f.family, f.style).eq_ignore_ascii_case(name)
            })
        })
        .map(FontRef::Design)
}

/// Where a font's glyphs come from.
enum Source<'a> {
    Face(Box<Face<'a>>),
    Embroidery(&'a EmbroideryFont),
}

/// A parsed font with the metrics layout needs.
pub struct LoadedFont<'a> {
    source: Source<'a>,
    /// 0.1 mm per font unit for a given size, per mm of size.
    unit_per_mm: f64,
}

/// A glyph's outline (font units) and stitches, or `None` when it has neither.
struct Outline {
    contours: Vec<Contour>,
    stitches: Option<StitchBlock>,
    segments: usize,
}

impl LoadedFont<'static> {
    /// A bundled font.
    pub fn load(name: &str) -> Result<Self, TextError> {
        LoadedFont::load_in(name, &[])
    }
}

impl<'a> LoadedFont<'a> {
    /// A bundled font or one of `fonts`.
    pub fn load_in(name: &str, fonts: &'a [DesignFont]) -> Result<Self, TextError> {
        let found = resolve(name, fonts).ok_or_else(|| TextError::UnknownFont(name.into()))?;
        let bad = || TextError::BadFont(found.id().to_string());
        let face = match found {
            FontRef::Bundled(info) => Face::parse(info.data, 0).map_err(|_| bad())?,
            FontRef::Design(f) => match &f.data {
                FontData::TrueType { face, bytes } => {
                    parse_face(&bytes.0, *face).ok_or_else(bad)?
                }
                FontData::Embroidery(e) => {
                    if !(e.cap_height.is_finite() && e.cap_height > 0.0) {
                        return Err(bad());
                    }
                    return Ok(LoadedFont {
                        source: Source::Embroidery(e),
                        unit_per_mm: 10.0 / e.cap_height,
                    });
                }
            },
        };
        let em = f64::from(face.units_per_em());
        let cap = face
            .capital_height()
            .filter(|&h| h > 0)
            .map(f64::from)
            .or_else(|| {
                face.glyph_index('H')
                    .and_then(|g| face.glyph_bounding_box(g))
                    .map(|b| f64::from(b.y_max))
                    .filter(|&h| h > 0.0)
            })
            // CJK fonts have no Latin capitals; their ideographs fill most of the em.
            .unwrap_or(0.7 * em)
            .clamp(0.05 * em, 4.0 * em);
        Ok(LoadedFont {
            source: Source::Face(Box::new(face)),
            unit_per_mm: 10.0 / cap,
        })
    }

    pub fn glyph_count(&self) -> usize {
        match &self.source {
            Source::Face(f) => usize::from(f.number_of_glyphs()),
            Source::Embroidery(e) => e.glyphs.len(),
        }
    }

    pub fn has_char(&self, c: char) -> bool {
        self.glyph(c).is_some()
    }

    /// The glyph of `c`: a glyph id, or an index into an embroidery font's glyphs.
    fn glyph(&self, c: char) -> Option<u32> {
        match &self.source {
            // Glyph 0 is .notdef: drawn by no character.
            Source::Face(f) => f
                .glyph_index(c)
                .filter(|g| g.0 != 0)
                .map(|g| u32::from(g.0)),
            Source::Embroidery(e) => e
                .glyphs
                .binary_search_by_key(&c, |g| g.ch)
                .ok()
                .map(|i| i as u32),
        }
    }

    /// Pen advance of `glyph` (the missing-glyph advance for `None`), font units.
    fn advance(&self, glyph: Option<u32>) -> f64 {
        match &self.source {
            Source::Face(f) => f64::from(
                f.glyph_hor_advance(GlyphId(glyph.unwrap_or(0) as u16))
                    .unwrap_or(0),
            ),
            Source::Embroidery(e) => glyph
                .and_then(|i| e.glyphs.get(i as usize))
                .map_or(e.cap_height * 0.5, |g| g.advance),
        }
    }

    /// Baseline to baseline, font units.
    fn line_height(&self) -> f64 {
        match &self.source {
            Source::Face(f) => f64::from(
                i32::from(f.ascender()) - i32::from(f.descender()) + i32::from(f.line_gap()),
            ),
            Source::Embroidery(e) => e.ascender + e.descender + e.line_gap,
        }
    }

    /// Font units → 0.1 mm at size 1 mm: TrueType is +y up, embroidery fonts +y down.
    fn units(&self, scale: f64) -> Transform {
        let flip = match self.source {
            Source::Face(_) => -1.0,
            Source::Embroidery(_) => 1.0,
        };
        Transform {
            a: scale,
            b: 0.0,
            c: 0.0,
            d: flip * scale,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// The outline and stitches of `glyph`; `Err` when it is over [`MAX_GLYPH_SEGMENTS`].
    fn outline(&self, glyph: u32) -> Result<Option<Outline>, ()> {
        match &self.source {
            Source::Face(f) => {
                let mut b = Builder::default();
                // Glyph data is untrusted: a parser panic loses the glyph, not the application.
                let outlined = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    f.outline_glyph(GlyphId(glyph as u16), &mut b).is_some()
                }));
                if !matches!(outlined, Ok(true)) {
                    return Ok(None);
                }
                if b.too_complex() {
                    return Err(());
                }
                Ok(Some(Outline {
                    segments: b.segments,
                    contours: b.contours,
                    stitches: None,
                }))
            }
            Source::Embroidery(e) => {
                let Some(g) = e.glyphs.get(glyph as usize) else {
                    return Ok(None);
                };
                let segments: usize = g.contours.iter().map(Contour::segment_count).sum();
                if segments > MAX_GLYPH_SEGMENTS {
                    return Err(());
                }
                Ok(
                    (!g.contours.is_empty() || g.stitches.is_some()).then(|| Outline {
                        contours: g.contours.clone(),
                        stitches: g.stitches.clone(),
                        segments: segments + g.stitches.as_ref().map_or(0, |b| b.stitches.len()),
                    }),
                )
            }
        }
    }
}

/// The outline of `ch` in font `name` set at `size` mm, in 0.1 mm with the origin on the
/// baseline at the pen position (+y down), and its advance (0.1 mm); a blank glyph (space)
/// has no contours. `None` when the font has no glyph for it. Font Creator converts TrueType
/// glyphs with this.
pub fn glyph_outline(
    name: &str,
    fonts: &[DesignFont],
    ch: char,
    size: f64,
) -> Result<Option<(Vec<Contour>, f64)>, TextError> {
    let font = LoadedFont::load_in(name, fonts)?;
    let Some(glyph) = font.glyph(ch) else {
        return Ok(None);
    };
    let scale = size * font.unit_per_mm;
    let advance = font.advance(Some(glyph)) * scale;
    let at = font.units(scale);
    match font.outline(glyph) {
        Ok(Some(outline)) => Ok(Some((
            outline
                .contours
                .iter()
                .map(|c| c.transformed(&at))
                .collect(),
            advance,
        ))),
        Ok(None) => Ok(Some((Vec::new(), advance))),
        Err(()) => Ok(None),
    }
}

/// Where each character's glyph sits: font-unit origin → design transform.
struct Placed {
    ch: char,
    glyph: Option<u32>,
    at: Transform,
}

/// Lays `spec` out with a bundled font; see [`layout_in`].
pub fn layout(spec: &TextSpec, path: Option<&Contour>) -> Result<Layout, TextError> {
    layout_in(spec, path, &[])
}

/// Lays `spec` out into glyph contours and stitches (0.1 mm, `spec.transform` applied), its
/// font taken from the bundled fonts or `fonts`. `path` is the contour of the path object when
/// the text is attached to one; lines are then joined by spaces.
pub fn layout_in(
    spec: &TextSpec,
    path: Option<&Contour>,
    fonts: &[DesignFont],
) -> Result<Layout, TextError> {
    if let (Some(nd), None) = (&spec.name_drop, path) {
        // Laid out untransformed, fitted into the box, then placed.
        let plain = TextSpec {
            name_drop: None,
            transform: Transform::IDENTITY,
            ..spec.clone()
        };
        let mut l = layout_in(&plain, None, fonts)?;
        let points = l
            .glyphs
            .iter()
            .flat_map(|g| {
                g.contours.iter().flat_map(Contour::outline_points).chain(
                    g.stitches
                        .iter()
                        .flat_map(|b| b.stitches.iter().map(|s| v2(s.x, s.y))),
                )
            })
            .collect::<Vec<_>>();
        if let Some(b) = Rect::enclosing(points) {
            let t = nd.fit(&b).then(&spec.transform);
            for g in &mut l.glyphs {
                for c in &mut g.contours {
                    *c = c.transformed(&t);
                }
                if let Some(block) = &mut g.stitches {
                    for st in &mut block.stitches {
                        let p = t.apply(v2(st.x, st.y));
                        (st.x, st.y) = (p.x, p.y);
                    }
                }
            }
        }
        return Ok(l);
    }
    let font = LoadedFont::load_in(&spec.font, fonts)?;
    let scale = spec.size * font.unit_per_mm;
    let spacing = spec.letter_spacing * 10.0;
    let line_height = font.line_height() * scale + spec.line_spacing * 10.0;
    let mut missing = Vec::new();
    let mut shaping = Vec::new();

    // Pen advance of each character, per line.
    let mut lines: Vec<Vec<(char, Option<u32>, f64)>> = Vec::new();
    let source: String = if path.is_some() {
        spec.text.replace(['\r', '\n'], " ")
    } else {
        spec.text.replace("\r\n", "\n")
    };
    for line in source.split('\n') {
        let mut out = Vec::new();
        for ch in line.chars() {
            let ch = if ch == '\t' { ' ' } else { ch };
            if ch.is_control() {
                continue;
            }
            if let Some(note) = shaping_note(ch)
                && !shaping.contains(&note)
            {
                shaping.push(note);
            }
            let glyph = font.glyph(ch);
            if glyph.is_none() && ch != ' ' && !missing.contains(&ch) {
                missing.push(ch);
            }
            out.push((ch, glyph, font.advance(glyph) * scale));
        }
        lines.push(out);
    }

    let width = |line: &[(char, Option<u32>, f64)]| -> f64 {
        line.iter().map(|g| g.2).sum::<f64>() + spacing * line.len().saturating_sub(1) as f64
    };

    let glyph_scale = font.units(scale);
    let mut placed = Vec::new();
    match path.map(PathWalker::new) {
        Some(walker) => {
            let line = lines.concat();
            let total = width(&line);
            let mut pen = match spec.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => (walker.length - total) / 2.0,
                TextAlign::Right => walker.length - total,
            };
            for (ch, glyph, advance) in line {
                let (p, angle) = walker.at(pen + advance / 2.0);
                let at = glyph_scale
                    .then(&Transform::translation(-advance / 2.0, 0.0))
                    .then(&Transform::rotation(angle.to_degrees(), v2(0.0, 0.0)))
                    .then(&Transform::translation(p.x, p.y));
                placed.push(Placed { ch, glyph, at });
                pen += advance + spacing;
            }
        }
        None => {
            for (row, line) in lines.iter().enumerate() {
                let mut pen = match spec.align {
                    TextAlign::Left => 0.0,
                    TextAlign::Center => -width(line) / 2.0,
                    TextAlign::Right => -width(line),
                };
                let baseline = row as f64 * line_height;
                for &(ch, glyph, advance) in line {
                    let at = glyph_scale.then(&Transform::translation(pen, baseline));
                    placed.push(Placed { ch, glyph, at });
                    pen += advance + spacing;
                }
            }
        }
    }

    let mut glyphs = Vec::new();
    let mut segments = 0usize;
    for g in placed {
        let Some(glyph) = g.glyph else { continue };
        let outline = match font.outline(glyph) {
            Ok(Some(o)) => o,
            Ok(None) => continue,
            Err(()) => {
                if !missing.contains(&g.ch) {
                    missing.push(g.ch);
                }
                continue;
            }
        };
        segments += outline.segments;
        if segments > MAX_LAYOUT_SEGMENTS {
            return Err(TextError::TooComplex);
        }
        let at = g.at.then(&spec.transform);
        let contours: Vec<Contour> = outline
            .contours
            .iter()
            .map(|c| c.transformed(&at))
            .collect();
        let stitches = outline.stitches.map(|mut b| {
            for s in &mut b.stitches {
                let p = at.apply(v2(s.x, s.y));
                (s.x, s.y) = (p.x, p.y);
            }
            b
        });
        glyphs.push(Glyph {
            ch: g.ch,
            contours,
            stitches,
        });
    }
    Ok(Layout {
        glyphs,
        missing,
        shaping,
    })
}

/// Arc-length lookup along a flattened contour.
struct PathWalker {
    points: Vec<Vec2>,
    /// Cumulative length at each point.
    at: Vec<f64>,
    length: f64,
}

impl PathWalker {
    fn new(contour: &Contour) -> Self {
        let mut points = contour.path();
        points.dedup();
        let mut at = Vec::with_capacity(points.len());
        let mut walked = 0.0;
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                walked += points[i - 1].distance(*p);
            }
            at.push(walked);
        }
        PathWalker {
            points,
            at,
            length: walked,
        }
    }

    /// Point and tangent angle (radians) at arc length `s`; beyond either end the path is
    /// extended along its end tangent.
    fn at(&self, s: f64) -> (Vec2, f64) {
        let n = self.points.len();
        if n < 2 {
            return (self.points.first().copied().unwrap_or_default(), 0.0);
        }
        let seg = match self.at.partition_point(|&a| a <= s) {
            0 => 0,
            k => (k - 1).min(n - 2),
        };
        let (a, b) = (self.points[seg], self.points[seg + 1]);
        let len = self.at[seg + 1] - self.at[seg];
        let d = b - a;
        let angle = d.y.atan2(d.x);
        let p = if len > 0.0 {
            a + d * ((s - self.at[seg]) / len)
        } else {
            a
        };
        (p, angle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::contains;
    use crate::model::{Design, DesignObject, RegionSewing, RegionType};
    use crate::{Compiler, generate};
    use openstitch_core::{Command, Thread};

    fn lay(text: &str, size: f64) -> Layout {
        layout(&TextSpec::new(text, DEFAULT_FONT, size), None).expect("layout")
    }

    fn bounds(contours: &[Contour]) -> Rect {
        Rect::enclosing(contours.iter().flat_map(Contour::outline_points)).expect("bounds")
    }

    #[test]
    fn bundled_fonts_parse_and_cover_ascii() {
        for f in &FONTS {
            let font = LoadedFont::load(f.id).expect(f.id);
            assert!(font.glyph_count() > 90, "{}", f.id);
            assert!((' '..='~').all(|c| font.has_char(c)), "{}", f.id);
        }
        assert!(font("Lato").is_some() && font("lato bold").is_some());
        assert!(font("Comic Sans").is_none());
    }

    #[test]
    fn quadratic_segments_become_exact_cubics() {
        let mut b = Builder::default();
        b.move_to(0.0, 0.0);
        b.quad_to(10.0, 20.0, 20.0, 0.0);
        b.line_to(0.0, 0.0);
        b.close();
        let c = &b.contours[0];
        assert_eq!(c.points, vec![v2(0.0, 0.0), v2(20.0, 0.0)]);
        let bz = c.curve(0).expect("curve");
        // Cubic at t = 1/2 must equal the quadratic's point (10, 10).
        let mid = (v2(0.0, 0.0) + bz.c1 * 3.0 + bz.c2 * 3.0 + v2(20.0, 0.0)) * 0.125;
        assert!(mid.distance(v2(10.0, 10.0)) < 1e-9, "{mid:?}");
        assert_eq!(c.curve(1), None, "closing line stays straight");
    }

    #[test]
    fn glyph_outlines_have_the_expected_contours() {
        for (ch, n) in [
            ('I', 1),
            ('L', 1),
            ('O', 2),
            ('o', 2),
            ('B', 3),
            ('i', 2),
            ('8', 3),
        ] {
            let l = lay(&ch.to_string(), 10.0);
            assert_eq!(l.glyphs.len(), 1, "{ch}");
            assert_eq!(l.glyphs[0].contours.len(), n, "{ch}");
            assert!(l.glyphs[0].contours.iter().all(|c| c.closed), "{ch}");
        }
        // Capital height is the size: 'H' sits on the baseline (y = 0) and is 10 mm tall.
        let h = bounds(&lay("H", 10.0).contours());
        assert!(h.max_y.abs() < 0.5, "{h:?}");
        assert!((h.height() - 100.0).abs() < 0.5, "{h:?}");
        // 'O' is round: its outline has curves; 'I' is a rectangle without any.
        assert!(lay("O", 10.0).glyphs[0].contours[0].has_curves());
        assert!(!lay("I", 10.0).glyphs[0].contours[0].has_curves());
    }

    #[test]
    fn counters_are_holes_under_even_odd_fill() {
        let o = lay("O", 20.0).contours();
        let b = bounds(&o);
        assert!(!contains(b.center(), &o), "counter of O is a hole");
        let ring = v2(b.min_x + 10.0, b.center().y);
        assert!(contains(ring, &o), "stroke of O is inside");

        let mut d = Design::default();
        let (obj, _) = DesignObject::text(
            TextSpec::new("O", DEFAULT_FONT, 20.0),
            Some(RegionSewing::new(RegionType::Fill, Thread::rgb(0))),
            None,
        )
        .expect("text");
        let inner = obj.contours[1].clone();
        d.add(obj);
        let p = Compiler::default().compile(&d);
        assert!(p.count(Command::Stitch) > 50);
        let edges = inner.edges();
        // Span ends lie on the counter's edge; nothing may sit strictly inside it.
        let inside_counter = p
            .stitches
            .iter()
            .filter(|s| s.command == Command::Stitch)
            .filter(|s| contains(v2(s.x, s.y), std::slice::from_ref(&inner)))
            .map(|s| {
                let q = v2(s.x, s.y);
                edges
                    .iter()
                    .map(|&(a, b)| {
                        let ab = b - a;
                        let t = (((q - a).x * ab.x + (q - a).y * ab.y)
                            / (ab.x * ab.x + ab.y * ab.y))
                            .clamp(0.0, 1.0);
                        q.distance(a + ab * t)
                    })
                    .fold(f64::INFINITY, f64::min)
            })
            .filter(|&d| d > 0.05)
            .count();
        assert_eq!(inside_counter, 0, "no needle point inside the counter of O");
    }

    #[test]
    fn size_scales_linearly() {
        let small = bounds(&lay("Hag", 10.0).contours());
        let big = bounds(&lay("Hag", 25.0).contours());
        assert!((big.width() / small.width() - 2.5).abs() < 1e-6);
        assert!((big.height() / small.height() - 2.5).abs() < 1e-6);
    }

    #[test]
    fn letter_spacing_lines_and_alignment() {
        let tight = bounds(&lay("II", 10.0).contours());
        let mut spec = TextSpec::new("II", DEFAULT_FONT, 10.0);
        spec.letter_spacing = 3.0;
        let wide = bounds(&layout(&spec, None).expect("layout").contours());
        assert!((wide.width() - tight.width() - 30.0).abs() < 1e-6);

        let mut spec = TextSpec::new("H\nH", DEFAULT_FONT, 10.0);
        spec.align = TextAlign::Center;
        let two = layout(&spec, None).expect("layout");
        let (a, b) = (
            bounds(&two.glyphs[0].contours),
            bounds(&two.glyphs[1].contours),
        );
        assert!(b.min_y - a.min_y > 100.0, "second line is lower");
        assert!(a.center().x.abs() < 0.5, "centred on x = 0");
        spec.line_spacing = 5.0;
        let spaced = layout(&spec, None).expect("layout");
        let b2 = bounds(&spaced.glyphs[1].contours);
        assert!((b2.min_y - b.min_y - 50.0).abs() < 1e-6);
    }

    #[test]
    fn missing_characters_are_reported_and_skipped() {
        let l = lay("A\u{4E2D}B\u{4E2D}", 10.0);
        assert_eq!(l.missing, vec!['\u{4E2D}']);
        assert_eq!(
            l.glyphs.iter().map(|g| g.ch).collect::<String>(),
            "AB".to_string()
        );
        assert!(lay(" \t ", 10.0).glyphs.is_empty());
    }

    #[test]
    fn unknown_font_is_an_error() {
        let e = layout(&TextSpec::new("A", "nope", 10.0), None).expect_err("unknown");
        assert_eq!(e, TextError::UnknownFont("nope".into()));
    }

    #[test]
    fn text_on_a_straight_path_matches_straight_text() {
        let straight = lay("Ab", 10.0).contours();
        let path = Contour::new(vec![v2(0.0, 300.0), v2(2000.0, 300.0)], false);
        let on = layout(&TextSpec::new("Ab", DEFAULT_FONT, 10.0), Some(&path))
            .expect("layout")
            .contours();
        let shifted: Vec<Contour> = straight
            .iter()
            .map(|c| c.transformed(&Transform::translation(0.0, 300.0)))
            .collect();
        for (a, b) in on.iter().zip(&shifted) {
            for (p, q) in a.points.iter().zip(&b.points) {
                assert!(p.distance(*q) < 1e-6);
            }
        }
    }

    #[test]
    fn text_on_a_circle_follows_it() {
        let center = v2(500.0, 500.0);
        let r = 300.0;
        let circle = Contour::ellipse(center, r, r, 180);
        let spec = TextSpec::new("OPENSTITCH", DEFAULT_FONT, 8.0);
        let l = layout(&spec, Some(&circle)).expect("layout");
        assert_eq!(l.glyphs.len(), 10);
        let mut last_angle = f64::NEG_INFINITY;
        for g in &l.glyphs {
            let c = bounds(&g.contours).center() - center;
            // Glyphs stand on the circle, up = left of travel = outside... or inside
            // depending on direction; either way within one cap height of the circle.
            let d = c.length();
            assert!((d - r).abs() < 80.0, "{} at {d}", g.ch);
            let angle = c.y.atan2(c.x);
            assert!(angle > last_angle, "glyphs advance along the circle");
            last_angle = angle;
        }
        // Vertical path: an 'I' turned to run along +y is wider than tall.
        let down = Contour::new(vec![v2(0.0, 0.0), v2(0.0, 1000.0)], false);
        let i = bounds(
            &layout(&TextSpec::new("I", DEFAULT_FONT, 10.0), Some(&down))
                .expect("layout")
                .contours(),
        );
        assert!(i.width() > 90.0 && i.height() < 40.0, "{i:?}");
    }

    #[test]
    fn script_font_fills_each_glyph_on_its_own() {
        let (obj, layout) = DesignObject::text(
            TextSpec::new("Hello", "pacifico", 15.0),
            Some(RegionSewing::new(RegionType::Fill, Thread::rgb(0))),
            None,
        )
        .expect("text");
        assert_eq!(layout.glyphs.len(), 5);
        assert_eq!(obj.glyph_groups().len(), 5);
        let closed: Vec<Contour> = obj.glyph_groups()[1].to_vec();
        assert!(
            !generate::fill(
                &closed,
                &generate::RegionParams {
                    angle: 0.0,
                    row_spacing: 4.0,
                    stitch_length: 40.0,
                    pull_compensation: 0.0,
                },
                &[]
            )
            .is_empty()
        );
        let mut d = Design::default();
        d.add(obj);
        assert!(Compiler::default().compile(&d).count(Command::Stitch) > 100);
    }
}
