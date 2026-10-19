//! The window chrome: Application button and menu, Quick Access Toolbar, ribbon tabs with
//! their groups (and the contextual tabs for the active tool or selection), the Option and
//! Help buttons. The layout is the model in [`openstitch_app::editor::ribbon`]; this module
//! draws it, decides which commands are enabled and runs them.

use crate::app::EditorApp;
use crate::{convert_ui, edit_ui, icons, panels};
use eframe::egui::{self, Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind, vec2};
use openstitch_app::editor::prefs::{Theme, UiPrefs};
use openstitch_app::editor::ribbon::{
    self, APPLICATION_MENU, Action, Facts, Item, OPTION_MENU, TabId,
};
use openstitch_app::editor::{CommandRequest, Tool, objects};
use openstitch_design::ObjectKind;
use openstitch_formats::FileType;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::PathBuf;

const MAC: bool = cfg!(target_os = "macos");
/// Height of the button area of a ribbon group (the caption goes under it).
const BODY: f32 = 68.0;
const LARGE_ICON: f32 = 30.0;
const SMALL_ICON: f32 = 16.0;
/// Colour of contextual tabs.
const CONTEXT: Color32 = Color32::from_rgb(0xe0, 0x8a, 0x1e);
pub const APP_BLUE: Color32 = Color32::from_rgb(0x1f, 0x5f, 0xb4);

/// The right-hand pane's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaneTab {
    Import,
    Color,
    #[default]
    Sewing,
    Text,
}

impl PaneTab {
    pub const ALL: [PaneTab; 4] = [
        PaneTab::Import,
        PaneTab::Color,
        PaneTab::Sewing,
        PaneTab::Text,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PaneTab::Import => "Import",
            PaneTab::Color => "Color",
            PaneTab::Sewing => "Sewing Attributes",
            PaneTab::Text => "Text Attributes",
        }
    }

    /// `--pane` value.
    pub fn from_key(key: &str) -> Option<PaneTab> {
        Some(match key {
            "import" => PaneTab::Import,
            "color" => PaneTab::Color,
            "attributes" => PaneTab::Sewing,
            "text" => PaneTab::Text,
            _ => return None,
        })
    }
}

/// Chrome state: active tab, preferences, the internal clipboard and the small windows.
pub struct RibbonState {
    pub tab: TabId,
    pub prefs: UiPrefs,
    /// Where the preferences are saved (`None` = not saved, e.g. tests and screenshots).
    pub prefs_path: Option<PathBuf>,
    /// Opens the Application menu on the next frame (`--show appmenu`).
    pub open_app_menu: bool,
    last_tool: Tool,
    /// Objects Copy remembered; Paste duplicates the ones still in the design.
    pub clipboard: BTreeSet<u64>,
    /// Pan: the primary button drags the view.
    pub pan: bool,
    pub property_open: bool,
    pub shortcuts_open: bool,
    pub about_open: bool,
    pub pane_tab: PaneTab,
    /// Edit Text: the characters being typed.
    pub text_draft: String,
    /// ScreenTip of the command under the pointer (status bar).
    pub hover: Option<&'static str>,
    applied_theme: Option<Theme>,
}

impl Default for RibbonState {
    fn default() -> Self {
        RibbonState {
            tab: TabId::Home,
            prefs: UiPrefs::default(),
            prefs_path: None,
            open_app_menu: false,
            last_tool: Tool::Select,
            clipboard: BTreeSet::new(),
            pan: false,
            property_open: false,
            shortcuts_open: false,
            about_open: false,
            pane_tab: PaneTab::default(),
            text_draft: String::new(),
            hover: None,
            applied_theme: None,
        }
    }
}

impl RibbonState {
    pub fn save_prefs(&self) {
        if let Some(p) = &self.prefs_path
            && let Err(e) = self.prefs.save(p)
        {
            eprintln!("cannot save {}: {e}", p.display());
        }
    }

    pub fn add_recent(&mut self, path: &std::path::Path) {
        self.prefs.add_recent(path);
        self.save_prefs();
    }

    /// Applies the colour theme when it changed.
    pub fn apply_theme(&mut self, ctx: &egui::Context) {
        let theme = self.prefs.theme;
        if self.applied_theme == Some(theme) {
            return;
        }
        crate::theme::install(ctx);
        ctx.set_theme(match theme {
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::System => egui::ThemePreference::System,
        });
        self.applied_theme = Some(theme);
    }
}

// MARK: selection facts

pub fn facts(app: &EditorApp) -> Facts {
    let d = app.session.design();
    let has = |k: ObjectKind| {
        app.selection
            .iter()
            .any(|&id| d.index_of(id).is_some_and(|i| d.objects[i].kind == k))
    };
    Facts {
        tool: app.tools.tool(),
        text: has(ObjectKind::Text),
        shape: has(ObjectKind::Shape),
        stitches: has(ObjectKind::Stitches),
        sections: crate::sections_ui::has_sections(app),
    }
}

/// Text and shape of a two-object selection, for Fit Text to outline.
pub fn fit_pair(app: &EditorApp) -> Option<(u64, u64)> {
    if app.selection.len() != 2 {
        return None;
    }
    let d = app.session.design();
    let kind = |id: u64| d.index_of(id).map(|i| d.objects[i].kind);
    let mut it = app.selection.iter().copied();
    let (a, b) = (it.next()?, it.next()?);
    match (kind(a), kind(b)) {
        (Some(ObjectKind::Text), Some(ObjectKind::Shape)) => Some((a, b)),
        (Some(ObjectKind::Shape), Some(ObjectKind::Text)) => Some((b, a)),
        _ => None,
    }
}

fn clipboard_ids(app: &EditorApp) -> BTreeSet<u64> {
    let d = app.session.design();
    app.ui
        .clipboard
        .iter()
        .copied()
        .filter(|id| d.index_of(*id).is_some())
        .collect()
}

