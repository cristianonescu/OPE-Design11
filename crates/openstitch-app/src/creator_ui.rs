//! Programmable Stitch Creator (Options ▸ Programmable Stitch Creator): a window of its own in
//! place of Layout & Editing, with the suite's Application menu, ribbon (Home, View), ScreenTips
//! and status bar ([`openstitch_app::editor::suite::CREATOR`]). The pattern and its editing live
//! in [`openstitch_app::editor::creator`]; this draws them. Add to Design runs one
//! `pattern.define` command (one Undo); the library folder holds patterns between sessions.

use crate::app::EditorApp;
use crate::suite_chrome::{self, Host};
use eframe::egui::{self, Color32, Pos2, Sense, Stroke};
use openstitch_app::editor::CommandRequest;
use openstitch_app::editor::creator::{
    self, Creator, DENSITIES, Guides, KINDS, LibraryEntry, Template, kind_name, library_dir,
    load_library, save_to_library,
};
use openstitch_app::editor::suite::{CREATOR, CreatorAction};
use openstitch_core::Command;
use openstitch_design::patterns::{self, MASK_CELLS, PatternKind, RegionEffect};
use openstitch_design::{Compiler, Vec2, v2};
use serde_json::json;
use std::path::PathBuf;

/// A sewn polyline in its thread colour.
type StitchLine = (Color32, Vec<Vec2>);

/// What a click on the Design Page does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CreatorTool {
    /// Picks the current line.
    Select,
    /// Moves and deletes points.
    EditPoint,
    /// Adds points to the current line (and moves and deletes them).
    #[default]
    DrawLine,
    /// Region tools: a click clears, engraves or embosses a closed area of a fill tile.
    Region(Option<RegionEffect>),
    /// Drags the magnified Design Page.
    Pan,
    /// Modify Template: moves and scales the template image.
    Template,
}

/// Window state.
pub struct CreatorState {
    pub open: bool,
    pub doc: Creator,
    pub library: Vec<LibraryEntry>,
    pub dir: Option<PathBuf>,
    loaded: bool,
    /// Point being dragged: (path, index).
    drag: Option<(usize, usize)>,
    /// Point last clicked or dragged (Split at Point): (path, index).
    pub selected: Option<(usize, usize)>,
    /// Template image behind the Design Page, and its texture.
    pub template: Option<Template>,
    texture: Option<egui::TextureHandle>,
    /// Modify Template drag: true while scaling from the corner.
    template_drag: Option<bool>,
    pub guides: Guides,
    /// Guideline being dragged: (vertical, index).
    guide_drag: Option<(bool, usize)>,
    /// Design Page scroll offset, and the one Pan asks for.
    scroll: egui::Vec2,
    scroll_to: Option<egui::Vec2>,
    pub message: String,
    /// Preview polylines for the document they were computed from.
    preview: Option<(Creator, Vec<StitchLine>)>,
    /// Ribbon tab (index into `CREATOR.tabs`).
    pub tab: usize,
    pub tool: CreatorTool,
    /// Design Page magnification (1 = fit).
    pub zoom: f32,
    /// Line copied with Copy or Cut.
    pub clipboard: Option<Vec<Vec2>>,
    pub show_preview: bool,
    pub show_status: bool,
    pub show_grid: bool,
    pub open_app_menu: bool,
    hover: Option<&'static str>,
}

impl Default for CreatorState {
    fn default() -> Self {
        CreatorState {
            open: false,
            doc: Creator::default(),
            library: Vec::new(),
            dir: None,
            loaded: false,
            drag: None,
            selected: None,
            template: None,
            texture: None,
            template_drag: None,
            guides: Guides::default(),
            guide_drag: None,
            scroll: egui::Vec2::ZERO,
            scroll_to: None,
            message: String::new(),
            preview: None,
            tab: 0,
            tool: CreatorTool::default(),
            zoom: 1.0,
            clipboard: None,
            show_preview: true,
            show_status: true,
            show_grid: true,
            open_app_menu: false,
            hover: None,
        }
    }
}

impl CreatorState {
    pub fn refresh_library(&mut self) {
        self.dir = library_dir();
        self.library = self.dir.as_deref().map(load_library).unwrap_or_default();
        self.loaded = true;
    }

    /// Replaces the pattern being edited (New, Open, a library or bundled pattern).
    fn set_doc(&mut self, doc: Creator) {
        self.doc = doc;
        self.selected = None;
        self.drag = None;
        if matches!(self.tool, CreatorTool::Region(_)) && self.doc.kind != PatternKind::Fill {
            self.tool = CreatorTool::DrawLine;
        }
        if let Some(t) = &self.template {
            // Keep the image, refitted to the new unit box.
            let fitted = Template::fitted(t.path.clone(), t.size, &self.doc);
            self.template = Some(Template {
                density: t.density,
                ..fitted
            });
        }
    }

