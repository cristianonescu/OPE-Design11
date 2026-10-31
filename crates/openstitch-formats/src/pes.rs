//! Brother PES: `#PESvvvv` signature, u32 offset of the embedded PEC section, an object section
//! (version dependent), then the PEC section machines sew from.
//!
//! Reading: the stitches always come from the PEC section. The object section is parsed as far
//! as public descriptions go (pyembroidery 1.5.1's `PesReader` and `PesWriter`, MIT): the
//! v4+ metadata strings, the v5+ thread table (exact RGB, names, catalogue numbers, brand,
//! chart and colour type), and
//! `CEmbOne` / `CSewSeg` stitch-group objects. Parsing stops at the first object class with no
//! public description (`CSewFigSeg`, `CEmbRect`, text and so on) and records its name in
//! [`ObjectSection::unparsed`]. A malformed object section never fails the read.
//!
//! Writing: version 6 with objects by default, or version 1 or 6, either "truncated" (no objects) or with one `CEmbOne` / `CSewSeg`
//! pair holding the whole stitch stream, byte-identical to pyembroidery's writer. Version 6
//! stores the RGB thread table, so colours and thread names survive a round trip; the PEC
//! section still carries the nearest Brother chart colours for the machine.

use crate::io::{ByteReader, ByteWriter};
use crate::{FormatError, Result, pec};
use openstitch_core::{
    Command, StitchPattern, Thread, nearest_index, pec_basic_chart, round_coordinate,
};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Metadata keys read from and written to the v4+ header, in file order.
pub const METADATA_KEYS: [&str; 5] = ["name", "category", "author", "keywords", "comments"];

const EMB_ONE: &str = "CEmbOne";
const SEW_SEG: &str = "CSewSeg";
const SECTION_END: u16 = 0x8003;

// MARK: Object section model

/// One stitch run of a `CSewSeg`: `flag` 0 = stitches, 1 = a jump (two points).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SewSegment {
    pub flag: u16,
    /// Thread table index (v5+) or PEC chart index (v1).
    pub color: u16,
    /// Points in 0.1 mm relative to the object's bottom-left corner (y down, so y ≤ 0).
    pub points: Vec<(i16, i16)>,
}

/// A `CEmbOne` stitch group and its `CSewSeg` stitch data.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbOne {
    /// Affine transform `[a, b, c, d, tx, ty]` placing the group on the design page.
    pub transform: [f32; 6],
    pub width: u16,
    pub height: u16,
    pub segments: Vec<SewSegment>,
    /// `(first segment, colour)` at every colour change.
    pub color_log: Vec<(u16, u16)>,
}

/// What OpenStitch understands of a PES object section.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectSection {
    /// The four characters after `#PES`, e.g. `0060`.
    pub version: String,
    /// `name`, `category`, `author`, `keywords`, `comments`, `pes.imageFile`, `pes.hoopName`.
    pub metadata: BTreeMap<String, String>,
    /// The v5+ thread table (empty for older versions).
    pub threads: Vec<Thread>,
    pub objects: Vec<EmbOne>,
    /// PE-DESIGN 11 (`#PES0110`) editable shapes (`CEmbLine`), in file order.
    pub shapes: Vec<crate::pes11::Shape>,
    /// PE-DESIGN 11 stitch groups (`CEmbOne`), each with its position in the object list.
    pub stitch_groups: Vec<crate::pes11::StitchGroup>,
    /// PE-DESIGN 11 texts (`CEmbNText`), each with its position in the object list.
    pub texts: Vec<crate::pes11::Text>,
    /// PE-DESIGN 11 groups: the member positions of each `CGroupObject`.
    pub groups: Vec<Vec<u16>>,
    /// PE-DESIGN 11 hole sewing pairs, `[outer, inner]` list positions.
    pub holes: Vec<[u16; 2]>,
    /// PE-DESIGN 11 Design Page size in mm, when the header had the mapped layout.
    pub page_mm: Option<(u16, u16)>,
    /// PE-DESIGN 11 Hoop Size page (header page mode 0) rather than a Custom Size page.
    pub hoop_page: bool,
    /// PE-DESIGN 11 header fields between the metadata strings and the thread table, verbatim.
    pub header: Option<Vec<u8>>,
    /// PE-DESIGN 11: the two characters after the signature's `01 00` (`10`, `01`).
    pub code: Option<[u8; 2]>,
    /// The first object class that was not parsed, if any.
    pub unparsed: Option<String>,
}