/// Whether `a` can run now (implemented, and the selection suits it).
pub fn enabled(app: &EditorApp, a: Action) -> bool {
    use Action::*;
    if !a.info().available {
        return false;
    }
    let any = !app.selection.is_empty();
    let many = app.selection.len() >= 2;
    let (outline, region, stitches) = convert_ui::kinds(app);
    let texts = !convert_ui::texts(app).is_empty();
    let drawn = app.stats.stitches > 0;
    let background = app.session.design().background.is_some();
    match a {
        Undo => app.session.can_undo(),
        Redo => app.session.can_redo(),
        Copy | Duplicate | Delete | ArrangeCopy | Arrange | RotateFlip | Size | DecorativeFrame => {
            any
        }
        Paste => !clipboard_ids(app).is_empty(),
        CopyAttributes => edit_ui::copy_attributes_source(app).is_some(),
        PasteAttributes => edit_ui::paste_attributes_source(app).is_some(),
        Group => many,
        Ungroup => edit_ui::can_ungroup(app),
        StampEmboss => edit_ui::single_shape(app).is_some() || app.selection.len() == 2,
        ModifyOverlap => many,
        FitTextToOutline => fit_pair(app).is_some(),
        Export | Print | SendEmbroidery => drawn,
        ExportSections => crate::sections_ui::has_sections(app),
        ImageDensity | DeleteImage => background,
        SaveImage | ModifyImage | ImageRotateRight | ImageRotateLeft => background,
        Font | TextSize | EditText => texts,
        LineColor | LineSewType | RegionColor | RegionSewType => outline,
        ConvertToBlocks | ConvertToOutlines => stitches,
        ConvertToOutline => stitches || texts,
        ConvertToStitches => outline,
        ConvertRegionToLine => region,
        SplitOutline | EntryExit => edit_ui::single_shape(app).is_some(),
        AddDirectionLine | DeleteDirectionLine | SewingPoints => {
            edit_ui::single_region(app).is_some()
        }
        OffsetLines | FloralPattern => edit_ui::single_outline(app).is_some(),
        DeletePoint => app.tools.tool() == Tool::Point && any,
        SplitAtPoint => {
            let (_, one, needles) = convert_ui::stitch_selection(app);
            one.is_some() && needles.len() == 1
        }
        SplitStitchData => convert_ui::stitch_selection(app).1.is_some(),
        DivideByColor => stitches,
        Connect => convert_ui::stitch_selection(app).0.len() == 2,
        _ => true,
    }
}

/// Whether an on/off command or a tool is on.
pub fn active(app: &EditorApp, a: Action) -> bool {
    use Action::*;
    let panes = &app.ui.prefs.panes;
    if let Some(t) = a.tool() {
        return app.tools.tool() == t && !app.workspace.measure.active && !app.ui.pan;
    }
    match a {
        StitchView => !app.workspace.realistic && !app.workspace.solid,
        RealisticView => app.workspace.realistic,
        SolidView => app.workspace.solid && !app.workspace.realistic,
        GridAxes => app.workspace.grid.axes,
        Guideline => app.workspace.guides,
        FontFilter => app.ui.prefs.font_filter.on,
        ViewThreadTrimming => app.show_travel,
        PaneSewingOrder => panes.sewing_order.visible,
        PaneSimulator => panes.simulator.visible,
        PaneAttributes => panes.attributes.visible,
        PaneImport => panes.attributes.visible && app.ui.pane_tab == PaneTab::Import,
        ReferenceWindow => app.workspace.reference,
        ShowGrid => app.workspace.grid.visible,
        SnapToGrid => app.workspace.grid.snap,
        Ruler => app.workspace.rulers,
        Pan => app.ui.pan,
        Measure => app.workspace.measure.active,
        _ => false,
    }
}

// MARK: running commands

fn set_tool(app: &mut EditorApp, t: Tool) {
    app.tools.set_tool(t);
    app.workspace.measure.active = false;
    app.ui.pan = false;
}

/// Zooms so the view shows `percent` % (100 % = real size on a 96 dpi screen).
pub fn zoom_to_percent(app: &mut EditorApp, percent: f64) {
    if let Some(vp) = &mut app.viewport {
        let now = vp.zoom_percent(96.0 / 25.4);
        if now > 0.0 && percent > 0.0 {
            let anchor = app.workspace.canvas.map_or_else(
                || vp.to_screen(app.session.design().page.center()),
                |r| openstitch_design::v2(f64::from(r.center().x), f64::from(r.center().y)),
            );
            vp.zoom_about(anchor, percent / now);
        }
    }
}

fn zoom_to_selection(app: &mut EditorApp) {
    let d = app.session.design();
    if let (Some(b), Some(r)) = (
        openstitch_app::editor::hit::bounds_of(d, &app.selection),
        app.workspace.canvas,
    ) {
        app.viewport = Some(openstitch_app::editor::Viewport::fit(
            b,
            crate::workspace_ui::screen_rect(r),
            48.0,
        ));
    }
}

fn paste(app: &mut EditorApp) {
    let ids = clipboard_ids(app);
    if ids.is_empty() {
        return;
    }
    if let Some(result) = app.execute(&CommandRequest::duplicate(&ids)) {
        let created = CommandRequest::created_ids(&result);
        if !created.is_empty() {
            app.selection = created;
        }
        app.status = "Pasted".into();
    }
}

fn delete(app: &mut EditorApp) {
    let mut point_deleted = false;
    app.with_tools(|t, c| {
        let effects = t.delete_point(c);
        point_deleted = effects.is_some();
        effects.unwrap_or_default()
    });
    if !point_deleted {
        app.on_selection(CommandRequest::delete);
    }
}

fn open_database(app: &mut EditorApp) {
    let folder = app
        .path
        .as_deref()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf);
    app.database.show(folder.as_deref());
}

fn toggle_pane(app: &mut EditorApp, f: impl FnOnce(&mut openstitch_app::editor::prefs::Panes)) {
    f(&mut app.ui.prefs.panes);
    app.ui.save_prefs();
}

fn image_wizard(app: &mut EditorApp, m: openstitch_app::editor::wizard::Method) {
    app.wizard.method = m;
    app.wizard.open = true;
}

