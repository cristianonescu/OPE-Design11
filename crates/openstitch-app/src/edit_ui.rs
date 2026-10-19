//! Shapes menu (presets, arcs, merge, remove overlap, combine, split, offset lines, entry/exit
//! points), the group / arrange / attribute items of the Edit menu, their dialogs and canvas
//! markers. Requests come from `openstitch_app::editor::edit_ops`; each action is one command,
//! so one Undo.

use crate::app::{CMD, EditorApp};
use eframe::egui::{self, Color32, Pos2, Stroke};
use openstitch_app::editor::edit_ops::{
    self, ArrangeForm, ArrangeMode, EntryExit, FloralForm, OffsetForm, Pick,
};
use openstitch_app::editor::objects;
use openstitch_app::editor::{CommandRequest, Tool, ToolEffect, Viewport};
use openstitch_design::{MachineType, ObjectKind, Vec2};
use std::collections::BTreeSet;

/// Editor state for these operations.
#[derive(Default)]
pub struct EditState {
    pub arrange_open: bool,
    pub arrange: ArrangeForm,
    pub offset_open: bool,
    pub offset: OffsetForm,
    pub floral_open: bool,
    pub floral: FloralForm,
    /// Decorative frames keep their own (square) proportions.
    pub frame_keep_aspect: bool,
    /// Source object of Copy Attributes.
    pub attributes_from: Option<u64>,
    /// The next canvas click sets this point of this object.
    pub pick: Option<(u64, Pick)>,
    /// The next open path drawn with the Pen tool becomes this shape's curved direction.
    pub curve_target: Option<u64>,
    /// The next canvas click places this stamp (object, pattern, effect) there.
    pub stamp_pick: Option<(u64, &'static str, &'static str)>,
    /// Stamp pattern, effect and size (mm) chosen in the Stamp menu.
    pub stamp_pattern: usize,
    pub stamp_effect: usize,
    /// The next polygon drawn is the cut outline splitting this shape.
    pub split_target: Option<u64>,
    /// Alt held: clicking a group member selects it alone.
    pub alt: bool,
    /// Sewing-order pane shows one frame per same-colour run.
    pub by_colour: bool,
}

impl EditState {
    /// Cancels a pending pick or split; whether there was one.
    pub fn cancel(&mut self) -> bool {
        self.pick.take().is_some()
            | self.split_target.take().is_some()
            | self.curve_target.take().is_some()
            | self.stamp_pick.take().is_some()
    }
}

fn single(app: &EditorApp) -> Option<u64> {
    (app.selection.len() == 1)
        .then(|| app.selection.iter().next().copied())
        .flatten()
}

fn kind_of(app: &EditorApp, id: u64) -> Option<ObjectKind> {
    let d = app.session.design();
    d.index_of(id).map(|i| d.objects[i].kind)
}

fn run_and_select(app: &mut EditorApp, r: CommandRequest) {
    if let Some(result) = app.execute(&r) {
        let created = CommandRequest::created_ids(&result);
        if !created.is_empty() {
            app.selection = created;
        }
        app.status = format!("{} done", r.command);
    }
}

/// Edit menu: group, ungroup, arrange copies, copy/paste attributes.
pub fn edit_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.separator();
    let d = app.session.design();
    let many = app.selection.len() >= 2;
    let grouped = edit_ops::any_grouped(d, &app.selection);
    if ui
        .add_enabled(
            many,
            egui::Button::new("Group").shortcut_text(format!("{CMD}G")),
        )
        .clicked()
    {
        group(app);
    }
    if ui
        .add_enabled(
            grouped,
            egui::Button::new("Ungroup").shortcut_text(format!("{CMD}Shift+G")),
        )
        .clicked()
    {
        ungroup(app);
    }
    if ui
        .add_enabled(
            !app.selection.is_empty(),
            egui::Button::new("Arrange Copies…"),
        )
        .clicked()
    {
        app.edit.arrange_open = true;
    }
    ui.separator();
    let source = single(app).filter(|&id| kind_of(app, id) != Some(ObjectKind::Stitches));
    if ui
        .add_enabled(source.is_some(), egui::Button::new("Copy Attributes"))
        .clicked()
    {
        app.edit.attributes_from = source;
        app.status = "Sewing attributes copied; select the targets and Paste Attributes".into();
    }
    let paste = app
        .edit
        .attributes_from
        .filter(|f| app.selection.iter().any(|id| id != f));
    if ui
        .add_enabled(paste.is_some(), egui::Button::new("Paste Attributes"))
        .clicked()
        && let Some(from) = paste
    {
        let r = edit_ops::paste_attributes(from, &app.selection);
        app.execute(&r);
    }
}

