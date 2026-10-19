//! Stitch Design Factory (Options ▸ Stitch Design Factory): a window of its own in place of
//! Layout & Editing, with the suite's Application menu, ribbon (Home, View), ScreenTips, Import
//! and Preview panes and status bar ([`openstitch_app::editor::suite::FACTORY`]). It edits
//! sewing-machine stitch patterns ([`openstitch_app::editor::sewing`]), keeps them in the
//! project (`sewing.define`, one Undo) and exports them (`sewing.export`: openstitch.sewing JSON,
//! SVG, experimental PMV). Swap and Duplicate change the pattern itself (as `sewing.swap` and
//! `sewing.duplicate` do for one in the project); the template image, orientation, grid
//! spacing and the Stitch Simulator are window state.

use crate::app::EditorApp;
use crate::suite_chrome::{self, Host};
use eframe::egui::{self, Color32, Pos2, Sense, Stroke};
use openstitch_app::editor::CommandRequest;
use openstitch_app::editor::sewing::{GRID_SPACINGS, Handle, SewingEditor};
use openstitch_app::editor::suite::{FACTORY, FactoryAction};
use openstitch_design::sewing::{self, MAX_WIDTH, SewingKind};
use openstitch_design::{Vec2, v2};
use serde_json::json;

/// What a click in the design area does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FactoryTool {
    /// Selects the whole pattern and moves it.
    SelectObject,
    /// Selects, moves and deletes points.
    SelectPoint,
    /// Adds points (and moves them).
    #[default]
    Line,
    /// Adds points along a dragged stroke.
    Pen,
    /// Scrolls the design area.
    Pan,
}

/// A template: an image of the user's own shown behind the pattern to trace, in pattern mm
/// (it turns with the design page).
pub struct Template {
    pub path: std::path::PathBuf,
    texture: egui::TextureHandle,
    /// Top-left corner (mm).
    pub origin: Vec2,
    /// Size (mm).
    pub size: Vec2,
}

/// The Stitch Simulator window: the needle points of three repeats sewn one by one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FactorySim {
    pub open: bool,
    pub playing: bool,
    /// Needle points sewn so far (`None`: all).
    pub pos: Option<usize>,
    pub speed: f64,
    acc: f64,
}

impl Default for FactorySim {
    fn default() -> Self {
        FactorySim {
            open: false,
            playing: false,
            pos: None,
            speed: 50.0,
            acc: 0.0,
        }
    }
}

/// Repeats the simulator and the design area show.
const REPEATS: usize = 3;
/// Template Density steps F6 goes through.
const DENSITIES: [f32; 5] = [1.0, 0.75, 0.5, 0.25, 0.0];

/// How a modify-template drag changes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TemplateDrag {
    Move,
    Scale,
}

/// Window state.
pub struct SewingState {
    pub open: bool,
    pub editor: SewingEditor,
    drag: Option<Handle>,
    pub message: String,
    /// Ribbon tab (index into `FACTORY.tabs`).
    pub tab: usize,
    pub tool: FactoryTool,
    /// Point Delete removes.
    pub selected: Option<Handle>,
    /// Design area magnification.
    pub zoom: f32,
    pub show_import: bool,
    pub show_preview: bool,
    pub show_status: bool,
    pub show_grid: bool,
    pub show_needles: bool,
    pub open_app_menu: bool,
    hover: Option<&'static str>,
    /// Select Object: the pattern is selected.
    pub object_selected: bool,
    /// Select Object drag: movement not applied yet (mm).
    move_rest: Vec2,
    /// Pen Tool: the stroke being drawn (mm).
    stroke: Vec<Vec2>,
    /// Landscape design page (feed to the right); off, the feed runs upwards.
    pub horizontal: bool,
    pub template: Option<Template>,
    /// Modify Template mode (red frame).
    pub modify_template: bool,
    template_drag: Option<TemplateDrag>,
    /// Template Density (0 hides it).
    pub template_density: f32,
    pub sim: FactorySim,
    /// Size of the visible design area (points), for Selected object zoom.
    view_size: egui::Vec2,
    /// Selected object zoom: scroll the pattern into view next frame.
    reveal: bool,
}

impl Default for SewingState {
    fn default() -> Self {
        SewingState {
            open: false,
            editor: SewingEditor::default(),
            drag: None,
            message: String::new(),
            tab: 0,
            tool: FactoryTool::default(),
            selected: None,
            zoom: 1.0,
            show_import: true,
            show_preview: true,
            show_status: true,
            show_grid: true,
            show_needles: true,
            open_app_menu: false,
            hover: None,
            object_selected: false,
            move_rest: v2(0.0, 0.0),
            stroke: Vec::new(),
            horizontal: true,
            template: None,
            modify_template: false,
            template_drag: None,
            template_density: 0.5,
            sim: FactorySim::default(),
            view_size: egui::vec2(800.0, 300.0),
            reveal: false,
        }
    }
}

/// Pixels per mm in the design area at 100 %.
const PX: f32 = 28.0;
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 3.0;

