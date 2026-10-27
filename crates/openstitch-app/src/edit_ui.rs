//! Shapes menu (presets, arcs, merge, remove overlap, combine, split, offset lines, entry/exit
//! points), the group / arrange / attribute items of the Edit menu, their dialogs and canvas
//! markers. Requests come from `openstitch_app::editor::edit_ops`; each action is one command,
//! so one Undo.

use crate::app::EditorApp;
use eframe::egui::{self, Color32, Pos2, Stroke};
use openstitch_app::editor::edit_ops::{
    self, ArrangeForm, ArrangeMode, EntryExit, FloralForm, OffsetForm, Pick,
};
use openstitch_app::editor::hit::bounds_of;
use openstitch_app::editor::objects;
use openstitch_app::editor::tools::snap_to_grid;
use openstitch_app::editor::{CommandRequest, Phase, SelectionHandles, Tool, ToolEffect, Viewport};
use openstitch_design::{Contour, MachineType, ObjectKind, Vec2};
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
    /// The Arrange Copies handle is being dragged on the canvas.
    pub arrange_drag: bool,
    /// An entry/exit marker being dragged: object, which marker, where it is now.
    pub marker_drag: Option<(u64, EntryExit, Vec2)>,
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

/// Copy Attributes: remembers the single selected shape or text.
pub fn copy_attributes_source(app: &EditorApp) -> Option<u64> {
    single(app).filter(|&id| kind_of(app, id) != Some(ObjectKind::Stitches))
}

pub fn copy_attributes(app: &mut EditorApp) {
    if let Some(source) = copy_attributes_source(app) {
        app.edit.attributes_from = Some(source);
        app.status = "Sewing attributes copied; select the targets and Paste Attributes".into();
    }
}

/// The object whose attributes Paste Attributes would give the selection.
pub fn paste_attributes_source(app: &EditorApp) -> Option<u64> {
    app.edit
        .attributes_from
        .filter(|f| app.selection.iter().any(|id| id != f))
}

pub fn paste_attributes(app: &mut EditorApp) {
    if let Some(from) = paste_attributes_source(app) {
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

pub fn can_ungroup(app: &EditorApp) -> bool {
    edit_ops::any_grouped(app.session.design(), &app.selection)
}

/// The single selected shape object.
pub fn single_shape(app: &EditorApp) -> Option<u64> {
    single(app).filter(|&id| kind_of(app, id) == Some(ObjectKind::Shape))
}

/// The single selected shape or text (Offset Lines, Floral Pattern).
pub fn single_outline(app: &EditorApp) -> Option<u64> {
    single(app).filter(|&id| matches!(kind_of(app, id), Some(ObjectKind::Shape | ObjectKind::Text)))
}

/// The single selected shape with region sewing (sewing direction, centre point).
pub fn single_region(app: &EditorApp) -> Option<u64> {
    single_shape(app).filter(|&id| {
        let d = app.session.design();
        d.index_of(id)
            .is_some_and(|i| d.objects[i].region.is_some())
    })
}

/// Shapes ▸ Insert: preset shapes placed in the middle of the Design Page.
pub fn insert_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    for (label, kind) in edit_ops::PRESETS {
        if ui.button(label).clicked() {
            let center = app.session.design().page.center();
            run_and_select(app, edit_ops::insert_preset(kind, center, 30.0));
        }
    }
}

/// Modify Overlap: merge, remove overlap, combine.
pub fn overlap_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
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
    ui.separator();
    for (label, op) in [
        ("Union", "union"),
        ("Subtract from first", "difference"),
        ("Intersect", "intersection"),
        ("Exclude overlap", "xor"),
    ] {
        if ui.add_enabled(many, egui::Button::new(label)).clicked() {
            let r = edit_ops::combine(app.session.design(), &app.selection, op);
            run_and_select(app, r);
        }
    }
}

