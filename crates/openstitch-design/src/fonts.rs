//! Fonts a design carries with it, kept inside the `.osd` so a design reopens anywhere:
//! user-chosen TrueType/OpenType files (PE-DESIGN's "TrueType fonts") and OpenStitch
//! embroidery fonts ([`EmbroideryFont`]), whose glyphs are outlines (Font Creator) or whole
//! stitch designs (user-mapped text).
//!
//! Font files are hostile input. A file is accepted only when it is at most
//! [`MAX_FONT_BYTES`], `ttf-parser` (pure safe Rust, bounds-checked) parses it, it has a Unicode
//! character map and at least one outlined glyph; a design holds at most [`MAX_DESIGN_FONTS`]
//! fonts and [`MAX_TOTAL_FONT_BYTES`] in all. Layout caps the outline size of each glyph
//! ([`MAX_GLYPH_SEGMENTS`]) and of a whole text ([`MAX_LAYOUT_SEGMENTS`]).
//!
//! There is no shaping engine: each character maps to one glyph through the font's `cmap`, laid
//! out left to right. That is right for Latin, Greek, Cyrillic, CJK ideographs, kana and
//! precomposed Hangul, but not for scripts that need contextual shaping; [`shaping_note`] names
//! them so commands can warn (see ARCHITECTURE.md → Lettering).

use crate::geometry::Contour;
use crate::model::StitchBlock;
use openstitch_core::Command;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;
use ttf_parser::{Face, name_id};

/// Largest font file accepted, bytes.
pub const MAX_FONT_BYTES: usize = 24 << 20;
/// Most fonts one design may carry.
pub const MAX_DESIGN_FONTS: usize = 16;
/// Most font bytes one design may carry (keeps `.osd` files under their 64 MiB limit).
pub const MAX_TOTAL_FONT_BYTES: usize = 40 << 20;
/// Most outline segments one glyph may have; a glyph above it is treated as missing.
pub const MAX_GLYPH_SEGMENTS: usize = 20_000;
/// Most outline segments one laid-out text may have.
pub const MAX_LAYOUT_SEGMENTS: usize = 600_000;

/// Raw font file bytes, shared between copies of a design (undo history clones designs) and
/// stored base64-encoded in JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontBytes(pub Arc<[u8]>);

impl FontBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        FontBytes(bytes.into())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for FontBytes {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use base64::Engine;
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for FontBytes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use base64::Engine;
        let text = String::deserialize(d)?;
        if text.len() > MAX_FONT_BYTES / 3 * 4 + 4 {
            return Err(serde::de::Error::custom("embedded font too large"));
        }
        base64::engine::general_purpose::STANDARD
            .decode(text.as_bytes())
            .map(FontBytes::new)
            .map_err(|_| serde::de::Error::custom("embedded font is not base64"))
    }
}

/// What a design font is made of.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FontData {
    /// A TrueType/OpenType file kept unchanged; `face` picks the font of a collection (.ttc).
    #[serde(rename_all = "camelCase")]
    TrueType {
        #[serde(default)]
        face: u32,
        bytes: FontBytes,
    },
    /// An OpenStitch embroidery font.
    Embroidery(EmbroideryFont),
}

/// Most glyphs an embroidery font may have.
pub const MAX_FONT_GLYPHS: usize = 4096;
/// Most needle points one stitch glyph may have.
pub const MAX_GLYPH_STITCHES: usize = 100_000;
/// Most needle points all stitch glyphs of a font may have.
pub const MAX_FONT_STITCHES: usize = 1_000_000;
/// Most outline points all glyphs of a font may have.
pub const MAX_FONT_POINTS: usize = 1_000_000;
/// Largest coordinate in a glyph, 0.1 mm (5 m).
const MAX_GLYPH_COORD: f64 = 50_000.0;