// MARK: Reading

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let magic = reader.read(8)?;
    if !magic.starts_with(b"#PES") {
        return Err(FormatError::BadSignature(
            String::from_utf8_lossy(magic).into_owned(),
        ));
    }
    let pec_offset = reader.u32le()? as usize;
    if pec_offset == 0 || pec_offset >= data.len() {
        return Err(FormatError::Truncated(format!(
            "PEC offset {pec_offset} outside file"
        )));
    }
    reader.offset = pec_offset;
    let mut pattern = pec::read_section(&mut reader)?;
    pattern.metadata.insert(
        "pes.version".into(),
        String::from_utf8_lossy(&magic[4..]).into_owned(),
    );
    if let Ok(section) = read_objects(data) {
        // A PE-DESIGN 11 table may list colours no block sews (an unused attribute's default),
        // so it is not mapped onto the PEC blocks one to one; it stays in the object section.
        if section.version != "0110" {
            apply_thread_table(&mut pattern, &section.threads);
        }
        pattern.metadata.extend(section.metadata);
    }
    Ok(pattern)
}

/// Replaces the PEC chart colours with the v5+ thread table, as pyembroidery does: one to one
/// when the table covers every colour block, otherwise table entries are handed out to distinct
/// PEC indexes in order of first use.
fn apply_thread_table(pattern: &mut StitchPattern, table: &[Thread]) {
    if table.is_empty() {
        return;
    }
    if table.len() >= pattern.threads.len() {
        pattern.threads = table.to_vec();
        return;
    }
    let mut assigned: Vec<(u32, Thread)> = Vec::new();
    let mut remaining = table.iter();
    for t in &mut pattern.threads {
        let key = t.rgb_value();
        if let Some((_, mapped)) = assigned.iter().find(|(k, _)| *k == key) {
            *t = mapped.clone();
        } else {
            let mapped = remaining.next().cloned().unwrap_or_else(|| t.clone());
            assigned.push((key, mapped.clone()));
            *t = mapped;
        }
    }
}

// MARK: Header strings

/// Windows-1252 characters for bytes 0x80–0x9F (`None`: unassigned); every other byte is the
/// Latin-1 code point.
const CP1252_HIGH: [Option<char>; 32] = [
    Some('€'),
    None,
    Some('‚'),
    Some('ƒ'),
    Some('„'),
    Some('…'),
    Some('†'),
    Some('‡'),
    Some('ˆ'),
    Some('‰'),
    Some('Š'),
    Some('‹'),
    Some('Œ'),
    None,
    Some('Ž'),
    None,
    None,
    Some('‘'),
    Some('’'),
    Some('“'),
    Some('”'),
    Some('•'),
    Some('–'),
    Some('—'),
    Some('˜'),
    Some('™'),
    Some('š'),
    Some('›'),
    Some('œ'),
    None,
    Some('ž'),
    Some('Ÿ'),
];

/// Header strings (Design Property, thread names) in the form PE-DESIGN 11 shows them: one
/// byte per character in Windows-1252, line breaks as CR LF. PE-DESIGN 11.50 showed
/// OpenStitch's former UTF-8 `é` as `Ã©` and a bare LF as no line break
/// (compat/reference/MANIFEST.json, pes-design-properties). A character outside Windows-1252
/// becomes `?`, as Windows' own conversion does.
pub fn encode_header_string(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut previous = None;
    for c in s.chars() {
        if c == '\n' && previous != Some('\r') {
            out.push(b'\r');
        }
        previous = Some(c);
        out.push(match u32::from(c) {
            v @ (0..=0x7F | 0xA0..=0xFF) => v as u8,
            _ => CP1252_HIGH
                .iter()
                .position(|&h| h == Some(c))
                .map_or(b'?', |i| 0x80 + i as u8),
        });
    }
    out
}

