//! Workspace: Design Settings window (machine type, hoops, user hoops, sewing area, jump-stitch
//! trimming), rulers, grid, measure tool, Reference window and realistic thread drawing. Only
//! the Design Settings window changes the design, through `machine.*` commands.

use crate::app::EditorApp;
use eframe::egui::{self, Color32, Pos2, Sense, Shape, Stroke};
use openstitch_app::editor::workspace::{GridSettings, MeasureTool, ReferenceMap, ruler_ticks};
use openstitch_app::editor::{CommandRequest, ScreenRect, Viewport};
use openstitch_design::{BROTHER_HOOPS, Estimate, MachineType, Rect, SewingArea, Vec2, v2};
use serde_json::json;

pub const RULER: f32 = 20.0;

/// View state for the workspace aids.
pub struct Workspace {
    pub grid: GridSettings,
    pub rulers: bool,
    pub reference: bool,
    pub realistic: bool,
    /// Solid View (never on together with `realistic`).
    pub solid: bool,
    /// View ▸ Guideline: guidelines shown and added from the rulers.
    pub guides: bool,
    /// View, Image and Text dialogs and the guideline being dragged.
    pub view: crate::view_ui::ViewState,
    pub measure: MeasureTool,
    /// Hoop Settings ▸ Move Sections.
    pub sections: openstitch_app::editor::sections::SectionMover,
    pub settings_open: bool,
    user_hoop: (f64, f64, String),
    /// Thread length and sewing time of the compiled design.
    pub estimate: Option<Estimate>,
    /// Where the canvas was drawn last frame (for the Reference window).
    pub canvas: Option<egui::Rect>,
    /// File ▸ Send to USB Media.
    pub usb: crate::transfer_ui::UsbDialog,
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace {
            grid: GridSettings::default(),
            rulers: true,
            reference: false,
            realistic: false,
            solid: false,
            guides: false,
            view: Default::default(),
            measure: MeasureTool::default(),
            sections: Default::default(),
            settings_open: false,
            user_hoop: (100.0, 100.0, String::new()),
            estimate: None,
            canvas: None,
            usb: Default::default(),
        }
    }
}

fn pos(p: Vec2) -> Pos2 {
    Pos2::new(p.x as f32, p.y as f32)
}

pub fn screen_rect(r: egui::Rect) -> ScreenRect {
    ScreenRect {
        x: f64::from(r.min.x),
        y: f64::from(r.min.y),
        width: f64::from(r.width()),
        height: f64::from(r.height()),
    }
}

// MARK: canvas overlays

/// Rulers along the top and left edge of the canvas, in mm from the Design Page origin, with
/// the pointer position marked.
pub fn draw_rulers(painter: &egui::Painter, rect: egui::Rect, vp: &Viewport, hover: Option<Pos2>) {
    let colors = crate::theme::canvas(painter.ctx());
    let (bg, fg) = (colors.ruler_bg, colors.ruler_fg);
    let top = egui::Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + RULER));
    let left = egui::Rect::from_min_max(rect.min, Pos2::new(rect.min.x + RULER, rect.max.y));
    painter.rect_filled(top, 0.0, bg);
    painter.rect_filled(left, 0.0, bg);
    let font = egui::FontId::monospace(9.0);
    let a = vp.to_model(v2(f64::from(rect.min.x), f64::from(rect.min.y)));
    let b = vp.to_model(v2(f64::from(rect.max.x), f64::from(rect.max.y)));
    for t in ruler_ticks(a.x, b.x, vp.scale, 6.0) {
        let x = vp.to_screen(v2(t.at, 0.0)).x as f32;
        if x < left.max.x {
            continue;
        }
        let len = if t.major { RULER * 0.6 } else { RULER * 0.25 };
        painter.line_segment(
            [Pos2::new(x, top.max.y - len), Pos2::new(x, top.max.y)],
            Stroke::new(1.0, fg),
        );
        if let Some(l) = t.label {
            painter.text(
                Pos2::new(x + 2.0, top.min.y + 1.0),
                egui::Align2::LEFT_TOP,
                l,
                font.clone(),
                fg,
            );
        }
    }
    for t in ruler_ticks(a.y, b.y, vp.scale, 6.0) {
        let y = vp.to_screen(v2(0.0, t.at)).y as f32;
        if y < top.max.y {
            continue;
        }
        let len = if t.major { RULER * 0.6 } else { RULER * 0.25 };
        painter.line_segment(
            [Pos2::new(left.max.x - len, y), Pos2::new(left.max.x, y)],
            Stroke::new(1.0, fg),
        );
        if let Some(l) = t.label {
            painter.text(
                Pos2::new(left.min.x + 1.0, y + 2.0),
                egui::Align2::LEFT_TOP,
                l,
                font.clone(),
                fg,
            );
        }
    }
    painter.rect_filled(
        egui::Rect::from_min_size(rect.min, egui::vec2(RULER, RULER)),
        0.0,
        bg,
    );
    if let Some(h) = hover {
        let mark = Stroke::new(1.0, Color32::from_rgb(0xff, 0x8a, 0x3d));
        painter.line_segment([Pos2::new(h.x, top.min.y), Pos2::new(h.x, top.max.y)], mark);
        painter.line_segment(
            [Pos2::new(left.min.x, h.y), Pos2::new(left.max.x, h.y)],
            mark,
        );
    }
}

