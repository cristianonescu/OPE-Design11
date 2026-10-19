//! Toolbar, inspector, object list and sewing simulator panels.

use crate::app::EditorApp;
use eframe::egui::{self, Color32, RichText, Sense};
use openstitch_app::editor::inspector::{self, Common, Control, InspectorModel};
use openstitch_app::editor::render::color_at;
use openstitch_app::editor::{CommandRequest, Tool, objects};
use openstitch_core::Thread;
use openstitch_design::{LineType, RegionType};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn swatch(ui: &mut egui::Ui, c: [u8; 3], size: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(size, size), Sense::click());
    ui.painter().rect(
        r,
        egui::CornerRadius::same(2),
        rgb(c),
        egui::Stroke::new(1.0, Color32::from_gray(90)),
        egui::StrokeKind::Inside,
    );
    resp
}

pub fn toolbar(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for t in Tool::ALL {
            let label = format!("{} ({})", t.label(), t.shortcut().to_ascii_uppercase());
            if ui.selectable_label(app.tools.tool() == t, label).clicked() {
                app.tools.set_tool(t);
            }
        }
        ui.separator();
        if ui.add_enabled(app.session.can_undo(), egui::Button::new("Undo")).clicked() {
            app.undo();
        }
        if ui.add_enabled(app.session.can_redo(), egui::Button::new("Redo")).clicked() {
            app.redo();
        }
        ui.separator();
        if ui.button("−").on_hover_text("Zoom out").clicked() {
            zoom_center(app, 1.0 / 1.25);
        }
        if ui.button("+").on_hover_text("Zoom in").clicked() {
            zoom_center(app, 1.25);
        }
        if ui.button("Fit").on_hover_text(format!("Fit Design Page ({}0)", crate::app::CMD)).clicked() {
            app.fit_requested = true;
        }
        let hint = match app.tools.tool() {
            Tool::Polygon => Some("Click to add points · click the first point, double-click or Return to close · Esc cancels"),
            Tool::Point => Some("Drag points or handles · double-click an edge to add a point · Delete removes it · C curve / S straight under the pointer"),
            Tool::Pen => Some("Click for a corner, drag for a smooth point · click the first point to close · Return finishes open · Esc cancels"),
            Tool::Punch => Some("Click both ends of each sewing-direction line in turn (one side first) · double-click or Return finishes · Esc cancels"),
            _ => None,
        };
        if let Some(hint) = hint {
            ui.separator();
            ui.label(RichText::new(hint).weak());
        }
    });
}

fn zoom_center(app: &mut EditorApp, factor: f64) {
    if let Some(vp) = &mut app.viewport {
        // Anchor at the model point currently in the middle of the page.
        let page = app.session.design().page.center();
        let anchor = vp.to_screen(page);
        vp.zoom_about(anchor, factor);
    }
}

// MARK: object list

pub fn object_list(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Sewing order");
        ui.checkbox(&mut app.edit.by_colour, "By colour")
            .on_hover_text("One frame per run of objects in the same colour");
    });
    if app.edit.by_colour {
        crate::edit_ui::colour_frames(app, ui);
        return;
    }
    let rows = objects::rows(app.session.design(), &app.selection);
    if rows.is_empty() {
        ui.label(
            RichText::new("No objects. Draw with the shape tools or File > Import into design.")
                .weak(),
        );
        return;
    }
    let mut pending: Option<CommandRequest> = None;
    let mut select: Option<(u64, bool)> = None;
    let modifiers = ui.input(|i| i.modifiers);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 80.0)
        .show(ui, |ui| {
            for row in &rows {
                let drop = ui
                    .dnd_drop_zone::<usize, ()>(egui::Frame::NONE, |ui| {
                        ui.horizontal(|ui| {
                            let mut visible = row.visible;
                            if ui
                                .checkbox(&mut visible, "")
                                .on_hover_text("Visible (hidden objects are not sewn)")
                                .changed()
                            {
                                pending = Some(objects::set_visible(row.id, visible));
                            }
                            let handle = ui.dnd_drag_source(
                                ui.scope_id().with(("row", row.id)),
                                row.index,
                                |ui| {
                                    ui.label(RichText::new("::").weak());
                                },
                            );
                            handle
                                .response
                                .on_hover_text("Drag to change the sewing order");
                            for &c in row.swatches.iter().take(4) {
                                swatch(ui, c, 12.0);
                            }
                            let text = format!("{}. {}", row.index + 1, row.label);
                            let resp = ui
                                .selectable_label(row.selected, text)
                                .on_hover_text(&row.detail);
                            if resp.clicked() {
                                select = Some((row.id, modifiers.shift || modifiers.command));
                            }
                        });
                    })
                    .1;
                if let Some(from) = drop {
                    pending = Some(objects::reorder(
                        rows[*from].id,
                        objects::drop_index(*from, row.index),
                    ));
                }
            }
        });
    if let Some(r) = crate::edit_ui::order_controls(app, ui) {
        pending = Some(r);
    }
    if let Some((id, toggle)) = select {
        if toggle {
            if !app.selection.remove(&id) {
                app.selection.insert(id);
            }
        } else {
            app.selection = BTreeSet::from([id]);
        }
    }
    if let Some(r) = pending {
        app.execute(&r);
    }
}