fn in_project(app: &EditorApp) -> bool {
    let id = &app.sewing.editor.doc.id;
    app.session.design().sewing.iter().any(|q| &q.id == id)
}

impl Host<FactoryAction> for EditorApp {
    fn enabled(&self, a: FactoryAction) -> bool {
        use FactoryAction::*;
        let st = &self.sewing;
        let cross = st.editor.doc.kind == SewingKind::Cross;
        match a {
            Save => st.editor.define_request().is_ok(),
            SaveAs => st.editor.doc.validate().is_ok(),
            Export => in_project(self),
            SingleStitch | TripleStitch => cross,
            Delete => st.selected.is_some() || (st.modify_template && st.template.is_some()),
            Duplicate => st.editor.doc.duplicate().is_ok(),
            ModifyTemplate | DeleteImage | TemplateDensity => st.template.is_some(),
            ZoomIn => st.zoom < MAX_ZOOM,
            ZoomOut => st.zoom > MIN_ZOOM,
            _ => true,
        }
    }

    fn active(&self, a: FactoryAction) -> bool {
        use FactoryAction::*;
        let st = &self.sewing;
        match a {
            SelectObject => st.tool == FactoryTool::SelectObject,
            SelectPoint => st.tool == FactoryTool::SelectPoint,
            LineTool => st.tool == FactoryTool::Line,
            PenTool => st.tool == FactoryTool::Pen,
            Pan => st.tool == FactoryTool::Pan,
            ModifyTemplate => st.modify_template,
            HorizontalOrientation => st.horizontal,
            StitchSimulator => st.sim.open,
            SingleStitch => !st.editor.doc.triple,
            TripleStitch => st.editor.doc.triple,
            NeedleDropPoint => st.show_needles,
            ImportPane => st.show_import,
            PreviewWindow => st.show_preview,
            StatusBar => st.show_status,
            ShowGrid => st.show_grid,
            SnapToGrid => st.editor.snap,
            _ => false,
        }
    }

    fn run(&mut self, a: FactoryAction, ctx: &egui::Context) {
        use FactoryAction::*;
        if !suite_chrome::enabled(self, a) {
            return;
        }
        let st = &mut self.sewing;
        match a {
            Open => open_file(st),
            Save => {
                if let Ok(r) = st.editor.define_request() {
                    save_in_project(self, &r);
                }
            }
            SaveAs => save_as(st),
            Export => export(self),
            Exit => st.open = false,
            SelectObject => set_tool(st, FactoryTool::SelectObject),
            SelectPoint => set_tool(st, FactoryTool::SelectPoint),
            LineTool => set_tool(st, FactoryTool::Line),
            PenTool => set_tool(st, FactoryTool::Pen),
            Pan => set_tool(st, FactoryTool::Pan),
            ZoomIn => st.zoom = (st.zoom * 1.25).min(MAX_ZOOM),
            ZoomOut => st.zoom = (st.zoom / 1.25).max(MIN_ZOOM),
            SelectedZoom => zoom_to_pattern(st),
            Swap => {
                st.editor.swap();
                st.message = if st.editor.doc.swapped {
                    "Start and end swapped: sewn from the other end".into()
                } else {
                    "Start and end swapped back".into()
                };
            }
            Duplicate => {
                st.message = match st.editor.duplicate() {
                    Ok(()) => format!("Duplicated: {:.1} mm long", st.editor.doc.length()),
                    Err(e) => format!("Cannot duplicate: {e}"),
                };
                st.selected = None;
            }
            SingleStitch => st.editor.doc.triple = false,
            TripleStitch => st.editor.doc.triple = true,
            FlipVertical => st.editor.flip(false),
            FlipHorizontal => {
                st.editor.flip(true);
                st.selected = None;
            }
            Delete => {
                if let Some(h) = st.selected.take() {
                    st.editor.delete(h);
                } else if st.modify_template {
                    delete_template(st);
                }
            }
            OpenTemplate => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Image",
                        &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff"],
                    )
                    .pick_file()
                {
                    st.message = match open_template(st, &path, ctx) {
                        Ok(()) => format!("Template {}", path.display()),
                        Err(e) => format!("Cannot open the template: {e}"),
                    };
                }
            }
            ModifyTemplate => {
                st.modify_template ^= true;
                st.template_drag = None;
            }
            DeleteImage => delete_template(st),
            HorizontalOrientation => {
                st.horizontal ^= true;
                st.reveal = true;
            }
            StitchSimulator => {
                st.sim.open ^= true;
                st.sim.playing = false;
            }
            NeedleDropPoint => st.show_needles ^= true,
            ImportPane => st.show_import ^= true,
            PreviewWindow => st.show_preview ^= true,
            StatusBar => st.show_status ^= true,
            ShowGrid => st.show_grid ^= true,
            SnapToGrid => st.editor.snap ^= true,
            _ => {}
        }
    }

    fn menu(&mut self, a: FactoryAction, ui: &mut egui::Ui) {
        use FactoryAction::*;
        match a {
            New => {
                for k in SewingKind::ALL {
                    if ui.button(k.label()).clicked() {
                        self.sewing.editor = SewingEditor::new(k);
                        self.sewing.selected = None;
                        ui.close();
                    }
                }
            }
            TemplateDensity => {
                let st = &mut self.sewing;
                ui.add(
                    egui::Slider::new(&mut st.template_density, 0.0..=1.0)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                );
                ui.horizontal(|ui| {
                    if ui.button("Lighter").clicked() {
                        st.template_density = (st.template_density - 0.25).max(0.0);
                    }
                    if ui.button("Darker").clicked() {
                        st.template_density = (st.template_density + 0.25).min(1.0);
                    }
                });
            }
            GridSpacing => {
                for g in GRID_SPACINGS {
                    let on = (self.sewing.editor.grid - g).abs() < 1e-9;
                    if ui.selectable_label(on, format!("{g} mm")).clicked() {
                        self.sewing.editor.grid = g;
                        ui.close();
                    }
                }
            }
            Options => suite_chrome::theme_menu(self, ui),
            Help => crate::ribbon::help_menu(self, ui),
            _ => {}
        }
    }

    fn set_hover(&mut self, tip: &'static str) {
        self.sewing.hover = Some(tip);
    }
}