/// The inverse of [`encode_header_string`]: CR LF becomes `\n`. Valid UTF-8 is read as UTF-8,
/// so files from earlier OpenStitch versions and from pyembroidery (both UTF-8) still read as
/// written; anything else is Windows-1252. The one string this misreads is Windows-1252 text
/// that happens to be valid UTF-8 (such as a literal `Ã©`).
pub fn decode_header_string(bytes: &[u8]) -> String {
    let s = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes
            .iter()
            .map(|&b| match b {
                0x80..=0x9F => CP1252_HIGH[usize::from(b - 0x80)].unwrap_or('\u{FFFD}'),
                _ => char::from(b),
            })
            .collect(),
    };
    s.replace("\r\n", "\n")
}

/// A length-prefixed string as MFC's `CArchive` stores a `CString`: a u8 length (`FF` then a
/// u16 for 255 or more, `FFFF` then a u32 beyond that) and single-byte text, or, for Unicode,
/// `FF FEFF` first and then the length in UTF-16 units and UTF-16LE text. PE-DESIGN 11 writes
/// the v6 thread table's strings in the Unicode form (tests/fixtures/pe_design_v6_trims.pes).
pub(crate) fn read_string8(r: &mut ByteReader) -> Result<Option<String>> {
    let mut unicode = false;
    let mut len = u32::from(r.u8()?);
    if len == 0xFF {
        len = r.u16le()?;
        if len == 0xFFFE {
            unicode = true;
            len = u32::from(r.u8()?);
            if len == 0xFF {
                len = r.u16le()?;
            }
        }
        if len == 0xFFFF {
            len = r.u32le()?;
        }
    }
    let len = len as usize;
    let s = if unicode {
        let units: Vec<u16> = r
            .read(len.saturating_mul(2))?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        String::from_utf16_lossy(&units).replace("\r\n", "\n")
    } else {
        decode_header_string(r.read(len)?)
    };
    Ok(Some(s).filter(|s| !s.is_empty()))
}

fn read_string16(r: &mut ByteReader) -> Result<String> {
    let len = r.u16le()? as usize;
    Ok(String::from_utf8_lossy(r.read(len)?).into_owned())
}