// MARK: inspector

/// A number field that issues one command when the drag ends or typing is confirmed.
fn number(
    app: &mut EditorApp,
    ui: &mut egui::Ui,
    key: &'static str,
    value: f64,
    range: (f64, f64),
    speed: f64,
    suffix: &str,
) -> Option<f64> {
    let mut v = app.drafts.get(key).copied().unwrap_or(value);
    let resp = ui.add(
        egui::DragValue::new(&mut v)
            .range(range.0..=range.1)
            .speed(speed)
            .max_decimals(2)
            .suffix(suffix),
    );
    if resp.changed() {
        app.drafts.insert(key, v);
    }
    let finished = resp.drag_stopped()
        || resp.lost_focus()
        || (resp.changed() && !resp.dragged() && !resp.has_focus());
    if finished
        && let Some(v) = app.drafts.remove(key)
        && (v - value).abs() > 1e-9
    {
        return Some(v);
    }
    None
}

/// PEC chart picker: returns the chart number chosen.
fn thread_picker(ui: &mut egui::Ui, id: &str, current: Option<&Thread>) -> Option<usize> {
    let mut chosen = None;
    let c = current.map_or([128, 128, 128], |t| [t.red, t.green, t.blue]);
    let index = current.and_then(inspector::pec_index);
    let name = current.and_then(|t| t.name.as_deref()).unwrap_or("Custom");
    let label = match (current, index) {
        (Some(t), Some(i)) if t.name.is_some() => format!("{i} {name}"),
        (Some(t), _) => t.hex(),
        (None, _) => "Mixed".into(),
    };
    ui.horizontal(|ui| {
        let resp = swatch(ui, c, 18.0);
        let button = ui.button(label);
        let popup_id = ui.make_persistent_id(id);
        let open = resp.clicked() || button.clicked();
        egui::Popup::from_response(&button)
            .id(popup_id)
            .open_memory(open.then_some(egui::SetOpenCommand::Toggle))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.label(RichText::new("Brother PEC thread chart").strong());
                egui::Grid::new(("pec", id))
                    .spacing(egui::vec2(3.0, 3.0))
                    .show(ui, |ui| {
                        for (n, (i, t)) in inspector::pec_palette().into_iter().enumerate() {
                            let tip =
                                format!("{i} {} ({})", t.name.as_deref().unwrap_or(""), t.hex());
                            let r = swatch(ui, [t.red, t.green, t.blue], 20.0).on_hover_text(tip);
                            if Some(i) == index && current.is_some_and(|c| c.name.is_some()) {
                                ui.painter().rect_stroke(
                                    r.rect.expand(1.5),
                                    egui::CornerRadius::same(3),
                                    egui::Stroke::new(2.0, Color32::WHITE),
                                    egui::StrokeKind::Outside,
                                );
                            }
                            if r.clicked() {
                                chosen = Some(i);
                                ui.close();
                            }
                            if n % 8 == 7 {
                                ui.end_row();
                            }
                        }
                    });
            });
    });
    chosen
}