/// Runs a command button (menu commands open their list instead; see [`menu`]).
pub fn run(app: &mut EditorApp, a: Action, ctx: &egui::Context) {
    use Action::*;
    use openstitch_app::editor::pattern_wizards::WizardKind;
    use openstitch_app::editor::wizard::Method;
    if !enabled(app, a) {
        return;
    }
    if let Some(t) = a.tool() {
        set_tool(app, t);
        return;
    }
    match a {
        New => app.new_design(),
        Open => app.open_dialog(),
        Save => app.save(false),
        SaveAs => app.save(true),
        DesignProperty => app.ui.property_open = true,
        DesignSettings => app.workspace.settings_open = true,
        SelectColorPalette | UserThreadCharts => app.settings.charts_open = true,
        Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        Undo => app.undo(),
        Redo => app.redo(),
        DesignDatabase => open_database(app),
        StitchCreator => app.creator.open = true,
        StitchDesignFactory => app.sewing.open = true,
        FontCreator => app.lettering.fonts.open = true,
        AttributeSets => app.settings.sets_open = true,
        SelectAll => app.select_all(),
        Copy => {
            app.ui.clipboard = app.selection.clone();
            app.status = format!("{} objects copied", app.ui.clipboard.len());
        }
        Paste => paste(app),
        Duplicate => app.on_selection(CommandRequest::duplicate),
        Delete => delete(app),
        ArrangeCopy => {
            app.edit.arrange_open = true;
            app.edit.arrange.center = None;
        }
        CopyAttributes => edit_ui::copy_attributes(app),
        PasteAttributes => edit_ui::paste_attributes(app),
        ZoomIn => panels::zoom_center(app, 1.25),
        ZoomOut => panels::zoom_center(app, 1.0 / 1.25),
        Pan => {
            app.ui.pan = !app.ui.pan;
            app.workspace.measure.active = false;
        }
        Measure => {
            app.ui.pan = false;
            app.toggle_measure();
        }
        ImportPatterns => app.import_dialog(),
        ImportSvg => app.import_svg_dialog(),
        Group => edit_ui::group(app),
        Ungroup => edit_ui::ungroup(app),
        FitTextToOutline => {
            if let Some((text, shape)) = fit_pair(app) {
                app.execute(&CommandRequest::new(
                    "text.setPath",
                    json!({ "id": text, "path": shape }),
                ));
            }
        }
        BackgroundFill => app.pattern_wizards.open = Some(WizardKind::Background),
        Applique => app.pattern_wizards.open = Some(WizardKind::Applique),
        Patch => app.pattern_wizards.open = Some(WizardKind::Patch),
        Cutwork => app.pattern_wizards.open = Some(WizardKind::Cutwork),
        OpenImage => app.background_dialog(),
        PasteImage => app.paste_background(),
        DeleteImage => {
            app.execute(&CommandRequest::new("background.clear", json!({})));
        }
        AutoPunch => image_wizard(app, Method::AutoPunch),
        CrossStitch => image_wizard(app, Method::CrossStitch),
        PhotoStitch1 => image_wizard(app, Method::PhotoFill),
        PhotoStitch2 => image_wizard(app, Method::PhotoLines),
        DesignCenter => image_wizard(app, Method::DesignCenter),
        StitchView => (app.workspace.realistic, app.workspace.solid) = (false, false),
        RealisticView => (app.workspace.realistic, app.workspace.solid) = (true, false),
        SolidView => (app.workspace.realistic, app.workspace.solid) = (false, true),
        RealisticSettings => app.workspace.view.realistic_open = true,
        GridAxes => app.workspace.grid.axes ^= true,
        Guideline => {
            app.workspace.guides ^= true;
            // Guidelines come out of the rulers.
            app.workspace.rulers |= app.workspace.guides;
        }
        ModifyImage => app.workspace.view.image_open = true,
        ImageRotateRight => crate::view_ui::rotate_image(app, "right"),
        ImageRotateLeft => crate::view_ui::rotate_image(app, "left"),
        FontFilter => {
            app.ui.prefs.font_filter.on ^= true;
            app.ui.save_prefs();
        }
        FontFilterSettings => app.workspace.view.fonts_open = true,
        FabricSelector => app.workspace.view.fabric_open = true,
        ViewThreadTrimming => app.show_travel = !app.show_travel,
        PaneSewingOrder => toggle_pane(app, |p| p.sewing_order.visible ^= true),
        PaneSimulator => toggle_pane(app, |p| p.simulator.visible ^= true),
        PaneAttributes => toggle_pane(app, |p| p.attributes.visible ^= true),
        PaneImport => {
            app.ui.pane_tab = PaneTab::Import;
            toggle_pane(app, |p| p.attributes.visible = true);
        }
        ReferenceWindow => app.workspace.reference ^= true,
        ShowGrid => app.workspace.grid.visible ^= true,
        SnapToGrid => app.toggle_snap_to_grid(),
        Ruler => app.workspace.rulers ^= true,
        TrueTypeAttributes => crate::lettering_ui::load_truetype(app),
        ConvertToBlocks => convert_ui::to_shapes(app, "blocks"),
        ConvertToOutlines => convert_ui::to_shapes(app, "parts"),
        ConvertToOutline => {
            if convert_ui::kinds(app).2 {
                convert_ui::to_shapes(app, "whole");
            } else {
                convert_ui::text_to_shapes(app);
            }
        }
        ConvertToStitches => convert_ui::to_stitches(app),
        ConvertRegionToLine => convert_ui::region_to_line(app),
        SplitOutline => edit_ui::split_outline(app),
        AddDirectionLine => edit_ui::add_direction(app),
        DeleteDirectionLine => edit_ui::delete_direction(app),
        DeletePoint => delete(app),
        SplitAtPoint => convert_ui::split_at_point(app),
        OffsetLines => app.edit.offset_open = true,
        FloralPattern => app.edit.floral_open = true,
        SplitStitchData => convert_ui::split_off_area(app),
        DivideByColor => convert_ui::divide_by_color(app),
        Connect => convert_ui::connect(app),
        ExportSections => crate::sections_ui::export_sections(app),
        // Tools are handled above; menus open their list; the rest are not available yet.
        SelectTool | PointTool | Rectangle | Ellipse | ClosedStraight | ClosedCurve | OpenCurve
        | StraightBlock | EditPoint => {}
        Print | Wizard | Export | Options | Help | TextTools | ShapesTools | OtherZoom
        | Arrange | RotateFlip | Size | StampEmboss | ModifyOverlap | DecorativeFrame
        | SendEmbroidery | SaveImage | ImageDensity | GridSpacing | Font | TextSize | EditText
        | LineColor | LineSewType | RegionColor | RegionSewType | EntryExit | SewingPoints => {}
        Cut | PencilClosed | OpenStraight | PencilOpen | EdgeRadius | CurvedBlock | Running
        | Feed | ReverseDirectionLine | ChangeColor | MoveSections => {}
    }
}