/// Split Outline: the next polygon drawn cuts the selected shape.
pub fn split_outline(app: &mut EditorApp) {
    if let Some(shape) = single_shape(app) {
        app.edit.split_target = Some(shape);
        app.tools.set_tool(Tool::Polygon);
        app.status = "Draw the cut outline; Return or double-click splits, Esc cancels".into();
    }
}

pub fn decorative_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.checkbox(&mut app.edit.frame_keep_aspect, "Maintain aspect ratio");
    ui.separator();
    for (label, kind) in edit_ops::FRAMES {
        if ui.button(label).clicked() {
            let r = edit_ops::decorative(kind, &app.selection, app.edit.frame_keep_aspect);
            run_and_select(app, r);
        }
    }
}

/// Stamp/Emboss: emboss/engrave with a second shape, or place stamp patterns.
pub fn stamp_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    let shape = single_shape(app);
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
    ui.add_enabled_ui(shape.is_some(), |ui| {
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
}

pub fn entry_exit_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    let Some(id) = single_shape(app) else {
        ui.label("Select one shape");
        return;
    };
    for (label, which) in [
        ("Set region entry (R1)", EntryExit::RegionEntry),
        ("Set region exit (R2)", EntryExit::RegionExit),
        ("Set line entry (L1)", EntryExit::LineEntry),
    ] {
        if ui.button(label).clicked() {
            app.edit.pick = Some((id, Pick::Point(which)));
            app.status = format!(
                "Click the outline where {} should be; Esc cancels",
                which.label()
            );
        }
    }
    if ui.button("Reset to automatic").clicked() {
        app.execute(&edit_ops::clear_entry_exit(id));
    }
}

/// Add Direction Line: the next open Pen path becomes the region's curved direction.
pub fn add_direction(app: &mut EditorApp) {
    if let Some(id) = single_region(app) {
        app.edit.curve_target = Some(id);
        app.tools.set_tool(Tool::Pen);
        app.status = "Draw an open curve across the shape; Return finishes, Esc cancels".into();
    }
}

pub fn delete_direction(app: &mut EditorApp) {
    if let Some(id) = single_region(app) {
        app.execute(&edit_ops::straight_direction(id));
    }
}

/// Centre point and reference point of the region's stitch pattern.
pub fn sewing_points_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    let Some(id) = single_region(app) else {
        ui.label("Select one shape with region sewing");
        return;
    };
    for (label, which) in [
        ("Set centre point", Pick::Center),
        ("Set reference point", Pick::Reference),
    ] {
        if ui.button(label).clicked() {
            app.edit.pick = Some((id, which));
            app.status = format!("Click where {} should be; Esc cancels", which.label());
        }
    }
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