pub fn inspector(app: &mut EditorApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Inspector");
            if app.selection.is_empty() {
                design_info(app, ui);
                return;
            }
            let m = InspectorModel::new(app.session.design(), &app.selection);
            ui.label(format!("{} selected", app.selection.len()));
            transform_section(app, ui);
            if m.shapes.is_empty() {
                ui.separator();
                ui.label(
                    RichText::new(
                        "Imported stitch patterns keep their stitches; only transforms apply.",
                    )
                    .weak(),
                );
                return;
            }
            let mut requests: Vec<CommandRequest> = Vec::new();
            region_section(app, ui, &m, &mut requests);
            line_section(app, ui, &m, &mut requests);
            for r in requests {
                app.execute(&r);
            }
        });
}

fn design_info(app: &mut EditorApp, ui: &mut egui::Ui) {
    let d = app.session.design();
    let (name, w, h, n) = (
        d.name.clone(),
        d.page.width / 10.0,
        d.page.height / 10.0,
        d.objects.len(),
    );
    egui::Grid::new("design").num_columns(2).show(ui, |ui| {
        ui.label("Design");
        ui.label(name);
        ui.end_row();
        ui.label("Design Page");
        ui.label(format!("{w:.0} × {h:.0} mm"));
        ui.end_row();
        ui.label("Objects");
        ui.label(n.to_string());
        ui.end_row();
        let m = &app.session.design().machine;
        ui.label("Machine");
        ui.label(match m.machine {
            openstitch_design::MachineType::SingleNeedle => "Single needle",
            openstitch_design::MachineType::MultiNeedle => "Multi-needle",
        });
        ui.end_row();
        ui.label("Hoop");
        ui.label(m.hoop.as_deref().unwrap_or("custom"));
        ui.end_row();
        if let Some(e) = &app.workspace.estimate {
            let per: Vec<String> = e
                .needle_thread
                .iter()
                .map(|mm| format!("{:.1}", mm / 1000.0))
                .collect();
            ui.label("Estimate");
            ui.label(crate::workspace_ui::estimate_text(e))
                .on_hover_text(format!("Needle thread per colour (m): {}", per.join(", ")));
            ui.end_row();
        }
    });
    if ui.button("Design Settings…").clicked() {
        app.workspace.settings_open = true;
    }
    ui.add_space(8.0);
    ui.label(RichText::new("Select an object to edit its sewing attributes.").weak());
}

fn transform_section(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.label(RichText::new("Transform").strong());
    ui.horizontal_wrapped(|ui| {
        if ui.button("Rotate −90°").clicked() {
            app.on_selection(|s| CommandRequest::rotate(s, -90.0));
        }
        if ui.button("Rotate +90°").clicked() {
            app.on_selection(|s| CommandRequest::rotate(s, 90.0));
        }
        if ui.button("Flip H").clicked() {
            app.on_selection(|s| CommandRequest::flip(s, true));
        }
        if ui.button("Flip V").clicked() {
            app.on_selection(|s| CommandRequest::flip(s, false));
        }
        if ui.button("Centre").clicked() {
            app.on_selection(|s| {
                CommandRequest::new("object.center", json!({ "ids": CommandRequest::ids(s) }))
            });
        }
    });
    ui.horizontal(|ui| {
        ui.label("Scale");
        ui.add(
            egui::DragValue::new(&mut app.scale_percent)
                .range(1.0..=1000.0)
                .speed(1.0)
                .suffix(" %"),
        );
        if ui.button("Apply").clicked() && (app.scale_percent - 100.0).abs() > 1e-9 {
            let f = app.scale_percent / 100.0;
            app.on_selection(|s| {
                CommandRequest::new(
                    "object.scale",
                    json!({ "ids": CommandRequest::ids(s), "sx": f }),
                )
            });
            app.scale_percent = 100.0;
        }
    });
    ui.horizontal(|ui| {
        if ui
            .button(format!("Duplicate ({}D)", crate::app::CMD))
            .clicked()
        {
            app.on_selection(CommandRequest::duplicate);
        }
        if ui.button("Delete").clicked() {
            app.on_selection(CommandRequest::delete);
        }
    });
}

fn kind_combo<K: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    current: &Common<K>,
    options: &[(K, &str)],
    enabled: bool,
) -> Option<K> {
    let text = match current {
        Common::Same(k) => options.iter().find(|o| o.0 == *k).map_or("?", |o| o.1),
        Common::Mixed => "Mixed",
        Common::Empty => "—",
    };
    let mut chosen = None;
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(id)
            .selected_text(text)
            .show_ui(ui, |ui| {
                for &(k, label) in options {
                    if ui
                        .selectable_label(current.same() == Some(&k), label)
                        .clicked()
                    {
                        chosen = Some(k);
                    }
                }
            });
    });
    chosen.filter(|k| current.same() != Some(k))
}

