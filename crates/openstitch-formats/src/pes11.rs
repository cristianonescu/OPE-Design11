//! PE-DESIGN 11 (`#PES0110`) editable objects: the object list after the header, as far as the
//! owner's PE-DESIGN output files map it (docs/FILE_FORMATS.md, "PES 0110 object list").
//!
//! The list is a u16 object count, then MFC `CArchive` objects: an object of a class not seen
//! before starts with `FFFF`, a u16 schema and the u16-length class name; a later object of an
//! already named class starts with `0x8000 | n`, where n is the class's entry in the archive's
//! map. MFC numbers classes and objects in one sequence from 1 (each new class takes the next
//! number, then each object), so after a text and its letters a class's number is not its
//! order of appearance. `CEmbLine` (shapes and lines drawn with the shape and line tools) and
//! `CEmbNText` with its `CTTFLetter`s (text from the Text tool) are read. Reading stops at the
//! first other class, or at a `CEmbLine` whose geometry doesn't fit its bounds (a variant not
//! mapped yet; Manual Punch blocks are read, see [`Shape::punch`]), and records the class in
//! [`super::ObjectSection::unparsed`].
//!
//! [`write`] is the inverse: a `#PES0110` file whose object list holds the same `CEmbLine`
//! records, thread table, groups and sewing-order list, laid out as PE-DESIGN 11.50 lays them
//! out (`read_objects(write(o)) == o`), with the PEC section the machine sews from. Fields not
//! mapped yet are written with the values of the owner's baseline file (`pes11_template`).

use crate::io::{ByteReader, ByteWriter};
use crate::{FormatError, Result, pec};
use openstitch_core::{
    BROTHER_EMBROIDERY_BRAND, OPTION_COLOR_BRAND, OPTION_COLOR_KIND, PEC_CHART, StitchPattern,
    Thread, VER4_OPTION_COLORS, brother_embroidery_code, nearest_index, pec_basic_chart,
    round_coordinate,
};
use std::borrow::Cow;

#[path = "pes11_template.rs"]
mod template;

const EMB_LINE: &str = "CEmbLine";
/// A shape from the Shapes gallery: a `CEmbLine` record under its own class name (o-gallery-heart).
const EMB_SPECIAL: &str = "CEmbSpecialShape";
const GROUP_OBJECT: &str = "CGroupObject";
/// A stitch group (a stitch pattern: an imported file, a converted shape) and its runs.
const EMB_ONE: &str = "CEmbOne";
const SEW_SEG: &str = "CSewSeg";
const EMB_TEXT: &str = "CEmbNText";
/// Text with a Text Transform: a `CEmbNText` with the transform after its tail.
const EMB_TEXT_TRANSFORM: &str = "CEmbDText";
/// Text fitted to an outline: a `CEmbNText` whose tail ends with the fit and the path.
const EMB_TEXT_FIT: &str = "CEmbAText";
const TTF_LETTER: &str = "CTTFLetter";
/// Bytes of a `CEmbDText`'s Text Transform after its tail.
const ENVELOPE: usize = 22;
/// Bytes of a `CTTFLetter` between its character code and its font name.
const LETTER_ATTRS: usize = 148;
/// Bytes of a `CEmbLine` body before its geometry with PE-DESIGN's default pattern block (53
/// polylines); the block is as long as the programmable pattern it holds (m01-straight-1: one
/// polyline, 462 bytes in all).
const LINE_BODY: usize = 4432;
/// The geometry header's three marks after its two lists.
const GEOMETRY_MARKS: usize = 52;
const VERTEX: usize = 28;
/// Where the body's pattern block starts and ends (`pattern_block`).
#[cfg(test)]
const PATTERN_START: usize = 386;
const PATTERN_END: usize = LINE_BODY - template::BODY_TAIL.len();

/// File units are 0.1 mm, and the page's top-left corner is at this offset on both axes in
/// every owner file (100 × 100 mm and 200 × 200 mm pages alike).
pub const PAGE_ORIGIN: f32 = 500.0;

/// Thread-table colour type of the Applique Wizard's colours: catalogue number `100` (Applique
/// Material, PEC 62), `101` (Applique Position, PEC 63) and `102` (Applique, PEC 64), RGB 0, no
/// chart index, brand or chart (a01-rect-applique … a05). PE-DESIGN's chart colours are 10.
pub const APPLIQUE_KIND: u32 = 2;

/// Region sew type (`CEmbLine` body +134).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionSewType {
    Satin,
    Fill,
    ProgrammableFill,
    /// Region "Not Sewn" (code 11): open lines, outlines with the region off, and the inner
    /// copy of a hole-sewing pair.
    NotSewn,
    /// A value not mapped yet.
    Other(u16),
}

impl RegionSewType {
    fn from_raw(v: u16) -> Self {
        match v {
            0 => Self::Satin,
            1 => Self::Fill,
            2 => Self::ProgrammableFill,
            11 => Self::NotSewn,
            v => Self::Other(v),
        }
    }

    fn raw(self) -> u16 {
        match self {
            Self::Satin => 0,
            Self::Fill => 1,
            Self::ProgrammableFill => 2,
            Self::NotSewn => 11,
            Self::Other(v) => v,
        }
    }
}

/// Vertex kind bit: the segment after the vertex is a curve (its first handle is `handles[1]`).
pub const KIND_CURVE_AFTER: u32 = 0x01;
/// Vertex kind bit: the segment before the vertex is a curve (its last handle is `handles[0]`).
pub const KIND_CURVE_BEFORE: u32 = 0x10;

/// One outline vertex in file units: 0.1 mm with the page's top-left corner at
/// ([`PAGE_ORIGIN`], [`PAGE_ORIGIN`]). `handles[0]` is the cubic Bézier handle of the segment
/// ending at the vertex and `handles[1]` the handle of the segment leaving it; a handle of a
/// straight segment is `(0, 0)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub handles: [(f32, f32); 2],
    /// [`KIND_CURVE_BEFORE`] | [`KIND_CURVE_AFTER`]: 0 a corner between straight segments,
    /// `0x11` a smooth curve point (the circle's four vertices).
    pub kind: u32,
}

/// What [`read_objects`] found after the header strings, and what [`write`] takes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Objects {
    /// The Design Page size in mm (header), when the header had the mapped layout.
    pub page_mm: Option<(u16, u16)>,
    /// The page is a Hoop Size page (header page mode 0), not a Custom Size page (1). PE-DESIGN
    /// shows a Custom Size page with a Hoop Settings tab and section outlines; a new design in
    /// PE-DESIGN starts on a Hoop Size page.
    pub hoop_page: bool,
    /// The header fields between the metadata strings and the thread table, as read: written
    /// back verbatim when present (hoop and machine fields not all mapped yet), so a PE-DESIGN
    /// file keeps its page and hoop; `None` writes the mapped templates from `page_mm` and
    /// `hoop_page`.
    pub header: Option<Vec<u8>>,
    /// The two characters after the signature's `01 00`: `10` in the multi-needle files, `01`
    /// in the single-needle ones (o-machine-single-100x100, PE-DESIGN's re-saves of version 6
    /// files); `None` writes `10`.
    pub code: Option<[u8; 2]>,
    /// The thread table, in its file order (the shapes' colour indexes point into it).
    pub threads: Vec<Thread>,
    pub shapes: Vec<Shape>,
    /// Texts (`CEmbNText`), each at its [`Text::position`] in the object list; the shapes fill
    /// the positions no text or stitch group takes, in order.
    pub texts: Vec<Text>,
    /// Stitch groups (`CEmbOne`), each at its [`StitchGroup::position`].
    pub stitch_groups: Vec<StitchGroup>,
    /// Groups (`CGroupObject` after the list): each the positions of its member objects.
    pub groups: Vec<Vec<u16>>,
    /// The first object class that was not read, if any.
    pub unparsed: Option<String>,
}

/// A `CEmbLine`: an outline with line sewing and, when closed, region sewing.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// `x0, y0, x1, y1` in file units.
    pub bounds: [i16; 4],
    pub closed: bool,
    pub line_sewn: bool,
    /// Line sew type code: 0 Zigzag (PE's default), 1 Running, 3 E or V stitch, 4 Triple,
    /// 9 while not sewn. Other codes, and how E and V differ, are not mapped yet.
    pub line_type: u16,
    /// Line colour: an index into [`Objects::threads`].
    pub line_color: u16,
    /// Zigzag width, mm.
    pub zigzag_width: f32,
    /// Zigzag density, line/mm.
    pub zigzag_density: f32,
    /// E or V stitch width, mm (body +90; PE-DESIGN's default 2.5, the Applique Wizard's
    /// tack-down 2.0: line-e-default, line-v-default, a01-rect-applique). An E or V line's sewn
    /// extent is the bounds plus this width.
    pub ev_width: f32,
    /// Running stitch pitch, mm. Kept while another line type is chosen, as PE's tool keeps it.
    pub run_pitch: f32,
    pub line_under_sewing: bool,
    pub region: RegionSewType,
    /// Region colour: an index into [`Objects::threads`].
    pub region_color: u16,
    /// Region density, line/mm.
    pub density: f32,
    pub gradation: bool,
    /// Region sewing direction, degrees.
    pub direction: u16,
    /// Fill Frequency, %.
    pub frequency: u16,
    pub under_sewing: bool,
    /// Running Stitch Path: 0 Inside, 1 Along outline, 2 On outline.
    pub running_path: u16,
    /// Region under sewing type, PE-DESIGN's code (body +182): 2 for its default type, 4, 0, 1, 3
    /// for the second to fifth (satin-col-us-type-2 … -5); 0 on Convert to Blocks' blocks.
    pub under_sewing_type: u16,
    /// Body +178: 1 in PE-DESIGN's tool defaults, 0 in the owner's machine-* files and on Convert
    /// to Blocks' blocks (meaning not mapped).
    pub flag_178: u16,
    /// From the Shapes gallery (`CEmbSpecialShape`): the same record, kept under that class.
    pub gallery: bool,
    /// A Manual Punch block (Straight or Curve Block, and PE-DESIGN's Convert to Blocks):
    /// its sewing-direction lines in order, each as the vertices of its two ends, empty on every
    /// other shape. The Manual Punch tool's blocks join vertex k and 2n − 1 − k (the outline is
    /// the first ends in click order, then the second ends back: m01, m03, o-manual-*); Convert
    /// to Blocks also puts outline points between line ends (stroke-blocks-A: [0, 11], [1, 10],
    /// [4, 7], [5, 6]).
    pub punch: Vec<[u32; 2]>,
    /// The vertex a block's sewing ends at: one end of its last line (tail +4426; m01: 2,
    /// stroke-blocks-H's first block: 2 of [2, 3]). Not decided by the geometry.
    pub punch_exit: u16,
    /// Both of the geometry header's position lists empty: the running pieces PE-DESIGN's
    /// Convert to Blocks writes between blocks (stroke-blocks-H, -A). Every other shape has one
    /// position in each.
    pub no_positions: bool,
    pub vertices: Vec<Vertex>,
}