pub fn group(app: &mut EditorApp) {
    if app.selection.len() >= 2 {
        let r = edit_ops::group(&app.selection);
        app.execute(&r);
    }
}

pub fn ungroup(app: &mut EditorApp) {
    if edit_ops::any_grouped(app.session.design(), &app.selection) {
        let r = edit_ops::ungroup(&app.selection);
        app.execute(&r);
    }
}

/// The Shapes menu.
pub fn shapes_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.menu_button("Shapes", |ui| {
        ui.menu_button("Insert", |ui| {
            for (label, kind) in edit_ops::PRESETS {
                if ui.button(label).clicked() {
                    let center = app.session.design().page.center();
                    run_and_select(app, edit_ops::insert_preset(kind, center, 30.0));
                }
            }
        });
        ui.separator();
        let many = app.selection.len() >= 2;
        if ui.add_enabled(many, egui::Button::new("Merge")).clicked() {
            let r = edit_ops::merge(&app.selection);
            run_and_select(app, r);
        }
        if ui
            .add_enabled(many, egui::Button::new("Remove Overlap"))
            .clicked()
        {
            let r = edit_ops::remove_overlap(&app.selection);
            app.execute(&r);
        }
        ui.add_enabled_ui(many, |ui| {
            ui.menu_button("Combine", |ui| {
                for (label, op) in [
                    ("Union", "union"),
                    ("Subtract from first", "difference"),
                    ("Intersect", "intersection"),
                    ("Exclude overlap", "xor"),
                ] {
                    if ui.button(label).clicked() {
                        let r = edit_ops::combine(app.session.design(), &app.selection, op);
                        run_and_select(app, r);
                    }
                }
            });
        });
        let one = single(app);
        let shape = one.filter(|&id| kind_of(app, id) == Some(ObjectKind::Shape));
        if ui
            .add_enabled(shape.is_some(), egui::Button::new("Split with Outline"))
            .clicked()
        {
            app.edit.split_target = shape;
            app.tools.set_tool(Tool::Polygon);
            app.status = "Draw the cut outline; Return or double-click splits, Esc cancels".into();
        }
        let outline = one
            .filter(|&id| matches!(kind_of(app, id), Some(ObjectKind::Shape | ObjectKind::Text)));
        if ui
            .add_enabled(outline.is_some(), egui::Button::new("Offset Lines…"))
            .clicked()
        {
            app.edit.offset_open = true;
        }
        if ui
            .add_enabled(outline.is_some(), egui::Button::new("Floral Pattern…"))
            .clicked()
        {
            app.edit.floral_open = true;
        }
        ui.menu_button("Decorative Frame", |ui| {
            ui.checkbox(&mut app.edit.frame_keep_aspect, "Maintain aspect ratio");
            ui.separator();
            for (label, kind) in edit_ops::FRAMES {
                if ui.button(label).clicked() {
                    let r = edit_ops::decorative(kind, &app.selection, app.edit.frame_keep_aspect);
                    run_and_select(app, r);
                }
            }
        });
        ui.separator();
        ui.menu_button("Stamp/Emboss", |ui| {
            let two = app.selection.len() == 2;
            for (label, effect) in edit_ops::EMBOSS {
                if ui
                    .add_enabled(two, egui::Button::new(label))
                    .on_hover_text("Select the shape and the shape on top that gives the outline")
                    .clicked()
                    && let Some(r) = edit_ops::emboss(app.session.design(), &app.selection, effect)
                {
                    run_and_select(app, r);
                }
            }
            ui.separator();
            let one = shape.is_some();
            ui.add_enabled_ui(one, |ui| {
                egui::ComboBox::from_label("Stamp")
                    .selected_text(openstitch_design::stamp::PATTERNS[app.edit.stamp_pattern])
                    .show_ui(ui, |ui| {
                        for (k, p) in openstitch_design::stamp::PATTERNS.iter().enumerate() {
                            ui.selectable_value(&mut app.edit.stamp_pattern, k, *p);
                        }
                    });
                egui::ComboBox::from_label("Effect")
                    .selected_text(edit_ops::EMBOSS[app.edit.stamp_effect].0)
                    .show_ui(ui, |ui| {
                        for (k, (label, _)) in edit_ops::EMBOSS.iter().enumerate() {
                            ui.selectable_value(&mut app.edit.stamp_effect, k, *label);
                        }
                    });
                if ui.button("Place stamp (click in the shape)").clicked()
                    && let Some(id) = shape
                {
                    app.edit.stamp_pick = Some((
                        id,
                        openstitch_design::stamp::PATTERNS[app.edit.stamp_pattern],
                        edit_ops::EMBOSS[app.edit.stamp_effect].1,
                    ));
                    app.status = "Click inside the shape to place the stamp; Esc cancels".into();
                }
                if ui.button("Remove stamps").clicked()
                    && let Some(id) = shape
                {
                    app.execute(&edit_ops::clear_stamps(id));
                }
            });
        });
        ui.separator();
        ui.add_enabled_ui(shape.is_some(), |ui| {
            ui.menu_button("Entry/Exit Points", |ui| {
                for (label, which) in [
                    ("Set region entry (R1)", EntryExit::RegionEntry),
                    ("Set region exit (R2)", EntryExit::RegionExit),
                    ("Set line entry (L1)", EntryExit::LineEntry),
                ] {
                    if ui.button(label).clicked()
                        && let Some(id) = shape
                    {
                        app.edit.pick = Some((id, Pick::Point(which)));
                        app.status = format!(
                            "Click the outline where {} should be; Esc cancels",
                            which.label()
                        );
                    }
                }
                if ui.button("Reset to automatic").clicked()
                    && let Some(id) = shape
                {
                    app.execute(&edit_ops::clear_entry_exit(id));
                }
            });
        });
        let region = shape.filter(|&id| {
            let d = app.session.design();
            d.index_of(id)
                .is_some_and(|i| d.objects[i].region.is_some())
        });
        ui.add_enabled_ui(region.is_some(), |ui| {
            ui.menu_button("Sewing Direction", |ui| {
                let id = region.unwrap_or_default();
                if ui.button("Draw curved direction (Pen)").clicked() {
                    app.edit.curve_target = Some(id);
                    app.tools.set_tool(Tool::Pen);
                    app.status =
                        "Draw an open curve across the shape; Return finishes, Esc cancels".into();
                }
                if ui.button("Straight direction").clicked() {
                    app.execute(&edit_ops::straight_direction(id));
                }
                ui.separator();
                for (label, which) in [
                    ("Set centre point", Pick::Center),
                    ("Set reference point", Pick::Reference),
                ] {
                    if ui.button(label).clicked() {
                        app.edit.pick = Some((id, which));
                        app.status =
                            format!("Click where {} should be; Esc cancels", which.label());
                    }
                }
            });
        });
    });
}

