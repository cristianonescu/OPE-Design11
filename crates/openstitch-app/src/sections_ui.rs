//! Split designs, multi-position hoops and the jumbo frame in the editor: the hoopings drawn
//! on the canvas, their settings in the Design Settings window (`sections.set`) and File ▸
//! Export Sections (`sections.export`).

use crate::app::EditorApp;
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};
use openstitch_app::editor::{CommandRequest, Viewport};
use openstitch_design::sections::{self, SectionKind, sectionable};
use openstitch_design::{BROTHER_HOOPS, Vec2};
use serde_json::json;

const SECTION: Color32 = Color32::from_rgb(0xe5, 0x39, 0x35);
const ALIGN: Color32 = Color32::from_rgb(0x9e, 0x9e, 0x9e);

fn pos(p: Vec2) -> Pos2 {
    Pos2::new(p.x as f32, p.y as f32)
}

/// Section boundaries (solid red, labelled), multi-position areas (dotted) and jumbo sides,
/// with the alignment stitching lines; nothing for a design sewn in one hooping.
pub fn draw_sections(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    let plan = sections::plan(app.session.design());
    let multi = plan.sections.len() > 1
        || plan
            .sections
            .first()
            .is_some_and(|s| s.kind != SectionKind::Split);
    if !multi {
        return;
    }
    for (i, s) in plan.sections.iter().enumerate() {
        let r = egui::Rect::from_two_pos(
            pos(vp.to_screen(openstitch_design::v2(s.rect.min_x, s.rect.min_y))),
            pos(vp.to_screen(openstitch_design::v2(s.rect.max_x, s.rect.max_y))),
        );
        let stroke = Stroke::new(1.5, SECTION.gamma_multiply(0.85));
        match s.kind {
            SectionKind::Split => {
                painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Middle);
            }
            SectionKind::Position | SectionKind::JumboSide => {
                let c = [
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                    r.left_top(),
                ];
                painter.extend(Shape::dashed_line(&c, stroke, 6.0, 4.0));
            }
        }
        for &(a, b) in s.marks_before.iter().chain(&s.marks_after) {
            painter.add(Shape::dashed_line(
                &[pos(vp.to_screen(a)), pos(vp.to_screen(b))],
                Stroke::new(1.0, ALIGN),
                3.0,
                3.0,
            ));
        }
        // Labels sit inside the top-left corner; positions that share an area offset theirs.
        let at = r.left_top()
            + egui::vec2(
                4.0,
                4.0 + 14.0 * (i % 3) as f32 * f32::from(u8::from(s.kind == SectionKind::Position)),
            );
        painter.text(
            at,
            egui::Align2::LEFT_TOP,
            &s.label,
            egui::FontId::proportional(12.0),
            SECTION,
        );
    }
    for e in &plan.errors {
        painter.text(
            painter.clip_rect().left_bottom() + egui::vec2(28.0, -24.0),
            egui::Align2::LEFT_BOTTOM,
            e,
            egui::FontId::proportional(12.0),
            SECTION,
        );
    }
}

/// The large-design part of the Design Settings window.
pub fn settings(app: &EditorApp, ui: &mut egui::Ui, requests: &mut Vec<CommandRequest>) {
    let d = app.session.design();
    let m = &d.machine;
    let set = |v: serde_json::Value| CommandRequest::new("sections.set", v);
    let hoop = m
        .hoop
        .as_deref()
        .and_then(openstitch_design::machine::brother_hoop);
    ui.separator();
    match hoop {
        Some(h) if h.multi_position => {
            ui.label(egui::RichText::new("Multi-position hoop").strong());
            let mut optimize = m.optimize_hoop_change;
            if ui
                .checkbox(&mut optimize, "Optimize hoop changes")
                .on_hover_text("Group patterns by installation position (a, b, c)")
                .changed()
            {
                requests.push(set(json!({ "optimizeHoopChange": optimize })));
            }
        }
        Some(h) if h.id == sections::JUMBO => {
            ui.label(egui::RichText::new("Jumbo frame").strong());
            ui.horizontal(|ui| {
                ui.label("Side overlap");
                let mut o = m.jumbo_overlap;
                let r = ui.add(egui::DragValue::new(&mut o).range(0.0..=50.0).suffix(" mm"));
                if r.drag_stopped() || r.lost_focus() {
                    requests.push(set(json!({ "jumboOverlap": o })));
                }
            });
        }
        Some(_) => {
            ui.label(
                egui::RichText::new("Split designs need Hoop: Custom size.")
                    .weak()
                    .small(),
            );
        }
        None => split_settings(app, ui, requests),
    }
    let n = sections::plan(d).sections.len();
    if n > 1 {
        ui.label(format!(
            "{n} hoopings · File ▸ Export Sections writes one file each"
        ));
    }
}

