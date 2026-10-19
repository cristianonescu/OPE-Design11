//! Native editable project file (`.osd`, JSON): encoding, and decoding with the checks a
//! hostile file needs. Here rather than in the engine so readers that only need the file
//! (the Design Database, thumbnailers) do not depend on the command engine.

use crate::fonts::{self, DesignFont, FontData, MAX_DESIGN_FONTS, MAX_TOTAL_FONT_BYTES};
use crate::{Design, text};
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "openstitch.design";
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "osd";
pub const MAX_SIZE: usize = 64 << 20;

#[derive(Serialize, Deserialize)]
struct Envelope {
    format: String,
    version: u32,
    design: Design,
}

#[derive(Deserialize)]
struct Header {
    format: String,
    version: u32,
}

pub fn encode(design: &Design) -> Result<Vec<u8>, String> {
    let env = Envelope {
        format: FORMAT.into(),
        version: VERSION,
        design: design.clone(),
    };
    serde_json::to_vec_pretty(&env).map_err(|e| e.to_string())
}

pub fn decode(data: &[u8]) -> Result<Design, String> {
    if data.len() > MAX_SIZE {
        return Err(format!("file larger than {} MiB", MAX_SIZE >> 20));
    }
    let header: Header =
        serde_json::from_slice(data).map_err(|_| "not an OpenStitch design file".to_string())?;
    if header.format != FORMAT {
        return Err(format!("unexpected format '{}'", header.format));
    }
    if header.version > VERSION {
        return Err(format!(
            "file version {} is newer than supported ({VERSION})",
            header.version
        ));
    }
    let env: Envelope =
        serde_json::from_slice(data).map_err(|e| format!("damaged design file: {e}"))?;
    validate(&env.design)?;
    Ok(env.design)
}

/// Checks a design read from an untrusted source (objects, coordinates, text, fonts).
pub fn validate(d: &Design) -> Result<(), String> {
    validate_fonts(&d.fonts)?;
    let mut ids: Vec<u64> = d.objects.iter().map(|o| o.id).collect();
    ids.sort_unstable();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err("duplicate object ids".into());
    }
    if d.next_object_id <= ids.last().copied().unwrap_or(0) {
        return Err("nextObjectID not above existing ids".into());
    }
    let finite = d.objects.iter().all(|o| {
        o.points()
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite())
            && o.contours.iter().all(|c| {
                c.curves.len() <= c.segment_count()
                    && c.curves.iter().flatten().all(|b| {
                        [b.c1, b.c2]
                            .iter()
                            .all(|q| q.x.is_finite() && q.y.is_finite())
                    })
            })
    });
    let text_ok = d.objects.iter().all(|o| {
        o.text.as_ref().is_none_or(|t| {
            t.transform.is_finite()
                && [t.size, t.letter_spacing, t.line_spacing]
                    .iter()
                    .all(|v| v.is_finite())
        })
    });
    if !finite || !text_ok || !(d.page.width > 0.0 && d.page.height > 0.0) {
        return Err("invalid coordinates".into());
    }
    Ok(())
}

/// Checks the fonts of a design read from a file.
fn validate_fonts(fonts_in: &[DesignFont]) -> Result<(), String> {
    if fonts_in.len() > MAX_DESIGN_FONTS {
        return Err(format!("more than {MAX_DESIGN_FONTS} fonts"));
    }
    let total: usize = fonts_in.iter().map(DesignFont::byte_size).sum();
    if total > MAX_TOTAL_FONT_BYTES {
        return Err("embedded fonts too large".into());
    }
    for (i, f) in fonts_in.iter().enumerate() {
        if !fonts::valid_id(&f.id)
            || text::font(&f.id).is_some_and(|b| b.id == f.id)
            || fonts_in[..i].iter().any(|g| g.id == f.id)
        {
            return Err(format!("bad or duplicate font id '{}'", f.id));
        }
        match &f.data {
            FontData::TrueType { face, bytes } => {
                fonts::check_truetype(&bytes.0, *face)
                    .map_err(|e| format!("font '{}': {e}", f.id))?;
            }
            FontData::Embroidery(e) => e.validate().map_err(|e| format!("font '{}': {e}", f.id))?,
        }
    }
    Ok(())
}