/// A canvas click while a point is being picked; whether it was consumed.
pub fn pick(app: &mut EditorApp, at: Vec2) -> bool {
    if let Some((id, pattern, effect)) = app.edit.stamp_pick.take() {
        let r = edit_ops::add_stamp(id, pattern, effect, at, 10.0);
        if app.execute(&r).is_some() {
            app.status = "Stamp placed".into();
        }
        return true;
    }
    let Some((id, which)) = app.edit.pick.take() else {
        return false;
    };
    let r = which.request(id, at);
    if app.execute(&r).is_some() {
        app.status = format!("{} set", which.label());
    }
    true
}

/// Tool effects with the editor's own rules: selections take whole groups (unless Alt), and a
/// polygon drawn for Split with Outline becomes the split.
pub fn filter_effect(app: &mut EditorApp, e: ToolEffect) -> ToolEffect {
    match e {
        ToolEffect::Select(sel) if app.tools.tool() == Tool::Select => {
            let d = app.session.design();
            ToolEffect::Select(edit_ops::group_selection(
                d,
                &app.selection,
                sel,
                app.edit.alt,
            ))
        }
        ToolEffect::Execute {
            requests,
            select_result,
        } => match (app.edit.split_target, app.edit.curve_target) {
            (_, Some(id)) if requests.len() == 1 && requests[0].command == "shape.path" => {
                app.edit.curve_target = None;
                app.tools.set_tool(Tool::Select);
                match edit_ops::direction_curve(id, &requests[0]) {
                    Some(r) => ToolEffect::Execute {
                        requests: vec![r],
                        select_result: false,
                    },
                    None => {
                        app.status = "The direction must be an open curve".into();
                        ToolEffect::Select(BTreeSet::from([id]))
                    }
                }
            }
            (Some(id), _) if requests.len() == 1 && requests[0].command == "shape.polygon" => {
                app.edit.split_target = None;
                app.tools.set_tool(Tool::Select);
                let points = requests[0].params["points"].clone();
                // A stitch pattern splits off the needle points inside the outline.
                let request = if kind_of(app, id) == Some(ObjectKind::Stitches) {
                    CommandRequest::new(
                        "stitches.splitArea",
                        serde_json::json!({ "id": id, "points": points }),
                    )
                } else {
                    edit_ops::split(id, points)
                };
                ToolEffect::Execute {
                    requests: vec![request],
                    select_result: true,
                }
            }
            _ => ToolEffect::Execute {
                requests,
                select_result,
            },
        },
        other => other,
    }
}