fn set_tool(st: &mut SewingState, tool: FactoryTool) {
    st.tool = tool;
    st.object_selected = tool == FactoryTool::SelectObject && st.object_selected;
    st.stroke.clear();
}

/// Selected object zoom: the pattern (the only object) fills the visible design area.
fn zoom_to_pattern(st: &mut SewingState) {
    let Some((lo, hi)) = st.editor.bounds() else {
        return;
    };
    let (along, across) = ((hi.x - lo.x) as f32 + 2.0, st.editor.doc.width as f32 + 2.0);
    let (w, h) = if st.horizontal {
        (along, across)
    } else {
        (across, along)
    };
    let fit = (st.view_size.x / (w * PX)).min(st.view_size.y / (h * PX));
    st.zoom = fit.clamp(MIN_ZOOM, MAX_ZOOM);
    st.reveal = true;
}

/// Reads a user-supplied image (size-limited decoder) and shows it behind the pattern, as
/// wide as the pattern's first repeat, centred across the feed. Replaces an open template.
pub fn open_template(
    st: &mut SewingState,
    path: &std::path::Path,
    ctx: &egui::Context,
) -> Result<(), String> {
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
    let texture = ctx.load_texture("sdf-template", image, egui::TextureOptions::LINEAR);
    let width = st.editor.doc.length().max(8.0);
    let height = width * f64::from(r.height()) / f64::from(r.width());
    st.template = Some(Template {
        path: path.to_path_buf(),
        texture,
        origin: v2(0.0, -height / 2.0),
        size: v2(width, height),
    });
    st.modify_template = false;
    Ok(())
}

fn delete_template(st: &mut SewingState) {
    st.template = None;
    st.modify_template = false;
    st.template_drag = None;
    st.message = "Template removed".into();
}

/// Advances the Stitch Simulator while it plays.
fn tick(sim: &mut FactorySim, total: usize, ctx: &egui::Context) {
    if !sim.open || !sim.playing {
        return;
    }
    let dt = f64::from(ctx.input(|i| i.stable_dt)).min(0.1);
    sim.acc += dt * sim.speed;
    let step = sim.acc.floor() as usize;
    sim.acc -= step as f64;
    let n = sim.pos.unwrap_or(0) + step;
    if n >= total {
        sim.pos = None;
        sim.playing = false;
    } else {
        sim.pos = Some(n);
    }
    ctx.request_repaint();
}

/// The needle points of `repeats` repeats in sewing order (mm).
fn repeated_needles(e: &SewingEditor, repeats: usize) -> Vec<Vec2> {
    let needles = e.doc.needles();
    let len = e.doc.length();
    // A swapped pattern sews each repeat from its far end, travelling the other way.
    let order: Vec<usize> = if e.doc.swapped {
        (0..repeats).rev().collect()
    } else {
        (0..repeats).collect()
    };
    order
        .into_iter()
        .flat_map(|r| {
            let shift = len * r as f64;
            needles.iter().map(move |p| v2(p.x + shift, p.y))
        })
        .collect()
}

/// Template Density: F6 steps to the next lighter level (from hidden back to full).
fn step_density(st: &mut SewingState) {
    let next = DENSITIES
        .iter()
        .position(|&d| d < st.template_density - 1e-3)
        .map_or(DENSITIES[0], |i| DENSITIES[i]);
    st.template_density = next;
}

fn open_file(st: &mut SewingState) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("OpenStitch sewing stitch", &["json"])
        .pick_file()
    else {
        return;
    };
    match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| sewing::parse(&t))
    {
        Ok(q) => {
            st.editor = SewingEditor::from_stitch(&q);
            st.selected = None;
            st.message = format!("Opened {}", q.name);
        }
        Err(e) => st.message = format!("Cannot open: {e}"),
    }
}

fn save_as(st: &mut SewingState) {
    if let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{}.json", st.editor.doc.id))
        .add_filter("OpenStitch sewing stitch", &["json"])
        .save_file()
    {
        st.message = match std::fs::write(&path, st.editor.doc.to_json(false)) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Cannot save: {e}"),
        };
    }
}