/// Parses the object section. Errors only for truncated or inconsistent data inside the parts
/// it understands; [`read`] ignores them.
pub fn read_objects(data: &[u8]) -> Result<ObjectSection> {
    let mut r = ByteReader::new(data);
    let magic = r.read(8)?;
    if !magic.starts_with(b"#PES") {
        return Err(FormatError::BadSignature(
            String::from_utf8_lossy(magic).into_owned(),
        ));
    }
    let version = String::from_utf8_lossy(&magic[4..]).into_owned();
    let pec_offset = r.u32le()? as usize;
    let mut section = ObjectSection {
        version: version.clone(),
        ..ObjectSection::default()
    };
    // Bytes skipped after the metadata, after the hoop name (v9+) and before the pattern counts
    // (pyembroidery's per-version offsets).
    let layout: Option<(usize, Option<usize>, usize)> = match version.as_str() {
        "0001" => {
            r.skip(4)?; // scale to fit, hoop
            read_blocks(&mut r, pec_offset, &mut section)?;
            return Ok(section);
        }
        "0040" => {
            r.skip(4)?;
            read_metadata(&mut r, &mut section)?;
            return Ok(section);
        }
        "0050" | "0055" | "0056" => Some((24, None, 24)),
        "0060" | "0070" => Some((36, None, 24)),
        "0080" => Some((38, None, 26)),
        "0090" => Some((14, Some(30), 34)),
        "0100" => Some((14, Some(38), 34)),
        // PE-DESIGN 11 (`#PES0110`). The header up to the metadata has the same documented shape
        // as 0040+ (pec offset, `01 00`, a two-char code, then the five length-prefixed strings),
        // confirmed against owner reference files in compat/reference/pes/. After it, the
        // editable shapes of the object list (crate::pes11; the hoop, page and thread-table
        // fields in between are not all mapped). Stitches and chart colours still come from the
        // PEC section. See docs/FILE_FORMATS.md.
        "0110" => {
            r.skip(4)?;
            read_metadata(&mut r, &mut section)?;
            let objects = crate::pes11::read_objects(data, r.offset, pec_offset)?;
            section.threads = objects.threads;
            section.shapes = objects.shapes;
            section.texts = objects.texts;
            section.stitch_groups = objects.stitch_groups;
            section.groups = objects.groups;
            section.holes = objects.holes;
            section.page_mm = objects.page_mm;
            section.hoop_page = objects.hoop_page;
            section.header = objects.header;
            section.code = objects.code;
            section.unparsed = objects.unparsed;
            return Ok(section);
        }
        _ => None,
    };
    let Some((first, hoop_then, before_counts)) = layout else {
        return Ok(section);
    };
    r.skip(4)?;
    read_metadata(&mut r, &mut section)?;
    r.skip(first)?;
    if let Some(after_hoop) = hoop_then {
        if let Some(hoop) = read_string8(&mut r)? {
            section.metadata.insert("pes.hoopName".into(), hoop);
        }
        r.skip(after_hoop)?;
    }
    if let Some(image) = read_string8(&mut r)? {
        section.metadata.insert("pes.imageFile".into(), image);
    }
    r.skip(before_counts)?;
    for what in ["programmable fill", "motif", "feather"] {
        if r.u16le()? != 0 {
            section.unparsed = Some(format!("{what} patterns"));
            return Ok(section);
        }
    }
    let threads = r.u16le()?;
    for _ in 0..threads {
        let catalog = read_string8(&mut r)?;
        let rgb = r.read(3)?;
        r.skip(1)?;
        let kind = r.u32le()?;
        let name = read_string8(&mut r)?;
        let brand = read_string8(&mut r)?;
        let chart = read_string8(&mut r)?;
        let mut t =
            Thread::rgb(u32::from(rgb[0]) << 16 | u32::from(rgb[1]) << 8 | u32::from(rgb[2]));
        t.name = name.map(Cow::Owned);
        t.catalog_number = catalog.map(Cow::Owned);
        t.brand = brand.map(Cow::Owned);
        t.chart = chart.map(Cow::Owned);
        t.kind = Some(kind).filter(|&k| k != Thread::CUSTOM_KIND);
        section.threads.push(t);
    }
    read_blocks(&mut r, pec_offset, &mut section)?;
    Ok(section)
}

fn read_metadata(r: &mut ByteReader, section: &mut ObjectSection) -> Result<()> {
    for key in METADATA_KEYS {
        if let Some(v) = read_string8(r)? {
            section.metadata.insert(key.into(), v);
        }
    }
    Ok(())
}

/// Object count, then `FFFF 0000`, then class-named objects separated by `FFFF 0000` and ended
/// by `0000 0000`.
fn read_blocks(r: &mut ByteReader, end: usize, section: &mut ObjectSection) -> Result<()> {
    let count = r.u16le()?;
    if count == 0 || r.offset >= end {
        return Ok(());
    }
    if r.u32le()? != 0x0000_FFFF {
        return Ok(());
    }
    loop {
        if r.offset >= end {
            return Ok(());
        }
        let class = read_string16(r)?;
        if class != EMB_ONE {
            section.unparsed = Some(class);
            return Ok(());
        }
        r.skip(16)?;
        let mut transform = [0_f32; 6];
        for v in &mut transform {
            *v = r.f32le()?;
        }
        r.skip(6)?;
        let width = r.u16le()? as u16;
        let height = r.u16le()? as u16;
        r.skip(8)?;
        let sections = r.u16le()?;
        if r.u32le()? != 0x0000_FFFF {
            return Err(FormatError::ValueOutOfRange(
                "PES: CEmbOne not followed by its CSewSeg".into(),
            ));
        }
        let class = read_string16(r)?;
        if class != SEW_SEG {
            section.unparsed = Some(class);
            return Ok(());
        }
        let mut segments = Vec::new();
        for i in 0..sections {
            if i > 0 && r.u16le()? != u32::from(SECTION_END) {
                return Err(FormatError::ValueOutOfRange(
                    "PES: CSewSeg section separator".into(),
                ));
            }
            let flag = r.u16le()? as u16;
            let color = r.u16le()? as u16;
            let n = r.u16le()? as usize;
            let raw = r.read(n * 4)?;
            let points = raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| {
                    (
                        i16::from_le_bytes([c[0], c[1]]),
                        i16::from_le_bytes([c[2], c[3]]),
                    )
                })
                .collect();
            segments.push(SewSegment {
                flag,
                color,
                points,
            });
        }
        let log_len = r.u16le()?;
        let mut color_log = Vec::new();
        for _ in 0..log_len {
            color_log.push((r.u16le()? as u16, r.u16le()? as u16));
        }
        section.objects.push(EmbOne {
            transform,
            width,
            height,
            segments,
            color_log,
        });
        if r.u32le()? != 0x0000_FFFF {
            return Ok(()); // 0000 0000: no more objects
        }
    }
}