    fn current_line(&self) -> Option<Vec<Vec2>> {
        self.doc.paths.get(self.doc.current).cloned()
    }

    fn save(&mut self) {
        let (Ok(p), Some(dir)) = (self.doc.pattern(), self.dir.clone()) else {
            return;
        };
        self.message = match save_to_library(&dir, &p) {
            Ok(path) => format!("Saved {}", path.display()),
            Err(e) => format!("Cannot save: {e}"),
        };
        self.refresh_library();
    }
}

const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 4.0;

impl Host<CreatorAction> for EditorApp {
    fn enabled(&self, a: CreatorAction) -> bool {
        use CreatorAction::*;
        let st = &self.creator;
        let lines = st.doc.paths.len();
        let many = st.doc.kind != PatternKind::Motif && lines < patterns::MAX_PATHS;
        match a {
            Save => st.doc.pattern().is_ok() && st.dir.is_some(),
            SaveAs => st.doc.pattern().is_ok(),
            AddToDesign => st.doc.define_request().is_ok(),
            NewLine | Duplicate => many,
            Paste => many && st.clipboard.is_some(),
            Cut | Delete => lines > 1,
            ZoomIn => st.zoom < MAX_ZOOM,
            ZoomOut => st.zoom > MIN_ZOOM,
            RegionReset | RegionEngrave | RegionEmboss => st.doc.kind == PatternKind::Fill,
            SplitAtPoint => st.selected.is_some_and(|(i, j)| st.doc.can_split(i, j)),
            ModifyTemplate | DeleteImage | ImageDensity => st.template.is_some(),
            _ => true,
        }
    }

    fn active(&self, a: CreatorAction) -> bool {
        use CreatorAction::*;
        let st = &self.creator;
        match a {
            SelectTool => st.tool == CreatorTool::Select,
            EditPoint => st.tool == CreatorTool::EditPoint,
            DrawLine => st.tool == CreatorTool::DrawLine,
            RegionReset => st.tool == CreatorTool::Region(None),
            RegionEngrave => st.tool == CreatorTool::Region(Some(RegionEffect::Engrave)),
            RegionEmboss => st.tool == CreatorTool::Region(Some(RegionEffect::Emboss)),
            Pan => st.tool == CreatorTool::Pan,
            ModifyTemplate => st.tool == CreatorTool::Template,
            Guideline => st.guides.show,
            PreviewWindow => st.show_preview,
            StatusBar => st.show_status,
            GridAxes => st.show_grid,
            _ => false,
        }
    }

    fn run(&mut self, a: CreatorAction, ctx: &egui::Context) {
        use CreatorAction::*;
        if !suite_chrome::enabled(self, a) {
            return;
        }
        let st = &mut self.creator;
        match a {
            NewFill => st.set_doc(Creator::new(PatternKind::Fill)),
            NewMotif => st.set_doc(Creator::new(PatternKind::Motif)),
            NewDecorative => st.set_doc(Creator::new(PatternKind::Decorative)),
            Open => open_file(st),
            Save => st.save(),
            SaveAs => save_as(st),
            Exit => st.open = false,
            SelectTool => st.tool = CreatorTool::Select,
            EditPoint => st.tool = CreatorTool::EditPoint,
            DrawLine => st.tool = CreatorTool::DrawLine,
            RegionReset => st.tool = CreatorTool::Region(None),
            RegionEngrave => st.tool = CreatorTool::Region(Some(RegionEffect::Engrave)),
            RegionEmboss => st.tool = CreatorTool::Region(Some(RegionEffect::Emboss)),
            Pan => st.tool = CreatorTool::Pan,
            SplitAtPoint => {
                if let Some((i, j)) = st.selected
                    && st.doc.split_at(i, j)
                {
                    st.selected = Some((i + 1, 0));
                    st.message = "Line split in two".into();
                }
            }
            OpenTemplate => open_template(st, ctx),
            ModifyTemplate => {
                st.tool = if st.tool == CreatorTool::Template {
                    CreatorTool::DrawLine
                } else {
                    CreatorTool::Template
                }
            }
            DeleteImage => {
                st.template = None;
                st.texture = None;
                if st.tool == CreatorTool::Template {
                    st.tool = CreatorTool::DrawLine;
                }
            }
            Guideline => st.guides.show ^= true,
            NewLine => {
                st.doc.new_path();
            }
            ZoomIn => st.zoom = (st.zoom * 1.25).min(MAX_ZOOM),
            ZoomOut => st.zoom = (st.zoom / 1.25).max(MIN_ZOOM),
            Cut => {
                st.clipboard = st.current_line();
                st.doc.delete_path();
            }
            Copy => {
                st.clipboard = st.current_line();
                st.message = "Line copied".into();
            }
            Paste => {
                if let Some(line) = st.clipboard.clone() {
                    st.doc.paste_path(&line);
                }
            }
            Duplicate => {
                if let Some(line) = st.current_line() {
                    st.doc.paste_path(&line);
                }
            }
            Delete => {
                st.doc.delete_path();
            }
            FlipVertical => st.doc.flip(false),
            FlipHorizontal => st.doc.flip(true),
            AddToDesign => {
                if let Ok(r) = st.doc.define_request() {
                    add_to_design(self, &r);
                }
            }
            PreviewWindow => st.show_preview ^= true,
            StatusBar => st.show_status ^= true,
            GridAxes => st.show_grid ^= true,
            _ => {}
        }
    }