/// Grid lines on the Design Page from the grid settings: solid with axes, else dotted.
pub fn draw_grid(painter: &egui::Painter, vp: &Viewport, page: &Rect, grid: &GridSettings) {
    let colors = crate::theme::canvas(painter.ctx());
    let (minor, major) = (colors.grid_minor, colors.grid_major);
    let line = |a: Pos2, b: Pos2, m: bool| {
        let c = if m { major } else { minor };
        if grid.axes {
            painter.line_segment([a, b], Stroke::new(1.0, c));
        } else {
            painter.extend(Shape::dotted_line(&[a, b], c, 4.0, 0.8));
        }
    };
    for (x, m) in grid.lines(page.max_x, vp.scale, 6.0) {
        line(
            pos(vp.to_screen(v2(x, 0.0))),
            pos(vp.to_screen(v2(x, page.max_y))),
            m,
        );
    }
    for (y, m) in grid.lines(page.max_y, vp.scale, 6.0) {
        line(
            pos(vp.to_screen(v2(0.0, y))),
            pos(vp.to_screen(v2(page.max_x, y))),
            m,
        );
    }
}

/// Stitches drawn as thread: true width, a dark edge, the colour and a highlight, so rows read
/// like sewn thread rather than lines (width, contrast and brightness from Realistic View
/// Settings).
pub fn draw_realistic(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    let settings = app.ui.prefs.realistic;
    let w = (settings.thread_width_mm * 10.0 * vp.scale).max(0.8) as f32;
    for layer in &app.render.layers {
        let [edge, body, light] = settings
            .shades(layer.rgb)
            .map(|c| Color32::from_rgb(c[0], c[1], c[2]));
        for s in &layer.strokes {
            for pair in s.windows(2) {
                let (a, b) = (pos(vp.to_screen(pair[0])), pos(vp.to_screen(pair[1])));
                painter.line_segment([a, b], Stroke::new(w, edge));
                painter.line_segment([a, b], Stroke::new(w * 0.7, body));
                if w >= 2.5 {
                    // The highlight sits towards the middle of each stitch, as light catches
                    // the top of the thread.
                    let d = b - a;
                    painter.line_segment([a + d * 0.2, b - d * 0.2], Stroke::new(w * 0.25, light));
                }
            }
            // Needle penetrations show as small dark dots between stitches.
            if w >= 3.0 {
                for &p in s {
                    painter.circle_filled(pos(vp.to_screen(p)), w * 0.18, edge);
                }
            }
        }
    }
}