/// A command inside a dropdown list: icon, label, shortcut and ScreenTip.
fn menu_item(app: &mut EditorApp, ui: &mut egui::Ui, a: Action) {
    let info = a.info();
    let mut b = egui::Button::new(info.label);
    if let Some(s) = info.shortcut_for(MAC) {
        b = b.shortcut_text(s);
    }
    let on = active(app, a);
    let r = ui
        .add_enabled(enabled(app, a), b.selected(on))
        .on_hover_text(info.screen_tip(MAC))
        .on_disabled_hover_text(info.screen_tip(MAC));
    if r.clicked() {
        run(app, a, ui.ctx());
        ui.close();
    }
}

fn export_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    for ty in FileType::writable() {
        if ty == FileType::Pes {
            ui.menu_button("PES", |ui| {
                for (v, label) in EditorApp::PES_VERSIONS {
                    if ui.button(format!("{label}…")).clicked() {
                        app.export_with(ty, Some(v));
                    }
                }
            });
            continue;
        }
        if ui
            .button(format!("{}…", ty.extension().to_uppercase()))
            .clicked()
        {
            app.export(ty);
        }
    }
    ui.separator();
    menu_item(app, ui, Action::ExportSections);
}

/// The list a menu command opens.
pub fn menu(app: &mut EditorApp, a: Action, ui: &mut egui::Ui) {
    use Action::*;
    ui.set_min_width(180.0);
    match a {
        Print => {
            if ui.button("Print to PDF…").clicked() {
                crate::print_ui::open(ui.ctx());
            }
        }
        Wizard => {
            if ui.button("Template Wizard…").clicked() {
                crate::lettering_ui::open_template_wizard(app);
            }
            if ui.button("Image to Embroidery…").clicked() {
                app.wizard.open = true;
            }
            ui.separator();
            for w in [BackgroundFill, Applique, Patch, Cutwork] {
                menu_item(app, ui, w);
            }
        }
        Export => export_menu(app, ui),
        SendEmbroidery => {
            if ui.button("Send to USB Media…").clicked() {
                crate::transfer_ui::open(app);
            }
            menu_item(app, ui, ExportSections);
            ui.menu_button("Export", |ui| export_menu(app, ui));
        }
        Options => options_menu(app, ui),
        Help => help_menu(app, ui),
        TextTools => crate::lettering_ui::text_tools_menu(app, ui),
        ShapesTools => {
            for t in [
                Rectangle,
                Ellipse,
                ClosedStraight,
                ClosedCurve,
                StraightBlock,
            ] {
                menu_item(app, ui, t);
            }
            ui.separator();
            ui.menu_button("Insert Preset Shape", |ui| edit_ui::insert_menu(app, ui));
        }
        OtherZoom => {
            if ui
                .add(
                    egui::Button::new("Fit Design Page")
                        .shortcut_text(format!("{}0", crate::app::CMD)),
                )
                .clicked()
            {
                app.fit_requested = true;
            }
            if ui
                .add_enabled(
                    !app.selection.is_empty(),
                    egui::Button::new("Zoom to Selection"),
                )
                .clicked()
            {
                zoom_to_selection(app);
            }
            ui.separator();
            for p in [50.0, 100.0, 200.0, 400.0] {
                if ui.button(format!("{p:.0} %")).clicked() {
                    zoom_to_percent(app, p);
                }
            }
        }
        Arrange => {
            if ui.button("Center on Design Page").clicked() {
                app.on_selection(|s| {
                    CommandRequest::new("object.center", json!({ "ids": CommandRequest::ids(s) }))
                });
            }
            ui.separator();
            for (label, to) in [
                ("Sew First", "first"),
                ("Sew Earlier", "earlier"),
                ("Sew Later", "later"),
                ("Sew Last", "last"),
            ] {
                if ui.button(label).clicked() {
                    let r = objects::move_to(&app.selection, to);
                    app.execute(&r);
                }
            }
        }
        RotateFlip => {
            for (label, deg) in [
                ("Rotate Right 90°", 90.0),
                ("Rotate Left 90°", -90.0),
                ("Rotate 180°", 180.0),
            ] {
                if ui.button(label).clicked() {
                    app.on_selection(|s| CommandRequest::rotate(s, deg));
                }
            }
            ui.separator();
            if ui.button("Flip Horizontal").clicked() {
                app.on_selection(|s| CommandRequest::flip(s, true));
            }
            if ui.button("Flip Vertical").clicked() {
                app.on_selection(|s| CommandRequest::flip(s, false));
            }
        }
        Size => {
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut app.scale_percent)
                        .range(1.0..=1000.0)
                        .suffix(" %"),
                );
                if ui.button("Apply").clicked() {
                    scale(app, app.scale_percent / 100.0);
                    app.scale_percent = 100.0;
                }
            });
            ui.separator();
            for p in [50.0, 90.0, 110.0, 200.0] {
                if ui.button(format!("{p:.0} %")).clicked() {
                    scale(app, p / 100.0);
                }
            }
        }
        StampEmboss => edit_ui::stamp_menu(app, ui),
        ModifyOverlap => edit_ui::overlap_menu(app, ui),
        DecorativeFrame => edit_ui::decorative_menu(app, ui),
        ImageDensity => image_density_menu(app, ui),
        SaveImage => crate::view_ui::save_image_menu(app, ui),
        GridSpacing => {
            for s in openstitch_app::editor::workspace::GridSettings::SPACINGS_MM {
                let label = if s == 25.4 {
                    "1 inch".to_string()
                } else {
                    format!("{s} mm")
                };
                ui.radio_value(&mut app.workspace.grid.spacing_mm, s, label);
            }
        }
        Font => {
            let fonts = crate::lettering_ui::fonts(app);
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    for f in fonts {
                        if ui.button(&f.label).clicked() {
                            set_text(app, json!({ "font": f.id }));
                        }
                    }
                });
        }
        TextSize => {
            for mm in [5.0, 8.0, 10.0, 15.0, 20.0, 30.0, 50.0] {
                if ui.button(format!("{mm} mm")).clicked() {
                    set_text(app, json!({ "size": mm }));
                }
            }
        }
        EditText => {
            if app.ui.text_draft.is_empty()
                && let Some(t) = first_text(app)
            {
                app.ui.text_draft = t;
            }
            ui.text_edit_multiline(&mut app.ui.text_draft);
            if ui.button("Apply").clicked() {
                let text = app.ui.text_draft.clone();
                set_text(app, json!({ "text": text }));
                app.ui.text_draft.clear();
                ui.close();
            }
        }
        LineColor | RegionColor => {
            let region = a == RegionColor;
            let m = openstitch_app::editor::inspector::InspectorModel::new(
                app.session.design(),
                &app.selection,
            );
            let current = if region {
                m.region.as_ref().map(|r| r.thread.clone())
            } else {
                m.line.as_ref().map(|l| l.thread.clone())
            };
            if let Some(i) = panels::pec_grid(
                ui,
                if region {
                    "ribbon-region"
                } else {
                    "ribbon-line"
                },
                current.as_ref(),
            ) {
                let r = if region {
                    m.set_region(json!({ "color": i }))
                } else {
                    m.set_line(json!({ "color": i }))
                };
                app.execute(&r);
            }
        }
        LineSewType => {
            let m = openstitch_app::editor::inspector::InspectorModel::new(
                app.session.design(),
                &app.selection,
            );
            for (k, label) in openstitch_app::editor::inspector::line_types() {
                if ui
                    .selectable_label(m.line_kind.same() == Some(&k), label)
                    .clicked()
                {
                    let r = m.set_line_type(k);
                    app.execute(&r);
                }
            }
        }
        RegionSewType => {
            let m = openstitch_app::editor::inspector::InspectorModel::new(
                app.session.design(),
                &app.selection,
            );
            for (k, label) in openstitch_app::editor::inspector::region_types() {
                if ui
                    .selectable_label(m.region_kind.same() == Some(&k), label)
                    .clicked()
                {
                    let r = m.set_region_type(k);
                    app.execute(&r);
                }
            }
        }
        EntryExit => edit_ui::entry_exit_menu(app, ui),
        SewingPoints => edit_ui::sewing_points_menu(app, ui),
        _ => {}
    }
}