    fn menu(&mut self, a: CreatorAction, ui: &mut egui::Ui) {
        use CreatorAction::*;
        match a {
            Help => crate::ribbon::help_menu(self, ui),
            OtherZoom => {
                for (label, z) in [("Fit to window", 1.0), ("200 %", 2.0), ("400 %", 4.0)] {
                    if ui.button(label).clicked() {
                        self.creator.zoom = z;
                        ui.close();
                    }
                }
            }
            GridInterval => {
                ui.checkbox(&mut self.creator.doc.snap, "Snap to 1/16 of the tile");
            }
            ImageDensity => {
                if let Some(t) = &mut self.creator.template {
                    for d in DENSITIES {
                        let label = if d == 0 {
                            "Hide".to_string()
                        } else {
                            format!("{d} %")
                        };
                        if ui.selectable_label(t.density == d, label).clicked() {
                            t.density = d;
                            ui.close();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn set_hover(&mut self, tip: &'static str) {
        self.creator.hover = Some(tip);
    }
}

fn open_file(st: &mut CreatorState) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("OpenStitch pattern", &["json"])
        .set_directory(st.dir.clone().unwrap_or_default())
        .pick_file()
    else {
        return;
    };
    match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| patterns::parse(&t))
    {
        Ok(p) => {
            st.set_doc(Creator::from_pattern(&p));
            st.message = format!("Opened {}", p.name);
        }
        Err(e) => st.message = format!("Cannot open: {e}"),
    }
}

/// Open Template: a PNG, JPEG or BMP of the user's own behind the Design Page (it replaces
/// the one shown; it is not part of the pattern).
fn open_template(st: &mut CreatorState, ctx: &egui::Context) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
        .pick_file()
    else {
        return;
    };
    match load_template(&path, ctx) {
        Ok((size, texture)) => {
            st.template = Some(Template::fitted(path.clone(), size, &st.doc));
            st.texture = Some(texture);
            st.message = format!("Template {}", path.display());
        }
        Err(e) => st.message = format!("Cannot open the template: {e}"),
    }
}

fn load_template(
    path: &std::path::Path,
    ctx: &egui::Context,
) -> Result<((u32, u32), egui::TextureHandle), String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > openstitch_digitize::raster::MAX_FILE_BYTES as u64 {
        return Err("the image file is too large".into());
    }
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let r = openstitch_digitize::Raster::decode(&data).map_err(|e| e.to_string())?;
    let size = [r.width() as usize, r.height() as usize];
    let mut rgba = Vec::with_capacity(size[0] * size[1] * 4);
    for y in 0..r.height() {
        for x in 0..r.width() {
            rgba.extend_from_slice(&r.pixel(x, y));
        }
    }
    let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
    let texture = ctx.load_texture("creator-template", image, egui::TextureOptions::LINEAR);
    Ok(((r.width(), r.height()), texture))
}

fn save_as(st: &mut CreatorState) {
    let Ok(p) = st.doc.pattern() else {
        return;
    };
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("OpenStitch pattern", &["json"])
        .set_file_name(creator::library_file_name(&p))
        .save_file()
    {
        st.message = match std::fs::write(&path, p.to_json()) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Cannot save: {e}"),
        };
    }
}

fn add_to_design(app: &mut EditorApp, r: &CommandRequest) {
    app.creator.message = if app.execute(r).is_some() {
        format!(
            "'{}' is in the design; choose it in the Sewing Attributes pane",
            r.params["pattern"]["id"].as_str().unwrap_or("?")
        )
    } else {
        app.status.clone()
    };
}

fn tool_hint(t: CreatorTool) -> &'static str {
    match t {
        CreatorTool::Select => "Click a line to make it the current one",
        CreatorTool::EditPoint => "Drag a point to move it · right-click deletes it",
        CreatorTool::DrawLine => "Click: add a point · drag: move · right-click: delete",
        CreatorTool::Region(None) => "Click an embossed or engraved area to clear its effect",
        CreatorTool::Region(Some(RegionEffect::Engrave)) => {
            "Click a closed area of the tile to engrave it"
        }
        CreatorTool::Region(Some(RegionEffect::Emboss)) => {
            "Click a closed area of the tile to emboss it"
        }
        CreatorTool::Pan => "Drag to scroll the Design Page",
        CreatorTool::Template => {
            "Drag inside the red frame to move the template, its corner to scale; click outside to finish"
        }
    }
}