fn region_section(
    app: &mut EditorApp,
    ui: &mut egui::Ui,
    m: &InspectorModel,
    out: &mut Vec<CommandRequest>,
) {
    ui.separator();
    ui.label(RichText::new("Region sewing").strong());
    if !m.closed {
        ui.label(RichText::new("Needs a closed outline.").weak());
        return;
    }
    egui::Grid::new("region")
        .num_columns(2)
        .spacing(egui::vec2(8.0, 6.0))
        .show(ui, |ui| {
            ui.label("Type");
            if let Some(k) = kind_combo::<Option<RegionType>>(
                ui,
                "region-type",
                &m.region_kind,
                &inspector::region_types(),
                true,
            ) {
                out.push(m.set_region_type(k));
            }
            ui.end_row();
            let Some(r) = m.region.clone() else { return };
            ui.label("Colour");
            let colour = if all_same_region_thread(app, m) {
                Some(&r.thread)
            } else {
                None
            };
            if let Some(i) = thread_picker(ui, "region-colour", colour) {
                out.push(m.set_region(json!({ "color": i })));
            }
            ui.end_row();
            let user = app.session.design().patterns.clone();
            for c in inspector::region_controls_with(&r, &user) {
                if let Some(v) = control(app, ui, &c) {
                    out.push(m.set_region(c.params(v)));
                }
            }
            ui.label("Hole sewing");
            let mut hole = matches!(m.hole_sewing, Common::Same(true));
            let text = if m.hole_sewing == Common::Mixed {
                "Mixed"
            } else {
                "Skip shapes sewn on top"
            };
            if ui.checkbox(&mut hole, text).changed()
                && let Some(r) = m.set_hole_sewing(app.session.design(), hole)
            {
                out.push(r);
            }
            ui.end_row();
        });
}

fn all_same_region_thread(app: &EditorApp, m: &InspectorModel) -> bool {
    let d = app.session.design();
    let threads = d
        .objects
        .iter()
        .filter(|o| m.shapes.contains(&o.id))
        .filter_map(|o| o.region.as_ref().map(|r| r.thread.clone()));
    matches!(Common::of(threads), Common::Same(_))
}

fn all_same_line_thread(app: &EditorApp, m: &InspectorModel) -> bool {
    let d = app.session.design();
    let threads = d
        .objects
        .iter()
        .filter(|o| m.shapes.contains(&o.id))
        .filter_map(|o| o.line.as_ref().map(|l| l.thread.clone()));
    matches!(Common::of(threads), Common::Same(_))
}

fn line_section(
    app: &mut EditorApp,
    ui: &mut egui::Ui,
    m: &InspectorModel,
    out: &mut Vec<CommandRequest>,
) {
    ui.separator();
    ui.label(RichText::new("Line sewing").strong());
    egui::Grid::new("line")
        .num_columns(2)
        .spacing(egui::vec2(8.0, 6.0))
        .show(ui, |ui| {
            ui.label("Type");
            if let Some(k) = kind_combo::<Option<LineType>>(
                ui,
                "line-type",
                &m.line_kind,
                &inspector::line_types(),
                true,
            ) {
                out.push(m.set_line_type(k));
            }
            ui.end_row();
            let Some(l) = m.line.clone() else { return };
            ui.label("Colour");
            let colour = if all_same_line_thread(app, m) {
                Some(&l.thread)
            } else {
                None
            };
            if let Some(i) = thread_picker(ui, "line-colour", colour) {
                out.push(m.set_line(json!({ "color": i })));
            }
            ui.end_row();
            let user = app.session.design().patterns.clone();
            for c in inspector::line_controls_with(&l, &user) {
                if let Some(v) = control(app, ui, &c) {
                    out.push(m.set_line(c.params(v)));
                }
            }
        });
}