fn save_in_project(app: &mut EditorApp, r: &CommandRequest) {
    app.sewing.message = if app.execute(r).is_some() {
        "Saved in the project".into()
    } else {
        app.status.clone()
    };
}

fn export(app: &mut EditorApp) {
    if let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{}.json", app.sewing.editor.doc.id))
        .add_filter("OpenStitch sewing stitch", &["json"])
        .add_filter("SVG drawing", &["svg"])
        .add_filter("Brother PMV (experimental)", &["pmv"])
        .save_file()
    {
        let r = app
            .sewing
            .editor
            .export_request(&path.display().to_string());
        app.sewing.message = match app.execute(&r) {
            Some(v) => format!("Exported {}", v["path"].as_str().unwrap_or("")),
            None => app.status.clone(),
        };
    }
}

fn hint(st: &SewingState) -> &'static str {
    if st.modify_template {
        return "Drag inside the red frame: move the template · drag its corner: scale · click outside: finish";
    }
    match st.tool {
        FactoryTool::SelectPoint => {
            return "Click a point to select it · drag: move · right-click: delete";
        }
        FactoryTool::SelectObject => {
            return "Click the pattern to select it · drag or arrow keys: move (Shift: straight)";
        }
        FactoryTool::Pen => {
            return "Drag to draw along the stroke · right-click: delete a point";
        }
        FactoryTool::Pan => return "Drag to scroll the design area",
        FactoryTool::Line => {}
    }
    match st.editor.doc.kind {
        SewingKind::Decorative => "Click: add needle point · drag: move · right-click: delete",
        SewingKind::Satin => "Click: add station · drag an edge: move · right-click: delete",
        SewingKind::DecorativeSatin => {
            "Click: add outline point · drag: move · right-click: delete"
        }
        SewingKind::Cross => "Click a cell to add or remove it",
    }
}

/// Draws the whole window in place of Layout & Editing.
pub fn factory(app: &mut EditorApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    app.sewing.hover = None;
    suite_chrome::shortcuts(app, &ctx, &FACTORY.actions());
    keys(&mut app.sewing, &ctx);
    let total = app.sewing.editor.doc.needles().len() * REPEATS;
    tick(&mut app.sewing.sim, total, &ctx);
    let mut tab = app.sewing.tab;
    let mut open_menu = std::mem::take(&mut app.sewing.open_app_menu);
    egui::Panel::top("sdf-ribbon").show(ui, |ui| {
        suite_chrome::ribbon(app, ui, &FACTORY, &mut tab, &mut open_menu);
    });
    app.sewing.tab = tab;
    if app.sewing.show_status {
        egui::Panel::bottom("sdf-status").show(ui, |ui| {
            let st = &app.sewing;
            let text = if st.message.is_empty() {
                hint(st)
            } else {
                st.message.as_str()
            };
            let doc = &st.editor.doc;
            suite_chrome::status_bar(ui, st.hover, text, |ui| {
                // Right to left.
                ui.label(format!("{:.0} %", st.zoom * 100.0));
                ui.separator();
                ui.label(format!("{} mm grid", st.editor.grid));
                ui.separator();
                ui.label(format!(
                    "{} needle points · {:.1} mm long · {:.1} mm wide",
                    doc.needles().len(),
                    doc.length(),
                    doc.width
                ));
                if let Some(name) = st.template.as_ref().and_then(|t| t.path.file_name()) {
                    ui.separator();
                    ui.label(format!(
                        "Template {} ({:.0} %)",
                        name.to_string_lossy(),
                        st.template_density * 100.0
                    ));
                }
            });
        });
    }
    let mut requests: Vec<CommandRequest> = Vec::new();
    if app.sewing.show_import {
        let stored = app.session.design().sewing.clone();
        egui::Panel::left("sdf-import")
            .default_size(260.0)
            .show(ui, |ui| {
                ui.strong("Import");
                ui.weak("Patterns in this project");
                ui.separator();
                if stored.is_empty() {
                    ui.weak("None yet. Save puts the pattern here.");
                }
                for q in &stored {
                    ui.label(format!("{} ({}, {})", q.name, q.kind.id(), q.id));
                    ui.horizontal(|ui| {
                        if ui.button("Edit").clicked() {
                            app.sewing.editor = SewingEditor::from_stitch(q);
                            app.sewing.selected = None;
                        }
                        if ui.button("Delete").clicked() {
                            requests
                                .push(CommandRequest::new("sewing.delete", json!({ "id": q.id })));
                        }
                    });
                }
                ui.add_space(8.0);
                ui.weak(
                    "Export writes openstitch.sewing JSON, SVG or PMV (experimental). \
                     Brother's PMP and PSF formats are undocumented and cannot be written.",
                );
            });
    }
    if app.sewing.show_preview {
        egui::Panel::bottom("sdf-preview")
            .default_size(150.0)
            .show(ui, |ui| {
                ui.strong("Preview");
                preview(ui, &app.sewing.editor);
            });
    }
    egui::CentralPanel::default().show(ui, |ui| {
        let st = &mut app.sewing;
        settings(ui, &mut st.editor);
        if let Err(e) = st.editor.doc.validate() {
            ui.colored_label(Color32::from_rgb(220, 90, 70), e);
        }
        ui.separator();
        st.view_size = ui.available_size();
        egui::ScrollArea::both().show(ui, |ui| canvas(ui, st));
    });
    for r in requests {
        save_in_project(app, &r);
    }
    simulator_window(&mut app.sewing, &ctx);
    crate::panes::about_window(app, &ctx);
}