/// The measure line and its readout.
pub fn draw_measure(painter: &egui::Painter, vp: &Viewport, tool: &MeasureTool) {
    let Some(m) = tool.current else { return };
    let (a, b) = (pos(vp.to_screen(m.from)), pos(vp.to_screen(m.to)));
    let c = Color32::from_rgb(0xe6, 0x3b, 0x7a);
    painter.add(Shape::dashed_line(&[a, b], Stroke::new(1.5, c), 6.0, 3.0));
    painter.circle_stroke(a, 3.0, Stroke::new(1.5, c));
    painter.circle_stroke(b, 3.0, Stroke::new(1.5, c));
    let text = m.label();
    let galley = painter.layout_no_wrap(text, egui::FontId::monospace(12.0), Color32::WHITE);
    let size = galley.size();
    // Beside the end point, flipped to the other side where it would leave the canvas.
    let clip = painter.clip_rect();
    let x = if b.x + 14.0 + size.x > clip.max.x {
        b.x - 14.0 - size.x
    } else {
        b.x + 14.0
    };
    let y = (b.y - 14.0 - size.y).max(clip.min.y + RULER + 6.0);
    let r = egui::Rect::from_min_size(Pos2::new(x, y), size).expand(4.0);
    painter.rect_filled(r, 3.0, c.gamma_multiply(0.9));
    painter.galley(r.min + egui::vec2(4.0, 4.0), galley, Color32::WHITE);
}

// MARK: windows

/// The Reference window: whole Design Page with the stitches and the canvas's visible area;
/// click or drag to move the canvas there.
pub fn reference_window(app: &mut EditorApp, ctx: &egui::Context, canvas: Option<egui::Rect>) {
    let mut open = app.workspace.reference;
    egui::Window::new("Reference")
        .open(&mut open)
        .default_size([220.0, 220.0])
        .resizable(true)
        .show(ctx, |ui| {
            let size = ui.available_size().max(egui::vec2(120.0, 120.0));
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let page = app.session.design().page;
            let page_rect = Rect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: page.width,
                max_y: page.height,
            };
            let map = ReferenceMap::new(page_rect, screen_rect(rect));
            let painter = ui.painter_at(rect);
            let colors = crate::theme::canvas(ui.ctx());
            painter.rect_filled(rect, 0.0, colors.work_area);
            let p = &map.thumb;
            painter.rect_filled(
                egui::Rect::from_two_pos(
                    pos(p.to_screen(v2(0.0, 0.0))),
                    pos(p.to_screen(v2(page.width, page.height))),
                ),
                0.0,
                colors.page,
            );
            app.ensure_render_model();
            for layer in &app.render.layers {
                let [r, g, b] = layer.rgb;
                let stroke = Stroke::new(1.0, Color32::from_rgb(r, g, b));
                for s in &layer.strokes {
                    let pts: Vec<Pos2> = s.iter().map(|&q| pos(p.to_screen(q))).collect();
                    painter.add(Shape::line(pts, stroke));
                }
            }
            let (Some(canvas), Some(mut vp)) = (canvas, app.viewport) else {
                return;
            };
            let screen = screen_rect(canvas);
            let v = map.visible(&vp, screen);
            painter.rect_stroke(
                egui::Rect::from_min_size(
                    Pos2::new(v.x as f32, v.y as f32),
                    egui::vec2(v.width as f32, v.height as f32),
                ),
                0.0,
                Stroke::new(1.5, Color32::from_rgb(0xe5, 0x39, 0x35)),
                egui::StrokeKind::Middle,
            );
            if (response.clicked() || response.dragged())
                && let Some(at) = response.interact_pointer_pos()
            {
                map.center_on(&mut vp, screen, v2(f64::from(at.x), f64::from(at.y)));
                app.viewport = Some(vp);
            }
        });
    app.workspace.reference = open;
}