fn split_settings(app: &EditorApp, ui: &mut egui::Ui, requests: &mut Vec<CommandRequest>) {
    let d = app.session.design();
    let s = d.machine.sections.clone();
    let set = |v: serde_json::Value| CommandRequest::new("sections.set", v);
    ui.label(egui::RichText::new("Split design").strong());
    ui.horizontal(|ui| {
        ui.label("Section size");
        let current = s.as_ref().map(|s| s.hoop.clone()).unwrap_or_default();
        let text = s
            .as_ref()
            .and_then(|s| {
                s.section_size()
                    .map(|(w, h)| format!("{w} × {h} mm ({})", s.hoop))
            })
            .unwrap_or_else(|| "Not split".into());
        egui::ComboBox::from_id_salt("section-hoop")
            .selected_text(text)
            .width(200.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(s.is_none(), "Not split").clicked() && s.is_some() {
                    requests.push(set(json!({ "hoop": "none" })));
                }
                for h in BROTHER_HOOPS.iter().filter(|h| sectionable(h)) {
                    let label = format!(
                        "{} × {} mm ({} × {} mm hoop)",
                        h.width - sections::SECTION_MARGIN_MM,
                        h.height - sections::SECTION_MARGIN_MM,
                        h.width,
                        h.height
                    );
                    if ui.selectable_label(current == h.id, label).clicked() && current != h.id {
                        requests.push(set(json!({ "hoop": h.id })));
                    }
                }
            });
    });
    let Some(s) = s else { return };
    let mut smaller = s.smaller;
    if ui
        .checkbox(&mut smaller, "Use smaller section sizes")
        .changed()
    {
        requests.push(set(json!({ "smaller": smaller })));
    }
    // Split positions: the current ones (even split when none are set), editable in mm.
    let plan = sections::plan(d);
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for sec in &plan.sections {
        if sec.rect.min_x > 0.0 && !xs.contains(&(sec.rect.min_x / 10.0)) {
            xs.push(sec.rect.min_x / 10.0);
        }
        if sec.rect.min_y > 0.0 && !ys.contains(&(sec.rect.min_y / 10.0)) {
            ys.push(sec.rect.min_y / 10.0);
        }
    }
    for (key, axis, values) in [
        ("splitsX", "Vertical splits", xs),
        ("splitsY", "Horizontal splits", ys),
    ] {
        if values.is_empty() {
            continue;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(axis);
            let mut edited = values.clone();
            let mut commit = false;
            for v in &mut edited {
                let r = ui.add(egui::DragValue::new(v).speed(0.5).suffix(" mm"));
                commit |= r.drag_stopped() || r.lost_focus();
            }
            if commit && edited != values {
                requests.push(set(json!({ key: edited })));
            }
        });
    }
    if (!s.splits_x.is_empty() || !s.splits_y.is_empty()) && ui.small_button("Even split").clicked()
    {
        requests.push(set(json!({ "splitsX": [], "splitsY": [] })));
    }
}

/// File ▸ Export Sections: one machine file per hooping, named from the chosen base name.
pub fn export_sections(app: &mut EditorApp) {
    let name = format!("{}.pes", app.session.design().name);
    let Some(path) = rfd::FileDialog::new()
        .add_filter("PES", &["pes"])
        .add_filter("DST", &["dst"])
        .add_filter("JEF", &["jef"])
        .add_filter("EXP", &["exp"])
        .set_file_name(name)
        .save_file()
    else {
        return;
    };
    let params = json!({ "path": path.display().to_string() });
    if let Some(r) = app.execute(&CommandRequest::new("sections.export", params)) {
        let n = r["files"].as_array().map_or(0, Vec::len);
        app.status = format!("Exported {n} section files next to {}", path.display());
    }
}

/// Whether the design is sewn in more than one hooping.
pub fn has_sections(app: &EditorApp) -> bool {
    sections::plan(app.session.design()).sections.len() > 1
}
