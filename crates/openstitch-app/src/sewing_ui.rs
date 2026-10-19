//! Stitch Design Factory window (Tools → Stitch Design Factory…): edits sewing-machine
//! stitch patterns ([`openstitch_app::editor::sewing`]), keeps them in the project
//! (`sewing.define`, one Undo) and exports them (`sewing.export`: openstitch.sewing JSON, SVG,
//! experimental PMV).

use crate::app::EditorApp;
use eframe::egui::{self, Color32, Pos2, Sense, Stroke};
use openstitch_app::editor::CommandRequest;
use openstitch_app::editor::sewing::{Handle, SewingEditor};
use openstitch_design::sewing::{MAX_WIDTH, SewingKind};
use openstitch_design::{Vec2, v2};
use serde_json::json;

/// Window state.
#[derive(Default)]
pub struct SewingState {
    pub open: bool,
    pub editor: SewingEditor,
    drag: Option<Handle>,
    pub message: String,
}

/// Pixels per mm in the editing canvas.
const PX: f32 = 28.0;

pub fn sewing_window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.sewing.open {
        return;
    }
    let mut open = true;
    let mut requests: Vec<CommandRequest> = Vec::new();
    let mut export = false;
    let project: Vec<(String, String)> = app
        .session
        .design()
        .sewing
        .iter()
        .map(|q| {
            (
                q.id.clone(),
                format!("{} ({}, {})", q.name, q.kind.id(), q.id),
            )
        })
        .collect();
    let stored = app.session.design().sewing.clone();
    egui::Window::new("Stitch Design Factory")
        .open(&mut open)
        .default_width(900.0)
        .resizable(true)
        .fade_in(false)
        .show(ctx, |ui| {
            let st = &mut app.sewing;
            settings(ui, &mut st.editor);
            ui.separator();
            ui.label(match st.editor.doc.kind {
                SewingKind::Decorative => {
                    "Click: add needle point · drag: move · right-click: delete"
                }
                SewingKind::Satin => {
                    "Click: add station · drag an edge: move · right-click: delete"
                }
                SewingKind::DecorativeSatin => {
                    "Click: add outline point · drag: move · right-click: delete"
                }
                SewingKind::Cross => "Click a cell to add or remove it",
            });
            egui::ScrollArea::horizontal().show(ui, |ui| canvas(ui, st));
            ui.horizontal(|ui| {
                let n = st.editor.doc.needles().len();
                ui.label(format!(
                    "{n} needle points · {:.1} mm long · {:.1} mm wide",
                    st.editor.doc.length(),
                    st.editor.doc.width
                ));
                ui.checkbox(&mut st.editor.snap, "Snap to 0.25 mm");
            });
            ui.separator();
            ui.horizontal(|ui| {
                match st.editor.define_request() {
                    Ok(r) => {
                        if ui.button("Add to project").clicked() {
                            requests.push(r);
                        }
                    }
                    Err(e) => {
                        ui.add_enabled(false, egui::Button::new("Add to project"));
                        ui.colored_label(Color32::from_rgb(220, 90, 70), e);
                    }
                }
                let in_project = stored.iter().any(|q| q.id == st.editor.doc.id);
                if ui
                    .add_enabled(in_project, egui::Button::new("Export…"))
                    .on_disabled_hover_text("Add the pattern to the project first")
                    .clicked()
                {
                    export = true;
                }
                ui.weak(&st.message);
            });
            ui.separator();
            ui.strong("In this project");
            if project.is_empty() {
                ui.weak("None yet.");
            }
            for (id, label) in &project {
                ui.horizontal(|ui| {
                    ui.label(label);
                    if ui.button("Edit").clicked()
                        && let Some(q) = stored.iter().find(|q| &q.id == id)
                    {
                        st.editor = SewingEditor::from_stitch(q);
                    }
                    if ui.button("Delete").clicked() {
                        requests.push(CommandRequest::new("sewing.delete", json!({ "id": id })));
                    }
                });
            }
            ui.weak(
                "Export writes openstitch.sewing JSON, SVG or PMV (experimental). Brother's PMP \
                 and PSF formats are undocumented and cannot be written.",
            );
        });
    app.sewing.open = open;
    for r in requests {
        let ok = app.execute(&r).is_some();
        app.sewing.message = if ok {
            "Saved in the project".into()
        } else {
            app.status.clone()
        };
    }
    if export
        && let Some(path) = rfd::FileDialog::new()
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
    let doc = &st.editor.doc;
    let len = doc.length().max(8.0);
    let repeats = 3.0;
    let w = ((len * repeats + 4.0) as f32 * PX).max(600.0);
    let h = (doc.width as f32 + 2.0) * PX;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_gray(250));
    let origin = Pos2::new(rect.min.x + PX, rect.center().y);
    let to = |p: Vec2| Pos2::new(origin.x + p.x as f32 * PX, origin.y + p.y as f32 * PX);
    let from = |p: Pos2| {
        v2(
            f64::from((p.x - origin.x) / PX),
            f64::from((p.y - origin.y) / PX),
        )
    };
    // Grid every millimetre, the width band, and where the first repeat ends.
    let grid = Stroke::new(1.0, Color32::from_gray(232));
    let half = doc.width / 2.0;
    let mut x = 0.0;
    while x <= len * repeats + 1.0 {
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
        for p in pts {
            painter.circle_filled(p, 2.2, Color32::from_gray(40));
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
    let tolerance = f64::from(8.0 / PX);
    if let Some(pos) = resp.interact_pointer_pos() {
        let p = from(pos);
        if resp.drag_started() {
            st.drag = st.editor.hit(p, tolerance);
        }
        if resp.dragged()
            && let Some(h) = st.drag
        {
            st.editor.drag(h, p);
        }
        if resp.clicked() && st.editor.hit(p, tolerance).is_none() {
            st.editor.add(p);
        }
        if resp.secondary_clicked()
            && let Some(h) = st.editor.hit(p, tolerance)
        {
            st.editor.delete(h);
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
    }
}