/// Keys of the design area: arrow keys move a selected pattern by one grid step (Select
/// Object), F6 steps the Template Density, Escape ends Modify Template.
fn keys(st: &mut SewingState, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    if st.template.is_some()
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F6))
    {
        step_density(st);
    }
    if st.modify_template && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        st.modify_template = false;
    }
    if !(st.tool == FactoryTool::SelectObject && st.object_selected) {
        return;
    }
    let g = st.editor.grid;
    for (key, d) in [
        (egui::Key::ArrowLeft, v2(-g, 0.0)),
        (egui::Key::ArrowRight, v2(g, 0.0)),
        (egui::Key::ArrowUp, v2(0.0, -g)),
        (egui::Key::ArrowDown, v2(0.0, g)),
    ] {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
            // On a vertical page the arrows follow the screen: up is along the feed.
            let d = if st.horizontal { d } else { v2(-d.y, d.x) };
            st.editor.move_by(d);
        }
    }
}

/// View ▸ Stitch Simulator: the needle points of three repeats sewn one by one, with the
/// editor simulator's controls.
fn simulator_window(st: &mut SewingState, ctx: &egui::Context) {
    if !st.sim.open {
        return;
    }
    let mut open = true;
    let needles = repeated_needles(&st.editor, REPEATS);
    let total = needles.len();
    egui::Window::new("Stitch Simulator")
        .open(&mut open)
        .default_size([620.0, 220.0])
        .show(ctx, |ui| {
            let sim = &mut st.sim;
            crate::panels::sim_controls(ui, &mut sim.playing, &mut sim.pos, &mut sim.speed, total);
            let shown = sim.pos.unwrap_or(total);
            let w = ui.available_width().max(300.0);
            let h = (ui.available_height() - 8.0).max(100.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 4.0, Color32::WHITE);
            let len = st.editor.doc.length().max(1.0) * REPEATS as f64;
            let width = st.editor.doc.width + 2.0;
            let scale = ((rect.width() - 16.0) / len as f32).min(rect.height() / width as f32);
            let origin = Pos2::new(rect.min.x + 8.0, rect.center().y);
            let to =
                |p: Vec2| Pos2::new(origin.x + p.x as f32 * scale, origin.y + p.y as f32 * scale);
            // What is still to sew, faint; what is sewn, in thread colour.
            let all: Vec<Pos2> = needles.iter().map(|&p| to(p)).collect();
            painter.add(egui::Shape::line(
                all.clone(),
                Stroke::new(1.0, Color32::from_gray(225)),
            ));
            let sewn = &all[..shown.min(total)];
            painter.add(egui::Shape::line(
                sewn.to_vec(),
                Stroke::new(1.6, Color32::from_rgb(10, 85, 163)),
            ));
            if st.show_needles {
                for p in sewn {
                    painter.circle_filled(*p, 1.8, Color32::from_gray(40));
                }
            }
            if let Some(&p) = sewn.last()
                && sim.pos.is_some()
            {
                painter.circle_stroke(
                    p,
                    5.0,
                    Stroke::new(2.0, Color32::from_rgb(0xff, 0x3d, 0x00)),
                );
            }
        });
    if !open {
        st.sim.open = false;
        st.sim.playing = false;
    }
}

/// The pattern repeated along a straight seam, as the machine sews it.
fn preview(ui: &mut egui::Ui, e: &SewingEditor) {
    let w = ui.available_width().max(200.0);
    let h = (ui.available_height() - 12.0).max(60.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::WHITE);
    let needles = e.doc.needles();
    let len = e.doc.length();
    if needles.len() < 2 || len <= 0.0 {
        return;
    }
    let scale = (rect.height() / (e.doc.width as f32 + 2.0)).min(12.0);
    let origin = Pos2::new(rect.min.x + 8.0, rect.center().y);
    let repeats = ((f64::from(rect.width() - 16.0) / f64::from(scale)) / len).ceil() as usize;
    let mut pts = Vec::new();
    for r in 0..repeats.min(200) {
        let shift = len * r as f64;
        pts.extend(needles.iter().map(|p| {
            Pos2::new(
                origin.x + (p.x + shift) as f32 * scale,
                origin.y + p.y as f32 * scale,
            )
        }));
    }
    pts.retain(|p| p.x <= rect.max.x - 8.0);
    painter.add(egui::Shape::line(
        pts,
        Stroke::new(1.4, Color32::from_rgb(10, 85, 163)),
    ));
}