// MARK: Writing

/// PES versions OpenStitch writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Version {
    V1,
    #[default]
    V6,
    /// PE-DESIGN 11's `#PES0110` with editable objects: written by [`crate::pes11::write`]
    /// from a design's objects, not from a stitch pattern ([`write_with`] refuses it).
    V11,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriteOptions {
    pub version: Version,
    /// No objects: just the header and the PEC section.
    pub truncated: bool,
    /// Hoop width × height in mm for the v6 header's hoop fields; `None` writes 100 × 100
    /// (pyembroidery's constant). PE-DESIGN 11.50 stores the Design Page's hoop there as two
    /// u16 mm values followed by the hoop name (owner's `#PES0110` files: 100, 100, "nv880e-S"
    /// for a 100 × 100 mm page; issue #125).
    pub hoop_mm: Option<(u16, u16)>,
    /// The v6 header's "Hoop Rotation" field (EduTechWiki: "90° 1 = rotate"): the hoop is
    /// turned 90°, as `machine.set` `rotate90` does. Written as 0 when false.
    pub hoop_rotated: bool,
    /// The v6 header's "Design Page Is Custom" field: the Design Page has a custom size rather
    /// than a hoop's. Written as 0 when false (pyembroidery's constant).
    pub custom_page: bool,
    /// The custom Design Page size in mm, for the v6 header's sixth and seventh u16
    /// (EduTechWiki's "DesignWidth" / "DesignHeight", typical 200): PE-DESIGN 11.50 reads a
    /// custom page's size there (it showed pyembroidery's constant as a 200 x 200 Custom Size
    /// page, reference session 2026-10-30). `None` keeps 200 x 200.
    pub page_mm: Option<(u16, u16)>,
}

impl Default for WriteOptions {
    /// Version 6 with the stitch-group object and the RGB thread table: the form every
    /// user-facing path writes. `1t` (version 1, no objects) stays an explicit "machine only,
    /// smallest" choice.
    fn default() -> Self {
        Self {
            version: Version::V6,
            truncated: false,
            hoop_mm: None,
            hoop_rotated: false,
            custom_page: false,
            page_mm: None,
        }
    }
}

impl WriteOptions {
    /// The `pesVersion` string of [`WriteOptions::default`].
    pub const DEFAULT_NAME: &str = "6";

    /// The version strings [`parse`](Self::parse) accepts, for messages.
    pub const NAMES: &str = "1t, 1, 6, 6t or 11";

    /// Parses pyembroidery's version strings `1`, `1t`, `6`, `6t`, and `11` for PE-DESIGN 11's
    /// editable `#PES0110` (no `t` form: its point is the objects).
    pub fn parse(s: &str) -> Option<Self> {
        if s == "11" {
            return Some(Self {
                version: Version::V11,
                ..Self::default()
            });
        }
        let (v, truncated) = match s.strip_suffix('t') {
            Some(v) => (v, true),
            None => (s, false),
        };
        let version = match v {
            "1" => Version::V1,
            "6" => Version::V6,
            _ => return None,
        };
        Some(Self {
            version,
            truncated,
            ..Self::default()
        })
    }
}

pub fn write(pattern: &StitchPattern) -> Result<Vec<u8>> {
    write_with(pattern, &WriteOptions::default())
}

