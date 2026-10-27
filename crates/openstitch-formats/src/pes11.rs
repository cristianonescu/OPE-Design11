//! PE-DESIGN 11 (`#PES0110`) editable objects: the object list after the header, as far as the
//! owner's PE-DESIGN output files map it (docs/FILE_FORMATS.md, "PES 0110 object list").
//!
//! The list is a u16 object count, then MFC `CArchive` objects: an object of a class not seen
//! before starts with `FFFF`, a u16 schema and the u16-length class name; a later object of an
//! already named class starts with `0x8000 | n`, n counting classes in order of first
//! appearance from 1. Only `CEmbLine` (shapes and lines drawn with the shape and line tools) is
//! read. Reading stops at the first other class, or at a `CEmbLine` whose geometry doesn't fit
//! its bounds (a variant not mapped yet, such as a Manual Punch block), and records the class in
//! [`super::ObjectSection::unparsed`].

use crate::Result;
use crate::io::ByteReader;

const EMB_LINE: &str = "CEmbLine";
/// Bytes of a `CEmbLine` body before its geometry, in every reference file mapped so far.
const LINE_BODY: usize = 4432;
/// Geometry header before the vertex count (three handle marks).
const GEOMETRY_HEADER: usize = 84;
const VERTEX: usize = 28;

/// Region sew type (`CEmbLine` body +134).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionSewType {
    Satin,
    Fill,
    ProgrammableFill,
    /// A value not mapped yet (11 is seen on the inner copy of a hole-sewing pair).
    Other(u16),
}

impl RegionSewType {
    fn from_raw(v: u16) -> Self {
        match v {
            0 => Self::Satin,
            1 => Self::Fill,
            2 => Self::ProgrammableFill,
            v => Self::Other(v),
        }
    }
}

/// One outline vertex in file units: 0.1 mm with the 100 × 100 mm reference page's top-left
/// corner at (500, 500). `handles` are the cubic Bézier handles before and after the vertex
/// (equal to the vertex, or zero, on straight segments).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub handles: [(f32, f32); 2],
    /// Vertex kind: 0 a corner, `0x11` a smooth curve point (the circle's four vertices).
    pub kind: u32,
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
    pub region: RegionSewType,
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

/// Reads the object list's shapes. `from` is the offset after the header strings, `end` the
/// PEC offset. Returns the shapes read and the first class not read, if any.
pub(crate) fn read_objects(
    data: &[u8],
    from: usize,
    end: usize,
) -> Result<(Vec<Shape>, Option<String>)> {
    let Some(at) = list_offset(data, from, end) else {
        return Ok((Vec::new(), None));
    };
    let end = end.min(data.len());
    let mut r = ByteReader::new(&data[..end]);
    r.offset = at;
    let count = r.u16le()?;
    let mut classes: Vec<String> = Vec::new();
    let mut shapes = Vec::new();
    for _ in 0..count {
        let tag = r.u16le()?;
        let class = if tag == 0xFFFF {
            r.u16le()?; // schema
            let n = r.u16le()? as usize;
            let name = String::from_utf8_lossy(r.read(n)?).into_owned();
            classes.push(name.clone());
            name
        } else if tag & 0x8000 != 0 {
            match ((tag & 0x7FFF) as usize)
                .checked_sub(1)
                .and_then(|i| classes.get(i))
            {
                Some(c) => c.clone(),
                None => return Ok((shapes, Some(format!("object tag {tag:#06x}")))),
            }
        } else {
            return Ok((shapes, Some(format!("object tag {tag:#06x}"))));
        };
        if class != EMB_LINE {
            return Ok((shapes, Some(class)));
        }
        match read_line(&mut r)? {
            Some(shape) => shapes.push(shape),
            None => return Ok((shapes, Some(format!("{EMB_LINE} (unmapped variant)")))),
        }
    }
    Ok((shapes, None))
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
        region: RegionSewType::from_raw(u16_at(body, 134)),
        density: f32::from(u16_at(body, 136)) / 10.0,
        gradation: u16_at(body, 138) != 0,
        direction: u16_at(body, 168),
        frequency: u16_at(body, 174),
        under_sewing: u16_at(body, 180) != 0,
        running_path: u16_at(body, 186),
        vertices,
    }))
}
