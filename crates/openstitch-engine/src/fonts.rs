//! Fonts carried by the design: user TrueType/OpenType files loaded from a path the user gives
//! (PE-DESIGN's TrueType text, including CJK fonts) and OpenStitch embroidery fonts, whose
//! glyphs can be whole stitch designs (user-mapped text: a character → embroidery pattern
//! mapping). Font files are hostile input; see `openstitch_design::fonts` for the checks and
//! limits. Embroidery fonts are saved and opened as `.osfont` files (docs/FILE_FORMATS.md).

use crate::CommandSpec;
use crate::commands::{Effect, MM, opt, read_file, req, spec};
use crate::params::{CommandError, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_core::{Command, StitchPattern};
use openstitch_design::fonts::{
    self, DesignFont, EmbroideryFont, EmbroideryGlyph, FontBytes, FontData, MAX_DESIGN_FONTS,
    MAX_FONT_BYTES, MAX_TOTAL_FONT_BYTES,
};
use openstitch_design::{Compiler, Design, DesignObject, Rect, StitchBlock, text};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// `.osfont` envelope.
pub const FONT_FORMAT: &str = "openstitch.font";
pub const FONT_VERSION: u32 = 1;
pub const FONT_EXTENSION: &str = "osfont";
/// Largest `.osfont` file read.
pub const MAX_FONT_FILE: usize = 64 << 20;
/// Largest stitch file registered as a glyph.
const MAX_STITCH_FILE: usize = 16 << 20;
/// Most files one `fonts.mapFiles` call takes.
const MAX_MAP_FILES: usize = 512;
/// Size range of a stitch design registered as a glyph, mm (PE-DESIGN skips designs outside
/// 5–500 mm in its multiple import; single imports here accept from 1 mm).
const GLYPH_MIN_MM: f64 = 1.0;
const BATCH_MIN_MM: f64 = 5.0;
const GLYPH_MAX_MM: f64 = 500.0;

#[derive(Serialize, Deserialize)]
struct FontEnvelope {
    format: String,
    version: u32,
    font: DesignFont,
}

pub(crate) fn commands() -> Vec<CommandSpec> {
    vec![
        spec(
            "fonts.loadTrueType",
            "Load a TrueType/OpenType font file (.ttf, .otf, .ttc) into the design for lettering",
            Effect::Edit,
            vec![
                req("path", "string", "Font file (at most 24 MiB)"),
                opt(
                    "id",
                    "string",
                    "Font id for text objects: a-z, 0-9, - and _ (default from the family name)",
                ),
                opt(
                    "face",
                    "int",
                    "Font of a collection (.ttc), from 0 (default 0)",
                ),
            ],
            load_truetype,
        ),
        spec(
            "fonts.create",
            "Create an empty embroidery font in the design (user-mapped text, Font Creator)",
            Effect::Edit,
            vec![
                req("id", "string", "Font id: a-z, 0-9, - and _"),
                opt("family", "string", "Display name (default the id)"),
                opt(
                    "height",
                    "mm",
                    "Standard character height (baseline to capital line), 1-500 (default 10)",
                ),
            ],
            |s, p| {
                let id = new_font_id(s, p)?;
                let height = p.opt("height", 10.0, 1.0, 500.0)?;
                let family = family_param(p, &id)?;
                s.design.fonts.push(DesignFont {
                    id: id.clone(),
                    family,
                    style: String::new(),
                    source: None,
                    data: FontData::Embroidery(EmbroideryFont::new(height * MM)),
                });
                Ok(json!({ "id": id }))
            },
        ),
        spec(
            "fonts.mapCharacter",
            "Register an embroidery pattern (a stitch file or a design object) as a character of an embroidery font",
            Effect::Edit,
            vec![
                req("font", "string", "Embroidery font id"),
                req("char", "string", "The character (one)"),
                opt(
                    "path",
                    "string",
                    "Stitch file (pes, dst, …); or give object",
                ),
                opt(
                    "object",
                    "int",
                    "Design object whose stitches become the glyph",
                ),
                opt(
                    "dx",
                    "mm",
                    "Move the pattern right of the left set line (default 0)",
                ),
                opt(
                    "dy",
                    "mm",
                    "Move the pattern down from sitting on the baseline (default 0; positive for descenders)",
                ),
                opt(
                    "advance",
                    "mm",
                    "Character width (set line to set line; default the pattern width + dx)",
                ),
            ],
            map_character,
        ),
        spec(
            "fonts.mapFiles",
            "Register stitch files as characters by file name (u_A, l_a, s_qm …, a single character, or U+XXXX); designs outside 5-500 mm are skipped",
            Effect::Edit,
            vec![
                req("font", "string", "Embroidery font id"),
                req("paths", "[string]", "Stitch files, at most 512"),
            ],
            map_files,
        ),
        spec(
            "fonts.unmapCharacter",
            "Remove a character from an embroidery font",
            Effect::Edit,
            vec![
                req("font", "string", "Embroidery font id"),
                req("char", "string", "The character"),
            ],
            |s, p| {
                let ch = char_param(p, "char")?;
                let f = embroidery_mut(s, p)?;
                if !f.remove_glyph(ch) {
                    return Err(p.not_found(format!("character {ch:?} in the font")));
                }
                Ok(json!({ "glyphs": f.glyphs.len() }))
            },
        ),
        spec(
            "fonts.save",
            "Save an embroidery font of the design as an OpenStitch font file (.osfont)",
            Effect::File,
            vec![
                req("font", "string", "Embroidery font id"),
                req("path", "string", "File path"),
            ],
            |s, p| {
                let id = p.string("font", None)?;
                let font = s
                    .design
                    .fonts
                    .iter()
                    .find(|f| f.id == id && matches!(f.data, FontData::Embroidery(_)))
                    .ok_or_else(|| p.not_found(format!("embroidery font '{id}'")))?
                    .clone();
                let path = s.resolve(&p.string("path", None)?, Access::Write, &p.command)?;
                let glyphs = match &font.data {
                    FontData::Embroidery(e) => e.glyphs.len(),
                    FontData::TrueType { .. } => 0,
                };
                let bytes = serde_json::to_vec_pretty(&FontEnvelope {
                    format: FONT_FORMAT.into(),
                    version: FONT_VERSION,
                    font,
                })
                .map_err(|e| p.failed(e.to_string()))?;
                std::fs::write(&path, bytes).map_err(|e| p.failed(e.to_string()))?;
                Ok(json!({ "path": path.display().to_string(), "glyphs": glyphs }))
            },
        ),
        spec(
            "fonts.open",
            "Load an OpenStitch font file (.osfont) into the design",
            Effect::Edit,
            vec![
                req("path", "string", "File path"),
                opt("id", "string", "Use this id instead of the file's"),
            ],
            |s, p| {
                let (_, data) = read_file(s, p, MAX_FONT_FILE)?;
                let mut font = decode_font(&data).map_err(|e| p.failed(e))?;
                if p.has("id") {
                    font.id = p.string("id", None)?;
                }
                if !fonts::valid_id(&font.id) {
                    return Err(p.invalid("id", "must be 1-64 of a-z, 0-9, - and _"));
                }
                if id_taken(s, &font.id) {
                    return Err(p.failed(format!(
                        "a font with id '{}' already exists; pass another id",
                        font.id
                    )));
                }
                check_room(s, p, font.byte_size())?;
                let glyphs = match &font.data {
                    FontData::Embroidery(e) => e.glyphs.len(),
                    FontData::TrueType { .. } => 0,
                };
                let id = font.id.clone();
                s.design.fonts.push(font);
                Ok(json!({ "id": id, "glyphs": glyphs }))
            },
        ),
        spec(
            "fonts.setGlyph",
            "Font Creator: make shape or text objects a character of an embroidery font (drawn at the font's standard height)",
            Effect::Edit,
            vec![
                req("font", "string", "Embroidery font id"),
                req("char", "string", "The character"),
                crate::commands::IDS,
                req("x", "mm", "Left set line on the Design Page"),
                req("y", "mm", "Baseline on the Design Page"),
                opt(
                    "advance",
                    "mm",
                    "Character width, set line to set line (default: the character's current width, or for a new one the outline's right edge plus 8 % of the height)",
                ),
            ],
            set_glyph,
        ),
        spec(
            "fonts.editGlyph",
            "Font Creator: put a character's outline on the Design Page as a shape to edit (then fonts.setGlyph with the same x and y)",
            Effect::Edit,
            vec![
                req("font", "string", "Embroidery font id"),
                req("char", "string", "The character"),
                req("x", "mm", "Left set line on the Design Page"),
                req("y", "mm", "Baseline on the Design Page"),
            ],
            edit_glyph,
        ),
        spec(
            "fonts.convertTrueType",
            "Font Creator: convert characters of a TrueType font (bundled or loaded) into outline glyphs of an embroidery font",
            Effect::Edit,
            vec![
                req("font", "string", "Embroidery font id"),
                req("from", "string", "TrueType font id or family (fonts.list)"),
                req(
                    "chars",
                    "string",
                    "Characters to convert, at most 1024 distinct",
                ),
            ],
            convert_truetype,
        ),
        spec(
            "fonts.setMetrics",
            "Font Creator: change an embroidery font's name, standard height or line metrics",
            Effect::Edit,
            vec![
                req("font", "string", "Embroidery font id"),
                opt("family", "string", "Display name"),
                opt(
                    "height",
                    "mm",
                    "Standard character height, 1-500 (glyphs keep their drawn size)",
                ),
                opt("ascender", "mm", "Space above the baseline, 0-2000"),
                opt("descender", "mm", "Space below the baseline, 0-2000"),
                opt("lineGap", "mm", "Extra space between lines, 0-2000"),
            ],
            |s, p| {
                let family = if p.has("family") {
                    let id = p.string("font", None)?;
                    Some(family_param(p, &id)?)
                } else {
                    None
                };
                let f = embroidery_mut(s, p)?;
                f.cap_height = p.opt("height", f.cap_height / MM, 1.0, 500.0)? * MM;
                f.ascender = p.opt("ascender", f.ascender / MM, 0.0, 2000.0)? * MM;
                f.descender = p.opt("descender", f.descender / MM, 0.0, 2000.0)? * MM;
                f.line_gap = p.opt("lineGap", f.line_gap / MM, 0.0, 2000.0)? * MM;
                let out = json!({
                    "heightMM": f.cap_height / MM, "ascenderMM": f.ascender / MM,
                    "descenderMM": f.descender / MM, "lineGapMM": f.line_gap / MM,
                });
                if let Some(family) = family {
                    let id = p.string("font", None)?;
                    if let Some(df) = s.design.fonts.iter_mut().find(|f| f.id == id) {
                        df.family = family;
                    }
                }
                Ok(out)
            },
        ),
        spec(
            "fonts.remove",
            "Remove a font from the design (refused while text uses it)",
            Effect::Edit,
            vec![req("id", "string", "Design font id")],
            |s, p| {
                let id = p.string("id", None)?;
                let i = s
                    .design
                    .fonts
                    .iter()
                    .position(|f| f.id == id)
                    .ok_or_else(|| p.not_found(format!("design font '{id}'")))?;
                if let Some(o) = s
                    .design
                    .objects
                    .iter()
                    .find(|o| o.text.as_ref().is_some_and(|t| t.font == id))
                {
                    return Err(p.failed(format!("font '{id}' is used by object {}", o.id)));
                }
                s.design.fonts.remove(i);
                Ok(json!({ "removed": id }))
            },
        ),
    ]
}

fn load_truetype(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    let face = p.int(
        "face",
        Some(0),
        0,
        i64::from(fonts::MAX_COLLECTION_FACES) - 1,
    )? as u32;
    let wanted = if p.has("id") {
        let id = p.string("id", None)?;
        if !fonts::valid_id(&id) {
            return Err(p.invalid("id", "must be 1-64 of a-z, 0-9, - and _"));
        }
        Some(id)
    } else {
        None
    };
    if s.design.fonts.len() >= MAX_DESIGN_FONTS {
        return Err(p.failed(format!("a design holds at most {MAX_DESIGN_FONTS} fonts")));
    }
    let (path, bytes) = read_file(s, p, MAX_FONT_BYTES)?;
    let info = fonts::check_truetype(&bytes, face).map_err(|e| p.failed(e))?;
    check_room(s, p, bytes.len())?;
    let taken = |id: &str| id_taken(s, id);
    let id = match wanted {
        Some(id) if taken(&id) => {
            return Err(p.invalid("id", format!("a font with id '{id}' already exists")));
        }
        Some(id) => id,
        None => {
            let base = fonts::slug(&format!("{} {}", info.family, info.style));
            let mut id = base.clone();
            let mut n = 2;
            while taken(&id) {
                id = format!("{base}-{n}");
                n += 1;
            }
            id
        }
    };
    let font = DesignFont {
        id: id.clone(),
        family: info.family.clone(),
        style: info.style.clone(),
        source: path.file_name().map(|n| n.to_string_lossy().into_owned()),
        data: FontData::TrueType {
            face,
            bytes: FontBytes::new(bytes),
        },
    };
    let glyphs = text::LoadedFont::load_in(&id, std::slice::from_ref(&font))
        .map(|f| f.glyph_count())
        .map_err(|e| p.failed(e.to_string()))?;
    s.design.fonts.push(font);
    Ok(json!({
        "id": id, "family": info.family, "style": info.style,
        "glyphs": glyphs, "faces": info.faces,
    }))
}

fn id_taken(s: &EngineState, id: &str) -> bool {
    text::font(id).is_some_and(|f| f.id == id) || s.design.fonts.iter().any(|f| f.id == id)
}

/// The `id` parameter of a new font: valid and not taken.
fn new_font_id(s: &EngineState, p: &Params) -> Result<String, CommandError> {
    let id = p.string("id", None)?;
    if !fonts::valid_id(&id) {
        return Err(p.invalid("id", "must be 1-64 of a-z, 0-9, - and _"));
    }
    if id_taken(s, &id) {
        return Err(p.invalid("id", format!("a font with id '{id}' already exists")));
    }
    if s.design.fonts.len() >= MAX_DESIGN_FONTS {
        return Err(p.failed(format!("a design holds at most {MAX_DESIGN_FONTS} fonts")));
    }
    Ok(id)
}

fn family_param(p: &Params, id: &str) -> Result<String, CommandError> {
    let family = p.string("family", Some(id))?.trim().to_string();
    if family.is_empty() || family.chars().count() > 64 {
        return Err(p.invalid("family", "must be 1-64 characters"));
    }
    Ok(family)
}

/// Refuses growth past the per-design font budget.
fn check_room(s: &EngineState, p: &Params, extra: usize) -> Result<(), CommandError> {
    let total: usize = s.design.fonts.iter().map(DesignFont::byte_size).sum();
    if s.design.fonts.len() >= MAX_DESIGN_FONTS || total + extra > MAX_TOTAL_FONT_BYTES {
        return Err(p.failed(format!(
            "the design's fonts would exceed {MAX_DESIGN_FONTS} fonts or {} MiB",
            MAX_TOTAL_FONT_BYTES >> 20
        )));
    }
    Ok(())
}

/// A one-character string parameter.
pub(crate) fn char_param(p: &Params, key: &str) -> Result<char, CommandError> {
    let v = p.string(key, None)?;
    let mut it = v.chars();
    match (it.next(), it.next()) {
        (Some(c), None) if !c.is_control() => Ok(c),
        _ => Err(p.invalid(key, "must be exactly one printable character")),
    }
}

/// The embroidery font named by the `font` parameter.
pub(crate) fn embroidery_mut<'a>(
    s: &'a mut EngineState,
    p: &Params,
) -> Result<&'a mut EmbroideryFont, CommandError> {
    let id = p.string("font", None)?;
    match s.design.fonts.iter_mut().find(|f| f.id == id) {
        Some(DesignFont {
            data: FontData::Embroidery(e),
            ..
        }) => Ok(e),
        Some(_) => Err(p.failed(format!(
            "font '{id}' is a TrueType font; only embroidery fonts can be edited"
        ))),
        None => Err(p.not_found(format!("embroidery font '{id}'"))),
    }
}