/// Draws the whole window in place of Layout & Editing.
pub fn creator(app: &mut EditorApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    if !app.creator.loaded {
        app.creator.refresh_library();
    }
    app.creator.hover = None;
    suite_chrome::shortcuts(app, &ctx, &CREATOR.actions());
    // F6 steps the template's Background Image Density down (100, 75, 50, 25, Hide).
    if !ctx.egui_wants_keyboard_input()
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F6))
        && let Some(t) = &mut app.creator.template
    {
        t.cycle_density();
    }
    let mut tab = app.creator.tab;
    let mut open_menu = std::mem::take(&mut app.creator.open_app_menu);
    egui::Panel::top("pc-ribbon").show(ui, |ui| {
        suite_chrome::ribbon(app, ui, &CREATOR, &mut tab, &mut open_menu);
    });
    app.creator.tab = tab;
    if app.creator.show_status {
        egui::Panel::bottom("pc-status").show(ui, |ui| {
            let st = &app.creator;
            let text = if st.message.is_empty() {
                tool_hint(st.tool)
            } else {
                st.message.as_str()
            };
            let points: usize = st.doc.paths.iter().map(Vec::len).sum();
            suite_chrome::status_bar(ui, st.hover, text, |ui| {
                ui.label(format!("{:.0} %", st.zoom * 100.0));
                ui.separator();
                ui.label(format!(
                    "{} · {} lines · {points} points",
                    kind_name(st.doc.kind),
                    st.doc.paths.len()
                ));
            });
        });
    }
    let mut requests: Vec<CommandRequest> = Vec::new();
    let user = app.session.design().patterns.clone();
    egui::Panel::left("pc-library")
        .default_size(300.0)
        .show(ui, |ui| library(ui, &mut app.creator, &mut requests));
    if app.creator.show_preview {
        egui::Panel::right("pc-preview")
            .default_size(440.0)
            .show(ui, |ui| {
                ui.strong("Preview");
                preview(ui, &mut app.creator);
            });
    }
    egui::CentralPanel::default().show(ui, |ui| {
        let st = &mut app.creator;
        header(ui, st, &user);
        if let Err(e) = st.doc.define_request() {
            ui.colored_label(Color32::from_rgb(220, 90, 70), e);
        }
        ui.separator();
        let mut area = egui::ScrollArea::both();
        if let Some(offset) = st.scroll_to.take() {
            area = area.scroll_offset(offset);
        }
        st.scroll = area
            .show(ui, |ui| edit_canvas(ui, st))
            .state
            .clamped_offset();
    });
    for r in requests {
        add_to_design(app, &r);
    }
    crate::panes::about_window(app, &ctx);
}

fn header(ui: &mut egui::Ui, st: &mut CreatorState, user: &[patterns::Pattern]) {
    ui.horizontal(|ui| {
        ui.label("Kind");
        let current = KINDS
            .iter()
            .find(|k| k.0 == st.doc.kind)
            .map_or("?", |k| k.1);
        egui::ComboBox::from_id_salt("creator-kind")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (k, label) in KINDS {
                    if ui.selectable_label(st.doc.kind == k, label).clicked() && st.doc.kind != k {
                        st.set_doc(Creator::new(k));
                    }
                }
            });
        ui.label("Id");
        ui.add(egui::TextEdit::singleline(&mut st.doc.id).desired_width(120.0));
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut st.doc.name).desired_width(140.0));
        if st.doc.kind == PatternKind::Decorative {
            ui.label("Row shift");
            ui.add(egui::Slider::new(&mut st.doc.row_shift, 0.0..=1.0));
        }
        let kind = st.doc.kind;
        egui::ComboBox::from_id_salt("creator-template")
            .selected_text("Start from…")
            .show_ui(ui, |ui| {
                for p in patterns::library()
                    .iter()
                    .chain(user)
                    .filter(|p| p.kind == kind)
                {
                    if ui.selectable_label(false, &p.name).clicked() {
                        let bundled = patterns::is_bundled(&p.id);
                        st.set_doc(Creator::from_pattern(p));
                        if bundled {
                            st.doc.id = format!("my-{}", p.id);
                            st.doc.name = format!("My {}", p.name.to_lowercase());
                        }
                    }
                }
            });
    });
}