/// Arrange Copies and Offset Lines dialogs.
pub fn windows(app: &mut EditorApp, ctx: &egui::Context) {
    app.edit.alt = ctx.input(|i| i.modifiers.alt);
    arrange_window(app, ctx);
    offset_window(app, ctx);
    floral_window(app, ctx);
}

fn arrange_window(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = app.edit.arrange_open;
    let mut apply = false;
    egui::Window::new("Arrange Copies")
        .open(&mut open)
        .resizable(false)
        .default_width(300.0)
        .show(ctx, |ui| {
            let f = &mut app.edit.arrange;
            egui::ComboBox::from_label("Arrangement")
                .selected_text(f.mode.label())
                .show_ui(ui, |ui| {
                    for m in ArrangeMode::ALL {
                        ui.selectable_value(&mut f.mode, m, m.label());
                    }
                });
            egui::Grid::new("arrange")
                .num_columns(2)
                .show(ui, |ui| match f.mode {
                    ArrangeMode::Circle => {
                        ui.label("Patterns");
                        ui.add(egui::DragValue::new(&mut f.count).range(2..=360));
                        ui.end_row();
                        ui.label("Radius");
                        ui.add(
                            egui::DragValue::new(&mut f.radius)
                                .range(0.0..=1000.0)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                        ui.label("");
                        ui.checkbox(&mut f.rotate, "Turn copies");
                        ui.end_row();
                    }
                    ArrangeMode::Matrix => {
                        ui.label("Rows × columns");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut f.rows).range(1..=100));
                            ui.add(egui::DragValue::new(&mut f.columns).range(1..=100));
                        });
                        ui.end_row();
                        ui.label("Spacing x / y");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut f.spacing_x)
                                    .range(-100.0..=1000.0)
                                    .suffix(" mm"),
                            );
                            ui.add(
                                egui::DragValue::new(&mut f.spacing_y)
                                    .range(-100.0..=1000.0)
                                    .suffix(" mm"),
                            );
                        });
                        ui.end_row();
                    }
                    _ => {
                        ui.label("Spacing");
                        ui.add(
                            egui::DragValue::new(&mut f.spacing)
                                .range(-100.0..=1000.0)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                    }
                });
            ui.checkbox(&mut f.merge, "Merge overlapping shapes");
            ui.checkbox(&mut f.optimize, "Optimize sewing order");
            apply = ui
                .add_enabled(!app.selection.is_empty(), egui::Button::new("Arrange"))
                .clicked();
        });
    if apply {
        let r = app.edit.arrange.request(&app.selection);
        if let Some(result) = app.execute(&r) {
            let mut sel: BTreeSet<u64> = CommandRequest::created_ids(&result);
            if let Some(all) = result.get("objects").and_then(|v| v.as_array()) {
                sel = all.iter().filter_map(|v| v.as_u64()).collect();
            }
            app.selection = sel;
        }
    }
    app.edit.arrange_open = open;
}