/// Shows one attribute control (label and widget, one grid row); returns a new value.
fn control(app: &mut EditorApp, ui: &mut egui::Ui, c: &Control) -> Option<Value> {
    let mut changed = None;
    match c {
        Control::Number {
            label,
            key,
            value,
            range,
            speed,
            suffix,
            ..
        } => {
            ui.label(*label);
            changed = number(app, ui, key, *value, *range, *speed, suffix).map(|v| json!(v));
        }
        Control::Choice {
            label,
            param,
            value,
            options,
        } => {
            ui.label(*label);
            let text = options
                .iter()
                .find(|o| o.0 == *value)
                .map_or(value.as_str(), |o| o.1.as_str());
            egui::ComboBox::from_id_salt(*param)
                .selected_text(text)
                .show_ui(ui, |ui| {
                    for (v, l) in options {
                        if ui.selectable_label(v == value, l).clicked() && v != value {
                            changed = Some(json!(v));
                        }
                    }
                });
        }
        Control::Toggle { label, value, .. } => {
            ui.label("");
            let mut on = *value;
            if ui.checkbox(&mut on, *label).changed() {
                changed = Some(json!(on));
            }
        }
    }
    ui.end_row();
    changed
}

// MARK: simulator

pub fn simulator(app: &mut EditorApp, ui: &mut egui::Ui) {
    let total = app.stats.stitches;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Sewing simulator").strong());
        let play = if app.playing { "Pause" } else { "Play" };
        if ui.add_enabled(total > 0, egui::Button::new(play)).clicked() {
            app.playing = !app.playing;
            if app.playing && app.simulate.is_none() {
                app.simulate = Some(0);
            }
        }
        if ui
            .add_enabled(total > 0, egui::Button::new("|<"))
            .on_hover_text("Start")
            .clicked()
        {
            app.simulate = Some(0);
        }
        if ui
            .add_enabled(total > 0, egui::Button::new("<"))
            .on_hover_text("Previous stitch")
            .clicked()
        {
            app.simulate = Some(app.simulate.unwrap_or(total).saturating_sub(1));
        }
        if ui
            .add_enabled(total > 0, egui::Button::new(">"))
            .on_hover_text("Next stitch")
            .clicked()
        {
            let n = app.simulate.map_or(total, |n| n + 1);
            app.simulate = (n < total).then_some(n);
        }
        if ui
            .add_enabled(total > 0, egui::Button::new(">|"))
            .on_hover_text("End")
            .clicked()
        {
            app.simulate = None;
            app.playing = false;
        }
        let mut n = app.simulate.unwrap_or(total);
        let slider = egui::Slider::new(&mut n, 0..=total.max(1)).show_value(false);
        let width = (ui.available_width() - 260.0).max(120.0);
        ui.spacing_mut().slider_width = width;
        if ui.add_enabled(total > 0, slider).changed() {
            app.simulate = (n < total).then_some(n);
            app.playing = false;
        }
        ui.label(format!("{} / {}", app.simulate.unwrap_or(total), total));
        egui::ComboBox::from_id_salt("speed")
            .selected_text(format!("{:.0}/s", app.play_speed))
            .width(70.0)
            .show_ui(ui, |ui| {
                for s in [50.0, 200.0, 400.0, 1000.0, 4000.0] {
                    ui.selectable_value(&mut app.play_speed, s, format!("{s:.0}/s"));
                }
            });
    });
    ui.horizontal(|ui| {
        let s = &app.stats;
        ui.label(format!(
            "Stitches {}   Jumps {}   Trims {}   Colour changes {}   Size {:.1} × {:.1} mm",
            s.stitches, s.jumps, s.trims, s.color_changes, s.width_mm, s.height_mm
        ));
        if let Some(e) = &app.workspace.estimate {
            ui.separator();
            ui.label(crate::workspace_ui::estimate_text(e));
        }
        ui.separator();
        let current = app.simulate.map(|n| color_at(&app.pattern, n.max(1)).0);
        for (i, &c) in s.colors.iter().enumerate() {
            let r = swatch(ui, c, 14.0);
            if current == Some(i) {
                ui.painter().rect_stroke(
                    r.rect.expand(2.0),
                    egui::CornerRadius::same(3),
                    egui::Stroke::new(2.0, Color32::from_rgb(0xff, 0x3d, 0x00)),
                    egui::StrokeKind::Outside,
                );
            }
        }
        if let Some(n) = app.simulate {
            let (c, blocks) = color_at(&app.pattern, n.max(1));
            ui.label(format!("colour {} of {}", c + 1, blocks));
        }
        ui.separator();
        ui.checkbox(&mut app.show_travel, "Show travel");
    });
    ui.add_space(4.0);
}