pub fn write_with(input: &StitchPattern, options: &WriteOptions) -> Result<Vec<u8>> {
    if options.version == Version::V11 {
        return Err(FormatError::UnsupportedFormat(
            "PES 0110 is written from a design's editable objects (pes11::write), not from stitches"
                .into(),
        ));
    }
    let mut w = ByteWriter::default();
    if options.version == Version::V1 && options.truncated {
        w.ascii("#PES0001");
        w.u32le(0x16);
        w.fill(0, 10);
        pec::write_section(input, &mut w)?;
        return Ok(w.bytes);
    }
    let pattern = pec::prepare(input);
    w.ascii(match options.version {
        Version::V1 => "#PES0001",
        Version::V6 | Version::V11 => "#PES0060",
    });
    let pec_placeholder = w.len();
    w.u32le(0);
    let has_blocks = !options.truncated && !pattern.stitches.is_empty();
    match options.version {
        Version::V1 => {
            w.u16le(1); // scale to fit
            w.u16le(1); // hoop
        }
        Version::V6 | Version::V11 => write_header_v6(&mut w, &pattern, options)?,
    }
    if options.truncated {
        w.u16le(0);
        w.fill(0, 5);
        w.u16le(0);
        w.u16le(0);
    } else if has_blocks {
        w.u16le(1);
        w.u16le(0xFFFF);
        w.u16le(0);
        let color_log = write_blocks(&mut w, &pattern, options.version);
        if options.version == Version::V6 {
            w.u32le(0);
            w.u32le(0);
            for i in 0..color_log.len() {
                w.u32le(i as i64);
                w.u32le(0);
            }
        }
    } else {
        w.u16le(0);
        w.u16le(0);
        w.u16le(0);
    }
    w.patch32le(pec_placeholder, w.len() as i64);
    let list = pec::write_section(&pattern, &mut w)?;
    if options.version == Version::V6 {
        if options.truncated {
            write_addendum(&mut w, &[0xFF], &[]);
        } else {
            write_addendum(&mut w, &list, &pattern.threads);
        }
        w.u16le(0);
    }
    Ok(w.bytes)
}

pub(crate) fn write_string8(w: &mut ByteWriter, s: Option<&str>) {
    let Some(s) = s else {
        w.u8(0);
        return;
    };
    let mut bytes = encode_header_string(s);
    bytes.truncate(255);
    if bytes.last() == Some(&b'\r') {
        bytes.pop(); // don't leave half a line break
    }
    w.u8(bytes.len() as i64);
    w.append(&bytes);
}

fn write_string16(w: &mut ByteWriter, s: &str) {
    w.u16le(s.len() as i64);
    w.ascii(s);
}

fn write_header_v6(
    w: &mut ByteWriter,
    pattern: &StitchPattern,
    options: &WriteOptions,
) -> Result<()> {
    w.u16le(1); // hoop
    w.ascii("02");
    for key in METADATA_KEYS {
        write_string8(w, pattern.metadata.get(key).map(String::as_str));
    }
    let (hoop_w, hoop_h) = options
        .hoop_mm
        .map_or((0x64, 0x64), |(w, h)| (i64::from(w), i64::from(h)));
    // EduTechWiki's v6 fields: optimize hoop change, design page is custom, hoop width and
    // height (mm), hoop rotation, design width and height (the custom page size PE-DESIGN
    // shows), then page sizes, colours, grid and display fields, which keep pyembroidery's
    // values.
    let (custom, rotated) = (
        i64::from(options.custom_page),
        i64::from(options.hoop_rotated),
    );
    let (page_w, page_h) = options
        .page_mm
        .map_or((0xC8, 0xC8), |(w, h)| (i64::from(w), i64::from(h)));
    for v in [
        0, custom, hoop_w, hoop_h, rotated, page_w, page_h, 0x64, 0x64, 0x64, 0x07, 0x13, 1, 1, 0,
        100, 1, 0,
    ] {
        w.u16le(v);
    }
    w.u8(0); // image file name
    for v in [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] {
        w.f32le(v);
    }
    w.u16le(0); // programmable fills
    w.u16le(0); // motifs
    w.u16le(0); // feather patterns
    if pattern.threads.len() > usize::from(u16::MAX) {
        return Err(FormatError::ValueOutOfRange("PES: too many threads".into()));
    }
    w.u16le(pattern.threads.len() as i64);
    for t in &pattern.threads {
        write_string8(w, t.catalog_number.as_deref());
        w.append(&[t.red, t.green, t.blue, 0]);
        w.u32le(i64::from(t.kind.unwrap_or(Thread::CUSTOM_KIND)));
        write_string8(w, t.name.as_deref());
        write_string8(w, t.brand.as_deref());
        write_string8(w, t.chart.as_deref());
    }
    Ok(())
}

