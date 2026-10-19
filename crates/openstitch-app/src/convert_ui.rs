//! The Convert menu (shape/text ↔ stitch pattern conversions, region → line) and the Stitches
//! menu (divide by colour, split at the selected needle point, connect two patterns, split off
//! an area). Each item is one engine command (one Undo); the result stays selected.

use crate::app::EditorApp;
use eframe::egui;
use openstitch_app::editor::CommandRequest;
use openstitch_app::editor::conversions::{self, TO_SHAPES};
use openstitch_app::editor::tools::Tool;
use openstitch_design::ObjectKind;

fn run(app: &mut EditorApp, r: CommandRequest) {
    if let Some(result) = app.execute(&r) {
        let ids = CommandRequest::created_ids(&result);
        if !ids.is_empty() {
            app.selection = ids;
        }
        app.status = match result.get("deleted").and_then(|d| d.as_array()) {
            Some(d) if !d.is_empty() => format!(
                "Converted; {} objects sewed nothing and were deleted",
                d.len()
            ),
            _ => "Converted".into(),
        };
    }
}

pub fn convert_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.menu_button("Convert", |ui| {
        let (outline, region, stitches) = conversions::kinds(app.session.design(), &app.selection);
        if ui
            .add_enabled(outline, egui::Button::new("Convert to Stitch Pattern"))
            .on_hover_text("Shapes and text become needle points (groups convert together)")
            .clicked()
        {
            let r = conversions::to_stitches(&app.selection);
            run(app, r);
        }
        if ui
            .add_enabled(region, egui::Button::new("Convert Region to Line"))
            .on_hover_text("The outline is sewn with a running stitch in the region's colour")
            .clicked()
        {
            let r = conversions::region_to_line(&app.selection);
            run(app, r);
        }
        ui.separator();
        for (label, mode) in TO_SHAPES {
            if ui
                .add_enabled(stitches, egui::Button::new(label))
                .on_hover_text("Traced from the area the stitches cover (approximation)")
                .clicked()
            {
                let r = conversions::to_shapes(app.session.design(), &app.selection, mode);
                run(app, r);
            }
        }
    });
}

pub fn stitches_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.menu_button("Stitches", |ui| {
        let d = app.session.design();
        let stitches: Vec<u64> = app
            .selection
            .iter()
            .copied()
            .filter(|&id| {
                d.index_of(id)
                    .is_some_and(|i| d.objects[i].kind == ObjectKind::Stitches)
            })
            .collect();
        let one = (stitches.len() == 1 && app.selection.len() == 1).then(|| stitches[0]);
        let needles = {
            let design = d.clone();
            app.tools.selected_needles(&app.tool_context(&design))
        };
        if ui
            .add_enabled(!stitches.is_empty(), egui::Button::new("Divide by Colour"))
            .clicked()
        {
            let r = conversions::divide_by_color(&stitches);
            run(app, r);
        }
        if ui
            .add_enabled(
                one.is_some() && needles.len() == 1,
                egui::Button::new("Split at Point"),
            )
            .on_hover_text("Select one needle point with the Point tool (A) first")
            .clicked()
            && let (Some(id), Some(&index)) = (one, needles.first())
        {
            run(app, conversions::split_at(id, index));
        }
        if ui
            .add_enabled(stitches.len() == 2, egui::Button::new("Connect"))
            .on_hover_text("The pattern sewn later continues from the earlier one's exit point")
            .clicked()
        {
            let r = conversions::connect(&stitches);
            run(app, r);
        }
        if ui
            .add_enabled(one.is_some(), egui::Button::new("Split Off Area"))
            .clicked()
        {
            app.edit.split_target = one;
            app.tools.set_tool(Tool::Polygon);
            app.status =
                "Draw around the stitches to split off; Return or double-click splits, Esc cancels"
                    .into();
        }
    });
}