fn settings(ui: &mut egui::Ui, e: &mut SewingEditor) {
    ui.horizontal(|ui| {
        ui.label("Kind");
        egui::ComboBox::from_id_salt("sdf-kind")
            .selected_text(e.doc.kind.label())
            .show_ui(ui, |ui| {
                for k in SewingKind::ALL {
                    if ui.selectable_label(e.doc.kind == k, k.label()).clicked() && e.doc.kind != k
                    {
                        *e = SewingEditor::new(k);
                    }
                }
            });
        ui.label("Id");
        ui.add(egui::TextEdit::singleline(&mut e.doc.id).desired_width(120.0));
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut e.doc.name).desired_width(140.0));
        ui.label("Width");
        ui.add(
            egui::DragValue::new(&mut e.doc.width)
                .range(1.0..=MAX_WIDTH)
                .speed(0.05)
                .suffix(" mm"),
        );
        match e.doc.kind {
            SewingKind::Satin | SewingKind::DecorativeSatin => {
                ui.label("Density");
                ui.add(
                    egui::DragValue::new(&mut e.doc.density)
                        .range(1.0..=10.0)
                        .speed(0.05)
                        .suffix(" /mm"),
                );
            }
            SewingKind::Cross => {
                ui.label("Cell");
                ui.add(
                    egui::DragValue::new(&mut e.doc.cell_size)
                        .range(0.5..=5.0)
                        .speed(0.05)
                        .suffix(" mm"),
                );
                ui.label("Rows");
                ui.add(egui::DragValue::new(&mut e.doc.rows).range(1..=18));
                ui.checkbox(&mut e.doc.triple, "Triple");
            }
            SewingKind::Decorative => {}
        }
    });
}

/// Pattern mm ↔ design-area points: landscape (feed to the right) or portrait (feed upwards,
/// +y to the right).
#[derive(Clone, Copy)]
struct View {
    origin: Pos2,
    px: f32,
    horizontal: bool,
}

impl View {
    fn to(self, p: Vec2) -> Pos2 {
        let (x, y) = (p.x as f32 * self.px, p.y as f32 * self.px);
        if self.horizontal {
            Pos2::new(self.origin.x + x, self.origin.y + y)
        } else {
            Pos2::new(self.origin.x + y, self.origin.y - x)
        }
    }

    fn from(self, p: Pos2) -> Vec2 {
        let (dx, dy) = (
            (p.x - self.origin.x) / self.px,
            (p.y - self.origin.y) / self.px,
        );
        if self.horizontal {
            v2(f64::from(dx), f64::from(dy))
        } else {
            v2(f64::from(-dy), f64::from(dx))
        }
    }

    /// A screen vector as a pattern-mm vector.
    fn delta(self, d: egui::Vec2) -> Vec2 {
        let (dx, dy) = (f64::from(d.x / self.px), f64::from(d.y / self.px));
        if self.horizontal {
            v2(dx, dy)
        } else {
            v2(-dy, dx)
        }
    }
}