impl Shape {
    /// Line sew type codes (`line_type`).
    pub const LINE_ZIGZAG: u16 = 0;
    pub const LINE_RUNNING: u16 = 1;
    pub const LINE_E_OR_V: u16 = 3;
    pub const LINE_TRIPLE: u16 = 4;
    pub const LINE_NOT_SEWN: u16 = 9;
    /// The Applique Wizard's cutting line (Applique Material: Cutting, a02-applique-cutting):
    /// PE-DESIGN sews it with its cutting colour (PEC 65) whatever the line colour says.
    pub const LINE_CUTTING: u16 = 7;

    /// A shape with PE-DESIGN 11.50's default attributes (the baseline file r01: line Zigzag
    /// 2.0 mm at 4.5 line/mm with under sewing, run pitch 2.0 mm; region Fill at 4.5 line/mm,
    /// 45°, Frequency 30 %, under sewing on, Running Stitch Path Inside) and no geometry.
    pub fn with_defaults() -> Self {
        Shape {
            bounds: [0; 4],
            closed: true,
            line_sewn: true,
            line_type: Self::LINE_ZIGZAG,
            line_color: 0,
            zigzag_width: 2.0,
            zigzag_density: 4.5,
            ev_width: 2.5,
            run_pitch: 2.0,
            line_under_sewing: true,
            region: RegionSewType::Fill,
            region_color: 0,
            density: 4.5,
            gradation: false,
            direction: 45,
            frequency: 30,
            under_sewing: true,
            running_path: 0,
            under_sewing_type: 2,
            flag_178: 1,
            gallery: false,
            punch: Vec::new(),
            punch_exit: 0,
            no_positions: false,
            vertices: Vec::new(),
        }
    }

    /// The bounds of `vertices` (the vertices themselves; a curve's bulge beyond them is not
    /// included, so callers with curves pass the flattened outline's bounds instead).
    pub fn bounds_of(vertices: &[Vertex]) -> [i16; 4] {
        if vertices.is_empty() {
            return [0; 4];
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for v in vertices {
            x0 = x0.min(v.x);
            y0 = y0.min(v.y);
            x1 = x1.max(v.x);
            y1 = y1.max(v.y);
        }
        [x0, y0, x1, y1].map(|v| v.round() as i16)
    }
}

/// A `CEmbNText`: text from the Text tool. PE-DESIGN stores no outlines, only the characters,
/// the font and the attributes: it lays the text out again from the font when it opens the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    /// Index of the text in the object list (shapes and texts share it).
    pub position: u16,
    /// `x0, y0, x1, y1` of the laid-out letters in file units.
    pub bounds: [i16; 4],
    pub letters: Vec<Letter>,
    /// Indexes of the letters that start a new line (line breaks are not letters).
    pub breaks: Vec<u16>,
    /// Line spacing, % of the character height (PE-DESIGN's default 100).
    pub line_spacing: f32,
    /// Vertical text (Text Attributes, direction).
    pub vertical: bool,
    /// Moves applied after the text was placed: `a b c d tx ty`, as f32 (identity when none).
    pub transform: [f32; 6],
    /// Text Attributes alignment: 0 left, 1 centre (r12 and o-text-transform-1 left, the
    /// round-4 centred texts 1). Right is not mapped.
    pub align: u16,
    /// Where the Text tool was clicked, file units: the first baseline's alignment point (its
    /// left end for left-aligned text, its middle for centred text).
    pub origin: (f32, f32),
    /// Text Transform (`CEmbDText`), if any.
    pub envelope: Option<Envelope>,
    /// Character spacing (Text Attributes), 0.1 mm (tail +0; −10 at −1 mm, 20 at +2 mm in the
    /// text-fit references).
    pub char_spacing: f32,
    /// Fit Text to Outline (`CEmbAText`), if any. A fitted text stores no click point.
    pub fit: Option<Fit>,
}

/// Fit Text to Outline: the end of a `CEmbAText`'s tail (compat text-fit-*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fit {
    /// Position in the object list of the shape the text follows; the file stores the shape's
    /// archive number, so the shape comes before the text.
    pub path: u16,
    /// Horizontal alignment: 0 left, 1 centre, 2 right, 4 spaced evenly.
    pub halign: u16,
    /// Vertical alignment: 0 above the outline, 1 centred on it, 2 below it.
    pub valign: u16,
    /// Text orientation upright instead of along the outline.
    pub upright: bool,
    /// The other side of the outline.
    pub other_side: bool,
}

/// A Text Transform: the 22 bytes after a `CEmbDText`'s tail.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    /// PE-DESIGN's code of the preset (not its place in the palette: preset 1 is 24, 3 is 0).
    pub code: u16,
    /// The preset's strength, from the level slider (preset 3: 0 at level 0, 50 at the default,
    /// 100 at level 100).
    pub strength: f32,
    /// The arch presets' circle (radius, centre x, centre y) and a fourth value; the other
    /// presets leave the last arch's numbers there.
    pub geometry: [f32; 4],
}

/// One `CTTFLetter` of a [`Text`].
#[derive(Clone, Debug, PartialEq)]
pub struct Letter {
    /// The character, as a UTF-16 code unit.
    pub ch: u16,
    /// Text size when the text was made, mm (PE-DESIGN's size: 0.7 of the em).
    pub size: f32,
    /// Scale applied to the size later (Text Attributes size changed on a placed text: r13).
    pub scale: f32,
    /// Region density, line/mm.
    pub density: f32,
    /// Region colour: an index into [`Objects::threads`].
    pub region_color: u16,
    /// Region sew type: Satin (the Text tool's default) or Fill.
    pub region: RegionSewType,
    pub line_sewn: bool,
    /// Line sew type code as in [`Shape::line_type`] (9 while not sewn).
    pub line_type: u16,
    /// Line colour: an index into [`Objects::threads`].
    pub line_color: u16,
    /// TrueType family name (`Lato`).
    pub font: String,
    /// A u16 that is 700 in the owner's first text files and 900 in later ones (the same
    /// Lato Bold font); likely the font weight. Kept as read.
    pub weight: u16,
}

impl Letter {
    /// A letter in Lato Bold at the Text tool's defaults (r12): size 10 mm, Satin at
    /// 5.0 line/mm, line not sewn.
    pub fn with_defaults(ch: u16) -> Self {
        Letter {
            ch,
            size: 10.0,
            scale: 1.0,
            density: 5.0,
            region_color: 0,
            region: RegionSewType::Satin,
            line_sewn: false,
            line_type: Shape::LINE_NOT_SEWN,
            line_color: 0,
            font: "Lato".into(),
            weight: 700,
        }
    }
}

impl Text {
    /// Number of objects in `objects`' list.
    fn count(objects: &Objects) -> usize {
        objects.shapes.len() + objects.texts.len()
    }
}

/// A `CEmbOne` stitch group in a `#PES0110` file: fixed needle points, as in version 6 but with
/// the group's bounds filled in and a longer run header (pes-default-v6, pes-csewseg-trims,
/// pes-design-properties and sew-a-moved10, PE-DESIGN's re-saves of OpenStitch's v6 files).
#[derive(Clone, Debug, PartialEq)]
pub struct StitchGroup {
    /// Index of the group in the object list.
    pub position: u16,
    /// `x0, y0, x1, y1` of the needle points in file units.
    pub bounds: [i16; 4],
    /// `a b c d tx ty`: the translation is the bounds' bottom-left corner.
    pub transform: [f32; 6],
    pub width: u16,
    pub height: u16,
    /// The runs (`CSewSeg`): `flag` 0 stitches, 1 a jump; `color` a thread-table index; points
    /// relative to the bottom-left corner (y ≤ 0).
    pub segments: Vec<crate::pes::SewSegment>,
    /// `(first run, thread)` at each colour change.
    pub color_log: Vec<(u16, u16)>,
}

