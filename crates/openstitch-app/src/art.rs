//! OpenStitch's own spot illustrations (empty states, thumbnail placeholders, wizard headers)
//! and the Realistic view's fabric. The SVGs in `assets/illustrations` and the texture in
//! `assets/textures` were generated for OpenStitch with Higgsfield (prompts in
//! `assets/illustrations/prompts.tsv`) in the command icons' style; they are rendered like the
//! icons, with the charcoal outline in the theme's text colour. When an SVG cannot be rendered
//! the illustration is left out and the text stands alone.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Rect, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Art {
    EmptyCanvas,
    DbChoose,
    DbEmpty,
    DbNoResults,
    ThumbLoading,
    ThumbBroken,
    WizardBackground,
    WizardApplique,
    WizardPatch,
    WizardCutwork,
    WizardImage,
    PatternLibrary,
    NameDrop,
    SewEmpty,
    NoMedia,
    NoImage,
    ObjectsEmpty,
    UpdateAvailable,
    UpdateCurrent,
    UpdateFailed,
}

impl Art {
    #[cfg(test)]
    const ALL: [Art; 20] = [
        Art::EmptyCanvas,
        Art::DbChoose,
        Art::DbEmpty,
        Art::DbNoResults,
        Art::ThumbLoading,
        Art::ThumbBroken,
        Art::WizardBackground,
        Art::WizardApplique,
        Art::WizardPatch,
        Art::WizardCutwork,
        Art::WizardImage,
        Art::PatternLibrary,
        Art::NameDrop,
        Art::SewEmpty,
        Art::NoMedia,
        Art::NoImage,
        Art::ObjectsEmpty,
        Art::UpdateAvailable,
        Art::UpdateCurrent,
        Art::UpdateFailed,
    ];

    fn svg(self) -> (&'static str, &'static str) {
        macro_rules! svg {
            ($n:literal) => {
                (
                    $n,
                    include_str!(concat!("../../../assets/illustrations/", $n, ".svg")),
                )
            };
        }
        match self {
            Art::EmptyCanvas => svg!("empty_canvas"),
            Art::DbChoose => svg!("db_choose"),
            Art::DbEmpty => svg!("db_empty"),
            Art::DbNoResults => svg!("db_noresults"),
            Art::ThumbLoading => svg!("thumb_loading"),
            Art::ThumbBroken => svg!("thumb_broken"),
            Art::WizardBackground => svg!("wizard_background"),
            Art::WizardApplique => svg!("wizard_applique"),
            Art::WizardPatch => svg!("wizard_patch"),
            Art::WizardCutwork => svg!("wizard_cutwork"),
            Art::WizardImage => svg!("wizard_image"),
            Art::PatternLibrary => svg!("pattern_library"),
            Art::NameDrop => svg!("name_drop"),
            Art::SewEmpty => svg!("sew_empty"),
            Art::NoMedia => svg!("no_media"),
            Art::NoImage => svg!("no_image"),
            Art::ObjectsEmpty => svg!("objects_empty"),
            Art::UpdateAvailable => svg!("update_available"),
            Art::UpdateCurrent => svg!("update_current"),
            Art::UpdateFailed => svg!("update_failed"),
        }
    }
}

/// The illustrations' own outline colour, for drawing them on white whatever the theme.
pub const OUTLINE: Color32 = Color32::from_rgb(0x2b, 0x2f, 0x36);

/// Rendered illustrations by (art, size in pixels, outline colour); `None` when the SVG failed.
#[derive(Clone, Default)]
struct Textures(HashMap<(Art, u32, [u8; 4]), Option<egui::TextureHandle>>);

fn texture(ctx: &egui::Context, art: Art, px: u32, fg: Color32) -> Option<egui::TextureHandle> {
    let key = (art, px, fg.to_array());
    let id = egui::Id::unique("openstitch-art");
    if let Some(t) = ctx.data(|d| {
        d.get_temp::<Textures>(id)
            .and_then(|c| c.0.get(&key).cloned())
    }) {
        return t;
    }
    let (name, src) = art.svg();
    let t = crate::icons::rasterize(src, px, fg).map(|img| {
        ctx.load_texture(
            format!("art-{name}-{px}"),
            img,
            egui::TextureOptions::LINEAR,
        )
    });
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Textures>(id)
            .0
            .insert(key, t.clone())
    });
    t
}