/// Canvas drags the editor handles before the tools: the Arrange Copies handle while the
/// dialog is open, and the entry/exit markers of the selected shape (Select tool). A marker
/// drag ends in one `object.setEntryExit`, so one Undo. Whether the event was consumed.
pub fn drag(app: &mut EditorApp, phase: Phase, at: Vec2) -> bool {
    let radius =
        SelectionHandles::RADIUS * 1.4 * app.viewport.unwrap_or_default().units_per_point();
    let snapped = app
        .workspace
        .grid
        .snap_step()
        .map_or(at, |s| snap_to_grid(at, s));
    let bounds = bounds_of(app.session.design(), &app.selection);
    if app.edit.arrange_drag {
        if let (Phase::Dragged | Phase::Up, Some(b)) = (phase, bounds) {
            app.edit.arrange.drag_handle(&b, snapped);
        }
        app.edit.arrange_drag = matches!(phase, Phase::Dragged | Phase::Moved);
        return true;
    }
    if let Some((id, which, _)) = app.edit.marker_drag {
        match phase {
            Phase::Dragged | Phase::Moved => app.edit.marker_drag = Some((id, which, at)),
            Phase::Up => {
                app.edit.marker_drag = None;
                let moved = edit_ops::entry_exit_markers(app.session.design(), id)
                    .into_iter()
                    .any(|(w, p)| w == which && p.distance(at) > radius / 2.0);
                if moved && app.execute(&which.set(id, at)).is_some() {
                    app.status = format!("{} moved", which.label());
                }
            }
            Phase::Down => app.edit.marker_drag = None,
        }
        return true;
    }
    if phase != Phase::Down {
        return false;
    }
    if app.edit.arrange_open
        && let Some(b) = bounds
        && app
            .edit
            .arrange
            .handle(&b)
            .is_some_and(|h| h.distance(at) <= radius)
    {
        app.edit.arrange_drag = true;
        return true;
    }
    if app.tools.tool() == Tool::Select
        && let Some(id) = single(app)
        && let Some((which, p)) = edit_ops::entry_exit_at(app.session.design(), id, at, radius)
    {
        app.edit.marker_drag = Some((id, which, p));
        return true;
    }
    false
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
    let bounds = bounds_of(app.session.design(), &app.selection);
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
                        if ui
                            .add(
                                egui::DragValue::new(&mut f.radius)
                                    .range(0.0..=1000.0)
                                    .suffix(" mm"),
                            )
                            .changed()
                            && let Some(b) = bounds
                        {
                            f.radius_changed(&b);
                        }
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
            ui.weak("Drag the handle on the canvas to set the spacing or circle centre.");
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

/// Buttons at the top of the Sewing Order pane: move the selection, sort, optimise, and jump-stitch
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
            .on_hover_text("Group colours (drawing order within a colour) and link entry and exit points; overlapping objects keep their order")
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
    if let Some(r) = pending {
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
        let p = match app.edit.marker_drag {
            Some((i, w, at)) if i == id && w == which => at,
            _ => p,
        };
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

/// While Arrange Copies is open: the copies it would make, faded, and its drag handle.
pub fn draw_arrange_preview(app: &EditorApp, painter: &egui::Painter, vp: &Viewport) {
    if !app.edit.arrange_open {
        return;
    }
    let d = app.session.design();
    let Some(b) = bounds_of(d, &app.selection) else {
        return;
    };
    let screen = |p: Vec2| {
        let s = vp.to_screen(p);
        Pos2::new(s.x as f32, s.y as f32)
    };
    let accent = Color32::from_rgb(0x1e, 0x88, 0xe5);
    let stroke = Stroke::new(1.2, accent.gamma_multiply(0.75));
    let selected: Vec<_> = d
        .objects
        .iter()
        .filter(|o| app.selection.contains(&o.id))
        .collect();
    for t in app.edit.arrange.transforms(&b) {
        for o in &selected {
            let paths: Vec<Vec<Vec2>> = match o.kind {
                ObjectKind::Stitches => o
                    .bounds()
                    .map(|r| Contour::rectangle(r.min_x, r.min_y, r.width(), r.height()).path())
                    .into_iter()
                    .collect(),
                _ => o.contours.iter().map(|c| c.path()).collect(),
            };
            for path in paths {
                let pts: Vec<Pos2> = path.into_iter().map(|p| screen(t.apply(p))).collect();
                painter.add(egui::Shape::dashed_line(&pts, stroke, 5.0, 3.0));
            }
        }
    }
    if let Some(h) = app.edit.arrange.handle(&b).map(screen) {
        let r = SelectionHandles::RADIUS as f32;
        painter.circle(h, r, Color32::WHITE, Stroke::new(1.5, accent));
        painter.line_segment([h - egui::vec2(r, 0.0), h + egui::vec2(r, 0.0)], stroke);
        painter.line_segment([h - egui::vec2(0.0, r), h + egui::vec2(0.0, r)], stroke);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Launch;
    use openstitch_app::editor::PointerEvent;
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

    fn gesture(app: &mut EditorApp, from: Vec2, to: Vec2) {
        for (phase, at) in [(Phase::Down, from), (Phase::Dragged, to), (Phase::Up, to)] {
            app.with_tools(|t, c| t.handle(PointerEvent::new(phase, at), c));
        }
    }

    fn bounds(app: &EditorApp, id: u64) -> openstitch_design::Rect {
        let d = app.session.design();
        d.objects[d.index_of(id).expect("object")]
            .bounds()
            .expect("bounds")
    }

    #[test]
    fn snap_to_grid_places_drawing_moves_and_points_on_the_grid() {
        let mut app = EditorApp::new(Launch::default());
        app.toggle_snap_to_grid();
        app.workspace.grid.spacing_mm = 10.0;
        app.tools.set_tool(Tool::Rectangle);
        gesture(&mut app, v2(93.0, 104.0), v2(296.0, 212.0));
        let b = bounds(&app, 1);
        assert_eq!(
            (b.min_x, b.min_y, b.max_x, b.max_y),
            (100.0, 100.0, 300.0, 200.0)
        );
        // A move the grid rounds away does nothing; a longer one lands on the next line.
        app.tools.set_tool(Tool::Select);
        gesture(&mut app, v2(150.0, 150.0), v2(183.0, 160.0));
        assert_eq!(bounds(&app, 1).min_x, 100.0);
        gesture(&mut app, v2(150.0, 150.0), v2(214.0, 163.0));
        let b = bounds(&app, 1);
        assert_eq!((b.min_x, b.min_y), (200.0, 100.0));
        app.tools.set_tool(Tool::Point);
        gesture(&mut app, v2(200.0, 100.0), v2(288.0, 113.0));
        let c = &app.session.design().objects[0].contours[0];
        assert!(c.points.contains(&v2(300.0, 100.0)), "{:?}", c.points);
        // Off: positions are taken as they are.
        app.toggle_snap_to_grid();
        app.tools.set_tool(Tool::Rectangle);
        gesture(&mut app, v2(503.0, 507.0), v2(611.0, 655.0));
        let b = bounds(&app, 2);
        assert_eq!((b.min_x, b.min_y), (503.0, 507.0));
    }

    #[test]
    fn dragging_an_entry_marker_is_one_undo() {
        let mut app = EditorApp::new(Launch::default());
        rect(&mut app, 10.0);
        app.selection = BTreeSet::from([1]);
        app.edit.pick = Some((1, Pick::Point(EntryExit::RegionEntry)));
        assert!(pick(&mut app, v2(90.0, 200.0)));
        let entry = |app: &EditorApp| {
            app.session.design().objects[0]
                .region
                .as_ref()
                .and_then(|r| r.entry)
        };
        assert_eq!(entry(&app), Some(v2(100.0, 200.0)));
        assert!(
            !drag(&mut app, Phase::Down, v2(200.0, 200.0)),
            "not on a marker"
        );
        assert!(drag(&mut app, Phase::Down, v2(102.0, 203.0)));
        assert!(drag(&mut app, Phase::Dragged, v2(210.0, 320.0)));
        assert_eq!(
            entry(&app),
            Some(v2(100.0, 200.0)),
            "unchanged until released"
        );
        assert!(drag(&mut app, Phase::Up, v2(210.0, 320.0)));
        assert!(app.edit.marker_drag.is_none());
        let moved = entry(&app).expect("entry");
        assert_eq!(moved.y, 300.0, "on the outline: {moved:?}");
        app.undo();
        assert_eq!(entry(&app), Some(v2(100.0, 200.0)));
    }

    fn preview_bounds(app: &EditorApp) -> Vec<[i64; 4]> {
        let b = bounds(app, 1);
        let mut out: Vec<[i64; 4]> = app
            .edit
            .arrange
            .transforms(&b)
            .iter()
            .map(|t| {
                let corners = [
                    v2(b.min_x, b.min_y),
                    v2(b.max_x, b.min_y),
                    v2(b.max_x, b.max_y),
                    v2(b.min_x, b.max_y),
                ]
                .map(|p| t.apply(p));
                let r = openstitch_design::Rect::enclosing(corners).expect("rect");
                [r.min_x, r.min_y, r.max_x, r.max_y].map(|v| v.round() as i64)
            })
            .collect();
        out.sort();
        out
    }

    /// Runs the arrangement and returns the copies' bounds; undoes it again.
    fn applied_bounds(app: &mut EditorApp) -> Vec<[i64; 4]> {
        let result = app
            .execute(&app.edit.arrange.request(&app.selection))
            .expect("arranged");
        let mut out: Vec<[i64; 4]> = CommandRequest::created_ids(&result)
            .into_iter()
            .map(|id| {
                let r = bounds(app, id);
                [r.min_x, r.min_y, r.max_x, r.max_y].map(|v| v.round() as i64)
            })
            .collect();
        out.sort();
        app.undo();
        out
    }

    #[test]
    fn arrange_preview_matches_the_copies_and_its_handle_drags() {
        let mut app = EditorApp::new(Launch::default());
        rect(&mut app, 10.0);
        app.selection = BTreeSet::from([1]);
        app.edit.arrange_open = true;
        // Mirror: the handle is the mirror axis; dragging it 5 mm right makes a 10 mm gap.
        app.edit.arrange.mode = ArrangeMode::MirrorHorizontal;
        let b = bounds(&app, 1);
        assert_eq!(app.edit.arrange.handle(&b), Some(v2(300.0, 200.0)));
        assert!(drag(&mut app, Phase::Down, v2(302.0, 199.0)));
        assert!(drag(&mut app, Phase::Dragged, v2(350.0, 230.0)));
        assert!(drag(&mut app, Phase::Up, v2(350.0, 230.0)));
        assert!(!app.edit.arrange_drag);
        assert_eq!(app.edit.arrange.spacing, 10.0);
        assert_eq!(preview_bounds(&app), vec![[400, 100, 600, 300]]);
        assert_eq!(preview_bounds(&app), applied_bounds(&mut app));
        assert_eq!(
            app.session.design().objects.len(),
            1,
            "dragging adds nothing"
        );
        // Circle: the handle is the centre; the radius follows it.
        app.edit.arrange.mode = ArrangeMode::Circle;
        app.edit.arrange.count = 4;
        assert!(drag(&mut app, Phase::Down, v2(200.0, 500.0)));
        assert!(drag(&mut app, Phase::Up, v2(200.0, 600.0)));
        assert_eq!(app.edit.arrange.radius, 40.0);
        assert_eq!(app.edit.arrange.center, Some(v2(20.0, 60.0)));
        assert_eq!(preview_bounds(&app).len(), 3);
        assert_eq!(preview_bounds(&app), applied_bounds(&mut app));
        app.edit.arrange.radius = 20.0;
        app.edit.arrange.radius_changed(&bounds(&app, 1));
        assert_eq!(app.edit.arrange.center, Some(v2(20.0, 40.0)));
        // Matrix: the handle is the last copy's corner; dragging sets the gap, never a stack.
        app.edit.arrange.mode = ArrangeMode::Matrix;
        app.edit.arrange.rows = 1;
        app.edit.arrange.columns = 3;
        assert_eq!(
            app.edit.arrange.handle(&bounds(&app, 1)),
            Some(v2(540.0, 100.0))
        );
        assert!(drag(&mut app, Phase::Down, v2(540.0, 100.0)));
        assert!(drag(&mut app, Phase::Up, v2(600.0, 100.0)));
        assert_eq!(app.edit.arrange.spacing_x, 5.0);
        assert_eq!(preview_bounds(&app), applied_bounds(&mut app));
        app.edit.arrange.drag_handle(&bounds(&app, 1), v2(0.0, 0.0));
        assert!(app.edit.arrange.spacing_x > -20.0);
        assert!(
            app.execute(&app.edit.arrange.request(&app.selection))
                .is_some()
        );
        // Closed: no handle to grab.
        app.undo();
        app.edit.arrange_open = false;
        assert!(!drag(&mut app, Phase::Down, v2(110.0, 100.0)));
    }
}