impl StitchGroup {
    /// The group of `pattern`'s needle points (0.1 mm, page coordinates without the file
    /// offset) at `position`, runs as the version 6 writer cuts them, each colour block's
    /// thread given by `thread` (a thread-table index for block k).
    pub fn from_pattern(
        pattern: &StitchPattern,
        position: u16,
        thread: impl Fn(usize) -> u16,
    ) -> Option<StitchGroup> {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for st in &pattern.stitches {
            let (x, y) = (round_coordinate(st.x) as f64, round_coordinate(st.y) as f64);
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        if pattern.stitches.is_empty() {
            return None;
        }
        let mut segments = crate::pes::segments(pattern, crate::pes::Version::V6, x0, y1);
        // The v6 writer numbers the colours by the pattern's own threads: map them.
        let mut block = 0;
        let mut color_log: Vec<(u16, u16)> = Vec::new();
        let mut last = None;
        for (k, seg) in segments.iter_mut().enumerate() {
            if last.is_some_and(|c| c != seg.color) {
                block += 1;
            }
            last = Some(seg.color);
            seg.color = thread(block);
            if color_log.last().is_none_or(|&(_, c)| c != seg.color) {
                color_log.push((k as u16, seg.color));
            }
        }
        let o = f64::from(PAGE_ORIGIN);
        let file = |v: f64| (v + o).round().clamp(-32768.0, 32767.0) as i16;
        Some(StitchGroup {
            position,
            bounds: [file(x0), file(y0), file(x1), file(y1)],
            transform: [1.0, 0.0, 0.0, 1.0, (x0 + o) as f32, (y1 + o) as f32],
            width: (x1 - x0).round().clamp(0.0, 65535.0) as u16,
            height: (y1 - y0).round().clamp(0.0, 65535.0) as u16,
            segments,
            color_log,
        })
    }
}

// MARK: Reading

/// Locates the object list after the header strings: the u16 count right before the first
/// `FFFF 0000` + class name. The fields between (hoop, page, thread table) are not all mapped.
fn list_offset(data: &[u8], from: usize, end: usize) -> Option<usize> {
    let end = end.min(data.len());
    (from.max(2)..end.saturating_sub(8))
        .find(|&i| {
            data[i..i + 4] == [0xFF, 0xFF, 0, 0] && {
                let n = usize::from(u16::from_le_bytes([data[i + 4], data[i + 5]]));
                (5..32).contains(&n)
                    && data
                        .get(i + 6..i + 6 + n)
                        .is_some_and(|s| s[0] == b'C' && s.iter().all(u8::is_ascii_alphabetic))
            }
        })
        .map(|i| i - 2)
}

/// The Design Page size from the header: after the metadata strings come 14 bytes (hoop
/// fields), a u8-length machine string, a u16, two optional u16 fields (each a u16 presence
/// flag, then the value when the flag is 1), a u32 and then the page width and height in mm.
fn read_page(data: &[u8], from: usize) -> Option<(u16, u16)> {
    let n = usize::from(*data.get(from + 14)?);
    let mut at = from + 15 + n + 2;
    for _ in 0..2 {
        let flag = data.get(at..at + 2)?;
        at += if flag == [1, 0] { 4 } else { 2 };
    }
    at += 4;
    let b = data.get(at..at + 4)?;
    let (w, h) = (
        u16::from_le_bytes([b[0], b[1]]),
        u16::from_le_bytes([b[2], b[3]]),
    );
    (w > 0 && h > 0 && w <= 2000 && h <= 2000).then_some((w, h))
}

/// The thread table that ends right where the object list starts: u16 count, then per thread
/// the catalogue number, RGB and a pad byte, u32 colour type, the PEC chart index (as a
/// string), brand and chart name, strings as MFC strings. The fields between the header
/// strings and the table are not all mapped (their length varies: a machine-model string), so
/// this tries each start and keeps the longest table that parses to exactly `list`.
fn read_threads(data: &[u8], from: usize, list: usize) -> Option<(usize, Vec<Thread>)> {
    let parse = |start: usize| -> Result<Vec<Thread>> {
        let mut r = ByteReader::new(&data[..list]);
        r.offset = start;
        let count = r.u16le()?;
        let mut out = Vec::new();
        for _ in 0..count {
            let catalog = crate::pes::read_string8(&mut r)?;
            let rgb = r.read(4)?;
            let kind = r.u32le()?;
            // PE-DESIGN's own colours carry their PEC chart index here; a thread it kept from
            // another file keeps its name (pes-default-v6: Red, Black).
            let name = crate::pes::read_string8(&mut r)?;
            let brand = crate::pes::read_string8(&mut r)?;
            let chart = crate::pes::read_string8(&mut r)?;
            let mut t =
                Thread::rgb(u32::from(rgb[0]) << 16 | u32::from(rgb[1]) << 8 | u32::from(rgb[2]));
            t.catalog_number = catalog.map(Cow::Owned);
            t.name = name.map(Cow::Owned);
            t.brand = brand.map(Cow::Owned);
            t.chart = chart.map(Cow::Owned);
            t.kind = Some(kind).filter(|&k| k != Thread::CUSTOM_KIND);
            out.push(t);
        }
        if r.offset == list && count > 0 {
            Ok(out)
        } else {
            Err(FormatError::ValueOutOfRange(String::new()))
        }
    };
    // Several starts can parse to exactly `list`: a run of records inside the table looks like
    // a shorter table of its own. The real one is the longest (PE-DESIGN's 29 Ver4 Option
    // colours, `ver4-option-color-29.pes`, also parse as an 11-thread table starting mid-way).
    (from..list.saturating_sub(2))
        .filter_map(|s| parse(s).ok().map(|t| (s, t)))
        .max_by_key(|(s, t)| (t.len(), std::cmp::Reverse(*s)))
}

/// Reads the thread table and the object list's shapes. `from` is the offset after the header
/// strings, `end` the PEC offset.
pub(crate) fn read_objects(data: &[u8], from: usize, end: usize) -> Result<Objects> {
    let Some(at) = list_offset(data, from, end) else {
        return Ok(Objects::default());
    };
    let (threads, header) = match read_threads(data, from, at) {
        Some((start, threads)) => (threads, Some(data[from..start].to_vec())),
        None => (Vec::new(), None),
    };
    let page_mm = read_page(data, from);
    let hoop_page = data.get(from + 2..from + 4) == Some(&[0, 0][..]);
    let mut stitch_groups: Vec<StitchGroup> = Vec::new();
    let done = |shapes, texts, stitch_groups, groups, unparsed| {
        Ok(Objects {
            page_mm,
            hoop_page,
            header: header.clone(),
            code: data.get(14..16).map(|c| [c[0], c[1]]),
            threads: threads.clone(),
            shapes,
            texts,
            stitch_groups,
            groups,
            unparsed,
        })
    };
    let end = end.min(data.len());
    let mut r = ByteReader::new(&data[..end]);
    r.offset = at;
    let count = r.u16le()?;
    // The archive map: classes by name, objects as `None`, numbered from 1.
    let mut map: Vec<Option<String>> = Vec::new();
    let mut shapes = Vec::new();
    let mut texts = Vec::new();
    // (archive number, list position) of every object read, for the paths of fitted texts.
    let mut numbers: Vec<(u16, u16)> = Vec::new();
    for position in 0..count {
        let class = match read_tag(&mut r, &mut map)? {
            Ok(class) => class,
            Err(tag) => {
                return done(
                    shapes,
                    texts,
                    stitch_groups,
                    Vec::new(),
                    Some(format!("object tag {tag:#06x}")),
                );
            }
        };
        map.push(None);
        numbers.push((map.len() as u16, position as u16));
        if class == EMB_TEXT || class == EMB_TEXT_TRANSFORM || class == EMB_TEXT_FIT {
            match read_text(&mut r, &mut map, position as u16, &class, &numbers)? {
                Some(text) => texts.push(text),
                None => {
                    return done(
                        shapes,
                        texts,
                        stitch_groups,
                        Vec::new(),
                        Some(format!("{EMB_TEXT} (unmapped variant)")),
                    );
                }
            }
            continue;
        }
        if class == EMB_ONE {
            match read_stitch_group(&mut r, &mut map, position as u16)? {
                Some(g) => stitch_groups.push(g),
                None => {
                    return done(
                        shapes,
                        texts,
                        stitch_groups,
                        Vec::new(),
                        Some(format!("{EMB_ONE} (unmapped variant)")),
                    );
                }
            }
            continue;
        }
        if class != EMB_LINE && class != EMB_SPECIAL {
            return done(shapes, texts, stitch_groups, Vec::new(), Some(class));
        }
        match read_line(&mut r)? {
            Some(shape) => shapes.push(Shape {
                gallery: class == EMB_SPECIAL,
                ..shape
            }),
            None => {
                return done(
                    shapes,
                    texts,
                    stitch_groups,
                    Vec::new(),
                    Some(format!("{EMB_LINE} (unmapped variant)")),
                );
            }
        }
    }
    let groups = read_groups(&mut r, &mut map).unwrap_or_default();
    done(shapes, texts, stitch_groups, groups, None)
}

/// An MFC object tag: `Ok(class name)`, or `Err(tag)` for a tag that refers to no class.
fn read_tag(
    r: &mut ByteReader<'_>,
    map: &mut Vec<Option<String>>,
) -> Result<std::result::Result<String, u32>> {
    let tag = r.u16le()?;
    if tag == 0xFFFF {
        r.u16le()?; // schema
        let n = r.u16le()? as usize;
        let name = String::from_utf8_lossy(r.read(n)?).into_owned();
        map.push(Some(name.clone()));
        return Ok(Ok(name));
    }
    if tag & 0x8000 != 0
        && let Some(Some(c)) = ((tag & 0x7FFF) as usize)
            .checked_sub(1)
            .and_then(|i| map.get(i))
    {
        return Ok(Ok(c.clone()));
    }
    Ok(Err(tag))
}

/// The group list after the objects: 10 zero bytes, a u16 count, then per group its MFC tag,
/// a u16 (the `CGroupObject` class's archive number), the u16 member count and the members'
/// object indexes as u16. Anything else ends the list quietly.
fn read_groups(r: &mut ByteReader<'_>, map: &mut Vec<Option<String>>) -> Result<Vec<Vec<u16>>> {
    if r.read(10)?.iter().any(|&b| b != 0) {
        return Ok(Vec::new());
    }
    let count = r.u16le()?;
    let mut groups = Vec::new();
    for _ in 0..count {
        match read_tag(r, map)? {
            Ok(class) if class == GROUP_OBJECT => {}
            _ => return Ok(groups),
        }
        map.push(None);
        r.u16le()?; // the class's archive number
        let n = r.u16le()? as usize;
        let mut members = Vec::with_capacity(n);
        for _ in 0..n {
            members.push(r.u16le()? as u16);
        }
        groups.push(members);
    }
    Ok(groups)
}

/// One `CEmbOne` at the reader's offset: bounds, extent, transform, u16 1 0 0 width height,
/// 8 zero bytes, the u16 run count, the runs (each a `CSewSeg`: u16 flag, u16 thread, u16
/// `FFFF`, u32 point count, i16 x y points), then the colour log (u16 count, (run, thread)
/// pairs). `None` (reader unchanged) when a run is not a `CSewSeg`.
fn read_stitch_group(
    r: &mut ByteReader<'_>,
    map: &mut Vec<Option<String>>,
    position: u16,
) -> Result<Option<StitchGroup>> {
    let start = r.offset;
    let head = r.read(16)?;
    let bounds = [0, 1, 2, 3].map(|k| u16_at(head, 2 * k) as i16);
    let mut transform = [0_f32; 6];
    for v in &mut transform {
        *v = r.f32le()?;
    }
    let fields = r.read(10)?;
    let (width, height) = (u16_at(fields, 6), u16_at(fields, 8));
    r.skip(8)?;
    let n = r.u16le()? as usize;
    let mut segments = Vec::with_capacity(n);
    for _ in 0..n {
        match read_tag(r, map)? {
            Ok(class) if class == SEW_SEG => {}
            _ => {
                r.offset = start;
                return Ok(None);
            }
        }
        map.push(None);
        let flag = r.u16le()? as u16;
        let color = r.u16le()? as u16;
        r.u16le()?; // FFFF
        let count = r.u32le()? as usize;
        if count.saturating_mul(4) > r.remaining() {
            r.offset = start;
            return Ok(None);
        }
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            let (x, y) = (r.u16le()? as u16 as i16, r.u16le()? as u16 as i16);
            points.push((x, y));
        }
        segments.push(crate::pes::SewSegment {
            flag,
            color,
            points,
        });
    }
    let logs = r.u16le()? as usize;
    let mut color_log = Vec::with_capacity(logs);
    for _ in 0..logs {
        color_log.push((r.u16le()? as u16, r.u16le()? as u16));
    }
    Ok(Some(StitchGroup {
        position,
        bounds,
        transform,
        width,
        height,
        segments,
        color_log,
    }))
}