/// Draws `art` filling the square `r`, faded by `opacity`. Returns false when it couldn't.
pub fn paint(painter: &egui::Painter, r: Rect, art: Art, fg: Color32, opacity: f32) -> bool {
    let px = (r.width() * painter.ctx().pixels_per_point())
        .round()
        .max(1.0) as u32;
    let Some(t) = texture(painter.ctx(), art, px, fg) else {
        return false;
    };
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(t.id(), r, uv, Color32::WHITE.gamma_multiply(opacity));
    true
}

/// An empty state: the illustration above a short weak text, centred in the space left.
pub fn empty_state(ui: &mut egui::Ui, art: Art, size: f32, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            let room = ui.available_height();
            ui.add_space(((room - size - 40.0) / 2.0).max(8.0));
            let (r, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
            paint(ui.painter(), r, art, ui.visuals().text_color(), 0.9);
            ui.add_space(8.0);
            ui.label(egui::RichText::new(text).weak());
        });
    });
}

/// A small empty state inside a pane: the illustration above a short weak text, taking only
/// the room they need.
pub fn notice(ui: &mut egui::Ui, art: Art, size: f32, text: &str) {
    ui.vertical_centered(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
        paint(ui.painter(), r, art, ui.visuals().text_color(), 0.9);
        ui.label(egui::RichText::new(text).weak());
    });
}

/// A dialog header: the illustration at the left of `contents`.
pub fn header<R>(ui: &mut egui::Ui, art: Art, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(48.0, 48.0), egui::Sense::hover());
        paint(ui.painter(), r, art, ui.visuals().text_color(), 1.0);
        ui.vertical(contents).inner
    })
    .inner
}

/// Size of one repeat of the fabric texture on the Design Page, in model units (0.1 mm).
pub const FABRIC_TILE: f64 = 120.0;

/// The Realistic view's cloth: a greyscale weave that tiles, tinted by the page colour.
pub fn fabric(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::unique("openstitch-fabric");
    if let Some(t) = ctx.data(|d| d.get_temp::<Option<egui::TextureHandle>>(id)) {
        return t;
    }
    let t = decode_fabric()
        .map(|img| ctx.load_texture("fabric", img, egui::TextureOptions::LINEAR_REPEAT));
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

fn decode_fabric() -> Option<egui::ColorImage> {
    let bytes = include_bytes!("../../../assets/textures/fabric_cotton.png");
    let r = openstitch_digitize::Raster::decode(bytes).ok()?;
    let mut rgba = Vec::with_capacity(r.width() as usize * r.height() as usize * 4);
    for y in 0..r.height() {
        for x in 0..r.width() {
            rgba.extend_from_slice(&r.pixel(x, y));
        }
    }
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [r.width() as usize, r.height() as usize],
        &rgba,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_illustration_renders_and_every_svg_is_used() {
        let mut used = std::collections::BTreeSet::new();
        for art in Art::ALL {
            let (name, src) = art.svg();
            assert!(src.contains("#2b2f36"), "{name}: no outline colour");
            let img = crate::icons::rasterize(src, 64, Color32::WHITE)
                .unwrap_or_else(|| panic!("{name}"));
            let inked = img.pixels.iter().filter(|p| p.a() > 128).count();
            assert!(inked >= 200, "{name}: draws nothing");
            used.insert(name.to_string());
        }
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/illustrations");
        let files: std::collections::BTreeSet<String> = std::fs::read_dir(dir)
            .expect("illustrations folder")
            .filter_map(|e| {
                e.ok()?
                    .file_name()
                    .into_string()
                    .ok()?
                    .strip_suffix(".svg")
                    .map(str::to_string)
            })
            .collect();
        assert_eq!(used, files);
    }

    #[test]
    fn fabric_decodes_and_is_light() {
        let img = decode_fabric().expect("fabric");
        assert_eq!(img.size, [192, 192]);
        let mean =
            img.pixels.iter().map(|p| u32::from(p.r())).sum::<u32>() / img.pixels.len() as u32;
        assert!((200..=250).contains(&mean), "mean {mean}");
    }
}