fn canvas(ui: &mut egui::Ui, st: &mut SewingState) {
    let px = PX * st.zoom;
    let doc = &st.editor.doc;
    let len = doc.length().max(8.0);
    let repeats = REPEATS as f64;
    let along = ((len * repeats + 4.0) as f32 * px).max(600.0);
    let across = (doc.width as f32 + 2.0) * px;
    let size = if st.horizontal {
        egui::vec2(along, across)
    } else {
        egui::vec2(across.max(300.0), along)
    };
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_gray(250));
    let view = View {
        origin: if st.horizontal {
            Pos2::new(rect.min.x + px, rect.center().y)
        } else {
            Pos2::new(rect.center().x, rect.max.y - px)
        },
        px,
        horizontal: st.horizontal,
    };
    let to = |p: Vec2| view.to(p);
    if std::mem::take(&mut st.reveal)
        && let Some((lo, hi)) = st.editor.bounds()
    {
        ui.scroll_to_rect(
            egui::Rect::from_two_pos(to(lo), to(hi)).expand(px),
            Some(egui::Align::Center),
        );
    }
    if let Some(t) = &st.template
        && st.template_density > 0.0
    {
        draw_template(&painter, t, view, st.template_density);
    }
    // Grid at the grid spacing (coarser when the lines would crowd), the width band, and
    // where the first repeat ends.
    let grid = Stroke::new(1.0, Color32::from_gray(232));
    let half = doc.width / 2.0;
    let mut g = st.editor.grid;
    while (g as f32) * px < 10.0 {
        g *= 2.0;
    }
    if st.show_grid {
        let mut x = 0.0;
        while x <= len * repeats + 1e-9 {
            painter.line_segment([to(v2(x, -half)), to(v2(x, half))], grid);
            x += g;
        }
        let mut y = -(half / g).floor() * g;
        while y <= half + 1e-9 {
            painter.line_segment([to(v2(0.0, y)), to(v2(len * repeats, y))], grid);
            y += g;
        }
    }
    let band = Stroke::new(1.0, Color32::from_rgb(120, 140, 200));
    for y in [-half, half] {
        painter.line_segment([to(v2(0.0, y)), to(v2(len * repeats, y))], band);
    }
    painter.line_segment(
        [to(v2(len, -half - 0.5)), to(v2(len, half + 0.5))],
        Stroke::new(1.0, Color32::from_rgb(200, 120, 120)),
    );
    // Cross grid.
    if doc.kind == SewingKind::Cross {
        let c = doc.cell_size;
        let top = -f64::from(doc.rows) * c / 2.0;
        for cell in &doc.cells {
            let a = to(v2(f64::from(cell[0]) * c, top + f64::from(cell[1]) * c));
            let b = to(v2(
                f64::from(cell[0] + 1) * c,
                top + f64::from(cell[1] + 1) * c,
            ));
            painter.rect_filled(
                egui::Rect::from_two_pos(a, b),
                0.0,
                Color32::from_rgb(225, 235, 250),
            );
        }
    }
    // Stitches: the first repeat dark, the next ones faint.
    let needles = doc.needles();
    let pattern_len = doc.length();
    for r in 0..REPEATS {
        let colour = if r == 0 {
            Color32::from_rgb(10, 85, 163)
        } else {
            Color32::from_rgb(150, 175, 215)
        };
        let shift = pattern_len * r as f64;
        let pts: Vec<Pos2> = needles.iter().map(|p| to(v2(p.x + shift, p.y))).collect();
        painter.add(egui::Shape::line(pts.clone(), Stroke::new(1.6, colour)));
        if st.show_needles {
            for p in pts {
                painter.circle_filled(p, 2.2, Color32::from_gray(40));
            }
        }
    }
    // Start (blue) and end (red) points of the first repeat.
    for (p, colour) in [
        (needles.first(), Color32::from_rgb(30, 90, 230)),
        (needles.last(), Color32::from_rgb(220, 40, 40)),
    ] {
        if let Some(&p) = p {
            painter.rect_stroke(
                egui::Rect::from_center_size(to(p), egui::vec2(9.0, 9.0)),
                0.0,
                Stroke::new(2.0, colour),
                egui::StrokeKind::Middle,
            );
        }
    }
    // Editable handles.
    let handle = Color32::from_rgb(230, 120, 30);
    match doc.kind {
        SewingKind::Satin => {
            for &[x, l, r] in &doc.profile {
                painter.circle_stroke(to(v2(x, l)), 5.0, Stroke::new(2.0, handle));
                painter.circle_stroke(to(v2(x, r)), 5.0, Stroke::new(2.0, handle));
            }
        }
        SewingKind::DecorativeSatin => {
            let mut pts: Vec<Pos2> = doc.outline.iter().map(|p| to(*p)).collect();
            if let Some(&f) = pts.first() {
                pts.push(f);
            }
            painter.add(egui::Shape::line(pts.clone(), Stroke::new(1.0, handle)));
            for p in pts {
                painter.circle_stroke(p, 5.0, Stroke::new(2.0, handle));
            }
        }
        SewingKind::Decorative => {
            for p in &doc.points {
                painter.circle_stroke(to(*p), 5.0, Stroke::new(2.0, handle));
            }
        }
        SewingKind::Cross => {}
    }
    if let Some(h) = st.selected
        && let Some(p) = handle_pos(&st.editor, h)
    {
        painter.circle_filled(to(p), 5.0, handle);
    }
    if st.object_selected
        && st.tool == FactoryTool::SelectObject
        && let Some((lo, hi)) = st.editor.bounds()
    {
        painter.rect_stroke(
            egui::Rect::from_two_pos(to(lo), to(hi)).expand(4.0),
            0.0,
            Stroke::new(1.0, Color32::from_gray(60)),
            egui::StrokeKind::Middle,
        );
    }
    if st.stroke.len() > 1 {
        let pts: Vec<Pos2> = st.stroke.iter().map(|p| to(*p)).collect();
        painter.add(egui::Shape::line(pts, Stroke::new(1.5, handle)));
    }
    if st.modify_template
        && let Some(t) = &st.template
    {
        let (frame, corner) = template_frame(t, view);
        let red = Color32::from_rgb(220, 30, 30);
        painter.rect_stroke(frame, 0.0, Stroke::new(2.0, red), egui::StrokeKind::Middle);
        painter.rect_filled(
            egui::Rect::from_center_size(corner, egui::vec2(8.0, 8.0)),
            0.0,
            red,
        );
    }
    if st.modify_template {
        modify_template(st, &resp, view);
    } else {
        edit(ui, st, &resp, view);
    }
}