fn scale(app: &mut EditorApp, f: f64) {
    if (f - 1.0).abs() > 1e-9 {
        app.on_selection(|s| {
            CommandRequest::new(
                "object.scale",
                json!({ "ids": CommandRequest::ids(s), "sx": f }),
            )
        });
    }
}

fn first_text(app: &EditorApp) -> Option<String> {
    let d = app.session.design();
    convert_ui::texts(app)
        .first()
        .and_then(|&id| d.index_of(id))
        .and_then(|i| d.objects[i].text.as_ref().map(|t| t.text.clone()))
}

/// `object.setText` on the selected text objects.
pub fn set_text(app: &mut EditorApp, mut params: serde_json::Value) {
    let ids: BTreeSet<u64> = convert_ui::texts(app).into_iter().collect();
    if ids.is_empty() {
        return;
    }
    params["ids"] = CommandRequest::ids(&ids);
    app.execute(&CommandRequest::new("object.setText", params));
}

pub fn image_density_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    let Some(b) = app.session.design().background.clone() else {
        ui.label("No background image");
        return;
    };
    let label = if b.visible { "Hide" } else { "Show" };
    if ui.button(label).clicked() {
        app.execute(&CommandRequest::new(
            "background.show",
            json!({ "visible": !b.visible }),
        ));
    }
    for (name, o) in [
        ("Opacity 25 %", 0.25),
        ("Opacity 50 %", 0.5),
        ("Opacity 75 %", 0.75),
        ("Opacity 100 %", 1.0),
    ] {
        if ui.radio(b.opacity == o, name).clicked() {
            app.execute(&CommandRequest::new(
                "background.show",
                json!({ "opacity": o }),
            ));
        }
    }
}

fn options_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    for a in OPTION_MENU {
        menu_item(app, ui, a);
    }
    ui.separator();
    ui.label(RichText::new("Theme").weak());
    let mut changed = false;
    for t in Theme::ALL {
        changed |= ui
            .radio_value(&mut app.ui.prefs.theme, t, t.label())
            .changed();
    }
    ui.separator();
    changed |= ui
        .checkbox(
            &mut app.ui.prefs.qat_below,
            "Quick Access Toolbar below the Ribbon",
        )
        .changed();
    if changed {
        app.ui.save_prefs();
    }
}

fn help_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    if ui.button("Instruction Manual").clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(
            "https://github.com/cristianonescu/OPE-Design11#readme",
        ));
    }
    if ui.button("Keyboard Shortcuts").clicked() {
        app.ui.shortcuts_open = true;
    }
    if ui.button("About OpenStitch").clicked() {
        app.ui.about_open = true;
    }
}

// MARK: drawing