/// An OpenStitch embroidery font (the format is documented in docs/FILE_FORMATS.md →
/// OpenStitch font). Glyph coordinates are 0.1 mm at the font's own size, +y down like the
/// Design Page, with the origin on the baseline at the glyph's left set line; text set at size
/// `s` mm scales them by `s * 10 / cap_height`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbroideryFont {
    /// Standard character height: baseline to capital line, 0.1 mm.
    pub cap_height: f64,
    /// Space above the baseline a line takes, 0.1 mm.
    pub ascender: f64,
    /// Space below the baseline a line takes, 0.1 mm (positive).
    pub descender: f64,
    /// Extra space between lines, 0.1 mm.
    #[serde(default)]
    pub line_gap: f64,
    /// Sorted by character, one per character.
    #[serde(default)]
    pub glyphs: Vec<EmbroideryGlyph>,
}

/// One character of an [`EmbroideryFont`]: outlines sewn with the text object's sewing
/// attributes, a stitch design sewn as registered, or both.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbroideryGlyph {
    #[serde(rename = "char")]
    pub ch: char,
    /// Distance from the left to the right set line, 0.1 mm.
    pub advance: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contours: Vec<Contour>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stitches: Option<StitchBlock>,
}

impl EmbroideryFont {
    /// An empty font whose capitals are `cap_height` (0.1 mm) tall.
    pub fn new(cap_height: f64) -> Self {
        EmbroideryFont {
            cap_height,
            ascender: cap_height * 1.2,
            descender: cap_height * 0.3,
            line_gap: 0.0,
            glyphs: Vec::new(),
        }
    }

    pub fn glyph(&self, ch: char) -> Option<&EmbroideryGlyph> {
        self.glyphs
            .binary_search_by_key(&ch, |g| g.ch)
            .ok()
            .map(|i| &self.glyphs[i])
    }

    /// Adds `glyph`, replacing the glyph of the same character.
    pub fn set_glyph(&mut self, glyph: EmbroideryGlyph) {
        match self.glyphs.binary_search_by_key(&glyph.ch, |g| g.ch) {
            Ok(i) => self.glyphs[i] = glyph,
            Err(i) => self.glyphs.insert(i, glyph),
        }
    }

    /// Removes the glyph of `ch`; whether there was one.
    pub fn remove_glyph(&mut self, ch: char) -> bool {
        match self.glyphs.binary_search_by_key(&ch, |g| g.ch) {
            Ok(i) => {
                self.glyphs.remove(i);
                true
            }
            Err(_) => false,
        }
    }

    /// Needle points of every stitch glyph.
    pub fn stitch_count(&self) -> usize {
        self.glyphs
            .iter()
            .filter_map(|g| g.stitches.as_ref())
            .map(|b| b.stitches.len())
            .sum()
    }