fn offset_window(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = app.edit.offset_open;
    let mut apply = false;
    egui::Window::new("Offset Lines")
        .open(&mut open)
        .resizable(false)
        .default_width(260.0)
        .show(ctx, |ui| {
            let f = &mut app.edit.offset;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut f.inward, false, "Outward");
                ui.selectable_value(&mut f.inward, true, "Inward");
            });
            egui::Grid::new("offset").num_columns(2).show(ui, |ui| {
                ui.label("Spacing");
                ui.add(
                    egui::DragValue::new(&mut f.spacing)
                        .range(0.1..=100.0)
                        .suffix(" mm"),
                );
                ui.end_row();
                ui.label("Lines");
                ui.add(egui::DragValue::new(&mut f.count).range(1..=20));
                ui.end_row();
            });
            ui.checkbox(&mut f.rounded, "Rounded corners");
            apply = ui
                .add_enabled(single(app).is_some(), egui::Button::new("Create"))
                .clicked();
        });
    if apply && let Some(id) = single(app) {
        let r = app.edit.offset.request(id);
        run_and_select(app, r);
    }
    app.edit.offset_open = open;
}

fn floral_window(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = app.edit.floral_open;
    let mut apply = false;
    egui::Window::new("Floral Pattern")
        .open(&mut open)
        .resizable(false)
        .default_width(260.0)
        .show(ctx, |ui| {
            let f = &mut app.edit.floral;
            ui.add(egui::Slider::new(&mut f.density, 1..=10).text("Density"));
            ui.add(egui::Slider::new(&mut f.pattern, 0..=251).text("Pattern"));
            apply = ui
                .add_enabled(single(app).is_some(), egui::Button::new("Create"))
                .clicked();
        });
    if apply && let Some(id) = single(app) {
        let r = app.edit.floral.request(id);
        run_and_select(app, r);
    }
    app.edit.floral_open = open;
}

/// Buttons under the sewing-order list: move the selection, sort, optimise, and jump-stitch
/// trimming for the selection (multi-needle machines).
pub fn order_controls(app: &mut EditorApp, ui: &mut egui::Ui) -> Option<CommandRequest> {
    let mut out = None;
    let any = !app.selection.is_empty();
    ui.horizontal_wrapped(|ui| {
        for (label, to, hint) in [
            ("First", "first", "Sew first"),
            ("▲", "earlier", "Sew earlier"),
            ("▼", "later", "Sew later"),
            ("Last", "last", "Sew last"),
        ] {
            if ui
                .add_enabled(any, egui::Button::new(label))
                .on_hover_text(hint)
                .clicked()
            {
                out = Some(objects::move_to(&app.selection, to));
            }
        }
        if ui
            .button("Sort by colour")
            .on_hover_text("Same-colour objects sew together")
            .clicked()
        {
            out = Some(CommandRequest::new("order.byColor", serde_json::json!({})));
        }
        if ui
            .button("Optimise")
            .on_hover_text("Group colours and shorten travel; overlapping objects keep their order")
            .clicked()
        {
            out = Some(CommandRequest::new("order.optimize", serde_json::json!({})));
        }
    });
    let d = app.session.design();
    if d.machine.machine == MachineType::MultiNeedle && any {
        let selected: Vec<_> = d
            .objects
            .iter()
            .filter(|o| app.selection.contains(&o.id))
            .collect();
        let mut on = selected.iter().any(|o| o.trim_jumps != Some(false));
        let mixed = on && selected.iter().any(|o| o.trim_jumps == Some(false));
        let label = if mixed {
            "Jump stitch trimming (mixed)"
        } else {
            "Jump stitch trimming"
        };
        if ui.checkbox(&mut on, label).changed() {
            out = Some(CommandRequest::new(
                "object.setJumpTrimming",
                serde_json::json!({ "ids": CommandRequest::ids(&app.selection), "enabled": on }),
            ));
        }
    }
    out
}

