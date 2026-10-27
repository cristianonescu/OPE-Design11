//! Native editable project file (`.osd`, JSON): encoding, and decoding with the checks a
//! hostile file needs. Here rather than in the engine so readers that only need the file
//! (the Design Database, thumbnailers) do not depend on the command engine.

use crate::fonts::{self, DesignFont, FontData, MAX_DESIGN_FONTS, MAX_TOTAL_FONT_BYTES};
use crate::{Design, text};
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "openstitch.design";
/// 2: direction angles are counter-clockwise on screen (PE-DESIGN's convention); version 1
/// stored them clockwise (see [`migrate_v1_angles`]).
pub const VERSION: u32 = 2;
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
    let mut env: Envelope =
        serde_json::from_slice(data).map_err(|e| format!("damaged design file: {e}"))?;
    validate(&env.design)?;
    if header.version < 2 {
        migrate_v1_angles(&mut env.design);
    }
    Ok(env.design)
}

/// Version 1 files stored direction angles clockwise on screen (from +x towards +y, y down).
/// Since version 2 they turn counter-clockwise, as in PE-DESIGN, so a version-1 file sews the
/// same stitches after loading: region angles are negated (0 ≤ a < 360) and stem line slants,
/// measured from the line, become 180 − a (an unset slant, 45°, becomes 135°).
pub fn migrate_v1_angles(design: &mut Design) {
    for o in &mut design.objects {
        if let Some(r) = &mut o.region {
            r.angle = crate::region::stored_angle(r.angle);
        }
        if let Some(l) = &mut o.line
            && l.kind == crate::model::LineType::Stem
        {
            // Unset meant 45° from the line, clockwise.
            l.angle = Some(180.0 - l.angle.unwrap_or(45.0));
        }
    }
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
                && t.characters.windows(2).all(|w| w[0].index < w[1].index)
                && t.characters.iter().all(|e| {
                    [e.x, e.y, e.rotation].iter().all(|v| v.is_finite())
                        && e.size.is_none_or(|s| s.is_finite() && s > 0.0)
                })
        })
    });
    let skeletons_ok = d.objects.iter().all(|o| {
        o.region
            .as_ref()
            .and_then(|r| r.skeleton.as_ref())
            .is_none_or(|s| s.validate().is_ok())
    });
    if !skeletons_ok {
        return Err("invalid satin skeleton".into());
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Contour;
    use crate::model::{DesignObject, DesignPage, LineSewing, LineType, RegionSewing, RegionType};
    use crate::{Compiler, Design};
    use openstitch_core::Thread;

    /// A 30° fill square and a stem-stitch outline, angles in the version-2 convention.
    fn design() -> Design {
        let mut d = Design::new("angles", DesignPage::default());
        let mut r = RegionSewing::new(RegionType::Fill, Thread::rgb(0xff0000));
        r.angle = 30.0;
        let mut l = LineSewing::new(LineType::Stem, Thread::rgb(0));
        l.angle = Some(60.0);
        d.add(DesignObject::shape(
            vec![Contour::rectangle(0.0, 0.0, 300.0, 200.0)],
            Some(r),
            Some(l),
        ));
        d
    }

    #[test]
    fn version_2_keeps_angles() {
        let bytes = encode(&design()).expect("encode");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(v["version"], 2);
        let back = decode(&bytes).expect("decode");
        assert_eq!(back, design());
    }

    #[test]
    fn version_1_angles_are_migrated_and_sew_the_same() {
        // The same design as a version-1 file wrote it: angles clockwise on screen.
        let mut v: serde_json::Value =
            serde_json::from_slice(&encode(&design()).expect("encode")).expect("json");
        v["version"] = serde_json::json!(1);
        let o = &mut v["design"]["objects"][0];
        o["region"]["angle"] = serde_json::json!(-30.0);
        o["line"]["angle"] = serde_json::json!(120.0);
        let old = decode(&serde_json::to_vec(&v).expect("bytes")).expect("decode");
        let (r, l) = (
            old.objects[0].region.as_ref().expect("region"),
            old.objects[0].line.as_ref().expect("line"),
        );
        assert_eq!((r.angle, l.angle), (30.0, Some(60.0)));
        let c = Compiler::default();
        assert_eq!(c.compile(&old).stitches, c.compile(&design()).stitches);
        // An unset stem slant was 45° clockwise: 135° now.
        o_unset(&mut v);
        let old = decode(&serde_json::to_vec(&v).expect("bytes")).expect("decode");
        assert_eq!(
            old.objects[0].line.as_ref().expect("line").angle,
            Some(135.0)
        );
    }

    fn o_unset(v: &mut serde_json::Value) {
        v["design"]["objects"][0]["line"]
            .as_object_mut()
            .expect("line")
            .remove("angle");
    }
}