    /// Checks a font from an untrusted file against the format's rules and limits.
    pub fn validate(&self) -> Result<(), String> {
        let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
        if !ok(self.cap_height, 10.0, 5000.0) {
            return Err("capHeight must be 1-500 mm".into());
        }
        if ![self.ascender, self.descender, self.line_gap]
            .iter()
            .all(|&v| ok(v, 0.0, 20_000.0))
        {
            return Err("ascender, descender and lineGap must be 0-2000 mm".into());
        }
        if self.glyphs.len() > MAX_FONT_GLYPHS {
            return Err(format!("more than {MAX_FONT_GLYPHS} glyphs"));
        }
        if self.glyphs.windows(2).any(|w| w[0].ch >= w[1].ch) {
            return Err("glyphs must be sorted by character, one per character".into());
        }
        let coord = |x: f64, y: f64| {
            ok(x, -MAX_GLYPH_COORD, MAX_GLYPH_COORD) && ok(y, -MAX_GLYPH_COORD, MAX_GLYPH_COORD)
        };
        let (mut points, mut stitches) = (0usize, 0usize);
        for g in &self.glyphs {
            let bad = |what: &str| Err(format!("glyph {:?}: {what}", g.ch));
            if g.ch.is_control() {
                return bad("control characters cannot be glyphs");
            }
            if !ok(g.advance, 0.0, MAX_GLYPH_COORD) {
                return bad("advance out of range");
            }
            for c in &g.contours {
                points += c.points.len() + 2 * c.curves.len();
                if c.points.is_empty()
                    || c.curves.len() > c.segment_count()
                    || !c.points.iter().all(|p| coord(p.x, p.y))
                    || !c
                        .curves
                        .iter()
                        .flatten()
                        .all(|b| coord(b.c1.x, b.c1.y) && coord(b.c2.x, b.c2.y))
                {
                    return bad("bad outline");
                }
            }
            if let Some(b) = &g.stitches {
                if b.stitches.len() > MAX_GLYPH_STITCHES {
                    return bad("too many stitches");
                }
                if b.threads.is_empty() || b.threads.len() > 256 {
                    return bad("stitches need 1-256 threads");
                }
                stitches += b.stitches.len();
                if !b.stitches.iter().all(|s| coord(s.x, s.y)) {
                    return bad("bad stitch coordinates");
                }
                if !b.stitches.iter().any(|s| s.command == Command::Stitch) {
                    return bad("stitch data without stitches");
                }
            }
        }
        if points > MAX_FONT_POINTS {
            return Err(format!("more than {MAX_FONT_POINTS} outline points"));
        }
        if stitches > MAX_FONT_STITCHES {
            return Err(format!("more than {MAX_FONT_STITCHES} stitches"));
        }
        Ok(())
    }
}

/// The character a stitch file is registered for in a batch import, from its file name
/// (without extension), after PE-DESIGN's rule for multiple import: `u_A` upper case, `l_a`
/// lower case, `s_bs` `\`, `s_sl` `/`, `s_cn` `:`, `s_as` `*`, `s_qm` `?`, `s_dq` `"`, `s_lt` `<`,
/// `s_gt` `>`, `s_vb` `|`. OpenStitch also takes a name that is a single character and
/// `U+XXXX` for any Unicode character.
pub fn char_for_file_stem(stem: &str) -> Option<char> {
    let one = |s: &str| {
        let mut it = s.chars();
        match (it.next(), it.next()) {
            (Some(c), None) if !c.is_control() => Some(c),
            _ => None,
        }
    };
    if let Some(rest) = stem.strip_prefix("u_") {
        return one(rest).filter(|c| !c.is_lowercase());
    }
    if let Some(rest) = stem.strip_prefix("l_") {
        return one(rest).filter(|c| !c.is_uppercase());
    }
    if let Some(rest) = stem.strip_prefix("s_") {
        return Some(match rest {
            "bs" => '\\',
            "sl" => '/',
            "cn" => ':',
            "as" => '*',
            "qm" => '?',
            "dq" => '"',
            "lt" => '<',
            "gt" => '>',
            "vb" => '|',
            _ => return None,
        });
    }
    if let Some(hex) = stem.strip_prefix("U+").or_else(|| stem.strip_prefix("u+")) {
        return u32::from_str_radix(hex, 16)
            .ok()
            .and_then(char::from_u32)
            .filter(|c| !c.is_control());
    }
    one(stem)
}

/// A font stored in the design (`Design::fonts`), referred to by [`TextSpec::font`]
/// (crate::TextSpec) like a bundled font.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignFont {
    /// Unique among bundled and design fonts: lower-case letters, digits, `-` and `_`.
    pub id: String,
    pub family: String,
    #[serde(default)]
    pub style: String,
    /// File name it was loaded from (informational).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub data: FontData,
}

impl DesignFont {
    pub fn kind(&self) -> &'static str {
        match self.data {
            FontData::TrueType { .. } => "trueType",
            FontData::Embroidery(_) => "embroidery",
        }
    }

    /// Bytes the font takes (estimated for embroidery fonts), for the per-design limit.
    pub fn byte_size(&self) -> usize {
        match &self.data {
            FontData::TrueType { bytes, .. } => bytes.len(),
            FontData::Embroidery(f) => {
                let points: usize = f
                    .glyphs
                    .iter()
                    .flat_map(|g| &g.contours)
                    .map(|c| c.points.len() + 2 * c.curves.len())
                    .sum();
                64 * f.glyphs.len() + 24 * (points + f.stitch_count())
            }
        }
    }
}