/// An `.osfont` file's font, checked.
pub fn decode_font(data: &[u8]) -> Result<DesignFont, String> {
    if data.len() > MAX_FONT_FILE {
        return Err(format!("file larger than {} MiB", MAX_FONT_FILE >> 20));
    }
    #[derive(Deserialize)]
    struct Header {
        format: String,
        version: u32,
    }
    let h: Header =
        serde_json::from_slice(data).map_err(|_| "not an OpenStitch font file".to_string())?;
    if h.format != FONT_FORMAT {
        return Err(format!("unexpected format '{}'", h.format));
    }
    if h.version > FONT_VERSION {
        return Err(format!(
            "font file version {} is newer than supported ({FONT_VERSION})",
            h.version
        ));
    }
    let env: FontEnvelope =
        serde_json::from_slice(data).map_err(|e| format!("damaged font file: {e}"))?;
    match &env.font.data {
        FontData::Embroidery(e) => e.validate()?,
        FontData::TrueType { .. } => {
            return Err("an .osfont file holds an embroidery font; load TrueType files with fonts.loadTrueType".into());
        }
    }
    if env.font.family.trim().is_empty() || env.font.family.chars().count() > 64 {
        return Err("family must be 1-64 characters".into());
    }
    Ok(env.font)
}

/// Reads a stitch file the user named.
fn read_stitch_file(s: &EngineState, p: &Params, path: &str) -> Result<StitchPattern, String> {
    let resolved = s
        .resolve(path, Access::Read, &p.command)
        .map_err(|e| e.to_string())?;
    let meta = std::fs::metadata(&resolved).map_err(|_| "cannot read the file".to_string())?;
    if meta.len() > MAX_STITCH_FILE as u64 {
        return Err("file too large".into());
    }
    let data = std::fs::read(&resolved).map_err(|_| "cannot read the file".to_string())?;
    decode_stitches(&resolved, &data)
}