/// The sewing order as colour frames: click selects a frame's objects; the arrows move it past
/// the neighbouring frame.
pub fn colour_frames(app: &mut EditorApp, ui: &mut egui::Ui) {
    let d = app.session.design();
    let frames = objects::colour_frames(d);
    let rows = objects::rows(d, &app.selection);
    let mut pending = None;
    let mut select = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 80.0)
        .show(ui, |ui| {
            for (k, frame) in frames.iter().enumerate() {
                let first = rows.iter().find(|r| r.id == frame[0]);
                let selected = frame.iter().all(|id| app.selection.contains(id));
                ui.horizontal(|ui| {
                    if let Some(c) = first.and_then(|r| r.swatches.first()) {
                        crate::panels::swatch(ui, *c, 12.0);
                    }
                    let text = match frame.len() {
                        1 => format!("{}. {}", k + 1, first.map_or("", |r| r.label.as_str())),
                        n => format!("{}. {n} objects", k + 1),
                    };
                    if ui.selectable_label(selected, text).clicked() {
                        select = Some(frame.clone());
                    }
                    if selected {
                        if ui.small_button("▲").on_hover_text("Sew earlier").clicked() {
                            pending = objects::shift_frame(&frames, k, -1);
                        }
                        if ui.small_button("▼").on_hover_text("Sew later").clicked() {
                            pending = objects::shift_frame(&frames, k, 1);
                        }
                    }
                });
            }
        });
    if let Some(ids) = select {
        app.selection = ids.into_iter().collect();
    }
    if let Some(r) = order_controls(app, ui).or(pending) {
        app.execute(&r);
    }
}