/// Most fonts a collection file (.ttc/.otc) may hold.
pub const MAX_COLLECTION_FACES: u32 = 64;

/// Fonts in `bytes` (1 for a single font), or `None` when a collection header is damaged:
/// `ttf-parser` asserts (panics) on offset tables that run past the file, so collection headers
/// are checked here first.
pub fn face_count(bytes: &[u8]) -> Option<u32> {
    if bytes.get(..4) != Some(b"ttcf") {
        return Some(1);
    }
    let n = u32::from_be_bytes(bytes.get(8..12)?.try_into().ok()?);
    if n == 0 || n > MAX_COLLECTION_FACES {
        return None;
    }
    let table_end = 12 + 4 * n as usize;
    let offsets = bytes.get(12..table_end)?;
    offsets
        .as_chunks::<4>()
        .0
        .iter()
        .all(|o| {
            let at = u32::from_be_bytes(*o) as usize;
            at >= table_end && at.checked_add(12).is_some_and(|end| end <= bytes.len())
        })
        .then_some(n)
}

/// Parses face `face` of an untrusted font file. Damaged collection headers are refused up
/// front and any panic inside the parser is contained, so no font file can bring the
/// application down.
pub fn parse_face(bytes: &[u8], face: u32) -> Option<Face<'_>> {
    if bytes.len() > MAX_FONT_BYTES || face >= face_count(bytes)? {
        return None;
    }
    std::panic::catch_unwind(|| Face::parse(bytes, face).ok())
        .ok()
        .flatten()
}