#[cfg(feature = "formats")]
fn decode_stitches(path: &std::path::Path, data: &[u8]) -> Result<StitchPattern, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    let ty = openstitch_formats::FileType::from_extension(ext)
        .ok_or_else(|| format!("unsupported stitch file type '{ext}'"))?;
    ty.read(data).map_err(|e| e.to_string())
}

#[cfg(not(feature = "formats"))]
fn decode_stitches(_: &std::path::Path, _: &[u8]) -> Result<StitchPattern, String> {
    Err("stitch file import is not built in".into())
}

/// The stitches of design object `id` alone, as the compiler sews them.
fn object_stitches(d: &Design, id: u64) -> Option<StitchPattern> {
    let o = d.objects.get(d.index_of(id)?)?.clone();
    let mut one = Design::new("glyph", d.page);
    one.fonts = d.fonts.clone();
    one.objects.push(DesignObject { visible: true, ..o });
    Some(Compiler::default().compile(&one))
}

/// `pattern` as a glyph: its box's bottom-left corner on the origin (left set line,
/// baseline), moved by `dx`, `dy` (0.1 mm). Checks its size against `min_mm`–500 mm.
fn glyph_from_pattern(
    ch: char,
    pattern: &StitchPattern,
    dx: f64,
    dy: f64,
    advance: Option<f64>,
    min_mm: f64,
) -> Result<EmbroideryGlyph, String> {
    let points: Vec<_> = pattern
        .stitches
        .iter()
        .filter(|s| s.command == Command::Stitch)
        .collect();
    let b = Rect::enclosing(points.iter().map(|s| openstitch_design::v2(s.x, s.y)))
        .ok_or("the pattern has no stitches")?;
    let longest = b.width().max(b.height()) / MM;
    if !(min_mm..=GLYPH_MAX_MM).contains(&longest) {
        return Err(format!(
            "the pattern is {longest:.1} mm; patterns of {min_mm}-{GLYPH_MAX_MM} mm can be registered"
        ));
    }
    let mut block = StitchBlock::from(pattern);
    block
        .stitches
        .retain(|s| !matches!(s.command, Command::End | Command::Stop));
    if block.threads.is_empty() {
        block.threads.push(openstitch_core::Thread::rgb(0));
    }
    if block.stitches.len() > fonts::MAX_GLYPH_STITCHES {
        return Err(format!(
            "the pattern has more than {} needle points",
            fonts::MAX_GLYPH_STITCHES
        ));
    }
    for s in &mut block.stitches {
        s.x += dx - b.min_x;
        s.y += dy - b.max_y;
    }
    Ok(EmbroideryGlyph {
        ch,
        advance: advance.unwrap_or(b.width() + dx).max(0.0),
        contours: Vec::new(),
        stitches: Some(block),
    })
}