/// A ribbon command button; `large` = icon over a two-line label.
fn command(app: &mut EditorApp, ui: &mut egui::Ui, a: Action, large: bool) {
    let info = a.info();
    let on = enabled(app, a);
    let pressed = active(app, a);
    let visuals = ui.visuals().clone();
    let fg = if on {
        visuals.text_color()
    } else {
        visuals.weak_text_color()
    };
    let menu = a.is_menu();
    let font = egui::FontId::proportional(11.0);
    let (size, galley) = if large {
        let g = ui.painter().layout(info.label.to_string(), font, fg, 62.0);
        (vec2(g.size().x.max(LARGE_ICON) + 8.0, BODY), g)
    } else {
        let g = ui
            .painter()
            .layout_no_wrap(info.label.to_string(), font, fg);
        let w = SMALL_ICON + 6.0 + g.size().x + if menu { 14.0 } else { 6.0 };
        (vec2(w, 21.0), g)
    };
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered() && on;
    if pressed || hovered {
        let fill = if pressed {
            visuals.selection.bg_fill.gamma_multiply(0.35)
        } else {
            visuals.widgets.hovered.weak_bg_fill
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(3),
            fill,
            Stroke::new(1.0, visuals.selection.bg_fill.gamma_multiply(0.6)),
            StrokeKind::Inside,
        );
    }
    let icon_fg = visuals.strong_text_color();
    if large {
        let icon = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.min.y + 4.0 + LARGE_ICON / 2.0),
            vec2(LARGE_ICON, LARGE_ICON),
        );
        icons::paint(ui.painter(), icon, a, icon_fg, on);
        let text_pos = egui::pos2(rect.center().x - galley.size().x / 2.0, icon.max.y + 2.0);
        let bottom = text_pos.y + galley.size().y;
        ui.painter().galley(text_pos, galley, fg);
        if menu {
            arrow(ui, egui::pos2(rect.center().x, bottom + 5.0), fg);
        }
    } else {
        let icon = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + 3.0, rect.center().y - SMALL_ICON / 2.0),
            vec2(SMALL_ICON, SMALL_ICON),
        );
        icons::paint(ui.painter(), icon, a, icon_fg, on);
        let y = rect.center().y - galley.size().y / 2.0;
        let x = icon.max.x + 4.0;
        let end = x + galley.size().x;
        ui.painter().galley(egui::pos2(x, y), galley, fg);
        if menu {
            arrow(ui, egui::pos2(end + 7.0, rect.center().y), fg);
        }
    }
    let tip = info.screen_tip(MAC);
    let resp = resp.on_hover_text(tip);
    if resp.hovered() {
        app.ui.hover = Some(info.tip);
    }
    if menu {
        if on {
            egui::Popup::menu(&resp).show(|ui| self::menu(app, a, ui));
        }
    } else if resp.clicked() && on {
        run(app, a, ui.ctx());
    }
}

fn arrow(ui: &egui::Ui, c: egui::Pos2, color: Color32) {
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            c + vec2(-3.5, -2.0),
            c + vec2(3.5, -2.0),
            c + vec2(0.0, 2.0),
        ],
        color,
        Stroke::NONE,
    ));
}

fn group(app: &mut EditorApp, ui: &mut egui::Ui, g: &ribbon::Group) {
    ui.vertical(|ui| {
        ui.set_height(BODY + 16.0);
        ui.horizontal(|ui| {
            ui.set_height(BODY);
            ui.spacing_mut().item_spacing.x = 2.0;
            for item in g.items {
                match item {
                    Item::Large(a) => command(app, ui, *a, true),
                    Item::Small(list) => {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            for &a in *list {
                                command(app, ui, a, false);
                            }
                        });
                    }
                }
            }
        });
        let w = ui.min_rect().width();
        ui.allocate_ui(vec2(w, 14.0), |ui| {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(g.name).size(11.0).weak());
            });
        });
    });
}

/// Tracks tool changes and the contextual tabs; picks the tab a new tool brings forward.
fn update_tab(app: &mut EditorApp) -> Vec<TabId> {
    let ctx_tabs = ribbon::contextual_tabs(&facts(app));
    let tool = app.tools.tool();
    if tool != app.ui.last_tool {
        app.ui.last_tool = tool;
        if let Some(t) = ribbon::tab_for_tool(tool) {
            app.ui.tab = t;
        }
    }
    if !TabId::FIXED.contains(&app.ui.tab) && !ctx_tabs.contains(&app.ui.tab) {
        app.ui.tab = TabId::Home;
    }
    ctx_tabs
}

/// Application button, ribbon tabs, Option and Help buttons.
fn tab_strip(app: &mut EditorApp, ui: &mut egui::Ui, ctx_tabs: &[TabId]) {
    ui.horizontal(|ui| {
        app_button(app, ui);
        ui.add_space(6.0);
        let mut tabs: Vec<TabId> = TabId::FIXED.to_vec();
        tabs.extend_from_slice(ctx_tabs);
        for t in tabs {
            let selected = app.ui.tab == t;
            let mut text = RichText::new(t.label()).size(13.0);
            if t.context().is_some() {
                text = text.color(CONTEXT);
            }
            if selected {
                text = text.strong();
            }
            let r = ui.add(egui::Button::selectable(selected, text));
            if let Some(c) = t.context() {
                ui.painter().line_segment(
                    [r.rect.left_top(), r.rect.right_top()],
                    Stroke::new(3.0, CONTEXT),
                );
                r.clone().on_hover_text(c);
            }
            if r.clicked() {
                app.ui.tab = t;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for a in [Action::Help, Action::Options] {
                let info = a.info();
                let r = ui
                    .add(egui::Button::new(format!("{} ▾", info.label)).frame(false))
                    .on_hover_text(info.screen_tip(MAC));
                egui::Popup::menu(&r).show(|ui| menu(app, a, ui));
            }
        });
    });
}

fn app_button(app: &mut EditorApp, ui: &mut egui::Ui) {
    let (rect, resp) = ui.allocate_exact_size(vec2(64.0, 24.0), Sense::click());
    let bg = if resp.hovered() {
        APP_BLUE.gamma_multiply(1.2)
    } else {
        APP_BLUE
    };
    ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
    icons::paint_logo(
        ui.painter(),
        egui::Rect::from_min_size(rect.min + vec2(4.0, 2.0), vec2(20.0, 20.0)),
        bg,
    );
    ui.painter().text(
        egui::pos2(rect.min.x + 28.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "File",
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
    let resp = resp.on_hover_text("Application menu: new, open, save, print, export, recent files");
    let force = std::mem::take(&mut app.ui.open_app_menu);
    let cmd = if resp.clicked() {
        Some(egui::SetOpenCommand::Toggle)
    } else if force {
        Some(egui::SetOpenCommand::Bool(true))
    } else {
        None
    };
    egui::Popup::menu(&resp)
        .open_memory(cmd)
        .layout(egui::Layout::top_down(egui::Align::Min))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| application_menu(app, ui));
}

fn application_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.set_width(470.0);
    let line = ui.visuals().widgets.noninteractive.bg_stroke;
    ui.horizontal_top(|ui| {
        let left = ui
            .vertical(|ui| {
                ui.set_width(200.0);
                for a in APPLICATION_MENU {
                    app_menu_row(app, ui, a);
                }
            })
            .response
            .rect;
        ui.painter().vline(left.max.x + 6.0, left.y_range(), line);
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.set_width(250.0);
            ui.label(RichText::new("Recent Files").strong());
            let recent = app.ui.prefs.recent.clone();
            if recent.is_empty() {
                ui.label(RichText::new("No files opened yet").weak());
            }
            for (k, p) in recent.iter().enumerate() {
                let name = p.file_name().map_or_else(
                    || p.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                if ui
                    .add(egui::Button::new(format!("{} {name}", k + 1)).frame(false))
                    .on_hover_text(p.display().to_string())
                    .clicked()
                {
                    if p.exists() {
                        app.open_path(p);
                    } else {
                        app.status = format!("{} no longer exists", p.display());
                    }
                    ui.close();
                }
            }
        });
    });
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.min.y + 3.0, line);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(250.0);
        ui.menu_button("Options", |ui| options_menu(app, ui));
        if ui.button("Exit").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
}