/// PE-DESIGN's Design Settings: machine type, hoop, rotation, custom size, user hoops, sewing
/// area and jump-stitch trimming. Every change is one `machine.*` command (one Undo).
pub fn design_settings_window(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = app.workspace.settings_open;
    let mut requests: Vec<CommandRequest> = Vec::new();
    egui::Window::new("Design Settings")
        .open(&mut open)
        .default_width(340.0)
        .resizable(false)
        .show(ctx, |ui| {
            let d = app.session.design();
            let m = d.machine.clone();
            let (pw, ph) = (d.page.width / 10.0, d.page.height / 10.0);
            let set = |v: serde_json::Value| CommandRequest::new("machine.set", v);
            egui::Grid::new("machine").num_columns(2).show(ui, |ui| {
                ui.label("Machine type");
                ui.horizontal(|ui| {
                    for (t, label) in [
                        (MachineType::SingleNeedle, "Single needle"),
                        (MachineType::MultiNeedle, "Multi-needle"),
                    ] {
                        if ui.selectable_label(m.machine == t, label).clicked() && m.machine != t {
                            requests.push(set(json!({ "machine": t.as_str() })));
                        }
                    }
                });
                ui.end_row();

                ui.label("Hoop");
                let current = m.hoop.clone().unwrap_or_else(|| "custom".into());
                let text = BROTHER_HOOPS
                    .iter()
                    .find(|h| h.id == current)
                    .map(|h| h.label.to_string())
                    .or_else(|| {
                        m.user_hoop(&current)
                            .map(|u| format!("{} × {} mm (user)", u.width, u.height))
                    })
                    .unwrap_or_else(|| "Custom size".into());
                egui::ComboBox::from_id_salt("hoop")
                    .selected_text(text)
                    .width(230.0)
                    .show_ui(ui, |ui| {
                        for h in BROTHER_HOOPS {
                            if ui
                                .selectable_label(current == h.id, h.label)
                                .on_hover_text(h.source)
                                .clicked()
                            {
                                requests.push(set(json!({ "hoop": h.id })));
                            }
                        }
                        if !m.user_hoops.is_empty() {
                            ui.separator();
                        }
                        for u in &m.user_hoops {
                            let key = openstitch_design::machine::user_hoop_key(u.id);
                            let label = if u.comment.is_empty() {
                                format!("{} × {} mm", u.width, u.height)
                            } else {
                                format!("{} × {} mm  {}", u.width, u.height, u.comment)
                            };
                            if ui.selectable_label(current == key, label).clicked() {
                                requests.push(set(json!({ "hoop": key })));
                            }
                        }
                        ui.separator();
                        if ui.selectable_label(current == "custom", "Custom size").clicked() {
                            requests.push(set(json!({ "hoop": "custom" })));
                        }
                    });
                ui.end_row();

                let user = m.hoop.as_deref().is_some_and(|k| m.user_hoop(k).is_some());
                ui.label("");
                let mut rotate = m.rotate90;
                if ui
                    .add_enabled(m.hoop.is_some() && !user, egui::Checkbox::new(&mut rotate, "Rotate 90°"))
                    .changed()
                {
                    requests.push(set(json!({ "rotate90": rotate })));
                }
                ui.end_row();

                ui.label("Design Page");
                ui.horizontal(|ui| {
                    let custom = m.hoop.is_none();
                    let (mut w, mut h) = (pw, ph);
                    let rw = ui.add_enabled(
                        custom,
                        egui::DragValue::new(&mut w).range(10.0..=2000.0).suffix(" mm"),
                    );
                    ui.label("×");
                    let rh = ui.add_enabled(
                        custom,
                        egui::DragValue::new(&mut h).range(10.0..=2000.0).suffix(" mm"),
                    );
                    let done = |r: &egui::Response| r.drag_stopped() || r.lost_focus();
                    if (done(&rw) || done(&rh)) && (w != pw || h != ph) {
                        requests.push(set(json!({ "width": w, "height": h })));
                    }
                });
                ui.end_row();

                ui.label("Sewing area");
                ui.vertical(|ui| {
                    for (a, label, tip) in [
                        (
                            SewingArea::DesignPage,
                            "Design Page area",
                            "The needle starts at the centre of the Design Page",
                        ),
                        (
                            SewingArea::DesignArea,
                            "Use existing design area",
                            "The needle starts at the centre of the design",
                        ),
                    ] {
                        if ui
                            .radio(m.sewing_area == a, label)
                            .on_hover_text(tip)
                            .clicked()
                            && m.sewing_area != a
                        {
                            requests.push(set(json!({ "sewingArea": a.as_str() })));
                        }
                    }
                });
                ui.end_row();

                ui.label("Jump stitch trimming");
                let multi = m.machine == MachineType::MultiNeedle;
                let mut trim = m.trim_distance(openstitch_design::Compiler::default().trim_distance)
                    / 10.0;
                let r = ui
                    .add_enabled(
                        multi,
                        egui::DragValue::new(&mut trim)
                            .range(0.5..=50.0)
                            .speed(0.1)
                            .suffix(" mm minimum"),
                    )
                    .on_disabled_hover_text("Multi-needle machines only");
                if (r.drag_stopped() || r.lost_focus()) && multi {
                    requests.push(set(json!({ "minJumpTrim": (trim * 10.0).round() / 10.0 })));
                }
                ui.end_row();

                ui.label("Speed (estimate)");
                let mut spm = m.speed();
                let r = ui.add(
                    egui::DragValue::new(&mut spm)
                        .range(100.0..=2000.0)
                        .speed(10.0)
                        .suffix(" st/min"),
                );
                if r.drag_stopped() || r.lost_focus() {
                    requests.push(set(json!({ "speed": spm.round() as i64 })));
                }
                ui.end_row();
            });
            crate::sections_ui::settings(app, ui, &mut requests);

            ui.separator();
            ui.label(egui::RichText::new("User hoops").strong());
            for (i, u) in m.user_hoops.iter().enumerate() {
                let key = openstitch_design::machine::user_hoop_key(u.id);
                ui.horizontal(|ui| {
                    ui.label(format!("{} × {} mm", u.width, u.height));
                    if !u.comment.is_empty() {
                        ui.label(egui::RichText::new(&u.comment).weak());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Delete").clicked() {
                            requests.push(CommandRequest::new(
                                "machine.deleteUserHoop",
                                json!({ "hoop": key }),
                            ));
                        }
                        if ui
                            .add_enabled(i + 1 < m.user_hoops.len(), egui::Button::new("↓").small())
                            .clicked()
                        {
                            requests.push(CommandRequest::new(
                                "machine.moveUserHoop",
                                json!({ "hoop": key, "direction": "down" }),
                            ));
                        }
                        if ui.add_enabled(i > 0, egui::Button::new("↑").small()).clicked() {
                            requests.push(CommandRequest::new(
                                "machine.moveUserHoop",
                                json!({ "hoop": key, "direction": "up" }),
                            ));
                        }
                    });
                });
            }
            ui.horizontal(|ui| {
                let (w, h, c) = &mut app.workspace.user_hoop;
                ui.add(egui::DragValue::new(w).range(10.0..=2000.0).suffix(" mm"));
                ui.label("×");
                ui.add(egui::DragValue::new(h).range(10.0..=2000.0).suffix(" mm"));
                ui.add(egui::TextEdit::singleline(c).hint_text("comment").desired_width(80.0));
                if ui.button("Add Hoop").clicked() {
                    requests.push(CommandRequest::new(
                        "machine.addUserHoop",
                        json!({ "width": *w, "height": *h, "comment": c.clone() }),
                    ));
                    c.clear();
                }
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Do not choose a hoop larger than your machine can use. Hoop sizes come from Brother's public pages (hover for the source).",
                )
                .weak()
                .small(),
            );
        });
    app.workspace.settings_open = open;
    for r in requests {
        if app.execute(&r).is_some() {
            app.status = format!("{} done", r.command);
            if r.command == "machine.set" {
                app.fit_requested = true;
            }
        }
    }
}

/// "12.3 m · 1 h 05 min" for the status line and design info.
pub fn estimate_text(e: &Estimate) -> String {
    let minutes = (e.seconds / 60.0).round() as i64;
    let time = if minutes >= 60 {
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    } else {
        format!("{} min", minutes.max(if e.seconds > 0.0 { 1 } else { 0 }))
    };
    format!(
        "Thread {:.1} m (bobbin {:.1} m) · about {time}",
        e.needle_total() / 1000.0,
        e.bobbin_thread / 1000.0
    )
}