/// Maps the unit box into `rect`, keeping its aspect ratio.
struct Map {
    origin: Pos2,
    scale: f32,
    x0: f64,
    y0: f64,
}

impl Map {
    fn new(rect: egui::Rect, doc: &Creator) -> Self {
        let (x0, y0, x1, y1) = doc.unit_box();
        let scale = (rect.width() / (x1 - x0) as f32).min(rect.height() / (y1 - y0) as f32);
        Map {
            origin: rect.min,
            scale,
            x0,
            y0,
        }
    }
    fn to_screen(&self, p: Vec2) -> Pos2 {
        Pos2::new(
            self.origin.x + ((p.x - self.x0) as f32) * self.scale,
            self.origin.y + ((p.y - self.y0) as f32) * self.scale,
        )
    }
    fn to_unit(&self, p: Pos2) -> Vec2 {
        v2(
            f64::from((p.x - self.origin.x) / self.scale) + self.x0,
            f64::from((p.y - self.origin.y) / self.scale) + self.y0,
        )
    }
}

/// Ruler width when guidelines are shown, points.
const RULER: f32 = 16.0;
const GUIDE: Color32 = Color32::from_rgb(40, 160, 90);
const ENGRAVE: Color32 = Color32::from_rgba_premultiplied(120, 20, 20, 70);
const EMBOSS: Color32 = Color32::from_rgba_premultiplied(20, 50, 130, 70);