/// After a v6 PEC section: the PEC colour list padded to 128 bytes with spaces, 0x90 zero bytes
/// per thread, then each thread's RGB as u24 little-endian.
fn write_addendum(w: &mut ByteWriter, list: &[u8], threads: &[Thread]) {
    w.append(list);
    w.fill(0x20, 128_usize.saturating_sub(list.len()));
    for _ in threads {
        w.fill(0, 0x90);
    }
    for t in threads {
        w.u24le(i64::from(t.rgb_value()));
    }
}

/// One `CEmbOne` + `CSewSeg` pair for the whole pattern; returns the colour log.
fn write_blocks(w: &mut ByteWriter, pattern: &StitchPattern, version: Version) -> Vec<(u16, u16)> {
    // Bounds over every record, as pyembroidery computes them.
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for s in &pattern.stitches {
        let (x, y) = (round_coordinate(s.x) as f64, round_coordinate(s.y) as f64);
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let (width, height) = (max_x - min_x, max_y - min_y);

    write_string16(w, EMB_ONE);
    w.fill(0, 16);
    let trans_x = 350.0 + 1300.0 / 2.0 - width / 2.0;
    let trans_y = 100.0 + height + 1800.0 / 2.0 - height / 2.0;
    for v in [1.0, 0.0, 0.0, 1.0, trans_x, trans_y] {
        w.f32le(v);
    }
    w.u16le(1);
    w.u16le(0);
    w.u16le(0);
    w.u16le(width as i64);
    w.u16le(height as i64);
    w.fill(0, 8);
    let sections_at = w.len();
    w.u16le(0);
    w.u16le(0xFFFF);
    w.u16le(0);

    write_string16(w, SEW_SEG);
    let segments = segments(pattern, version, min_x, max_y);
    let mut log: Vec<(u16, u16)> = Vec::new();
    for (i, seg) in segments.iter().enumerate() {
        if i > 0 {
            w.u16le(i64::from(SECTION_END));
        }
        if log.last().is_none_or(|&(_, c)| c != seg.color) {
            log.push((i as u16, seg.color));
        }
        w.u16le(i64::from(seg.flag));
        w.u16le(i64::from(seg.color));
        w.u16le(seg.points.len() as i64);
        for &(x, y) in &seg.points {
            w.u16le(i64::from(x));
            w.u16le(i64::from(y));
        }
    }
    w.u16le(log.len() as i64);
    for &(section, color) in &log {
        w.u16le(i64::from(section));
        w.u16le(i64::from(color));
    }
    w.patch16le(sections_at, segments.len() as i64);
    w.u16le(0);
    w.u16le(0);
    log
}

/// The stitch stream as `CSewSeg` runs: consecutive stitches form one run, a run of jumps
/// becomes a two-point jump from the last stitch, colour changes switch the thread. A run
/// after a jump starts at the jump's landing point: PE-DESIGN 11 sews only the points of
/// stitch runs, so without it the needle never goes down where the jump lands.
pub(crate) fn segments(
    pattern: &StitchPattern,
    version: Version,
    min_x: f64,
    max_y: f64,
) -> Vec<SewSegment> {
    let chart: Vec<Option<Thread>> = match version {
        Version::V1 => pec_basic_chart().to_vec(),
        Version::V6 | Version::V11 => pattern.threads.iter().cloned().map(Some).collect(),
    };
    let code = |i: usize| {
        pattern
            .threads
            .get(i)
            .and_then(|t| nearest_index(t, &chart))
            .unwrap_or(0) as u16
    };
    let point = |x: f64, y: f64| {
        (
            (round_coordinate(x) as f64 - min_x) as i16,
            (round_coordinate(y) as f64 - max_y) as i16,
        )
    };
    let mut thread = 0;
    let mut color = code(0);
    let (mut sx, mut sy) = (0.0, 0.0);
    let mut landing = None;
    let mut out = Vec::new();
    let stitches = &pattern.stitches;
    let mut start = 0;
    while start < stitches.len() {
        let command = stitches[start].command;
        let end = stitches[start..]
            .iter()
            .position(|s| s.command != command)
            .map_or(stitches.len(), |n| start + n);
        let run = &stitches[start..end];
        start = end;
        match command {
            Command::Jump => {
                let last = run[run.len() - 1];
                landing = Some(point(last.x, last.y));
                // PE-DESIGN 11's own v6 files start with the first stitch run: no jump segment
                // from the origin (tests/fixtures/pe_design_v6_trims.pes).
                if out.is_empty() {
                    continue;
                }
                out.push(SewSegment {
                    flag: 1,
                    color,
                    points: vec![point(sx, sy), point(last.x, last.y)],
                });
            }
            Command::ColorChange => {
                thread += 1;
                color = code(thread);
            }
            Command::Stitch => {
                let last = run[run.len() - 1];
                (sx, sy) = (last.x, last.y);
                let mut points: Vec<_> = run.iter().map(|s| point(s.x, s.y)).collect();
                if let Some(p) = landing.take()
                    && points.first() != Some(&p)
                {
                    points.insert(0, p);
                }
                out.push(SewSegment {
                    flag: 0,
                    color,
                    points,
                });
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string8(s: &str) -> Vec<u8> {
        let mut v = vec![s.len() as u8];
        v.extend_from_slice(s.as_bytes());
        v
    }

    #[test]
    fn pes_0110_header_metadata_is_read() {
        // Minimal `#PES0110` (PE-DESIGN 11) header: pec offset, `01 00`, a two-char code, then
        // the five length-prefixed metadata strings, in METADATA_KEYS order. The layout up to the
        // metadata matches the documented 0040+ shape and is confirmed against the owner's
        // PE-DESIGN 11 reference files in compat/reference/pes/. Before the 0110 branch the reader
        // left this metadata unparsed and the harness fell back to the PEC label.
        let mut data = b"#PES0110".to_vec();
        data.extend_from_slice(&32u32.to_le_bytes()); // pec offset (unused by the 0110 branch)
        data.extend_from_slice(&[0x01, 0x00, b'0', b'1']);
        for s in ["Rose", "Flowers", "Ana", "red", "hi"] {
            data.extend(string8(s));
        }
        let section = read_objects(&data).expect("read_objects");
        assert_eq!(section.version, "0110");
        let get = |k: &str| section.metadata.get(k).map(String::as_str);
        assert_eq!(get("name"), Some("Rose"));
        assert_eq!(get("category"), Some("Flowers"));
        assert_eq!(get("author"), Some("Ana"));
        assert_eq!(get("keywords"), Some("red"));
        assert_eq!(get("comments"), Some("hi"));
    }

    #[test]
    fn stitch_runs_after_a_jump_start_at_the_landing() {
        // PE-DESIGN 11.50 sews only the points of CSewSeg stitch runs: a run that started after
        // the jump's landing lost that needle point when PE-DESIGN reopened the file
        // (compat/reference/pes/pes-default-v6.pes, MANIFEST notes).
        let mut p = StitchPattern::new();
        p.jump_to(0.0, 0.0);
        p.stitch_to(25.0, 0.0);
        p.stitch_to(50.0, 0.0);
        p.jump_to(100.0, 0.0);
        p.stitch_to(125.0, 0.0);
        let segs = segments(&p, Version::V6, 0.0, 0.0);
        let runs: Vec<_> = segs.iter().filter(|s| s.flag == 0).collect();
        assert_eq!(runs[0].points, [(0, 0), (25, 0), (50, 0)]);
        assert_eq!(runs[1].points, [(100, 0), (125, 0)]);
        // A run that already starts at the landing is not given a second copy.
        let mut q = StitchPattern::new();
        q.jump_to(0.0, 0.0);
        q.stitch_to(0.0, 0.0);
        q.stitch_to(25.0, 0.0);
        let segs = segments(&q, Version::V6, 0.0, 0.0);
        assert_eq!(
            segs.iter()
                .find(|s| s.flag == 0)
                .expect("stitch run")
                .points,
            [(0, 0), (25, 0)]
        );
    }
}