fn app_menu_row(app: &mut EditorApp, ui: &mut egui::Ui, a: Action) {
    let info = a.info();
    let on = enabled(app, a);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
        icons::paint(ui.painter(), r, a, ui.visuals().strong_text_color(), on);
        let text = RichText::new(info.label).size(14.0);
        if a.is_menu() {
            ui.add_enabled_ui(on, |ui| {
                ui.menu_button(text, |ui| menu(app, a, ui))
                    .response
                    .on_hover_text(info.screen_tip(MAC));
            });
        } else {
            let mut b = egui::Button::new(text).frame(false);
            if let Some(s) = info.shortcut_for(MAC) {
                b = b.shortcut_text(s);
            }
            let r = ui
                .add_enabled(on, b)
                .on_hover_text(info.screen_tip(MAC))
                .on_disabled_hover_text(info.screen_tip(MAC));
            if r.clicked() {
                run(app, a, ui.ctx());
                ui.close();
            }
        }
    });
}

/// Quick Access Toolbar: small icon buttons and the customise list.
pub fn quick_access(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for a in app.ui.prefs.qat.clone() {
            let info = a.info();
            let on = enabled(app, a);
            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
            if resp.hovered() && on {
                ui.painter().rect_filled(
                    r,
                    CornerRadius::same(3),
                    ui.visuals().widgets.hovered.weak_bg_fill,
                );
            }
            icons::paint(
                ui.painter(),
                egui::Rect::from_center_size(r.center(), vec2(16.0, 16.0)),
                a,
                ui.visuals().strong_text_color(),
                on,
            );
            let resp = resp.on_hover_text(info.screen_tip(MAC));
            if a.is_menu() {
                if on {
                    egui::Popup::menu(&resp).show(|ui| menu(app, a, ui));
                }
            } else if resp.clicked() {
                run(app, a, ui.ctx());
            }
        }
        let r = ui
            .add(egui::Button::new("▾").frame(false))
            .on_hover_text("Customize Quick Access Toolbar");
        egui::Popup::menu(&r).show(|ui| {
            ui.label(RichText::new("Customize Quick Access Toolbar").strong());
            for a in ribbon::qat_candidates() {
                let mut on = app.ui.prefs.qat.contains(&a);
                if ui.checkbox(&mut on, a.info().label).changed() {
                    app.ui.prefs.toggle_qat(a);
                    app.ui.save_prefs();
                }
            }
            ui.separator();
            if ui.button("Reset to default").clicked() {
                app.ui.prefs.qat = ribbon::QAT_DEFAULT.to_vec();
                app.ui.save_prefs();
            }
            if ui
                .checkbox(&mut app.ui.prefs.qat_below, "Show below the Ribbon")
                .changed()
            {
                app.ui.save_prefs();
            }
        });
    });
}