fn f32_at(b: &[u8], i: usize) -> f32 {
    f32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// One `CEmbNText` at the reader's offset: bounds, extent, transform, the u16 letter count,
/// the letters (each a `CTTFLetter`: char code, 148 attribute bytes, font name, u16 0), then
/// the line-break list and the text's own fields. `None` (reader unchanged) when a letter is
/// not a `CTTFLetter` or a field is out of range.
fn read_text(
    r: &mut ByteReader<'_>,
    map: &mut Vec<Option<String>>,
    position: u16,
    class: &str,
    numbers: &[(u16, u16)],
) -> Result<Option<Text>> {
    let transformed = class == EMB_TEXT_TRANSFORM;
    let fitted = class == EMB_TEXT_FIT;
    let start = r.offset;
    let fail = |r: &mut ByteReader<'_>| {
        r.offset = start;
        Ok(None)
    };
    let head = r.read(42)?;
    let bounds = [0, 1, 2, 3].map(|k| u16_at(head, 2 * k) as i16);
    let n = usize::from(u16_at(head, 40));
    let mut letters = Vec::with_capacity(n);
    for _ in 0..n {
        match read_tag(r, map)? {
            Ok(class) if class == TTF_LETTER => {}
            _ => return fail(r),
        }
        map.push(None);
        let ch = r.u16le()? as u16;
        let a = r.read(LETTER_ATTRS)?;
        let Some(font) = crate::pes::read_string8(r)? else {
            return fail(r);
        };
        r.u16le()?;
        letters.push(Letter {
            ch,
            size: f32_at(a, 2) / 10.0,
            scale: f32_at(a, 6),
            density: f32::from(u16_at(a, 22)) / 10.0,
            region_color: u16_at(a, 24),
            region: RegionSewType::from_raw(u16_at(a, 26)),
            line_sewn: u16_at(a, 60) != 0,
            line_type: u16_at(a, 62),
            line_color: u16_at(a, 64),
            font,
            weight: u16_at(a, 144),
        });
    }
    let breaks_n = r.u16le()? as usize;
    if breaks_n > n {
        return fail(r);
    }
    let mut breaks = Vec::with_capacity(breaks_n);
    for _ in 0..breaks_n {
        breaks.push(r.u16le()? as u16);
    }
    let tail = r.read(if fitted {
        template::FIT_TAIL.len()
    } else {
        template::TEXT_TAIL.len()
    })?;
    let fit = if fitted {
        let number = u16_at(tail, 118);
        let Some(&(_, path)) = numbers.iter().find(|&&(n, _)| n == number) else {
            return fail(r);
        };
        Some(Fit {
            path,
            halign: u16_at(tail, 104),
            valign: u16_at(tail, 106),
            upright: u16_at(tail, 108) != 0,
            other_side: u16_at(tail, 110) != 0,
        })
    } else {
        None
    };
    let mut transform = [0_f32; 6];
    for (k, v) in transform.iter_mut().enumerate() {
        *v = f32_at(tail, 78 + 4 * k);
    }
    let envelope = if transformed {
        let e = r.read(ENVELOPE)?;
        Some(Envelope {
            code: u16_at(e, 0),
            strength: f32_at(e, 2),
            geometry: [6, 10, 14, 18].map(|k| f32_at(e, k)),
        })
    } else {
        None
    };
    Ok(Some(Text {
        position,
        bounds,
        letters,
        breaks,
        line_spacing: f32_at(tail, 4),
        vertical: u16_at(tail, 10) != 0,
        transform,
        align: u16_at(tail, 8),
        origin: if fitted {
            (0.0, 0.0)
        } else {
            (f32_at(tail, 104), f32_at(tail, 108))
        },
        envelope,
        char_spacing: f32_at(tail, 0),
        fit,
    }))
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

/// One `CEmbLine` body at the reader's offset; `None` (reader unchanged) when it doesn't have
/// the mapped layout.
fn read_line(r: &mut ByteReader<'_>) -> Result<Option<Shape>> {
    let start = r.offset;
    let Some(len) = line_body_len(&r.bytes[start..]) else {
        return Ok(None);
    };
    if r.remaining() < len + 2 * 4 + GEOMETRY_MARKS + 4 {
        return Ok(None);
    }
    let body = r.read(len)?;
    let bounds = [0, 1, 2, 3].map(|k| u16_at(body, 2 * k) as i16);
    // The geometry header: two lists of outline positions (u32 count, then u32 vertex and f64
    // t each), then the three marks.
    let mut lists: [Vec<(u32, f64)>; 2] = [Vec::new(), Vec::new()];
    for list in &mut lists {
        let count = r.u32le()? as usize;
        if count.saturating_mul(12) > r.remaining() {
            r.offset = start;
            return Ok(None);
        }
        for _ in 0..count {
            let at = r.u32le()?;
            let t = f64::from_le_bytes(r.read(8)?.try_into().expect("8 bytes"));
            list.push((at, t));
        }
    }
    r.skip(GEOMETRY_MARKS)?;
    let n = r.u32le()? as usize;
    if n == 0 || n.saturating_mul(VERTEX) > r.remaining() {
        r.offset = start;
        return Ok(None);
    }
    let mut vertices = Vec::with_capacity(n);
    for _ in 0..n {
        let mut v = [0_f32; 6];
        for x in &mut v {
            *x = r.f32le()?;
        }
        vertices.push(Vertex {
            x: v[0],
            y: v[1],
            handles: [(v[2], v[3]), (v[4], v[5])],
            kind: r.u32le()?,
        });
    }
    // The vertices lie inside the bounds (to 1 unit): the check that the layout was right.
    let inside = vertices.iter().all(|v| {
        f32::from(bounds[0]) - 1.0 <= v.x
            && v.x <= f32::from(bounds[2]) + 1.0
            && f32::from(bounds[1]) - 1.0 <= v.y
            && v.y <= f32::from(bounds[3]) + 1.0
    });
    if !inside {
        r.offset = start;
        return Ok(None);
    }
    // One entry per list on a shape, none on Convert to Blocks' running pieces; a Manual
    // Punch block lists its lines' two ends, at t 0 (anything else is not mapped).
    let (punch, no_positions) = match &lists {
        [a, b] if a.len() == 1 && b.len() == 1 => (Vec::new(), false),
        [a, b] if a.is_empty() && b.is_empty() => (Vec::new(), true),
        [a, b] => {
            let block = a.len() >= 2
                && b.len() == a.len()
                && a.iter()
                    .chain(b)
                    .all(|&(v, t)| t == 0.0 && (v as usize) < n);
            if !block {
                r.offset = start;
                return Ok(None);
            }
            (
                a.iter().zip(b).map(|(&(p, _), &(q, _))| [p, q]).collect(),
                false,
            )
        }
    };
    let punch_exit = if punch.is_empty() {
        0
    } else {
        u16_at(body, body.len() - template::BODY_TAIL.len() + 66)
    };
    Ok(Some(Shape {
        bounds,
        closed: u16_at(body, 40) == 0,
        line_sewn: u16_at(body, 42) != 0,
        line_type: u16_at(body, 44),
        line_color: u16_at(body, 46),
        zigzag_width: f32::from(u16_at(body, 48)) / 10.0,
        zigzag_density: f32::from(u16_at(body, 50)) / 10.0,
        ev_width: f32::from(u16_at(body, 90)) / 10.0,
        run_pitch: f32::from(u16_at(body, 52)) / 10.0,
        line_under_sewing: u16_at(body, 60) != 0,
        region: RegionSewType::from_raw(u16_at(body, 134)),
        region_color: u16_at(body, 170),
        density: f32::from(u16_at(body, 136)) / 10.0,
        gradation: u16_at(body, 138) != 0,
        direction: u16_at(body, 168),
        frequency: u16_at(body, 174),
        under_sewing: u16_at(body, 180) != 0,
        running_path: u16_at(body, 186),
        under_sewing_type: u16_at(body, 182),
        flag_178: u16_at(body, 178),
        gallery: false,
        punch,
        punch_exit,
        no_positions,
        vertices,
    }))
}

/// The length of the `CEmbLine` body at the start of `data`: the 386 bytes before the pattern
/// block, the block (u16 size, u16 polyline count, per polyline six zero bytes, u16 1, u16
/// point count and the points as two u16 each) and the 72 bytes after it. `None` when the
/// block is not laid out that way.
fn line_body_len(data: &[u8]) -> Option<usize> {
    let head = PATTERN_END - template::PATTERN_BYTES;
    let at = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    // The pattern's size (10000 for PE-DESIGN's default; 750 and no polylines on the Applique
    // Wizard's covering line, a01-rect-applique's fifth object), then its polylines.
    at(head)?;
    let polylines = at(head + 2)?;
    let mut o = head + 4;
    for _ in 0..polylines {
        if at(o + 6)? != 1 {
            return None;
        }
        o += 10 + 4 * usize::from(at(o + 8)?);
    }
    let len = o + template::BODY_TAIL.len();
    (len <= data.len()).then_some(len)
}

// MARK: Writing

/// Writes a `#PES0110` file: the header with `pattern`'s metadata strings and the page size,
/// the thread table, one `CEmbLine` per shape, the groups and sewing-order list, then the PEC
/// section of `pattern` (the stitches the machine sews; PE-DESIGN regenerates its own from the
/// objects) and the colour addendum after it. `objects.unparsed` is ignored.
///
/// Fails when a shape has fewer than two vertices, a colour index points outside the thread
/// table, a group member outside the object list, or a count overflows its field.
pub fn write(objects: &Objects, pattern: &StitchPattern) -> Result<Vec<u8>> {
    if objects.threads.is_empty() || objects.threads.len() > usize::from(u16::MAX) {
        return Err(FormatError::ValueOutOfRange(format!(
            "PES 0110: {} threads (1 to 65535)",
            objects.threads.len()
        )));
    }
    let total = Text::count(objects) + objects.stitch_groups.len();
    if total == 0 || total > 0x3FFF {
        return Err(FormatError::ValueOutOfRange(format!(
            "PES 0110: {total} objects (1 to 16383)"
        )));
    }
    let mut at_position: Vec<Option<&Text>> = vec![None; total];
    let mut group_at: Vec<Option<&StitchGroup>> = vec![None; total];
    for g in &objects.stitch_groups {
        let slot = group_at.get_mut(usize::from(g.position)).ok_or_else(|| {
            FormatError::ValueOutOfRange(format!(
                "PES 0110: stitch group at position {}",
                g.position
            ))
        })?;
        if slot.replace(g).is_some() || at_position_has_text(objects, g.position) {
            return Err(FormatError::ValueOutOfRange(format!(
                "PES 0110: two objects at position {}",
                g.position
            )));
        }
        if g.segments.is_empty()
            || g.segments.len() > 0x7FFF
            || g.segments
                .iter()
                .any(|s| usize::from(s.color) >= objects.threads.len())
        {
            return Err(FormatError::ValueOutOfRange(format!(
                "PES 0110: stitch group at position {} has no runs or a thread outside the table",
                g.position
            )));
        }
    }
    for t in &objects.texts {
        let slot = at_position
            .get_mut(usize::from(t.position))
            .ok_or_else(|| {
                FormatError::ValueOutOfRange(format!("PES 0110: text at position {}", t.position))
            })?;
        if slot.replace(t).is_some() {
            return Err(FormatError::ValueOutOfRange(format!(
                "PES 0110: two texts at position {}",
                t.position
            )));
        }
        if t.letters.is_empty() || t.letters.len() > 0x3FFF {
            return Err(FormatError::ValueOutOfRange(format!(
                "PES 0110: text at position {} has {} letters",
                t.position,
                t.letters.len()
            )));
        }
        if t.fit.is_some_and(|f| f.path >= t.position) {
            return Err(FormatError::ValueOutOfRange(format!(
                "PES 0110: the text at position {} must follow an object written before it",
                t.position
            )));
        }
        if t.breaks.iter().any(|&b| usize::from(b) >= t.letters.len()) {
            return Err(FormatError::ValueOutOfRange(
                "PES 0110: line break after the last letter".into(),
            ));
        }
        for l in &t.letters {
            for c in [l.line_color, l.region_color] {
                if usize::from(c) >= objects.threads.len() {
                    return Err(FormatError::ValueOutOfRange(format!(
                        "PES 0110: text at position {} uses thread {c} of {}",
                        t.position,
                        objects.threads.len()
                    )));
                }
            }
        }
    }
    for (i, s) in objects.shapes.iter().enumerate() {
        if s.vertices.len() < 2 {
            return Err(FormatError::ValueOutOfRange(format!(
                "PES 0110: object {i} has {} vertices",
                s.vertices.len()
            )));
        }
        for c in [s.line_color, s.region_color] {
            if usize::from(c) >= objects.threads.len() {
                return Err(FormatError::ValueOutOfRange(format!(
                    "PES 0110: object {i} uses thread {c} of {}",
                    objects.threads.len()
                )));
            }
        }
    }
    for g in &objects.groups {
        if g.is_empty() || g.iter().any(|&m| usize::from(m) >= total) {
            return Err(FormatError::ValueOutOfRange(
                "PES 0110: group member outside the object list".into(),
            ));
        }
    }
    let pattern = pec::prepare(pattern);
    let mut w = ByteWriter::default();
    w.ascii("#PES0110");
    let pec_placeholder = w.len();
    w.u32le(0);
    w.u16le(1);
    // A file's own code as read (`10`, `01`, a02's `B1 30`); a design's is `10`, with bit 0x80
    // of the first character set when it has a sewn cutting line (`B1 30`: a02-applique-cutting,
    // the one owner file with a cutting line, against `10` or `01` in all 854 others).
    let cutting = objects
        .shapes
        .iter()
        .any(|s| s.line_sewn && s.line_type == Shape::LINE_CUTTING);
    let code = objects
        .code
        .unwrap_or(if cutting { [b'1' | 0x80, b'0'] } else { *b"10" });
    w.append(&code);
    for key in crate::pes::METADATA_KEYS {
        crate::pes::write_string8(&mut w, pattern.metadata.get(key).map(String::as_str));
    }
    let page = objects.page_mm.unwrap_or((100, 100));
    if let Some(h) = &objects.header {
        w.append(h);
    } else if objects.hoop_page && page != (100, 100) {
        return Err(FormatError::ValueOutOfRange(format!(
            "PES 0110: a Hoop Size page of {} x {} mm (only PE-DESIGN's 100 x 100 mm hoop page is mapped)",
            page.0, page.1
        )));
    } else {
        write_header(&mut w, page, objects.hoop_page);
    }
    write_threads(&mut w, &objects.threads);
    w.u16le(total as i64);
    let mut archive = Archive::default();
    let mut shapes = objects.shapes.iter();
    for (i, slot) in at_position.iter().enumerate() {
        if let Some(g) = group_at[i] {
            archive.tag(&mut w, EMB_ONE);
            archive.objects.push(archive.next);
            write_stitch_group(&mut w, &mut archive, g);
            continue;
        }
        match slot {
            Some(text) => {
                let class = match (&text.fit, &text.envelope) {
                    (Some(_), _) => EMB_TEXT_FIT,
                    (None, Some(_)) => EMB_TEXT_TRANSFORM,
                    (None, None) => EMB_TEXT,
                };
                archive.tag(&mut w, class);
                archive.objects.push(archive.next);
                write_text(&mut w, &mut archive, text)?;
            }
            None => {
                let shape = shapes.next().expect("one shape per free position");
                archive.tag(&mut w, if shape.gallery { EMB_SPECIAL } else { EMB_LINE });
                archive.objects.push(archive.next);
                write_line(&mut w, shape);
            }
        }
    }
    write_trailer(&mut w, &mut archive, objects, &at_position, &group_at);
    w.patch32le(pec_placeholder, w.len() as i64);
    let pec_start = w.len();
    let list = pec::write_section(&pattern, &mut w)?;
    patch_pec_header(&mut w.bytes[pec_start..], objects.hoop_page);
    write_addendum(&mut w, &list, &pattern.threads, objects.hoop_page);
    Ok(w.bytes)
}

/// The PEC header fields PE-DESIGN 11.50 writes with each page mode (all of its `#PES0110`
/// files in compat/reference, tests/fixtures/pe11 and the round-4 captures, 2026-10-29): 0x1F
/// `FF`; on a multi-needle Custom Size page 0x24–0x2F `20 FF 00 00 64 00 00 00 00 FF 20 20` and
/// 2900 × 1900 at 0x208 (663 files), on a Hoop Size page `20 20 20 20 64 20 00 20 00 FF 20 20`
/// and the design's size there, as the PEC writer puts it (46 files).
fn patch_pec_header(pec: &mut [u8], hoop_page: bool) {
    pec[0x1F] = 0xFF;
    if hoop_page {
        pec[0x24..0x30].copy_from_slice(&[
            0x20, 0x20, 0x20, 0x20, 0x64, 0x20, 0x00, 0x20, 0x00, 0xFF, 0x20, 0x20,
        ]);
        return;
    }
    pec[0x24..0x30].copy_from_slice(&[
        0x20, 0xFF, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x20, 0x20,
    ]);
    pec[0x208..0x20A].copy_from_slice(&2900_u16.to_le_bytes());
    pec[0x20A..0x20C].copy_from_slice(&1900_u16.to_le_bytes());
}

fn at_position_has_text(objects: &Objects, position: u16) -> bool {
    objects.texts.iter().any(|t| t.position == position)
}

/// One `CEmbOne` and its `CSewSeg` runs, as [`read_stitch_group`] reads them.
fn write_stitch_group(w: &mut ByteWriter, archive: &mut Archive, g: &StitchGroup) {
    for _ in 0..2 {
        for v in g.bounds {
            w.u16le(i64::from(v));
        }
    }
    for v in g.transform {
        w.f32le(f64::from(v));
    }
    for v in [1, 0, 0, g.width, g.height] {
        w.u16le(i64::from(v));
    }
    w.fill(0, 8);
    w.u16le(g.segments.len() as i64);
    for seg in &g.segments {
        archive.tag(w, SEW_SEG);
        w.u16le(i64::from(seg.flag));
        w.u16le(i64::from(seg.color));
        w.u16le(0xFFFF);
        w.u32le(seg.points.len() as i64);
        for &(x, y) in &seg.points {
            w.u16le(i64::from(x));
            w.u16le(i64::from(y));
        }
    }
    w.u16le(g.color_log.len() as i64);
    for &(run, thread) in &g.color_log {
        w.u16le(i64::from(run));
        w.u16le(i64::from(thread));
    }
}

/// MFC `CArchive` numbering while writing: classes and objects share one count from 1.
#[derive(Default)]
struct Archive {
    next: u16,
    classes: Vec<(&'static str, u16)>,
    /// Archive number of the object at each list position written so far.
    objects: Vec<u16>,
}

impl Archive {
    /// Writes the tag of a new object of `class` (the class name the first time, else
    /// `0x8000 | n`) and counts the object.
    fn tag(&mut self, w: &mut ByteWriter, class: &'static str) {
        match self.classes.iter().find(|(c, _)| *c == class) {
            Some(&(_, n)) => w.u16le(i64::from(0x8000 | n)),
            None => {
                self.next += 1;
                self.classes.push((class, self.next));
                w.u16le(0xFFFF);
                w.u16le(0);
                w.u16le(class.len() as i64);
                w.ascii(class);
            }
        }
        self.next += 1;
    }

    fn number(&self, class: &str) -> u16 {
        self.classes
            .iter()
            .find(|(c, _)| *c == class)
            .map_or(0, |&(_, n)| n)
    }
}

/// One `CEmbNText`: bounds, the same as its extent, the identity transform, the letters, the
/// line breaks and the text's own fields (r12's, with the line spacing, direction, transform
/// and origin patched).
fn write_text(w: &mut ByteWriter, archive: &mut Archive, t: &Text) -> Result<()> {
    if t.fit.is_some() && t.envelope.is_some() {
        return Err(FormatError::ValueOutOfRange(
            "PES 0110: a text both fitted to an outline and transformed".into(),
        ));
    }
    for _ in 0..2 {
        for v in t.bounds {
            w.u16le(i64::from(v));
        }
    }
    for v in t.transform {
        w.f32le(f64::from(v));
    }
    w.u16le(t.letters.len() as i64);
    for l in &t.letters {
        archive.tag(w, TTF_LETTER);
        w.u16le(i64::from(l.ch));
        let mut a = template::LETTER_ATTRS;
        a[2..6].copy_from_slice(&(l.size * 10.0).to_le_bytes());
        a[6..10].copy_from_slice(&l.scale.to_le_bytes());
        put16(&mut a, 22, tenths(l.density));
        put16(&mut a, 24, l.region_color);
        put16(&mut a, 26, l.region.raw());
        put16(&mut a, 60, u16::from(l.line_sewn));
        put16(&mut a, 62, l.line_type);
        put16(&mut a, 64, l.line_color);
        put16(&mut a, 144, l.weight);
        w.append(&a);
        write_mfc_unicode(w, &l.font);
        w.u16le(0);
    }
    w.u16le(t.breaks.len() as i64);
    for &b in &t.breaks {
        w.u16le(i64::from(b));
    }
    let mut tail = match t.fit {
        Some(_) => template::FIT_TAIL.to_vec(),
        None => template::TEXT_TAIL.to_vec(),
    };
    tail[0..4].copy_from_slice(&t.char_spacing.to_le_bytes());
    tail[4..8].copy_from_slice(&t.line_spacing.to_le_bytes());
    put16(&mut tail, 8, t.align);
    put16(&mut tail, 10, u16::from(t.vertical));
    for (k, v) in t.transform.iter().enumerate() {
        tail[78 + 4 * k..82 + 4 * k].copy_from_slice(&v.to_le_bytes());
    }
    match &t.fit {
        Some(f) => {
            let number = *archive.objects.get(usize::from(f.path)).ok_or_else(|| {
                FormatError::ValueOutOfRange(format!(
                    "PES 0110: the text at position {} follows the object at {}, which comes after it",
                    t.position, f.path
                ))
            })?;
            put16(&mut tail, 104, f.halign);
            put16(&mut tail, 106, f.valign);
            put16(&mut tail, 108, u16::from(f.upright));
            put16(&mut tail, 110, u16::from(f.other_side));
            put16(&mut tail, 118, number);
        }
        None => {
            tail[104..108].copy_from_slice(&t.origin.0.to_le_bytes());
            tail[108..112].copy_from_slice(&t.origin.1.to_le_bytes());
        }
    }
    w.append(&tail);
    if let Some(e) = &t.envelope {
        w.u16le(i64::from(e.code));
        for v in std::iter::once(e.strength).chain(e.geometry) {
            w.f32le(f64::from(v));
        }
    }
    Ok(())
}

/// The header fields after the metadata strings up to the thread count: the hoop (150 × 150 mm,
/// `nv880e-M`, multi-needle), the page size in mm, and fields not mapped yet, as the baseline
/// file has them.
fn write_header(w: &mut ByteWriter, page_mm: (u16, u16), hoop_page: bool) {
    if hoop_page {
        w.append(&template::HEADER_HOOP_100);
        return;
    }
    let mut h = template::HEADER;
    // After 14 hoop bytes and the machine string come a u16, two absent optional fields (u16 0
    // each) and a u32, then the page width and height.
    let at = 15 + usize::from(h[14]) + 10;
    h[at..at + 2].copy_from_slice(&page_mm.0.to_le_bytes());
    h[at + 2..at + 4].copy_from_slice(&page_mm.1.to_le_bytes());
    w.append(&h);
}

/// An MFC `CString` in its Unicode form, as PE-DESIGN 11 writes the thread table's strings:
/// `FF FE FF`, the length in UTF-16 units (u8, or `FF` + u16 from 255 up), UTF-16LE text.
fn write_mfc_unicode(w: &mut ByteWriter, s: &str) {
    let units: Vec<u16> = s.encode_utf16().take(0xFFFE).collect();
    w.append(&[0xFF, 0xFE, 0xFF]);
    if units.len() >= 0xFF {
        w.u8(0xFF);
        w.u16le(units.len() as i64);
    } else {
        w.u8(units.len() as i64);
    }
    for u in units {
        w.u16le(i64::from(u));
    }
}

/// `t` as PE-DESIGN writes the colour in its thread tables. A colour from OpenStitch's Brother
/// PEC chart (brand `Brother`, its chart number as the catalogue number, the chart's exact RGB)
/// gets its BROTHER EMBROIDERY catalogue number and brand `EMBROIDERY`, for all 61 entries
/// (`openstitch_core::BROTHER_EMBROIDERY`; PE-DESIGN showed
/// `16 / Brother` with no thread chart otherwise: compat pes-0110-editable-shapes, the owner's
/// check of 2026-10-29); every thread gets its nearest PEC chart index as its name, where
/// PE-DESIGN writes it for its own colours. A Ver4 Option Color thread (brand `Option Color`,
/// one of [`VER4_OPTION_COLORS`]' RGB) gets its sew palette id 66–94 there instead, an empty
/// code and chart and type 0x0B, as in PE-DESIGN's own file of the 29 colours
/// (`tests/fixtures/pe11/ver4-option-color-29.pes`, #136). For a design's threads:
/// [`write`] writes a thread table as given, so a PE-DESIGN file keeps its own strings.
pub fn as_pe_writes(t: &Thread) -> Thread {
    if let Some(&(id, _, _)) = (t.brand.as_deref() == Some(OPTION_COLOR_BRAND))
        .then(|| VER4_OPTION_COLORS.iter().find(|c| c.2 == t.rgb_value()))
        .flatten()
    {
        return Thread {
            name: Some(Cow::Owned(id.to_string())),
            catalog_number: None,
            chart: None,
            kind: Some(OPTION_COLOR_KIND),
            ..t.clone()
        };
    }
    // PE-DESIGN matches to its 61-entry chart only (#119).
    let Some(index) = nearest_index(t, pec_basic_chart()) else {
        return t.clone();
    };
    // PE-DESIGN writes its colours' PEC chart index where a thread's name goes.
    let t = &Thread {
        name: Some(Cow::Owned(index.to_string())),
        ..t.clone()
    };
    let from_pec_chart = t.brand.as_deref() == Some("Brother")
        && t.catalog_number.as_deref() == Some(index.to_string().as_str())
        && PEC_CHART[index]
            .as_ref()
            .is_some_and(|c| c.rgb_value() == t.rgb_value());
    match brother_embroidery_code(index) {
        Some(catalog) if from_pec_chart => Thread {
            catalog_number: Some(Cow::Borrowed(catalog)),
            brand: Some(Cow::Borrowed(BROTHER_EMBROIDERY_BRAND)),
            chart: None,
            ..t.clone()
        },
        _ => t.clone(),
    }
}

/// The thread table: catalogue number, RGB and a pad byte, colour type, the thread's name (the
/// PEC chart index of the nearest chart colour when it has none, as PE-DESIGN writes its own
/// colours), brand and chart name.
fn write_threads(w: &mut ByteWriter, threads: &[Thread]) {
    w.u16le(threads.len() as i64);
    for t in threads {
        let index = nearest_index(t, pec_basic_chart()).unwrap_or(0);
        write_mfc_unicode(w, t.catalog_number.as_deref().unwrap_or(""));
        w.append(&[t.red, t.green, t.blue, 0]);
        w.u32le(i64::from(t.kind.unwrap_or(Thread::CUSTOM_KIND)));
        match t.name.as_deref() {
            Some(name) => write_mfc_unicode(w, name),
            // The Applique Wizard's colours (kind 2: Applique Material, Position, Applique) have
            // no chart index (a01-rect-applique).
            None if t.kind == Some(APPLIQUE_KIND) => write_mfc_unicode(w, ""),
            None => write_mfc_unicode(w, &index.to_string()),
        }
        write_mfc_unicode(w, t.brand.as_deref().unwrap_or(""));
        write_mfc_unicode(w, t.chart.as_deref().unwrap_or(""));
    }
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put32(b: &mut [u8], at: usize, v: i32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn tenths(mm: f32) -> u16 {
    (mm * 10.0).round().clamp(0.0, 65535.0) as u16
}

/// OpenStitch's placeholder for the pattern block every `CEmbLine` carries (the default
/// programmable pattern in PE-DESIGN's files, which is Brother's and is not copied): the same
/// layout, u16 size 10000, u16 polyline count, then per polyline six zero bytes, u16 1, the
/// u16 point count and i16 x, y points in a 1024-unit cell, drawn as evenly spaced horizontal
/// lines with the baseline file's point counts.
fn pattern_block() -> Vec<u8> {
    let counts = template::PATTERN_COUNTS;
    let mut w = ByteWriter::default();
    w.u16le(10000);
    w.u16le(counts.len() as i64);
    for (k, &n) in counts.iter().enumerate() {
        w.fill(0, 6);
        w.u16le(1);
        w.u16le(i64::from(n));
        let y = ((k as f64 + 0.5) * 1024.0 / counts.len() as f64).round() as i64;
        for j in 0..n {
            let x = (f64::from(j) * 1024.0 / f64::from(n.max(2) - 1)).round() as i64;
            w.u16le(x);
            w.u16le(y);
        }
    }
    w.bytes
}

/// One `CEmbLine` record body, geometry header and vertices.
fn write_line(w: &mut ByteWriter, s: &Shape) {
    let [x0, y0, x1, y1] = s.bounds;
    let (cx, cy) = (
        (i32::from(x0) + i32::from(x1)) / 2,
        (i32::from(y0) + i32::from(y1)) / 2,
    );
    let mut head = template::BODY_HEAD;
    for (k, v) in s.bounds.iter().enumerate() {
        put16(&mut head, 2 * k, *v as u16);
    }
    // Sewn extent: the bounds plus half the zigzag width while a zigzag line is sewn, plus
    // the E or V width while an E or V line is.
    let half = match (s.line_sewn, s.line_type) {
        (true, Shape::LINE_ZIGZAG) => (s.zigzag_width * 5.0).round() as i16,
        (true, Shape::LINE_E_OR_V) => (s.ev_width * 10.0).round() as i16,
        _ => 0,
    };
    for (k, v) in [x0 - half, y0 - half, x1 + half, y1 + half]
        .iter()
        .enumerate()
    {
        put16(&mut head, 8 + 2 * k, *v as u16);
    }
    put16(&mut head, 40, u16::from(!s.closed));
    put16(&mut head, 42, u16::from(s.line_sewn));
    put16(&mut head, 44, s.line_type);
    put16(&mut head, 46, s.line_color);
    put16(&mut head, 48, tenths(s.zigzag_width));
    put16(&mut head, 50, tenths(s.zigzag_density));
    put16(&mut head, 52, tenths(s.run_pitch));
    put16(&mut head, 60, u16::from(s.line_under_sewing));
    put16(&mut head, 90, tenths(s.ev_width));
    // +122: 1 on PE-DESIGN's cutting line where every other object has 5 (a02-applique-
    // cutting; the field's meaning is not mapped).
    if s.line_type == Shape::LINE_CUTTING {
        put16(&mut head, 122, 1);
    }
    put16(
        &mut head,
        132,
        u16::from(s.region != RegionSewType::NotSewn),
    );
    put16(&mut head, 134, s.region.raw());
    put16(&mut head, 136, tenths(s.density));
    put16(&mut head, 138, u16::from(s.gradation));
    // +162: a thread index PE-DESIGN points at its default colour (800 RED) whatever is sewn;
    // OpenStitch points it at the line colour.
    put16(&mut head, 162, s.line_color);
    put16(&mut head, 168, s.direction);
    put16(&mut head, 170, s.region_color);
    put16(&mut head, 174, s.frequency);
    put16(&mut head, 178, s.flag_178);
    put16(&mut head, 180, u16::from(s.under_sewing));
    put16(&mut head, 182, s.under_sewing_type);
    put16(&mut head, 186, s.running_path);
    // Reference points: five at the bounds' top-left corner, two at their centre (i32 x, y).
    for at in [204, 262, 286, 354, 378] {
        put32(&mut head, at, i32::from(x0));
        put32(&mut head, at + 4, i32::from(y0));
    }
    for at in [298, 310] {
        put32(&mut head, at, cx);
        put32(&mut head, at + 4, cy);
    }
    w.append(&head);
    w.append(&pattern_block());
    let mut tail = template::BODY_TAIL;
    let base = PATTERN_END;
    for at in [4372, 4388] {
        put32(&mut tail, at - base, cx);
        put32(&mut tail, at - base + 4, cy);
    }
    // Two outline positions (segment, f32 t): the end of the last segment, twice; a Manual
    // Punch block's second is the vertex its sewing ends at (t 0).
    let segments = s.vertices.len() - usize::from(!s.closed);
    put16(&mut tail, 4414 - base, segments.saturating_sub(1) as u16);
    if !s.punch.is_empty() {
        put16(&mut tail, 4426 - base, s.punch_exit);
        put32(&mut tail, 4428 - base, 0);
    } else {
        put16(&mut tail, 4426 - base, segments.saturating_sub(1) as u16);
    }
    w.append(&tail);
    write_geometry_header(w, s);
    w.u32le(s.vertices.len() as i64);
    for v in &s.vertices {
        for f in [
            v.x,
            v.y,
            v.handles[0].0,
            v.handles[0].1,
            v.handles[1].0,
            v.handles[1].1,
        ] {
            w.f32le(f64::from(f));
        }
        w.u32le(i64::from(v.kind));
    }
}

/// The geometry header: two lists of outline positions (u32 count, then u32 vertex and f64 t;
/// one entry each on a shape, a Manual Punch object's line ends), then three marks, each a
/// position on the outline (a segment and a parameter, not decoded) with its coordinates. The
/// coordinates of PE-DESIGN's closed shapes sit at fixed fractions of the bounds: a tenth of
/// the width outside the left and right edges at 60 % of the height, and the centre at 25 % of
/// the height. The segment and parameter fields are the baseline file's (open lines: the
/// open-line file's). Manual Punch objects: [`write_punch_geometry_header`].
fn write_geometry_header(w: &mut ByteWriter, s: &Shape) {
    if !s.punch.is_empty() {
        return write_punch_geometry_header(w, s);
    }
    let [x0, y0, x1, y1] = s.bounds.map(f64::from);
    let (width, height) = (x1 - x0, y1 - y0);
    // Whole units, rounded (a05-applique-replace's 210-unit square: the centre mark at
    // 595 + 52.5 → 648).
    let (left, right, mid) = (
        ((x0 - width / 10.0).round(), (y0 + height * 0.6).round()),
        ((x1 + width / 10.0).round(), (y0 + height * 0.6).round()),
        (((x0 + x1) / 2.0).round(), (y0 + height * 0.25).round()),
    );
    let (seg_b, t_ab, flag_c, t_cd, seg_d): (i64, f64, i64, f64, i64) = if s.closed {
        (2, 1.0, 1, 0.5, 3)
    } else {
        (0, 0.0, 0, 0.0, 0)
    };
    if s.no_positions {
        // Convert to Blocks' running pieces: both lists empty.
        w.u32le(0);
        w.u32le(0);
    } else {
        w.u32le(1);
        w.u32le(0);
        w.append(&t_ab.to_le_bytes());
        w.u32le(1);
        w.u32le(seg_b);
        w.append(&t_ab.to_le_bytes());
    }
    w.f32le(left.0);
    w.f32le(left.1);
    w.u32le(flag_c);
    w.append(&t_cd.to_le_bytes());
    w.u16le(1);
    w.f32le(mid.0);
    w.f32le(mid.1);
    w.u32le(seg_d);
    w.append(&t_cd.to_le_bytes());
    w.f32le(right.0);
    w.f32le(right.1);
    w.u16le(0);
}

/// A Manual Punch block's geometry header: the lists of its lines' ends, then the three marks.
/// The marks hang off the outline where the horizontal line midway between its top and bottom
/// (as its curves reach them: [`outline_extent`], m11's sag to 970.25) crosses it furthest left
/// and furthest right, each crossing given as a segment and a parameter along it (a crossing at
/// a vertex as the end of the segment before it, t 1). Mark A is the left crossing offset by a
/// tenth of the bounds' width outward and a tenth of their height down, C likewise at the right,
/// and B midway between them a quarter of the height up, each tenth and quarter in whole units
/// (rounded down); the parameters are single-precision values (m11: 0.07437500357627869). On
/// the Manual Punch tool's blocks the crossings are the edges that close the block (m01 … m11,
/// o-manual-*); Convert to Blocks' blocks cross on their sides too (stroke-blocks-H, -A).
fn write_punch_geometry_header(w: &mut ByteWriter, s: &Shape) {
    for end in 0..2 {
        w.u32le(s.punch.len() as i64);
        for line in &s.punch {
            w.u32le(i64::from(line[end]));
            w.append(&0f64.to_le_bytes());
        }
    }
    let [_, y_top, _, y_bottom] = outline_extent(&s.vertices);
    let cut_y = (y_top + y_bottom) / 2.0;
    let crossings = crossings_at(&s.vertices, cut_y);
    let leftmost = crossings
        .iter()
        .copied()
        .min_by(|a, b| a.2.0.total_cmp(&b.2.0));
    let rightmost = crossings
        .iter()
        .copied()
        .max_by(|a, b| a.2.0.total_cmp(&b.2.0));
    let ((seg_l, t_l, left), (seg_r, t_r, right)) = match (leftmost, rightmost) {
        (Some(l), Some(r)) => (l, r),
        _ => {
            let at = (f64::from(s.vertices[0].x), f64::from(s.vertices[0].y));
            ((0, 0.0, at), (0, 0.0, at))
        }
    };
    let [x0, y0, x1, y1] = s.bounds.map(i32::from);
    let (tenth_w, tenth_h, quarter_h) = (
        f64::from((x1 - x0) / 10),
        f64::from((y1 - y0) / 10),
        f64::from((y1 - y0) / 4),
    );
    w.f32le(left.0 - tenth_w);
    w.f32le(left.1 + tenth_h);
    w.u32le(seg_l as i64);
    w.append(&f64::from(t_l as f32).to_le_bytes());
    w.u16le(1);
    w.f32le((left.0 + right.0) / 2.0);
    w.f32le((left.1 + right.1) / 2.0 - quarter_h);
    w.u32le(seg_r as i64);
    w.append(&f64::from(t_r as f32).to_le_bytes());
    w.f32le(right.0 + tenth_w);
    w.f32le(right.1 + tenth_h);
    w.u16le(0);
}

/// Where the closed outline crosses the height `y`: (segment, parameter, point) for each
/// crossing, a crossing at a vertex counted once, as the end of the segment before it (t 1). A
/// curved segment is searched along its Bézier curve.
fn crossings_at(vertices: &[Vertex], y: f64) -> Vec<(usize, f64, (f64, f64))> {
    let n = vertices.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b) = (&vertices[i], &vertices[(i + 1) % n]);
        let p0 = (f64::from(a.x), f64::from(a.y));
        let p3 = (f64::from(b.x), f64::from(b.y));
        let curved = a.kind & KIND_CURVE_AFTER != 0 || b.kind & KIND_CURVE_BEFORE != 0;
        let point = |t: f64| {
            if !curved {
                return (p0.0 + t * (p3.0 - p0.0), p0.1 + t * (p3.1 - p0.1));
            }
            let p1 = (f64::from(a.handles[1].0), f64::from(a.handles[1].1));
            let p2 = (f64::from(b.handles[0].0), f64::from(b.handles[0].1));
            let u = 1.0 - t;
            let c = |k0: f64, k1: f64, k2: f64, k3: f64| {
                u * u * u * k0 + 3.0 * u * u * t * k1 + 3.0 * u * t * t * k2 + t * t * t * k3
            };
            (c(p0.0, p1.0, p2.0, p3.0), c(p0.1, p1.1, p2.1, p3.1))
        };
        if !curved {
            if p0.1 == p3.1 {
                continue;
            }
            let t = (y - p0.1) / (p3.1 - p0.1);
            if t > 0.0 && t <= 1.0 {
                out.push((i, t, point(t)));
            }
            continue;
        }
        // A curve: bracket each sign change on 64 steps, then bisect.
        const STEPS: usize = 64;
        let f = |t: f64| point(t).1 - y;
        let mut prev = f(0.0);
        for k in 1..=STEPS {
            let (mut lo, mut hi) = ((k - 1) as f64 / STEPS as f64, k as f64 / STEPS as f64);
            let cur = f(hi);
            if prev != 0.0 && (prev < 0.0) != (cur < 0.0) || cur == 0.0 {
                for _ in 0..60 {
                    let mid = (lo + hi) / 2.0;
                    if (f(lo) < 0.0) == (f(mid) < 0.0) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                out.push((i, hi, point(hi)));
            }
            prev = cur;
        }
    }
    out
}

/// `x0, y0, x1, y1` of a closed outline as its segments reach (file units, unrounded): a
/// segment is a cubic Bézier when its start has [`KIND_CURVE_AFTER`] or its end
/// [`KIND_CURVE_BEFORE`], else straight. PE-DESIGN's Manual Punch objects store this extent
/// rounded outward as their bounds (m10's curve bulges to x 591.48: bounds 591).
pub fn outline_extent(vertices: &[Vertex]) -> [f64; 4] {
    let mut e = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut take = |x: f64, y: f64| {
        e[0] = e[0].min(x);
        e[1] = e[1].min(y);
        e[2] = e[2].max(x);
        e[3] = e[3].max(y);
    };
    let n = vertices.len();
    for i in 0..n {
        let (a, b) = (&vertices[i], &vertices[(i + 1) % n]);
        let p0 = [f64::from(a.x), f64::from(a.y)];
        let p3 = [f64::from(b.x), f64::from(b.y)];
        take(p0[0], p0[1]);
        let curved = a.kind & KIND_CURVE_AFTER != 0 || b.kind & KIND_CURVE_BEFORE != 0;
        if !curved {
            continue;
        }
        let p1 = [f64::from(a.handles[1].0), f64::from(a.handles[1].1)];
        let p2 = [f64::from(b.handles[0].0), f64::from(b.handles[0].1)];
        // Where each coordinate's derivative is zero inside the segment.
        for axis in 0..2 {
            let (a0, a1, a2, a3) = (p0[axis], p1[axis], p2[axis], p3[axis]);
            let qa = -a0 + 3.0 * a1 - 3.0 * a2 + a3;
            let qb = 2.0 * (a0 - 2.0 * a1 + a2);
            let qc = a1 - a0;
            let mut roots = Vec::new();
            if qa.abs() < 1e-12 {
                if qb.abs() > 1e-12 {
                    roots.push(-qc / qb);
                }
            } else {
                let disc = qb * qb - 4.0 * qa * qc;
                if disc >= 0.0 {
                    roots.push((-qb + disc.sqrt()) / (2.0 * qa));
                    roots.push((-qb - disc.sqrt()) / (2.0 * qa));
                }
            }
            for t in roots.into_iter().filter(|t| *t > 0.0 && *t < 1.0) {
                let u = 1.0 - t;
                let point = |k: usize| {
                    u * u * u * p0[k]
                        + 3.0 * u * u * t * p1[k]
                        + 3.0 * u * t * t * p2[k]
                        + t * t * t * p3[k]
                };
                take(point(0), point(1));
            }
        }
    }
    e
}

/// After the object list: 10 zero bytes, the group list (u16 count; per group its MFC tag,
/// the `CGroupObject` class's archive number, the u16 member count and the members' object
/// positions), 12 zero bytes, then the sewing-order list: u32 1 and (position, part, 1)
/// triples. A shape lists the connection from the previous object (part 0, every object but
/// the first), its region (2) and its line (1); a text of n letters lists the connections into
/// its letters (parts 0, 2 … 2n − 2; the first object's first skipped), then the letters
/// (1, 3 … 2n − 1). PE-DESIGN's last third field is sometimes not 1 (the text-line-spacing
/// files end in `08 CE 53 00`); OpenStitch writes 1.
fn write_trailer(
    w: &mut ByteWriter,
    archive: &mut Archive,
    objects: &Objects,
    at_position: &[Option<&Text>],
    group_at: &[Option<&StitchGroup>],
) {
    w.fill(0, 10);
    w.u16le(objects.groups.len() as i64);
    for members in &objects.groups {
        archive.tag(w, GROUP_OBJECT);
        w.u16le(i64::from(archive.number(GROUP_OBJECT)));
        w.u16le(members.len() as i64);
        for &m in members {
            w.u16le(i64::from(m));
        }
    }
    w.fill(0, 12);
    w.u32le(1);
    for (i, slot) in at_position.iter().enumerate() {
        let parts: Vec<i64> = match slot {
            // A stitch group lists its colour blocks (one block: none for the first object).
            None if group_at[i].is_some() => {
                let blocks = group_at[i].map_or(1, |g| g.color_log.len().max(1)) as i64;
                (0..blocks).skip(usize::from(i == 0)).collect()
            }
            None if i == 0 => vec![2, 1],
            None => vec![0, 2, 1],
            Some(t) => {
                let n = t.letters.len() as i64;
                let skip = usize::from(i == 0);
                (0..n)
                    .map(|k| 2 * k)
                    .skip(skip)
                    .chain((0..n).map(|k| 2 * k + 1))
                    .collect()
            }
        };
        for part in parts {
            w.u32le(i as i64);
            w.u32le(part);
            w.u32le(1);
        }
    }
}

/// After the PEC section, per colour block: the PEC colour list padded to 128 bytes, a 48 × 24
/// 1-bpp icon (PE-DESIGN draws the thread's number and name; OpenStitch leaves it blank), the
/// RGB as R, G, B (zero on a Custom Size page, as in all of PE-DESIGN's own files), then a u16
/// count of preview images (0: PE-DESIGN's own Hoop-Size files have none) and a three-byte
/// code per block ([`addendum_code`]).
fn write_addendum(w: &mut ByteWriter, list: &[u8], threads: &[Thread], hoop_page: bool) {
    w.append(list);
    w.fill(0x20, 128_usize.saturating_sub(list.len()));
    for _ in threads {
        w.fill(0, 0x90);
    }
    for t in threads {
        if hoop_page {
            w.append(&[t.red, t.green, t.blue]);
        } else {
            w.fill(0, 3);
        }
    }
    w.u16le(0);
    for t in threads {
        w.append(&addendum_code(t));
    }
}

/// A colour block's code in the list after the PEC section, as PE-DESIGN 11.50 writes it:
/// `0D` and the catalogue number as u16 (BROTHER EMBROIDERY's for a Brother chart colour,
/// `0D 20 03` 800 RED), `FA`, `FB`, `FC` + `FF FF` for the Applique Wizard's material, position
/// and appliqué colours (PEC 62–64), `FE FF FF` for a thread with no number.
fn addendum_code(t: &Thread) -> [u8; 3] {
    let applique = match (t.kind, t.catalog_number.as_deref()) {
        (Some(APPLIQUE_KIND), Some(c)) => ["100", "101", "102"].iter().position(|&a| a == c),
        (_, Some(c)) if t.brand.as_deref() == Some("Brother") => ["62", "63", "64"]
            .iter()
            .position(|&a| a == c)
            .filter(|&k| {
                PEC_CHART[62 + k]
                    .as_ref()
                    .is_some_and(|p| p.rgb_value() == t.rgb_value())
            }),
        _ => None,
    };
    if let Some(k) = applique {
        return [0xFA + k as u8, 0xFF, 0xFF];
    }
    match as_pe_writes(t)
        .catalog_number
        .as_deref()
        .and_then(|c| c.trim().parse::<u16>().ok())
    {
        Some(n) => {
            let [lo, hi] = n.to_le_bytes();
            [0x0D, lo, hi]
        }
        None => [0xFE, 0xFF, 0xFF],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The colour list after the PEC section, from its RGB on, as PE-DESIGN 11.50 writes it in
    /// its own `#PES0110` files (pe11 fixtures and compat/reference): for every block whose
    /// code is a catalogue number or an appliqué colour, OpenStitch's list for the colours the
    /// reader gives (the PEC chart entries) has PE's RGB and code; the RGB is zero on a Custom
    /// Size page.
    #[test]
    fn colour_list_after_the_pec_is_written_as_pe_design_writes_it() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).expect("dir").flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pes")) {
                    out.push(p);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        walk(&root.join("tests/fixtures/pe11"), &mut files);
        walk(&root.join("../../compat/reference"), &mut files);
        let (mut blocks_checked, mut pages) = (0, [0, 0]);
        for path in &files {
            let b = std::fs::read(path).expect("read");
            if !b.starts_with(b"#PES0110") {
                continue;
            }
            let (Ok(section), Ok(pattern)) = (crate::pes::read_objects(&b), crate::pes::read(&b))
            else {
                continue;
            };
            let pec = u32::from_le_bytes(b[8..12].try_into().expect("u32")) as usize;
            if b[pec + 0x30] == 0xFF {
                continue;
            }
            let n = usize::from(b[pec + 0x30]) + 1;
            let codes_at = b.len() - 3 * n;
            let rgb_at = codes_at - 2 - 3 * n;
            if b[codes_at - 2..codes_at] != [0, 0] || pattern.threads.len() != n {
                continue; // preview images follow: not mapped
            }
            let hoop_page = section.hoop_page;
            let mut w = ByteWriter::default();
            let list = &b[pec + 0x31..pec + 0x31 + n];
            write_addendum(&mut w, list, &pattern.threads, hoop_page);
            let ours = &w.bytes[w.bytes.len() - (6 * n + 2)..];
            let pe = &b[rgb_at..];
            pages[usize::from(hoop_page)] += 1;
            for k in 0..n {
                let code = &pe[3 * n + 2 + 3 * k..3 * n + 2 + 3 * k + 3];
                if !(code[0] == 0x0D || (0xFA..=0xFC).contains(&code[0])) {
                    continue; // FE: a thread with no number, F9: a cut
                }
                let name = path.display();
                assert_eq!(
                    &ours[3 * n + 2 + 3 * k..3 * n + 2 + 3 * k + 3],
                    code,
                    "{name} block {k} code"
                );
                assert_eq!(
                    &ours[3 * k..3 * k + 3],
                    &pe[3 * k..3 * k + 3],
                    "{name} block {k} RGB"
                );
                blocks_checked += 1;
            }
        }
        assert!(
            blocks_checked > 400 && pages[0] > 300 && pages[1] > 60,
            "{blocks_checked} {pages:?}"
        );
    }

    #[test]
    fn body_template_and_pattern_block_have_the_mapped_length() {
        assert_eq!(
            template::BODY_HEAD.len() + pattern_block().len() + template::BODY_TAIL.len(),
            LINE_BODY
        );
        assert_eq!(PATTERN_START, template::BODY_HEAD.len());
        assert_eq!(template::BODY_TAIL[..4], [0xE8, 0x03, 0xE8, 0x03]);
    }

    #[test]
    fn brother_chart_colours_get_pe_designs_embroidery_numbers() {
        let strings = |t: &Thread| {
            let t = as_pe_writes(t);
            (t.catalog_number, t.brand, t.chart)
        };
        let silver = PEC_CHART[16].clone().expect("silver");
        assert_eq!(
            strings(&silver),
            (Some("005".into()), Some("EMBROIDERY".into()), None)
        );
        let red = PEC_CHART[5].clone().expect("red");
        assert_eq!(strings(&red).0.as_deref(), Some("800"));
        // Every chart entry 1–61 has its code now (6 REDDISH BROWN is 337, from the chart's
        // names); a custom colour and an entry outside PE's chart keep their own strings
        // (every thread gets its chart index as its name, as PE-DESIGN writes its colours).
        let reddish = PEC_CHART[6].clone().expect("chart colour");
        assert_eq!(
            strings(&reddish),
            (Some("337".into()), Some("EMBROIDERY".into()), None)
        );
        assert_eq!(as_pe_writes(&reddish).name.as_deref(), Some("6"));
        let mut custom = Thread::rgb(0x12_34_56);
        custom.catalog_number = Some("42".into());
        assert_eq!(strings(&custom), (Some("42".into()), None, None));
        let orange = PEC_CHART[62].clone().expect("62");
        assert_eq!(
            strings(&orange),
            (Some("62".into()), Some("Brother".into()), None)
        );
        // A chart RGB under another brand is not relabelled.
        let mut other = silver.clone();
        other.brand = Some("Madeira".into());
        assert_eq!(
            strings(&other),
            (Some("16".into()), Some("Madeira".into()), None)
        );
    }

    #[test]
    fn page_size_is_patched_into_the_header() {
        let mut w = ByteWriter::default();
        write_header(&mut w, (130, 180), false);
        let s = String::from_utf8_lossy(&w.bytes[15..23]).into_owned();
        assert_eq!(s, "nv880e-M");
        assert_eq!(&w.bytes[33..37], &[130, 0, 180, 0]);
    }
}