/// Sets `glyph` in the embroidery font of the `font` parameter, keeping the font within its
/// limits.
pub(crate) fn store_glyph(
    s: &mut EngineState,
    p: &Params,
    glyph: EmbroideryGlyph,
) -> Result<usize, CommandError> {
    let total: usize = s.design.fonts.iter().map(DesignFont::byte_size).sum();
    let f = embroidery_mut(s, p)?;
    let mut next = f.clone();
    next.set_glyph(glyph);
    next.validate().map_err(|e| p.failed(e))?;
    let grown = DesignFont {
        id: String::new(),
        family: String::new(),
        style: String::new(),
        source: None,
        data: FontData::Embroidery(next.clone()),
    }
    .byte_size();
    let before = DesignFont {
        id: String::new(),
        family: String::new(),
        style: String::new(),
        source: None,
        data: FontData::Embroidery(f.clone()),
    }
    .byte_size();
    if total - before + grown > MAX_TOTAL_FONT_BYTES {
        return Err(p.failed(format!(
            "the design's fonts would exceed {} MiB",
            MAX_TOTAL_FONT_BYTES >> 20
        )));
    }
    *f = next;
    Ok(f.glyphs.len())
}

fn map_character(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    let ch = char_param(p, "char")?;
    embroidery_mut(s, p)?;
    let dx = p.opt("dx", 0.0, -500.0, 500.0)? * MM;
    let dy = p.opt("dy", 0.0, -500.0, 500.0)? * MM;
    let advance = if p.has("advance") {
        Some(p.req("advance", 0.0, 1000.0)? * MM)
    } else {
        None
    };
    let pattern = match (p.has("path"), p.has("object")) {
        (true, false) => {
            read_stitch_file(s, p, &p.string("path", None)?).map_err(|e| p.failed(e))?
        }
        (false, true) => {
            let id = p.int("object", None, 0, i64::MAX)? as u64;
            object_stitches(&s.design, id).ok_or_else(|| p.not_found(format!("object {id}")))?
        }
        _ => return Err(p.invalid("path", "give either path or object")),
    };
    let glyph =
        glyph_from_pattern(ch, &pattern, dx, dy, advance, GLYPH_MIN_MM).map_err(|e| p.failed(e))?;
    let stitches = glyph.stitches.as_ref().map_or(0, |b| b.stitches.len());
    let glyphs = store_glyph(s, p, glyph)?;
    Ok(json!({ "char": ch.to_string(), "stitches": stitches, "glyphs": glyphs }))
}