/// The top of the window: Quick Access Toolbar, tabs and the active tab's groups.
pub fn ribbon(app: &mut EditorApp, ui: &mut egui::Ui) {
    app.ui.hover = None;
    let ctx_tabs = update_tab(app);
    if !app.ui.prefs.qat_below {
        quick_access(app, ui);
    }
    tab_strip(app, ui, &ctx_tabs);
    let fill = ui.visuals().extreme_bg_color;
    egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::symmetric(6, 4))
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("ribbon-body")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        let groups = app.ui.tab.groups();
                        for (k, g) in groups.iter().enumerate() {
                            group(app, ui, g);
                            if k + 1 < groups.len() {
                                ui.add(egui::Separator::default().spacing(4.0));
                            }
                        }
                    });
                });
        });
    if app.ui.prefs.qat_below {
        quick_access(app, ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Launch;
    use openstitch_app::editor::ribbon::{TabId, ribbon_actions};

    /// Actions that open a native file dialog, a URL or close the window.
    fn interactive(a: Action) -> bool {
        use Action::*;
        matches!(
            a,
            Open | Save
                | SaveAs
                | Exit
                | ImportPatterns
                | ImportSvg
                | OpenImage
                | PasteImage
                | TrueTypeAttributes
                | ExportSections
        )
    }

    fn demo() -> EditorApp {
        EditorApp::new(Launch {
            demo: true,
            ..Launch::default()
        })
    }

    #[test]
    fn every_ribbon_button_runs_or_is_marked_unavailable() {
        let ctx = egui::Context::default();
        let selections: [&[u64]; 4] = [&[], &[1], &[2], &[1, 2]];
        for a in ribbon_actions()
            .into_iter()
            .chain(APPLICATION_MENU)
            .chain(OPTION_MENU)
        {
            let info = a.info();
            if !info.available {
                let app = demo();
                assert!(!enabled(&app, a), "{a:?} is unavailable but enabled");
                assert!(info.screen_tip(false).contains("Not yet available"));
                continue;
            }
            if interactive(a) {
                continue;
            }
            for sel in selections {
                let mut app = demo();
                app.selection = sel.iter().copied().collect();
                // Runs the command (or, for a menu, draws its list) without panicking.
                if a.is_menu() {
                    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| menu(&mut app, a, ui));
                    out.textures_delta.clear();
                } else {
                    run(&mut app, a, &ctx);
                }
            }
        }
    }

    #[test]
    fn ribbon_commands_change_the_design_and_view() {
        let ctx = egui::Context::default();
        let mut app = demo();
        let n = app.session.design().objects.len();
        app.selection = BTreeSet::from([1]);
        run(&mut app, Action::Copy, &ctx);
        assert!(enabled(&app, Action::Paste));
        run(&mut app, Action::Paste, &ctx);
        assert_eq!(app.session.design().objects.len(), n + 1);
        assert!(!app.selection.contains(&1), "the copy is selected");
        run(&mut app, Action::Delete, &ctx);
        assert_eq!(app.session.design().objects.len(), n);
        run(&mut app, Action::Undo, &ctx);
        assert_eq!(app.session.design().objects.len(), n + 1);
        run(&mut app, Action::ClosedStraight, &ctx);
        assert_eq!(app.tools.tool(), Tool::Polygon);
        assert!(active(&app, Action::ClosedStraight));
        run(&mut app, Action::RealisticView, &ctx);
        assert!(app.workspace.realistic && !active(&app, Action::StitchView));
        run(&mut app, Action::PaneSimulator, &ctx);
        assert!(!app.ui.prefs.panes.simulator.visible);
        run(&mut app, Action::PaneImport, &ctx);
        assert_eq!(app.ui.pane_tab, PaneTab::Import);
        run(&mut app, Action::Applique, &ctx);
        assert!(app.pattern_wizards.open.is_some());
        run(&mut app, Action::CrossStitch, &ctx);
        assert!(app.wizard.open);
        run(&mut app, Action::Cut, &ctx); // unavailable: nothing happens
        assert_eq!(app.session.design().objects.len(), n + 1);
    }

    #[test]
    fn view_image_text_and_fabric_commands() {
        let ctx = egui::Context::default();
        let mut app = demo();
        // View modes are exclusive.
        run(&mut app, Action::SolidView, &ctx);
        assert!(active(&app, Action::SolidView) && !active(&app, Action::StitchView));
        run(&mut app, Action::RealisticView, &ctx);
        assert!(!active(&app, Action::SolidView) && active(&app, Action::RealisticView));
        run(&mut app, Action::StitchView, &ctx);
        assert!(active(&app, Action::StitchView));
        run(&mut app, Action::GridAxes, &ctx);
        assert!(!app.workspace.grid.axes);
        app.workspace.rulers = false;
        run(&mut app, Action::Guideline, &ctx);
        assert!(active(&app, Action::Guideline) && app.workspace.rulers);
        run(&mut app, Action::FontFilter, &ctx);
        assert!(active(&app, Action::FontFilter));

        // Image commands need a background image.
        assert!(!enabled(&app, Action::ImageRotateRight));
        let dir = std::env::temp_dir().join(format!("openstitch-view-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let png = dir.join("bg.png");
        let raster =
            openstitch_digitize::Raster::from_fn(40, 20, |x, _| [x as u8 * 6, 90, 30, 255])
                .expect("raster");
        std::fs::write(&png, raster.encode_png().expect("png")).expect("write");
        app.execute(&CommandRequest::new(
            "background.set",
            json!({ "path": png.display().to_string(), "width": 40 }),
        ));
        for a in [
            Action::SaveImage,
            Action::ModifyImage,
            Action::ImageRotateLeft,
        ] {
            assert!(enabled(&app, a), "{a:?}");
        }
        run(&mut app, Action::ImageRotateRight, &ctx);
        let b = app.session.design().background.clone().expect("background");
        assert_eq!((b.adjust.rotation, b.width, b.height), (1, 200.0, 400.0));
        let shown = crate::view_ui::shown_image(&app).expect("image");
        assert_eq!((shown.width(), shown.height()), (20, 40));

        // Every new window draws without panicking.
        for a in [
            Action::RealisticSettings,
            Action::ModifyImage,
            Action::FontFilterSettings,
            Action::FabricSelector,
        ] {
            run(&mut app, a, &ctx);
        }
        let v = &app.workspace.view;
        assert!(v.realistic_open && v.image_open && v.fonts_open && v.fabric_open);
        app.workspace.solid = true;
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                crate::view_ui::windows(&mut app, ui.ctx());
                crate::canvas::canvas(&mut app, ui);
            });
            out.textures_delta.clear();
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn contextual_tabs_appear_for_tool_and_selection() {
        let mut app = demo();
        assert!(update_tab(&mut app).is_empty());
        assert_eq!(app.ui.tab, TabId::Home);
        app.tools.set_tool(Tool::Rectangle);
        assert_eq!(update_tab(&mut app), [TabId::OutlineRect]);
        assert_eq!(
            app.ui.tab,
            TabId::OutlineRect,
            "the tool brings its tab forward"
        );
        app.tools.set_tool(Tool::Select);
        app.selection = BTreeSet::from([2]);
        assert_eq!(update_tab(&mut app), [TabId::Shapes]);
        assert_eq!(
            app.ui.tab,
            TabId::Home,
            "a tab that went away falls back to Home"
        );
        app.ui.tab = TabId::Shapes;
        app.execute(&CommandRequest::new(
            "text.add",
            json!({ "text": "Hi", "x": 50, "y": 20 }),
        ));
        let text = app.session.design().objects.last().expect("text").id;
        app.selection = BTreeSet::from([text, 2]);
        assert_eq!(update_tab(&mut app), [TabId::Text, TabId::Shapes]);
        assert!(enabled(&app, Action::FitTextToOutline));
        assert!(enabled(&app, Action::Font) && enabled(&app, Action::RegionColor));
        app.selection = BTreeSet::from([text]);
        let ctx = egui::Context::default();
        run(&mut app, Action::ConvertToOutline, &ctx);
        assert!(
            app.session
                .design()
                .objects
                .iter()
                .all(|o| o.kind != ObjectKind::Text),
            "text converted to shapes"
        );
        app.open_path(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../openstitch-engine/tests/fixtures/badge-golden.pes"),
        );
        app.select_all();
        assert_eq!(update_tab(&mut app), [TabId::Stitches]);
        assert!(enabled(&app, Action::DivideByColor));
    }
}