fn edit_canvas(ui: &mut egui::Ui, st: &mut CreatorState) {
    let (x0, y0, x1, y1) = st.doc.unit_box();
    let w = 480.0 * st.zoom;
    let h = (w * ((y1 - y0) / (x1 - x0)) as f32).min(480.0 * st.zoom);
    let ruler = if st.guides.show { RULER } else { 0.0 };
    let (outer, _) = ui.allocate_exact_size(egui::vec2(w + ruler, h + ruler), Sense::hover());
    let rect = egui::Rect::from_min_size(outer.min + egui::vec2(ruler, ruler), egui::vec2(w, h));
    let resp = ui.interact(rect, ui.scope_id().with("pc-page"), Sense::click_and_drag());
    let painter = ui.painter_at(outer);
    painter.rect_filled(rect, 4.0, Color32::from_gray(250));
    let map = Map::new(rect, &st.doc);
    // Template image, faded by the Background Image Density.
    if let (Some(t), Some(tex)) = (&st.template, &st.texture)
        && t.density > 0
    {
        let r = egui::Rect::from_two_pos(map.to_screen(t.min), map.to_screen(t.max()));
        let uv = egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let alpha = (f32::from(t.density) / 100.0 * 255.0) as u8;
        painter
            .with_clip_rect(rect)
            .image(tex.id(), r, uv, Color32::from_white_alpha(alpha));
    }
    // Embossed (blue) and engraved (red) areas of a fill tile.
    if let Some(mask) = st.doc.region_mask() {
        let n = MASK_CELLS;
        let cell = 1.0 / n as f64;
        for y in 0..n {
            let mut x = 0;
            while x < n {
                let e = mask.cell(x, y);
                let mut end = x + 1;
                while end < n && mask.cell(end, y) == e {
                    end += 1;
                }
                if let Some(e) = e {
                    let colour = match e {
                        RegionEffect::Engrave => ENGRAVE,
                        RegionEffect::Emboss => EMBOSS,
                    };
                    let r = egui::Rect::from_two_pos(
                        map.to_screen(v2(x as f64 * cell, y as f64 * cell)),
                        map.to_screen(v2(end as f64 * cell, (y + 1) as f64 * cell)),
                    );
                    painter.rect_filled(r, 0.0, colour);
                }
                x = end;
            }
        }
    }
    // Grid and the tile (or motif cell) itself.
    if st.show_grid {
        let grid = Stroke::new(1.0, Color32::from_gray(230));
        let steps = ((x1 - x0) / creator::SNAP).round() as i32;
        for i in 0..=steps {
            let x = x0 + f64::from(i) * creator::SNAP;
            painter.line_segment([map.to_screen(v2(x, y0)), map.to_screen(v2(x, y1))], grid);
        }
        let steps = ((y1 - y0) / creator::SNAP).round() as i32;
        for i in 0..=steps {
            let y = y0 + f64::from(i) * creator::SNAP;
            painter.line_segment([map.to_screen(v2(x0, y)), map.to_screen(v2(x1, y))], grid);
        }
    }
    let cell = egui::Rect::from_two_pos(
        map.to_screen(v2(
            0.0,
            if st.doc.kind == PatternKind::Motif {
                -1.0
            } else {
                0.0
            },
        )),
        map.to_screen(v2(1.0, 1.0)),
    );
    painter.rect_stroke(
        cell,
        0.0,
        Stroke::new(1.5, Color32::from_rgb(120, 140, 200)),
        egui::StrokeKind::Inside,
    );
    if st.doc.kind == PatternKind::Motif {
        painter.line_segment(
            [map.to_screen(v2(x0, 0.0)), map.to_screen(v2(x1, 0.0))],
            Stroke::new(1.0, Color32::from_rgb(120, 140, 200)),
        );
    }
    // Neighbouring repeats, faint, so tiling and joins are visible.
    let ghost = Stroke::new(1.0, Color32::from_gray(190));
    let shifts: &[(f64, f64)] = match st.doc.kind {
        PatternKind::Fill => &[(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)],
        _ => &[(-1.0, 0.0), (1.0, 0.0)],
    };
    let page = painter.with_clip_rect(rect);
    for &(dx, dy) in shifts {
        for path in &st.doc.paths {
            let pts: Vec<Pos2> = path
                .iter()
                .map(|p| map.to_screen(v2(p.x + dx, p.y + dy)))
                .collect();
            page.add(egui::Shape::line(pts, ghost));
        }
    }
    for (i, path) in st.doc.paths.iter().enumerate() {
        let colour = if i == st.doc.current {
            Color32::from_rgb(10, 85, 163)
        } else {
            Color32::from_rgb(120, 120, 120)
        };
        let pts: Vec<Pos2> = path.iter().map(|p| map.to_screen(*p)).collect();
        painter.add(egui::Shape::line(pts.clone(), Stroke::new(2.0, colour)));
        for (j, p) in pts.into_iter().enumerate() {
            painter.circle_filled(p, 4.0, colour);
            if st.selected == Some((i, j)) {
                painter.circle_stroke(p, 7.0, Stroke::new(1.5, Color32::from_rgb(230, 120, 0)));
            }
        }
    }
    if let Some(t) = &st.template
        && st.tool == CreatorTool::Template
    {
        let r = egui::Rect::from_two_pos(map.to_screen(t.min), map.to_screen(t.max()));
        let red = Color32::from_rgb(220, 40, 40);
        page.rect_stroke(r, 0.0, Stroke::new(1.5, red), egui::StrokeKind::Outside);
        page.rect_filled(
            egui::Rect::from_center_size(r.max, egui::vec2(9.0, 9.0)),
            0.0,
            red,
        );
    }
    if st.guides.show {
        guides(ui, st, &painter, outer, rect, &map);
    }
    // Interaction.
    let nearest = |pos: Pos2, doc: &Creator| -> Option<(usize, usize)> {
        let mut best = None;
        let mut best_d = 9.0f32;
        for (i, path) in doc.paths.iter().enumerate() {
            for (j, p) in path.iter().enumerate() {
                let d = map.to_screen(*p).distance(pos);
                if d < best_d {
                    (best, best_d) = (Some((i, j)), d);
                }
            }
        }
        best
    };
    let tool = st.tool;
    match tool {
        CreatorTool::Pan => {
            if resp.hovered() {
                ui.ctx().set_cursor_icon(if resp.dragged() {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                });
            }
            if resp.dragged() {
                st.scroll_to = Some(st.scroll - resp.drag_delta());
            }
            return;
        }
        CreatorTool::Region(effect) => {
            if resp.clicked()
                && let Some(pos) = resp.interact_pointer_pos()
            {
                st.message = match st.doc.set_region(map.to_unit(pos), effect) {
                    Ok(()) => String::new(),
                    Err(e) => e,
                };
            }
            return;
        }
        CreatorTool::Template => {
            template_tool(st, &resp, &map);
            return;
        }
        _ => {}
    }
    if let Some(pos) = resp.interact_pointer_pos() {
        if tool == CreatorTool::Select {
            if resp.clicked()
                && let Some((i, _)) = nearest(pos, &st.doc)
            {
                st.doc.current = i;
            }
            return;
        }
        if resp.drag_started() {
            st.drag = nearest(pos, &st.doc);
            if let Some((i, _)) = st.drag {
                st.doc.current = i;
                st.selected = st.drag;
            }
        }
        if resp.dragged()
            && let Some((i, j)) = st.drag
        {
            st.doc.move_point(i, j, map.to_unit(pos));
        }
        if resp.clicked() {
            match nearest(pos, &st.doc) {
                Some((i, j)) => {
                    st.doc.current = i;
                    st.selected = Some((i, j));
                }
                None if tool == CreatorTool::DrawLine => {
                    let path = st.doc.current;
                    st.selected = st.doc.add_point(map.to_unit(pos)).map(|j| (path, j));
                }
                None => {}
            }
        }
        if resp.secondary_clicked()
            && let Some((i, j)) = nearest(pos, &st.doc)
            && st.doc.delete_point(i, j)
        {
            st.selected = None;
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
    }
}

/// Modify Template: drag inside the frame moves the image, near its bottom-right corner
/// scales it; a click outside ends the mode.
fn template_tool(st: &mut CreatorState, resp: &egui::Response, map: &Map) {
    let Some(t) = &mut st.template else {
        st.tool = CreatorTool::DrawLine;
        return;
    };
    let Some(pos) = resp.interact_pointer_pos() else {
        return;
    };
    let at = map.to_unit(pos);
    if resp.drag_started() {
        let corner = map.to_screen(t.max()).distance(pos) < 10.0;
        st.template_drag = (corner || t.contains(at)).then_some(corner);
    }
    if resp.dragged() {
        match st.template_drag {
            Some(true) => t.resize_to(at),
            Some(false) => {
                let d = resp.drag_delta();
                t.translate(v2(f64::from(d.x / map.scale), f64::from(d.y / map.scale)));
            }
            None => {}
        }
    }
    if resp.drag_stopped() {
        st.template_drag = None;
    }
    if resp.clicked() && !t.contains(at) {
        st.tool = CreatorTool::DrawLine;
    }
}

/// Rulers along the top (vertical guidelines) and left (horizontal ones) and the guidelines
/// over the Design Page.
fn guides(
    ui: &mut egui::Ui,
    st: &mut CreatorState,
    painter: &egui::Painter,
    outer: egui::Rect,
    rect: egui::Rect,
    map: &Map,
) {
    let snap = st.show_grid;
    let tolerance = f64::from(5.0 / map.scale);
    for vertical in [true, false] {
        let ruler = if vertical {
            egui::Rect::from_min_max(
                Pos2::new(rect.min.x, outer.min.y),
                Pos2::new(rect.max.x, rect.min.y),
            )
        } else {
            egui::Rect::from_min_max(
                Pos2::new(outer.min.x, rect.min.y),
                Pos2::new(rect.min.x, rect.max.y),
            )
        };
        painter.rect_filled(ruler, 0.0, Color32::from_gray(225));
        // Ticks every 1/4 of the unit box.
        let (x0, y0, x1, y1) = st.doc.unit_box();
        let (lo, hi) = if vertical { (x0, x1) } else { (y0, y1) };
        let mut v = (lo * 4.0).ceil() / 4.0;
        while v <= hi + 1e-9 {
            let (a, b) = if vertical {
                let x = map.to_screen(v2(v, 0.0)).x;
                (Pos2::new(x, ruler.max.y - 6.0), Pos2::new(x, ruler.max.y))
            } else {
                let y = map.to_screen(v2(0.0, v)).y;
                (Pos2::new(ruler.max.x - 6.0, y), Pos2::new(ruler.max.x, y))
            };
            painter.line_segment([a, b], Stroke::new(1.0, Color32::from_gray(120)));
            v += 0.25;
        }
        let resp = ui.interact(
            ruler,
            ui.scope_id().with(("pc-ruler", vertical)),
            Sense::click_and_drag(),
        );
        if let Some(pos) = resp.interact_pointer_pos() {
            let u = map.to_unit(pos);
            let at = if vertical { u.x } else { u.y };
            let near = st.guides.nearest(vertical, at, tolerance);
            if resp.drag_started() {
                st.guide_drag = near
                    .or_else(|| st.guides.add(vertical, at, snap))
                    .map(|i| (vertical, i));
            }
            if resp.dragged()
                && let Some((v, i)) = st.guide_drag
                && v == vertical
            {
                st.guides.move_to(v, i, at, snap);
            }
            if resp.clicked() && near.is_none() {
                st.guides.add(vertical, at, snap);
            }
            if resp.secondary_clicked()
                && let Some(i) = near
            {
                st.guides.remove(vertical, i);
            }
        }
        if resp.drag_stopped() {
            st.guide_drag = None;
        }
    }
    let stroke = Stroke::new(1.0, GUIDE);
    for &x in &st.guides.vertical {
        let sx = map.to_screen(v2(x, 0.0)).x;
        painter.line_segment(
            [Pos2::new(sx, outer.min.y), Pos2::new(sx, rect.max.y)],
            stroke,
        );
    }
    for &y in &st.guides.horizontal {
        let sy = map.to_screen(v2(0.0, y)).y;
        painter.line_segment(
            [Pos2::new(outer.min.x, sy), Pos2::new(rect.max.x, sy)],
            stroke,
        );
    }
}

fn preview(ui: &mut egui::Ui, st: &mut CreatorState) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(420.0, 280.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::WHITE);
    let fresh = st.preview.as_ref().is_some_and(|(doc, _)| *doc == st.doc);
    if !fresh {
        let lines = st
            .doc
            .preview_design()
            .map(|d| stitch_lines(&Compiler::default().compile(&d)))
            .unwrap_or_default();
        st.preview = Some((st.doc.clone(), lines));
    }
    let Some((_, lines)) = &st.preview else {
        return;
    };
    if lines.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Fix the pattern to see a preview",
            egui::FontId::proportional(13.0),
            Color32::GRAY,
        );
        return;
    }
    // The preview design is 60 × 40 mm (600 × 400 units).
    let scale = (rect.width() / 600.0).min(rect.height() / 400.0);
    let to = |p: Vec2| {
        Pos2::new(
            rect.min.x + p.x as f32 * scale,
            rect.min.y + p.y as f32 * scale,
        )
    };
    // Thin thread with every needle point, so programmable-fill textures show.
    for (colour, line) in lines {
        let pts: Vec<Pos2> = line.iter().map(|p| to(*p)).collect();
        painter.add(egui::Shape::line(pts.clone(), Stroke::new(0.6, *colour)));
        for p in pts {
            painter.circle_filled(p, 0.9, Color32::from_gray(30));
        }
    }
}