/// Most distinct characters one `fonts.convertTrueType` call converts.
const MAX_CONVERT_CHARS: usize = 1024;

fn set_glyph(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    let ch = char_param(p, "char")?;
    let f = embroidery_mut(s, p)?;
    let (cap, kept) = (f.cap_height, f.glyph(ch).map(|g| g.advance));
    let x = p.req("x", -5000.0, 5000.0)? * MM;
    let y = p.req("y", -5000.0, 5000.0)? * MM;
    let idx = crate::commands::indexes(p, &s.design)?;
    let mut contours = Vec::new();
    for &i in &idx {
        let o = &s.design.objects[i];
        if !o.is_outline() || o.contours.is_empty() {
            return Err(p.failed(format!(
                "object {} has no outline (only shapes and text can become glyphs)",
                o.id
            )));
        }
        contours.extend(o.contours.iter().cloned());
    }
    let to_glyph = openstitch_design::Transform::translation(-x, -y);
    let contours: Vec<_> = contours.iter().map(|c| c.transformed(&to_glyph)).collect();
    let right =
        Rect::enclosing(contours.iter().flat_map(|c| c.outline_points())).map_or(0.0, |b| b.max_x);
    let advance = if p.has("advance") {
        p.req("advance", 0.0, 1000.0)? * MM
    } else {
        kept.unwrap_or((right + 0.08 * cap).max(0.0))
    };
    let glyphs = store_glyph(
        s,
        p,
        EmbroideryGlyph {
            ch,
            advance,
            contours,
            stitches: None,
        },
    )?;
    Ok(json!({ "char": ch.to_string(), "advanceMM": advance / MM, "glyphs": glyphs }))
}