/// Whether every table of face `face`'s directory lies inside the file (a truncated file keeps
/// a directory pointing past its end; the parser would quietly drop those tables).
fn tables_in_bounds(bytes: &[u8], face: u32) -> bool {
    let be32 = |at: usize| -> Option<usize> {
        Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    let start = if bytes.get(..4) == Some(b"ttcf") {
        match be32(12 + 4 * face as usize) {
            Some(at) => at,
            None => return false,
        }
    } else {
        0
    };
    let Some(n) = bytes
        .get(start + 4..start + 6)
        .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
    else {
        return false;
    };
    n > 0
        && (0..n).all(|i| {
            let record = start + 12 + 16 * i;
            match (be32(record + 8), be32(record + 12)) {
                (Some(at), Some(len)) => at.checked_add(len).is_some_and(|end| end <= bytes.len()),
                _ => false,
            }
        })
}

/// What [`check_truetype`] found in a font file.
#[derive(Clone, Debug, PartialEq)]
pub struct TrueTypeInfo {
    pub family: String,
    pub style: String,
    pub glyphs: u16,
    /// Fonts in the file (1 unless it is a collection).
    pub faces: u32,
}

/// Validates an untrusted TrueType/OpenType file (or face `face` of a collection).
pub fn check_truetype(bytes: &[u8], face: u32) -> Result<TrueTypeInfo, String> {
    if bytes.len() > MAX_FONT_BYTES {
        return Err(format!(
            "font file is larger than {} MiB",
            MAX_FONT_BYTES >> 20
        ));
    }
    let faces = face_count(bytes).ok_or("damaged font collection header")?;
    if face >= faces {
        return Err(format!(
            "the file has {faces} font(s); face {face} does not exist"
        ));
    }
    if !tables_in_bounds(bytes, face) {
        return Err("damaged font file (tables run past its end)".into());
    }
    let f = parse_face(bytes, face).ok_or("not a usable TrueType/OpenType font file")?;
    let cmap = f
        .tables()
        .cmap
        .ok_or("the font has no character map (cmap)")?;
    let mut mapped = false;
    for sub in cmap.subtables.into_iter().filter(|s| s.is_unicode()) {
        sub.codepoints(|c| {
            if !mapped && char::from_u32(c).is_some_and(|ch| f.glyph_index(ch).is_some()) {
                mapped = true;
            }
        });
        if mapped {
            break;
        }
    }
    if !mapped {
        return Err("the font maps no Unicode characters".into());
    }
    if f.number_of_glyphs() < 2 {
        return Err("the font has no glyphs".into());
    }
    let name = |ids: &[u16]| {
        ids.iter().find_map(|&id| {
            f.names()
                .into_iter()
                .filter(|n| n.name_id == id && n.is_unicode())
                .find_map(|n| n.to_string())
                .map(|s| s.trim().chars().take(64).collect::<String>())
                .filter(|s| !s.is_empty())
        })
    };
    Ok(TrueTypeInfo {
        family: name(&[name_id::TYPOGRAPHIC_FAMILY, name_id::FAMILY])
            .unwrap_or_else(|| "Untitled".into()),
        style: name(&[name_id::TYPOGRAPHIC_SUBFAMILY, name_id::SUBFAMILY]).unwrap_or_default(),
        glyphs: f.number_of_glyphs(),
        faces,
    })
}

/// `family` turned into a font id: lower case, runs of other characters become `-`.
pub fn slug(family: &str) -> String {
    let mut out = String::new();
    for c in family.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out
        .trim_end_matches('-')
        .chars()
        .take(48)
        .collect::<String>();
    if out.is_empty() { "font".into() } else { out }
}

/// Whether `id` may name a font: 1–64 of `a-z 0-9 - _`.
pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Scripts that need shaping this layout does not do: the name of `c`'s script, or `None` when
/// one glyph per character, left to right, is right for it.
pub fn shaping_note(c: char) -> Option<&'static str> {
    let u = c as u32;
    Some(match u {
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F => {
            "combining marks (not positioned; use precomposed characters)"
        }
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => "Hebrew (right-to-left)",
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => {
            "Arabic (right-to-left, contextual joining)"
        }
        0x0700..=0x074F => "Syriac (right-to-left, contextual joining)",
        0x0780..=0x07BF => "Thaana (right-to-left)",
        0x07C0..=0x07FF => "N'Ko (right-to-left, contextual joining)",
        0x0900..=0x0DFF => "Indic (reordering, conjuncts)",
        0x0E00..=0x0EFF => "Thai/Lao (mark positioning)",
        0x0F00..=0x0FFF => "Tibetan (stacking)",
        0x1000..=0x109F => "Myanmar (reordering)",
        0x1100..=0x11FF | 0xA960..=0xA97F | 0xD7B0..=0xD7FF => {
            "conjoining Hangul jamo (use precomposed syllables)"
        }
        0x1780..=0x17FF => "Khmer (reordering)",
        0x1800..=0x18AF => "Mongolian (vertical, contextual joining)",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    static LATO: &[u8] = include_bytes!("../fonts/Lato-Bold.ttf");

    #[test]
    fn bundled_font_passes_the_checks() {
        let info = check_truetype(LATO, 0).expect("lato");
        assert_eq!(info.family, "Lato");
        assert_eq!(info.faces, 1);
        assert!(info.glyphs > 90);
        assert!(check_truetype(LATO, 1).is_err(), "no second face");
    }

    #[test]
    fn hostile_font_files_are_refused() {
        let mut no_cmap = LATO.to_vec();
        // Rename the cmap table in the table directory: the font then maps no characters.
        let at = no_cmap
            .windows(4)
            .take(12 + 16 * 64)
            .position(|w| w == b"cmap")
            .expect("cmap tag");
        no_cmap[at..at + 4].copy_from_slice(b"xmap");
        let mut ttc_bomb = b"ttcf\x00\x01\x00\x00\xff\xff\xff\xff".to_vec();
        ttc_bomb.extend([0u8; 64]);
        let mut garbage = Vec::new();
        let mut x = 0x2545_F491_u32;
        for _ in 0..4096 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            garbage.push(x as u8);
        }
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("empty", Vec::new()),
            ("tiny", b"\x00\x01\x00\x00".to_vec()),
            ("truncated", LATO[..2000].to_vec()),
            ("garbage", garbage),
            ("no cmap", no_cmap),
            ("collection bomb", ttc_bomb),
            ("too large", vec![0; MAX_FONT_BYTES + 1]),
        ];
        for (what, bytes) in cases {
            assert!(check_truetype(&bytes, 0).is_err(), "{what} accepted");
        }
    }

    #[test]
    fn bytes_round_trip_as_base64_and_refuse_garbage() {
        let f = FontBytes::new(vec![0, 1, 2, 250, 255]);
        let json = serde_json::to_string(&f).expect("json");
        assert_eq!(json, "\"AAEC+v8=\"");
        assert_eq!(serde_json::from_str::<FontBytes>(&json).expect("back"), f);
        assert!(serde_json::from_str::<FontBytes>("\"***\"").is_err());
        assert!(serde_json::from_str::<FontBytes>("12").is_err());
    }

    #[test]
    fn file_names_map_to_characters() {
        for (stem, c) in [
            ("u_A", Some('A')),
            ("l_a", Some('a')),
            ("u_a", None),
            ("l_A", None),
            ("s_bs", Some('\\')),
            ("s_qm", Some('?')),
            ("s_xx", None),
            ("7", Some('7')),
            ("\u{4E2D}", Some('\u{4E2D}')),
            ("U+4E2D", Some('\u{4E2D}')),
            ("U+0007", None),
            ("U+D800", None),
            ("heart", None),
            ("", None),
        ] {
            assert_eq!(char_for_file_stem(stem), c, "{stem}");
        }
    }

    #[test]
    fn embroidery_fonts_keep_glyphs_sorted_and_validate() {
        use crate::geometry::v2;
        let mut f = EmbroideryFont::new(100.0);
        let square = Contour::new(
            vec![
                v2(0.0, 0.0),
                v2(50.0, 0.0),
                v2(50.0, -100.0),
                v2(0.0, -100.0),
            ],
            true,
        );
        for ch in ['b', 'a', 'c', 'a'] {
            f.set_glyph(EmbroideryGlyph {
                ch,
                advance: 60.0,
                contours: vec![square.clone()],
                stitches: None,
            });
        }
        assert_eq!(f.glyphs.iter().map(|g| g.ch).collect::<String>(), "abc");
        assert!(f.validate().is_ok());
        assert!(f.remove_glyph('b') && !f.remove_glyph('b'));
        let mut bad = f.clone();
        bad.glyphs.swap(0, 1);
        assert!(bad.validate().is_err(), "unsorted");
        let mut bad = f.clone();
        bad.cap_height = f64::NAN;
        assert!(bad.validate().is_err());
        let mut bad = f.clone();
        bad.glyphs[0].contours[0].points[0].x = 1e12;
        assert!(bad.validate().is_err());
        let mut bad = f.clone();
        bad.glyphs[0].stitches = Some(StitchBlock {
            stitches: Vec::new(),
            threads: Vec::new(),
        });
        assert!(bad.validate().is_err(), "empty stitch data");
    }

    #[test]
    fn ids_and_shaping_notes() {
        assert_eq!(slug("Noto Sans JP"), "noto-sans-jp");
        assert_eq!(slug("  ** "), "font");
        assert!(valid_id("noto-sans_jp2") && !valid_id("Noto") && !valid_id(""));
        assert!(!valid_id(&"a".repeat(65)));
        for c in ['A', 'é', 'Ж', '中', 'か', '한'] {
            assert_eq!(shaping_note(c), None, "{c}");
        }
        assert!(shaping_note('ب').is_some_and(|s| s.starts_with("Arabic")));
        assert!(shaping_note('א').is_some_and(|s| s.starts_with("Hebrew")));
        assert!(shaping_note('क').is_some_and(|s| s.starts_with("Indic")));
        assert!(shaping_note('\u{0301}').is_some());
    }
}
