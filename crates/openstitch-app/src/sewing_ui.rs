//! Stitch Design Factory (Options ▸ Stitch Design Factory): a window of its own in place of
//! Layout & Editing, with the suite's Application menu, ribbon (Home, View), ScreenTips, Import
//! and Preview panes and status bar ([`openstitch_app::editor::suite::FACTORY`]). It edits
//! sewing-machine stitch patterns ([`openstitch_app::editor::sewing`]), keeps them in the
//! project (`sewing.define`, one Undo) and exports them (`sewing.export`: openstitch.sewing JSON,
//! SVG, experimental PMV).

use crate::app::EditorApp;
use crate::suite_chrome::{self, Host};
use eframe::egui::{self, Color32, Pos2, Sense, Stroke};
use openstitch_app::editor::CommandRequest;
use openstitch_app::editor::sewing::{Handle, SewingEditor};
use openstitch_app::editor::suite::{FACTORY, FactoryAction};
use openstitch_design::sewing::{self, MAX_WIDTH, SewingKind};
use openstitch_design::{Vec2, v2};
use serde_json::json;

/// What a click in the design area does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FactoryTool {
    /// Selects, moves and deletes points.
    SelectPoint,
    /// Adds points (and moves them).
    #[default]
    Line,
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
            Delete => st.selected.is_some(),
            ZoomIn => st.zoom < MAX_ZOOM,
            ZoomOut => st.zoom > MIN_ZOOM,
            _ => true,
        }
    }

    fn active(&self, a: FactoryAction) -> bool {
        use FactoryAction::*;
        let st = &self.sewing;
        match a {
            SelectPoint => st.tool == FactoryTool::SelectPoint,
            LineTool => st.tool == FactoryTool::Line,
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

    fn run(&mut self, a: FactoryAction, _ctx: &egui::Context) {
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
            SelectPoint => st.tool = FactoryTool::SelectPoint,
            LineTool => st.tool = FactoryTool::Line,
            ZoomIn => st.zoom = (st.zoom * 1.25).min(MAX_ZOOM),
            ZoomOut => st.zoom = (st.zoom / 1.25).max(MIN_ZOOM),
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
                }
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
            Options => suite_chrome::theme_menu(self, ui),
            Help => crate::ribbon::help_menu(self, ui),
            _ => {}
        }
    }

    fn set_hover(&mut self, tip: &'static str) {
        self.sewing.hover = Some(tip);
    }
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
    if st.tool == FactoryTool::SelectPoint {
        return "Click a point to select it · drag: move · right-click: delete";
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
                ui.label(format!("{:.0} %", st.zoom * 100.0));
                ui.separator();
                ui.label(format!(
                    "{} needle points · {:.1} mm long · {:.1} mm wide",
                    doc.needles().len(),
                    doc.length(),
                    doc.width
                ));
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
        egui::ScrollArea::both().show(ui, |ui| canvas(ui, st));
    });
    for r in requests {
        save_in_project(app, &r);
    }
    crate::panes::about_window(app, &ctx);
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

fn canvas(ui: &mut egui::Ui, st: &mut SewingState) {
    let px = PX * st.zoom;
    let doc = &st.editor.doc;
    let len = doc.length().max(8.0);
    let repeats = 3.0;
    let w = ((len * repeats + 4.0) as f32 * px).max(600.0);
    let h = (doc.width as f32 + 2.0) * px;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_gray(250));
    let origin = Pos2::new(rect.min.x + px, rect.center().y);
    let to = |p: Vec2| Pos2::new(origin.x + p.x as f32 * px, origin.y + p.y as f32 * px);
    let from = |p: Pos2| {
        v2(
            f64::from((p.x - origin.x) / px),
            f64::from((p.y - origin.y) / px),
        )
    };
    // Grid every millimetre, the width band, and where the first repeat ends.
    let grid = Stroke::new(1.0, Color32::from_gray(232));
    let half = doc.width / 2.0;
    let mut x = 0.0;
    while st.show_grid && x <= len * repeats + 1.0 {
        painter.line_segment([to(v2(x, -half - 0.5)), to(v2(x, half + 0.5))], grid);
        x += 1.0;
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
    if doc.kind == openstitch_design::sewing::SewingKind::Cross {
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
    for r in 0..repeats as usize {
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
    let tolerance = f64::from(8.0 / px);
    if let Some(pos) = resp.interact_pointer_pos() {
        let p = from(pos);
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
        if resp.clicked() {
            match st.editor.hit(p, tolerance) {
                Some(h) => st.selected = Some(h),
                None if st.tool == FactoryTool::Line => st.editor.add(p),
                None => st.selected = None,
            }
        }
        if resp.secondary_clicked()
            && let Some(h) = st.editor.hit(p, tolerance)
        {
            st.editor.delete(h);
            st.selected = None;
        }
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