fn edit_glyph(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    let ch = char_param(p, "char")?;
    let x = p.req("x", -5000.0, 5000.0)? * MM;
    let y = p.req("y", -5000.0, 5000.0)? * MM;
    let f = embroidery_mut(s, p)?;
    let (cap, glyph) = (f.cap_height, f.glyph(ch).cloned());
    let glyph = glyph.ok_or_else(|| p.not_found(format!("character {ch:?} in the font")))?;
    if glyph.contours.is_empty() {
        return Err(p.failed(format!(
            "character {ch:?} is a stitch pattern; only outlines can be edited as shapes"
        )));
    }
    let at = openstitch_design::Transform::translation(x, y);
    let contours = glyph.contours.iter().map(|c| c.transformed(&at)).collect();
    let mut o = DesignObject::shape(
        contours,
        Some(openstitch_design::RegionSewing::new(
            openstitch_design::RegionType::Fill,
            crate::commands::default_fill(),
        )),
        None,
    );
    o.name = Some(format!("Glyph '{ch}'"));
    let id = s.design.add(o);
    Ok(json!({
        "id": id, "advanceMM": glyph.advance / MM, "heightMM": cap / MM,
        "baseline": y / MM, "capLine": (y - cap) / MM, "left": x / MM,
        "right": (x + glyph.advance) / MM,
    }))
}

