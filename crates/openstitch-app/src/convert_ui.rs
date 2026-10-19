//! Conversions (shape/text ↔ stitch pattern, region → line) and stitch pattern edits (divide
//! by colour, split at the selected needle point, connect two patterns, split off an area),
//! run from the Text, Shapes and Stitches ribbon tabs. Each item is one engine command (one Undo); the result stays selected.

use crate::app::EditorApp;
use openstitch_app::editor::CommandRequest;
use openstitch_app::editor::conversions;
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

/// What the selection holds: (shapes or text, any with region sewing, stitch patterns).
pub fn kinds(app: &EditorApp) -> (bool, bool, bool) {
    conversions::kinds(app.session.design(), &app.selection)
}

/// Convert to Stitches: shapes and text become needle points (groups convert together).
pub fn to_stitches(app: &mut EditorApp) {
    let r = conversions::to_stitches(&app.selection);
    run(app, r);
}

/// Convert Region to Line: the outline is sewn with a running stitch in the region's colour.
pub fn region_to_line(app: &mut EditorApp) {
    let r = conversions::region_to_line(&app.selection);
    run(app, r);
}

/// Stitch patterns traced back into shapes; `mode` is one of `conversions::TO_SHAPES`.
pub fn to_shapes(app: &mut EditorApp, mode: &str) {
    let r = conversions::to_shapes(app.session.design(), &app.selection, mode);
    run(app, r);
}

/// The selected text objects.
pub fn texts(app: &EditorApp) -> Vec<u64> {
    let d = app.session.design();
    app.selection
        .iter()
        .copied()
        .filter(|&id| {
            d.index_of(id)
                .is_some_and(|i| d.objects[i].kind == ObjectKind::Text)
        })
        .collect()
}

/// Text ▸ Convert to Outline: one shape per character.
pub fn text_to_shapes(app: &mut EditorApp) {
    let ids: std::collections::BTreeSet<u64> = texts(app).into_iter().collect();
    if !ids.is_empty() {
        let r = CommandRequest::new(
            "text.toShapes",
            serde_json::json!({ "ids": CommandRequest::ids(&ids) }),
        );
        run(app, r);
    }
}

/// The selected stitch patterns, the single one when only it is selected, and the needle
/// points selected with the Point tool.
pub fn stitch_selection(app: &EditorApp) -> (Vec<u64>, Option<u64>, Vec<usize>) {
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
        app.tools
            .selected_needles(&app.tool_context(&design))
            .into_iter()
            .collect()
    };
    (stitches, one, needles)
}

pub fn divide_by_color(app: &mut EditorApp) {
    let (stitches, _, _) = stitch_selection(app);
    if !stitches.is_empty() {
        run(app, conversions::divide_by_color(&stitches));
    }
}

/// Split at Point: needs one needle point selected with the Point tool.
pub fn split_at_point(app: &mut EditorApp) {
    if let (_, Some(id), needles) = stitch_selection(app)
        && let [index] = needles[..]
    {
        run(app, conversions::split_at(id, index));
    }
}

pub fn connect(app: &mut EditorApp) {
    let (stitches, _, _) = stitch_selection(app);
    if stitches.len() == 2 {
        run(app, conversions::connect(&stitches));
    }
}

/// Split Stitch Data: the next polygon drawn splits off the stitches inside it.
pub fn split_off_area(app: &mut EditorApp) {
    let (_, one, _) = stitch_selection(app);
    if one.is_some() {
        app.edit.split_target = one;
        app.tools.set_tool(Tool::Polygon);
        app.status =
            "Draw around the stitches to split off; Return or double-click splits, Esc cancels"
                .into();
    }
}