/// Sewn polylines of a compiled pattern (jumps, trims and colour changes break lines).
fn stitch_lines(p: &openstitch_core::StitchPattern) -> Vec<StitchLine> {
    let mut out = Vec::new();
    let mut block = 0;
    let mut current: Vec<Vec2> = Vec::new();
    let colour = |b: usize| {
        p.threads.get(b).map_or(Color32::BLACK, |t| {
            Color32::from_rgb(t.red, t.green, t.blue)
        })
    };
    for s in &p.stitches {
        match s.command {
            Command::Stitch => current.push(v2(s.x, s.y)),
            _ => {
                if current.len() > 1 {
                    out.push((colour(block), std::mem::take(&mut current)));
                }
                current.clear();
                if s.command == Command::ColorChange {
                    block += 1;
                }
                if s.command == Command::Jump {
                    current.push(v2(s.x, s.y));
                }
            }
        }
    }
    if current.len() > 1 {
        out.push((colour(block), current));
    }
    out
}

fn library(ui: &mut egui::Ui, st: &mut CreatorState, requests: &mut Vec<CommandRequest>) {
    ui.horizontal(|ui| {
        ui.strong("User library");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Refresh").clicked() {
                st.refresh_library();
            }
        });
    });
    match &st.dir {
        Some(d) => {
            let text = d.display().to_string();
            ui.add(egui::Label::new(egui::RichText::new(&text).weak()).truncate())
                .on_hover_text(text);
        }
        None => {
            ui.weak("No user folder (set OPENSTITCH_PATTERNS)");
        }
    }
    ui.separator();
    if st.library.is_empty() {
        crate::art::header(ui, crate::art::Art::PatternLibrary, |ui| {
            ui.add_space(8.0);
            ui.weak("No saved patterns yet.");
            ui.weak("Save a pattern to add it here.");
        });
        return;
    }
    let mut edit = None;
    let mut remove = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, e) in st.library.iter().enumerate() {
            let file = e
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match &e.pattern {
                Ok(p) => {
                    ui.label(format!("{} ({}, {})", p.name, kind_name(p.kind), p.id));
                    ui.horizontal(|ui| {
                        if ui.button("Edit").clicked() {
                            edit = Some(i);
                        }
                        if ui.button("Add to Design").clicked() {
                            let doc: serde_json::Value =
                                serde_json::from_str(&p.to_json()).unwrap_or_default();
                            requests.push(CommandRequest::new(
                                "pattern.define",
                                json!({ "pattern": doc }),
                            ));
                        }
                        if ui.button("Delete file").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                Err(err) => {
                    ui.colored_label(Color32::from_rgb(220, 90, 70), &file);
                    ui.weak(err);
                }
            }
            ui.add_space(4.0);
        }
    });
    if let Some(i) = edit
        && let Ok(p) = st.library[i].pattern.clone()
    {
        st.set_doc(Creator::from_pattern(&p));
    }
    if let Some(i) = remove {
        let path = st.library[i].path.clone();
        st.message = match std::fs::remove_file(&path) {
            Ok(()) => format!("Deleted {}", path.display()),
            Err(e) => format!("Cannot delete: {e}"),
        };
        st.refresh_library();
    }
}
