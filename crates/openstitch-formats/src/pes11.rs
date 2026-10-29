//! PE-DESIGN 11 (`#PES0110`) editable objects: the object list after the header, as far as the
//! owner's PE-DESIGN output files map it (docs/FILE_FORMATS.md, "PES 0110 object list").
//!
//! The list is a u16 object count, then MFC `CArchive` objects: an object of a class not seen
//! before starts with `FFFF`, a u16 schema and the u16-length class name; a later object of an
//! already named class starts with `0x8000 | n`, where n is the class's entry in the archive's
//! map. MFC numbers classes and objects in one sequence from 1 (each new class takes the next
//! number, then each object), so after a text and its letters a class's number is not its
//! order of appearance. Only `CEmbLine` (shapes and lines drawn with the shape and line tools) is
//! read. Reading stops at the first other class, or at a `CEmbLine` whose geometry doesn't fit
//! its bounds (a variant not mapped yet, such as a Manual Punch block), and records the class in
//! [`super::ObjectSection::unparsed`].
//!
//! [`write`] is the inverse: a `#PES0110` file whose object list holds the same `CEmbLine`
//! records, thread table, groups and sewing-order list, laid out as PE-DESIGN 11.50 lays them
//! out (`read_objects(write(o)) == o`), with the PEC section the machine sews from. Fields not
//! mapped yet are written with the values of the owner's baseline file (`pes11_template`).

use crate::io::{ByteReader, ByteWriter};
use crate::{FormatError, Result, pec};
use openstitch_core::{PEC_CHART, StitchPattern, Thread, nearest_index};
use std::borrow::Cow;

#[path = "pes11_template.rs"]
mod template;

const EMB_LINE: &str = "CEmbLine";
const GROUP_OBJECT: &str = "CGroupObject";
/// Bytes of a `CEmbLine` body before its geometry, in every reference file mapped so far.
const LINE_BODY: usize = 4432;
/// Geometry header before the vertex count (three handle marks).
const GEOMETRY_HEADER: usize = 84;
const VERTEX: usize = 28;
/// Where the body's pattern block starts and ends (`pattern_block`).
#[cfg(test)]
const PATTERN_START: usize = 386;
const PATTERN_END: usize = LINE_BODY - template::BODY_TAIL.len();

/// File units are 0.1 mm, and the page's top-left corner is at this offset on both axes in
/// every owner file (100 × 100 mm and 200 × 200 mm pages alike).
pub const PAGE_ORIGIN: f32 = 500.0;

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
    /// The thread table, in its file order (the shapes' colour indexes point into it).
    pub threads: Vec<Thread>,
    pub shapes: Vec<Shape>,
    /// Groups (`CGroupObject` after the list): each the indexes of its member objects.
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
    pub vertices: Vec<Vertex>,
}