fn convert_truetype(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    let from = p.string("from", None)?;
    let cap = embroidery_mut(s, p)?.cap_height;
    match text::resolve(&from, &s.design.fonts) {
        Some(text::FontRef::Bundled(_)) => {}
        Some(text::FontRef::Design(DesignFont {
            data: FontData::TrueType { .. },
            ..
        })) => {}
        _ => {
            return Err(p.invalid(
                "from",
                format!("'{from}' is not a TrueType font; see fonts.list"),
            ));
        }
    }
    let mut chars: Vec<char> = p
        .string("chars", None)?
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    chars.sort_unstable();
    chars.dedup();
    if chars.is_empty() || chars.len() > MAX_CONVERT_CHARS {
        return Err(p.invalid(
            "chars",
            format!("must hold 1-{MAX_CONVERT_CHARS} distinct printable characters"),
        ));
    }
    let (mut converted, mut missing) = (String::new(), String::new());
    for ch in chars {
        let outline = text::glyph_outline(&from, &s.design.fonts, ch, cap / MM)
            .map_err(|e| p.failed(e.to_string()))?;
        match outline {
            Some((contours, advance)) => {
                store_glyph(
                    s,
                    p,
                    EmbroideryGlyph {
                        ch,
                        advance,
                        contours,
                        stitches: None,
                    },
                )?;
                converted.push(ch);
            }
            None => missing.push(ch),
        }
    }
    if converted.is_empty() {
        return Err(p.failed(format!("the font has none of {missing}")));
    }
    Ok(json!({ "converted": converted, "missing": missing }))
}