/// R1 / R2 / L1 markers of the selected shape.
pub fn draw_markers(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    let Some(id) = single(app) else { return };
    let d = app.session.design();
    let screen = |p: Vec2| {
        let s = vp.to_screen(p);
        Pos2::new(s.x as f32, s.y as f32)
    };
    let purple = Color32::from_rgb(0x8e, 0x24, 0xaa);
    let (center, reference, curve) = edit_ops::direction_markers(d, id);
    if let Some(path) = curve {
        let pts: Vec<Pos2> = path.into_iter().map(screen).collect();
        painter.add(egui::Shape::line(
            pts,
            Stroke::new(2.0, Color32::from_rgb(0xd3, 0x2f, 0x2f)),
        ));
    }
    if let Some(c) = center.map(screen) {
        for (a, b) in [
            (egui::vec2(-7.0, 0.0), egui::vec2(7.0, 0.0)),
            (egui::vec2(0.0, -7.0), egui::vec2(0.0, 7.0)),
        ] {
            painter.line_segment([c + a, c + b], Stroke::new(2.0, purple));
        }
    }
    if let Some(c) = reference.map(screen) {
        painter.rect_stroke(
            egui::Rect::from_center_size(c, egui::vec2(9.0, 9.0)),
            0.0,
            Stroke::new(2.0, purple),
            egui::StrokeKind::Middle,
        );
    }
    // Stamps: engraved red, embossed blue (PE-DESIGN's colours).
    for (embossed, paths) in edit_ops::stamp_outlines(d, id) {
        let colour = if embossed {
            Color32::from_rgb(0x1e, 0x63, 0xd6)
        } else {
            Color32::from_rgb(0xd3, 0x2f, 0x2f)
        };
        for path in paths {
            let pts: Vec<Pos2> = path
                .into_iter()
                .map(|p| {
                    let s = vp.to_screen(p);
                    Pos2::new(s.x as f32, s.y as f32)
                })
                .collect();
            painter.add(egui::Shape::line(pts, Stroke::new(1.5, colour)));
        }
    }
    for (which, p) in edit_ops::entry_exit_markers(d, id) {
        let s = vp.to_screen(p);
        let c = Pos2::new(s.x as f32, s.y as f32);
        let colour = match which {
            EntryExit::RegionEntry | EntryExit::LineEntry => Color32::from_rgb(0x2e, 0x7d, 0x32),
            EntryExit::RegionExit => Color32::from_rgb(0xc6, 0x28, 0x28),
        };
        painter.circle_filled(c, 4.5, colour);
        painter.circle_stroke(c, 4.5, Stroke::new(1.0, Color32::WHITE));
        painter.text(
            c + egui::vec2(6.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            which.label(),
            egui::FontId::proportional(11.0),
            colour,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Launch;
    use openstitch_app::editor::{Phase, PointerEvent};
    use openstitch_design::v2;
    use serde_json::json;

    fn rect(app: &mut EditorApp, x: f64) {
        app.execute(&CommandRequest::new(
            "shape.rectangle",
            json!({ "x": x, "y": 10, "width": 20, "height": 20 }),
        ));
    }

    #[test]
    fn clicking_a_group_member_selects_the_group() {
        let mut app = EditorApp::new(Launch::default());
        rect(&mut app, 10.0);
        rect(&mut app, 50.0);
        app.selection = BTreeSet::from([1, 2]);
        group(&mut app);
        app.selection.clear();
        let click = |app: &mut EditorApp, at: Vec2| {
            for phase in [Phase::Down, Phase::Up] {
                app.with_tools(|t, c| t.handle(PointerEvent::new(phase, at), c));
            }
        };
        click(&mut app, v2(200.0, 200.0));
        assert_eq!(app.selection, BTreeSet::from([1, 2]));
        app.selection.clear();
        app.edit.alt = true;
        click(&mut app, v2(200.0, 200.0));
        assert_eq!(app.selection, BTreeSet::from([1]));
        app.edit.alt = false;
        app.selection = BTreeSet::from([1, 2]);
        ungroup(&mut app);
        app.selection.clear();
        click(&mut app, v2(200.0, 200.0));
        assert_eq!(app.selection, BTreeSet::from([1]));
    }

    #[test]
    fn split_with_outline_uses_the_polygon_tool() {
        let mut app = EditorApp::new(Launch::default());
        rect(&mut app, 10.0);
        app.edit.split_target = Some(1);
        app.tools.set_tool(Tool::Polygon);
        for p in [
            v2(50.0, 50.0),
            v2(200.0, 50.0),
            v2(200.0, 400.0),
            v2(50.0, 400.0),
        ] {
            for phase in [Phase::Down, Phase::Up] {
                app.with_tools(|t, c| t.handle(PointerEvent::new(phase, p), c));
            }
        }
        let effects = app.tools.finish();
        app.apply_effects(effects);
        assert_eq!(app.session.design().objects.len(), 2, "{}", app.status);
        assert_eq!(app.tools.tool(), Tool::Select);
        assert!(app.edit.split_target.is_none());
        app.undo();
        assert_eq!(app.session.design().objects.len(), 1);
    }

    #[test]
    fn picking_sets_entry_points_and_escape_cancels() {
        let mut app = EditorApp::new(Launch::default());
        rect(&mut app, 10.0);
        app.edit.pick = Some((1, Pick::Point(EntryExit::RegionEntry)));
        assert!(pick(&mut app, v2(90.0, 200.0)));
        let r = app.session.design().objects[0]
            .region
            .clone()
            .expect("region");
        assert_eq!(r.entry, Some(v2(100.0, 200.0)));
        assert!(!pick(&mut app, v2(0.0, 0.0)));
        app.edit.pick = Some((1, Pick::Point(EntryExit::RegionExit)));
        assert!(app.edit.cancel());
        assert!(!app.edit.cancel());
    }

    #[test]
    fn pen_draws_the_curved_direction() {
        let mut app = EditorApp::new(Launch::default());
        rect(&mut app, 10.0);
        app.edit.curve_target = Some(1);
        app.tools.set_tool(Tool::Pen);
        for p in [v2(50.0, 200.0), v2(350.0, 150.0)] {
            for phase in [Phase::Down, Phase::Up] {
                app.with_tools(|t, c| t.handle(PointerEvent::new(phase, p), c));
            }
        }
        let effects = app.tools.finish();
        app.apply_effects(effects);
        let r = app.session.design().objects[0]
            .region
            .clone()
            .expect("region");
        assert!(r.direction_curve.is_some(), "{}", app.status);
        assert_eq!(app.session.design().objects.len(), 1, "no new shape");
        assert_eq!(app.tools.tool(), Tool::Select);
        app.edit.pick = Some((1, Pick::Center));
        assert!(pick(&mut app, v2(150.0, 150.0)));
        assert!(
            app.status.contains("failed") || app.status.contains("no such"),
            "fill has no centre: {}",
            app.status
        );
    }

    #[test]
    fn stamp_pick_places_a_stamp() {
        let mut app = EditorApp::new(Launch::default());
        rect(&mut app, 10.0);
        app.edit.stamp_pick = Some((1, "star", "engravedRegion"));
        assert!(pick(&mut app, v2(200.0, 200.0)));
        let r = app.session.design().objects[0]
            .region
            .clone()
            .expect("region");
        assert_eq!(r.stamps.len(), 1);
        assert!(app.edit.stamp_pick.is_none());
    }
}