impl Shape {
    /// Line sew type codes (`line_type`).
    pub const LINE_ZIGZAG: u16 = 0;
    pub const LINE_RUNNING: u16 = 1;
    pub const LINE_E_OR_V: u16 = 3;
    pub const LINE_TRIPLE: u16 = 4;
    pub const LINE_NOT_SEWN: u16 = 9;

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
/// this tries each start and keeps the one that parses to exactly `list`.
fn read_threads(data: &[u8], from: usize, list: usize) -> Option<Vec<Thread>> {
    let parse = |start: usize| -> Result<Vec<Thread>> {
        let mut r = ByteReader::new(&data[..list]);
        r.offset = start;
        let count = r.u16le()?;
        let mut out = Vec::new();
        for _ in 0..count {
            let catalog = crate::pes::read_string8(&mut r)?;
            let rgb = r.read(4)?;
            let kind = r.u32le()?;
            let _pec_index = crate::pes::read_string8(&mut r)?;
            let brand = crate::pes::read_string8(&mut r)?;
            let chart = crate::pes::read_string8(&mut r)?;
            let mut t =
                Thread::rgb(u32::from(rgb[0]) << 16 | u32::from(rgb[1]) << 8 | u32::from(rgb[2]));
            t.catalog_number = catalog.map(Cow::Owned);
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
    (from..list.saturating_sub(2))
        .rev()
        .find_map(|s| parse(s).ok())
}

/// Reads the thread table and the object list's shapes. `from` is the offset after the header
/// strings, `end` the PEC offset.
pub(crate) fn read_objects(data: &[u8], from: usize, end: usize) -> Result<Objects> {
    let Some(at) = list_offset(data, from, end) else {
        return Ok(Objects::default());
    };
    let threads = read_threads(data, from, at).unwrap_or_default();
    let page_mm = read_page(data, from);
    let done = |shapes, groups, unparsed| {
        Ok(Objects {
            page_mm,
            threads: threads.clone(),
            shapes,
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
    for _ in 0..count {
        let class = match read_tag(&mut r, &mut map)? {
            Ok(class) => class,
            Err(tag) => return done(shapes, Vec::new(), Some(format!("object tag {tag:#06x}"))),
        };
        map.push(None);
        if class != EMB_LINE {
            return done(shapes, Vec::new(), Some(class));
        }
        match read_line(&mut r)? {
            Some(shape) => shapes.push(shape),
            None => {
                return done(
                    shapes,
                    Vec::new(),
                    Some(format!("{EMB_LINE} (unmapped variant)")),
                );
            }
        }
    }
    let groups = read_groups(&mut r, &mut map).unwrap_or_default();
    done(shapes, groups, None)
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

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

/// One `CEmbLine` body at the reader's offset; `None` (reader unchanged) when it doesn't have
/// the mapped layout.
fn read_line(r: &mut ByteReader<'_>) -> Result<Option<Shape>> {
    let start = r.offset;
    if r.remaining() < LINE_BODY + GEOMETRY_HEADER + 4 {
        return Ok(None);
    }
    let body = r.read(LINE_BODY)?;
    let bounds = [0, 1, 2, 3].map(|k| u16_at(body, 2 * k) as i16);
    r.skip(GEOMETRY_HEADER)?;
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
    Ok(Some(Shape {
        bounds,
        closed: u16_at(body, 40) == 0,
        line_sewn: u16_at(body, 42) != 0,
        line_type: u16_at(body, 44),
        line_color: u16_at(body, 46),
        zigzag_width: f32::from(u16_at(body, 48)) / 10.0,
        zigzag_density: f32::from(u16_at(body, 50)) / 10.0,
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
        vertices,
    }))
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
    if objects.shapes.is_empty() || objects.shapes.len() > 0x7FFE {
        return Err(FormatError::ValueOutOfRange(format!(
            "PES 0110: {} objects (1 to 32766)",
            objects.shapes.len()
        )));
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
        if g.is_empty() || g.iter().any(|&m| usize::from(m) >= objects.shapes.len()) {
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
    w.ascii("10");
    for key in crate::pes::METADATA_KEYS {
        crate::pes::write_string8(&mut w, pattern.metadata.get(key).map(String::as_str));
    }
    write_header(&mut w, objects.page_mm.unwrap_or((100, 100)));
    write_threads(&mut w, &objects.threads);
    w.u16le(objects.shapes.len() as i64);
    // Archive map: CEmbLine is entry 1, the objects 2..; CGroupObject (if any) comes next.
    for (i, shape) in objects.shapes.iter().enumerate() {
        if i == 0 {
            w.u16le(0xFFFF);
            w.u16le(0);
            w.u16le(EMB_LINE.len() as i64);
            w.ascii(EMB_LINE);
        } else {
            w.u16le(0x8001);
        }
        write_line(&mut w, shape);
    }
    write_trailer(&mut w, objects);
    w.patch32le(pec_placeholder, w.len() as i64);
    let list = pec::write_section(&pattern, &mut w)?;
    write_addendum(&mut w, &list, &pattern.threads);
    Ok(w.bytes)
}

/// The header fields after the metadata strings up to the thread count: the hoop (150 × 150 mm,
/// `nv880e-M`, multi-needle), the page size in mm, and fields not mapped yet, as the baseline
/// file has them.
fn write_header(w: &mut ByteWriter, page_mm: (u16, u16)) {
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

/// The thread table: catalogue number, RGB and a pad byte, colour type, the PEC chart index of
/// the nearest chart colour as a string, brand and chart name.
fn write_threads(w: &mut ByteWriter, threads: &[Thread]) {
    w.u16le(threads.len() as i64);
    for t in threads {
        write_mfc_unicode(w, t.catalog_number.as_deref().unwrap_or(""));
        w.append(&[t.red, t.green, t.blue, 0]);
        w.u32le(i64::from(t.kind.unwrap_or(Thread::CUSTOM_KIND)));
        let index = nearest_index(t, &PEC_CHART).unwrap_or(0);
        write_mfc_unicode(w, &index.to_string());
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
    // Sewn extent: the bounds plus half the zigzag width while a zigzag line is sewn.
    let half = if s.line_sewn && s.line_type == Shape::LINE_ZIGZAG {
        (s.zigzag_width * 5.0).round() as i16
    } else {
        0
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
    put16(&mut head, 180, u16::from(s.under_sewing));
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
    // The index of the outline's last segment.
    let segments = s.vertices.len() - usize::from(!s.closed);
    for at in [4414, 4426] {
        put16(&mut tail, at - base, segments.saturating_sub(1) as u16);
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

/// The 84-byte geometry header: three marks, each a position on the outline (a segment and a
/// parameter, not decoded) with its coordinates. The coordinates of PE-DESIGN's closed shapes
/// sit at fixed fractions of the bounds: a tenth of the width outside the left and right
/// edges at 60 % of the height, and the centre at 25 % of the height. The segment and
/// parameter fields are the baseline file's (open lines: the open-line file's).
fn write_geometry_header(w: &mut ByteWriter, s: &Shape) {
    let [x0, y0, x1, y1] = s.bounds.map(f64::from);
    let (width, height) = (x1 - x0, y1 - y0);
    let (left, right, mid) = (
        (x0 - width / 10.0, y0 + height * 0.6),
        (x1 + width / 10.0, y0 + height * 0.6),
        ((x0 + x1) / 2.0, y0 + height * 0.25),
    );
    let (seg_b, t_ab, flag_c, t_cd, seg_d): (i64, f64, i64, f64, i64) = if s.closed {
        (2, 1.0, 1, 0.5, 3)
    } else {
        (0, 0.0, 0, 0.0, 0)
    };
    w.u32le(1);
    w.u32le(0);
    w.append(&t_ab.to_le_bytes());
    w.u32le(1);
    w.u32le(seg_b);
    w.append(&t_ab.to_le_bytes());
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

/// After the object list: 10 zero bytes, the group list (u16 count; per group its MFC tag,
/// the `CGroupObject` class's archive number, the u16 member count and the members' object
/// indexes), 12 zero
/// bytes, then the sewing-order list: u32 1 and, per object in order, (index, part, 1) triples
/// for the connection from the previous object (part 0, every object but the first), the
/// region (2) and the line (1).
fn write_trailer(w: &mut ByteWriter, objects: &Objects) {
    w.fill(0, 10);
    w.u16le(objects.groups.len() as i64);
    // Archive numbers: 1 CEmbLine, 2.. the objects, then the CGroupObject class and each group.
    let class_number = objects.shapes.len() as i64 + 2;
    for (g, members) in objects.groups.iter().enumerate() {
        if g == 0 {
            w.u16le(0xFFFF);
            w.u16le(0);
            w.u16le(GROUP_OBJECT.len() as i64);
            w.ascii(GROUP_OBJECT);
        } else {
            w.u16le(0x8000 | class_number);
        }
        w.u16le(class_number);
        w.u16le(members.len() as i64);
        for &m in members {
            w.u16le(i64::from(m));
        }
    }
    w.fill(0, 12);
    w.u32le(1);
    for i in 0..objects.shapes.len() as i64 {
        let parts: &[i64] = if i == 0 { &[2, 1] } else { &[0, 2, 1] };
        for &part in parts {
            w.u32le(i);
            w.u32le(part);
            w.u32le(1);
        }
    }
}

/// After the PEC section, per colour block: the PEC colour list padded to 128 bytes, a 48 × 24
/// 1-bpp icon (PE-DESIGN draws the thread's number and name; OpenStitch leaves it blank), the
/// RGB as u24, then a u16 count of preview images (0: PE-DESIGN's own Hoop-Size files have
/// none) and a `0x0D` + u16 catalogue number per block.
fn write_addendum(w: &mut ByteWriter, list: &[u8], threads: &[Thread]) {
    w.append(list);
    w.fill(0x20, 128_usize.saturating_sub(list.len()));
    for _ in threads {
        w.fill(0, 0x90);
    }
    for t in threads {
        w.u24le(i64::from(t.rgb_value()));
    }
    w.u16le(0);
    for t in threads {
        let catalog = t
            .catalog_number
            .as_deref()
            .and_then(|c| c.trim().parse::<u16>().ok())
            .unwrap_or(0);
        w.u8(0x0D);
        w.u16le(i64::from(catalog));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn page_size_is_patched_into_the_header() {
        let mut w = ByteWriter::default();
        write_header(&mut w, (130, 180));
        let s = String::from_utf8_lossy(&w.bytes[15..23]).into_owned();
        assert_eq!(s, "nv880e-M");
        assert_eq!(&w.bytes[33..37], &[130, 0, 180, 0]);
    }
}