/// The template drawn as a textured quad over its mm corners (it turns with the page).
fn draw_template(painter: &egui::Painter, t: &Template, view: View, density: f32) {
    let (o, s) = (t.origin, t.size);
    let corners = [
        o,
        v2(o.x + s.x, o.y),
        v2(o.x + s.x, o.y + s.y),
        v2(o.x, o.y + s.y),
    ];
    let uvs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    let tint = Color32::from_white_alpha((density.clamp(0.0, 1.0) * 255.0) as u8);
    let mut mesh = egui::Mesh::with_texture(t.texture.id());
    for (c, (u, v)) in corners.into_iter().zip(uvs) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: view.to(c),
            uv: Pos2::new(u, v),
            color: tint,
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// The template's red frame and its scale corner (the far corner from its origin).
fn template_frame(t: &Template, view: View) -> (egui::Rect, Pos2) {
    let far = v2(t.origin.x + t.size.x, t.origin.y + t.size.y);
    (
        egui::Rect::from_two_pos(view.to(t.origin), view.to(far)),
        view.to(far),
    )
}

/// Modify Template: drag inside the frame moves it, its corner scales it (keeping the
/// image's proportions); a click outside ends.
fn modify_template(st: &mut SewingState, resp: &egui::Response, view: View) {
    let Some(t) = &mut st.template else {
        st.modify_template = false;
        return;
    };
    let (frame, corner) = template_frame(t, view);
    if let Some(pos) = resp.interact_pointer_pos() {
        if resp.drag_started() {
            st.template_drag = if pos.distance(corner) <= 10.0 {
                Some(TemplateDrag::Scale)
            } else if frame.contains(pos) {
                Some(TemplateDrag::Move)
            } else {
                None
            };
        }
        if resp.dragged() {
            let d = view.delta(resp.drag_delta());
            match st.template_drag {
                Some(TemplateDrag::Move) => t.origin = t.origin + d,
                Some(TemplateDrag::Scale) => {
                    let ratio = t.size.y / t.size.x;
                    let w = (t.size.x + d.x).clamp(1.0, 400.0);
                    t.size = v2(w, w * ratio);
                }
                None => {}
            }
        }
        if resp.clicked() && !frame.contains(pos) {
            st.modify_template = false;
        }
    }
    if resp.drag_stopped() {
        st.template_drag = None;
    }
}

/// Clicks and drags of the current tool.
fn edit(ui: &egui::Ui, st: &mut SewingState, resp: &egui::Response, view: View) {
    if st.tool == FactoryTool::Pan {
        if resp.dragged() {
            ui.scroll_with_delta(resp.drag_delta());
        }
        return;
    }
    let tolerance = f64::from(8.0 / view.px);
    let Some(pos) = resp.interact_pointer_pos() else {
        if resp.drag_stopped() {
            st.drag = None;
        }
        return;
    };
    let p = view.from(pos);
    if st.tool == FactoryTool::SelectObject {
        let inside = st.editor.bounds().is_some_and(|(lo, hi)| {
            p.x >= lo.x - tolerance
                && p.x <= hi.x + tolerance
                && p.y >= lo.y - tolerance
                && p.y <= hi.y + tolerance
        });
        if resp.clicked() {
            st.object_selected = inside;
        }
        if resp.drag_started() {
            st.object_selected = inside;
            st.move_rest = v2(0.0, 0.0);
        }
        if resp.dragged() && st.object_selected {
            let mut rest = st.move_rest + view.delta(resp.drag_delta());
            if ui.input(|i| i.modifiers.shift) {
                rest = if rest.x.abs() >= rest.y.abs() {
                    v2(rest.x, 0.0)
                } else {
                    v2(0.0, rest.y)
                };
            }
            let g = if st.editor.snap { st.editor.grid } else { 0.0 };
            let step = |v: f64| if g > 0.0 { (v / g).round() * g } else { v };
            let snapped = v2(step(rest.x), step(rest.y));
            if snapped.x != 0.0 || snapped.y != 0.0 {
                st.move_rest = if st.editor.move_by(snapped) {
                    rest - snapped
                } else {
                    v2(0.0, 0.0)
                };
            } else {
                st.move_rest = rest;
            }
        }
        return;
    }
    if st.tool == FactoryTool::Pen {
        if resp.drag_started() {
            st.stroke = vec![p];
        } else if resp.dragged() {
            st.stroke.push(p);
        }
        if resp.drag_stopped() {
            let stroke = std::mem::take(&mut st.stroke);
            st.editor.pen(&stroke);
            st.selected = None;
            return;
        }
    } else {
        if resp.drag_started() {
            st.drag = st.editor.hit(p, tolerance);
            if st.drag.is_some() {
                st.selected = st.drag;
            }
        }
        if resp.dragged()
            && let Some(h) = st.drag
        {
            st.editor.drag(h, p);
        }
    }
    if resp.clicked() {
        match st.editor.hit(p, tolerance) {
            Some(h) => st.selected = Some(h),
            None if matches!(st.tool, FactoryTool::Line | FactoryTool::Pen) => st.editor.add(p),
            None => st.selected = None,
        }
    }
    if resp.secondary_clicked()
        && let Some(h) = st.editor.hit(p, tolerance)
    {
        st.editor.delete(h);
        st.selected = None;
    }
    if resp.drag_stopped() {
        st.drag = None;
    }
}

/// Where a handle is drawn (mm).
fn handle_pos(e: &SewingEditor, h: Handle) -> Option<Vec2> {
    let d = &e.doc;
    match h {
        Handle::Point(i) => d.points.get(i).copied(),
        Handle::Vertex(i) => d.outline.get(i).copied(),
        Handle::Edge(i, right) => d
            .profile
            .get(i)
            .map(|s| v2(s[0], if right { s[2] } else { s[1] })),
    }
}