fn map_files(s: &mut EngineState, p: &Params) -> Result<Value, CommandError> {
    embroidery_mut(s, p)?;
    let paths: Vec<String> = p
        .array("paths")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| p.invalid("paths", "must contain strings"))
        })
        .collect::<Result<_, _>>()?;
    if paths.is_empty() || paths.len() > MAX_MAP_FILES {
        return Err(p.invalid("paths", format!("must hold 1-{MAX_MAP_FILES} files")));
    }
    let (mut mapped, mut skipped) = (Vec::new(), Vec::new());
    for path in &paths {
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let result = fonts::char_for_file_stem(stem)
            .ok_or_else(|| "the file name names no character (use u_A, l_a, s_qm, a single character or U+XXXX)".to_string())
            .and_then(|ch| {
                let pattern = read_stitch_file(s, p, path)?;
                glyph_from_pattern(ch, &pattern, 0.0, 0.0, None, BATCH_MIN_MM)
            });
        match result {
            Ok(glyph) => {
                let ch = glyph.ch;
                match store_glyph(s, p, glyph) {
                    Ok(_) => mapped.push(ch.to_string()),
                    Err(e) => skipped.push(json!({ "path": path, "reason": e.to_string() })),
                }
            }
            Err(reason) => skipped.push(json!({ "path": path, "reason": reason })),
        }
    }
    if mapped.is_empty() {
        return Err(p.failed(format!(
            "no file could be registered: {}",
            Value::Array(skipped)
        )));
    }
    Ok(json!({ "mapped": mapped, "skipped": skipped }))
}

/// `fonts.list` rows: bundled fonts, then the design's.
pub(crate) fn list(s: &EngineState) -> Value {
    let mut rows: Vec<Value> = text::FONTS
        .iter()
        .map(|f| {
            let glyphs = text::LoadedFont::load(f.id)
                .map(|l| l.glyph_count())
                .unwrap_or(0);
            json!({
                "id": f.id, "family": f.family, "style": f.style,
                "license": f.license, "glyphs": glyphs,
                "default": f.id == text::DEFAULT_FONT, "source": "bundled", "kind": "trueType",
            })
        })
        .collect();
    rows.extend(s.design.fonts.iter().map(|f| {
        let glyphs = text::LoadedFont::load_in(&f.id, &s.design.fonts)
            .map(|l| l.glyph_count())
            .unwrap_or(0);
        let mut row = json!({
            "id": f.id, "family": f.family, "style": f.style, "glyphs": glyphs,
            "default": false, "source": "design", "kind": f.kind(),
            "bytes": f.byte_size(),
        });
        if let Some(file) = &f.source {
            row["file"] = json!(file);
        }
        if let FontData::Embroidery(e) = &f.data {
            row["heightMM"] = json!(e.cap_height / MM);
            row["stitchGlyphs"] = json!(e.glyphs.iter().filter(|g| g.stitches.is_some()).count());
        }
        row
    }));
    Value::Array(rows)
}
