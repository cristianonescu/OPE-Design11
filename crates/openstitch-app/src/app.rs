//! Application state, command dispatch, file actions, menus and shortcuts. Every change to the
//! design goes through [`Session::execute`]; this module only keeps view state (selection,
//! viewport, simulator position) and the compiled pattern the canvas draws.

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use openstitch_app::editor::render::{PatternStats, RenderModel};
use openstitch_app::editor::{
    CommandRequest, Tool, ToolContext, ToolController, ToolEffect, Viewport,
};
use openstitch_core::StitchPattern;
use openstitch_design::Compiler;
use openstitch_engine::{Effect, Session};
use openstitch_formats::FileType;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Modifier name shown in hints.
pub const CMD: &str = if cfg!(target_os = "macos") {
    "Cmd+"
} else {
    "Ctrl+"
};

/// Options for scripted runs (`--screenshot`).
#[derive(Default, Clone, Debug)]
pub struct Launch {
    pub open: Option<PathBuf>,
    pub demo: bool,
    pub screenshot: Option<PathBuf>,
    pub select: Vec<u64>,
    /// Simulator position as a fraction of all stitches.
    pub simulate: Option<f64>,
    pub tool: Option<Tool>,
    /// Scripted input (see [`crate::script`]).
    pub script: Vec<crate::script::Step>,
    /// Zoom factor applied after fitting the page.
    pub zoom: Option<f64>,
    /// Workspace views to open (see [`SHOW`]).
    pub show: Vec<String>,
    /// Image to Embroidery: image to preselect (opens the dialog) and method to run.
    pub image: Option<PathBuf>,
    pub convert: Option<openstitch_app::editor::wizard::Method>,
    /// Pattern file to open in the Programmable Stitch Creator (opens the window).
    pub pattern: Option<PathBuf>,
    /// Pattern wizard dialog to open for the selection, and whether to apply it once.
    pub wizard: Option<openstitch_app::editor::pattern_wizards::WizardKind>,
    pub apply_wizard: bool,
    /// User settings file (attribute sets, thread charts); `None` keeps them in memory.
    pub user_settings: Option<PathBuf>,
    /// Design Database: folder (or file, which is selected) to open the mode at.
    pub database: Option<PathBuf>,
    pub database_view: Option<openstitch_app::editor::database::View>,
    /// Design Database dialog to open: search, convert, catalog, delete, newfolder.
    pub database_dialog: Option<String>,
    /// Automation socket (`--listen`); started by `main` before the window opens.
    pub listen: Option<openstitch_remote::ServerConfig>,
    /// Ribbon tab and right-hand pane tab to show (`--ribbon`, `--pane`).
    pub ribbon: Option<openstitch_app::editor::ribbon::TabId>,
    pub pane: Option<crate::ribbon::PaneTab>,
    /// Colour theme (`--theme`); otherwise the saved preference.
    pub theme: Option<openstitch_app::editor::prefs::Theme>,
}

/// Values `--show` accepts.
pub const SHOW: [&str; 22] = [
    "realistic",
    "solid",
    "fabric",
    "reference",
    "settings",
    "measure",
    "norulers",
    "text",
    "creator",
    "sdf",
    "attributes",
    "charts",
    "usb",
    "fonts",
    "monogram",
    "namedrop",
    "templates",
    "arrange",
    "appmenu",
    "property",
    "shortcuts",
    "floating",
];

pub struct EditorApp {
    pub session: Session,
    pub selection: BTreeSet<u64>,
    pub tools: ToolController,
    /// `None` until the canvas size is known; then fitted to the Design Page.
    pub viewport: Option<Viewport>,
    pub pattern: StitchPattern,
    pub stats: PatternStats,
    pub render: RenderModel,
    render_key: Option<(u64, usize)>,
    /// Bumped after every change to the design.
    pub revision: u64,
    saved_revision: u64,
    /// Simulator: stitches shown (`None` = the whole design).
    pub simulate: Option<usize>,
    pub playing: bool,
    pub play_speed: f64,
    play_accumulator: f64,
    pub show_travel: bool,
    pub path: Option<PathBuf>,
    pub status: String,
    effects: HashMap<&'static str, Effect>,
    pub scale_takes_origin: bool,
    /// Inspector fields being dragged or typed, committed as one command when released.
    pub drafts: HashMap<&'static str, f64>,
    /// Scale percentage typed in the inspector.
    pub scale_percent: f64,
    title: String,
    screenshot: Option<Screenshot>,
    pub canvas_pointer_captured: bool,
    pub clicks: openstitch_app::editor::ClickCounter,
    pub fit_requested: bool,
    /// Zoom applied once after the first fit (`--zoom`).
    pub initial_zoom: Option<f64>,
    player: Option<crate::script::Player>,
    frames: u64,
    /// Engine history steps per user-level edit, so one gesture (e.g. scale + move, or
    /// new design + import + centre) is one Undo.
    pub(crate) undo_groups: Vec<usize>,
    pub(crate) redo_groups: Vec<usize>,
    /// Rulers, grid, measure tool, Reference window, Design Settings, estimates.
    pub workspace: crate::workspace_ui::Workspace,
    /// Groups, arrange copies, shape operations, attributes, entry/exit points.
    pub edit: crate::edit_ui::EditState,
    /// Image to Embroidery dialog.
    pub wizard: openstitch_app::editor::wizard::ImageWizard,
    /// Lettering menu dialogs.
    pub lettering: crate::lettering_ui::LetteringState,
    /// Programmable Stitch Creator window.
    pub creator: crate::creator_ui::CreatorState,
    /// Stitch Design Factory window.
    pub sewing: crate::sewing_ui::SewingState,
    /// Background fill, appliqué, patch and cutwork dialogs.
    pub pattern_wizards: openstitch_app::editor::pattern_wizards::PatternWizards,
    /// Attribute sets and user thread charts windows.
    pub settings: crate::settings_ui::SettingsState,
    /// The background image as shown, keyed by its path and adjustments (`None` when it
    /// cannot be read).
    pub background_texture: Option<(
        String,
        openstitch_design::ImageAdjust,
        Option<eframe::egui::TextureHandle>,
    )>,
    /// Design Database mode (File ▸ Design Database…).
    pub database: openstitch_app::editor::database::Database,
    pub db_textures: crate::database_ui::Textures,
    pub db_chrome: crate::database_ui::Chrome,
    /// Loopback automation socket; its requests run in [`EditorApp::serve_remote`].
    pub remote: Option<openstitch_remote::Server>,
    /// Ribbon, Quick Access Toolbar, panes and window preferences.
    pub ui: crate::ribbon::RibbonState,
}

struct Screenshot {
    path: PathBuf,
    frames: u32,
    requested: bool,
    done: Arc<AtomicBool>,
}

impl EditorApp {
    pub fn new(launch: Launch) -> Self {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let session = Session::new(base);
        let effects = session
            .commands()
            .iter()
            .map(|c| (c.id, c.effect))
            .collect();
        let scale_takes_origin = session
            .commands()
            .iter()
            .any(|c| c.id == "object.scale" && c.params.iter().any(|p| p.name == "originX"));
        let mut app = EditorApp {
            session,
            selection: BTreeSet::new(),
            tools: ToolController::default(),
            viewport: None,
            pattern: StitchPattern::new(),
            stats: PatternStats::default(),
            render: RenderModel::default(),
            render_key: None,
            revision: 0,
            saved_revision: 0,
            simulate: None,
            playing: false,
            play_speed: 400.0,
            play_accumulator: 0.0,
            show_travel: true,
            path: None,
            status: "Ready".into(),
            effects,
            scale_takes_origin,
            drafts: HashMap::new(),
            scale_percent: 100.0,
            title: String::new(),
            screenshot: None,
            canvas_pointer_captured: false,
            clicks: Default::default(),
            fit_requested: false,
            initial_zoom: None,
            player: None,
            frames: 0,
            undo_groups: Vec::new(),
            redo_groups: Vec::new(),
            workspace: Default::default(),
            edit: Default::default(),
            wizard: Default::default(),
            lettering: Default::default(),
            creator: Default::default(),
            sewing: Default::default(),
            pattern_wizards: Default::default(),
            settings: Default::default(),
            background_texture: None,
            database: Default::default(),
            db_textures: Default::default(),
            db_chrome: Default::default(),
            remote: None,
            ui: Default::default(),
        };
        app.refresh();
        app.saved_revision = app.revision;
        app.launch(launch);
        app
    }

    fn launch(&mut self, l: Launch) {
        if let Some(path) = &l.user_settings
            && let Err(e) = self.session.with_user_settings(path)
        {
            self.status = format!("Could not read the user settings ({e}); they are not saved");
        }
        // Screenshots use the default window layout, not the user's.
        if let Some(path) = &l.user_settings
            && l.screenshot.is_none()
        {
            let ui = openstitch_app::editor::prefs::path_beside(path);
            self.ui.prefs = openstitch_app::editor::prefs::UiPrefs::load(&ui);
            self.ui.prefs_path = Some(ui);
        }
        if let Some(t) = l.theme {
            self.ui.prefs.theme = t;
        }
        if l.demo {
            for r in demo_design() {
                self.execute(&r);
            }
        }
        if let Some(p) = l.open {
            self.open_path(&p);
        }
        if !l.select.is_empty() {
            self.selection = l
                .select
                .into_iter()
                .filter(|id| self.session.design().index_of(*id).is_some())
                .collect();
        }
        if let Some(f) = l.simulate {
            self.simulate =
                Some(((self.stats.stitches as f64) * f.clamp(0.0, 1.0)).round() as usize);
        }
        if let Some(t) = l.tool {
            self.tools.set_tool(t);
        }
        self.initial_zoom = l.zoom;
        for v in &l.show {
            match v.as_str() {
                "realistic" => self.workspace.realistic = true,
                "solid" => self.workspace.solid = true,
                "fabric" => self.workspace.view.fabric_open = true,
                "reference" => self.workspace.reference = true,
                "settings" => self.workspace.settings_open = true,
                "attributes" => self.settings.sets_open = true,
                "charts" => self.settings.charts_open = true,
                "text" => self.lettering.text.open = true,
                "fonts" => self.lettering.fonts.open = true,
                "monogram" => self.lettering.monogram.open = true,
                "namedrop" => self.lettering.name_drop.open = true,
                "templates" => crate::lettering_ui::open_template_wizard(self),
                "measure" => self.toggle_measure(),
                "norulers" => self.workspace.rulers = false,
                "creator" => self.creator.open = true,
                "sdf" => {
                    self.sewing.open = true;
                    if let Some(q) = self.session.design().sewing.first() {
                        self.sewing.editor =
                            openstitch_app::editor::sewing::SewingEditor::from_stitch(q);
                    }
                }
                "usb" => crate::transfer_ui::open(self),
                "arrange" => self.edit.arrange_open = true,
                "appmenu" => {
                    // Whichever window shows: the editor's or a suite window's.
                    self.ui.open_app_menu = true;
                    self.creator.open_app_menu = true;
                    self.sewing.open_app_menu = true;
                }
                "property" => self.ui.property_open = true,
                "shortcuts" => self.ui.shortcuts_open = true,
                "floating" => {
                    let p = &mut self.ui.prefs.panes;
                    p.sewing_order.floating = true;
                    p.attributes.floating = true;
                }
                _ => {}
            }
        }
        if let Some(p) = &l.pattern {
            self.creator.open = true;
            match std::fs::read_to_string(p)
                .map_err(|e| e.to_string())
                .and_then(|t| openstitch_design::patterns::parse(&t))
            {
                Ok(pattern) => {
                    self.creator.doc =
                        openstitch_app::editor::creator::Creator::from_pattern(&pattern)
                }
                Err(e) => self.status = format!("{}: {e}", p.display()),
            }
        }
        if let Some(p) = l.image {
            self.wizard.path = Some(p);
            self.wizard.open = true;
            if let Some(m) = l.convert {
                self.wizard.method = m;
                crate::wizard_ui::run(self);
            }
        }
        if let Some(kind) = l.wizard {
            if l.apply_wizard {
                // Keep the patterns selected for the dialog, unless the wizard replaced them.
                let mut picked = self.selection.clone();
                crate::pattern_wizard_ui::run(self, kind);
                picked.retain(|id| self.session.design().index_of(*id).is_some());
                if !picked.is_empty() {
                    self.selection = picked;
                }
            }
            self.pattern_wizards.open = Some(kind);
        }
        if let Some(p) = &l.database {
            self.database.show(Some(p));
        }
        if let Some(v) = l.database_view {
            self.database.view = v;
        }
        if let Some(d) = &l.database_dialog {
            use openstitch_app::editor::database::Dialog;
            match d.as_str() {
                "search" => self.database.dialog = Dialog::Search,
                "convert" => self.database.dialog = Dialog::Convert,
                "catalog" => self.database.dialog = Dialog::Catalog,
                "delete" => self.database.request_delete(),
                "newfolder" => self.database.dialog = Dialog::NewFolder("New Folder".into()),
                _ => {}
            }
        }
        if let Some(t) = l.ribbon {
            self.ui.tab = t;
        }
        if let Some(p) = l.pane {
            self.ui.pane_tab = p;
        }
        if !l.script.is_empty() {
            self.player = Some(crate::script::Player::new(l.script));
        }
        if l.demo {
            self.saved_revision = self.revision;
        }
        self.screenshot = l.screenshot.map(|path| Screenshot {
            path,
            frames: 0,
            requested: false,
            done: Arc::new(AtomicBool::new(false)),
        });
    }

    // MARK: commands

    /// Runs one command; reports failures in the status bar. Returns the command's result.
    pub fn execute(&mut self, r: &CommandRequest) -> Option<Value> {
        let effect = self.effects.get(r.command).copied();
        let before = (effect == Some(Effect::Edit)).then(|| self.session.design().clone());
        match self.session.execute(r.command, Some(&r.params)) {
            Ok(v) => {
                if let Some(before) = before {
                    // An edit that changed nothing records no engine history step.
                    if *self.session.design() != before {
                        self.undo_groups.push(1);
                        self.redo_groups.clear();
                    }
                }
                if matches!(effect, Some(Effect::Edit | Effect::History)) {
                    self.refresh();
                }
                Some(v)
            }
            Err(e) => {
                self.status = e.to_string();
                None
            }
        }
    }

    /// Runs requests in order, stopping at the first failure; they undo as one step.
    pub fn execute_all(&mut self, requests: &[CommandRequest]) -> Option<Value> {
        self.grouped(|app| {
            let mut last = None;
            for r in requests {
                last = Some(app.execute(r)?);
            }
            last
        })
    }

    /// Runs `f` so that every edit it makes is undone by a single Undo.
    pub fn grouped<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let start = self.undo_groups.len();
        let out = f(self);
        let steps: usize = self.undo_groups.drain(start..).sum();
        if steps > 0 {
            self.undo_groups.push(steps);
        }
        out
    }

    /// What the tools see of the document; `design` is a snapshot of the session's design.
    pub fn tool_context<'a>(&'a self, design: &'a openstitch_design::Design) -> ToolContext<'a> {
        ToolContext {
            design,
            selection: &self.selection,
            units_per_point: self.viewport.unwrap_or_default().units_per_point(),
            scale_takes_origin: self.scale_takes_origin,
        }
    }

    /// Runs `f` on the tool controller with the current context and applies its effects.
    pub fn with_tools(
        &mut self,
        f: impl FnOnce(&mut ToolController, &ToolContext) -> Vec<ToolEffect>,
    ) {
        let design = self.session.design().clone();
        let ctx = ToolContext {
            design: &design,
            selection: &self.selection,
            units_per_point: self.viewport.unwrap_or_default().units_per_point(),
            scale_takes_origin: self.scale_takes_origin,
        };
        self.tools.set_snap(self.workspace.grid.snap_step());
        let effects = f(&mut self.tools, &ctx);
        self.apply_effects(effects);
    }

    pub fn apply_effects(&mut self, effects: Vec<ToolEffect>) {
        for e in effects {
            match crate::edit_ui::filter_effect(self, e) {
                ToolEffect::Select(sel) => self.selection = sel,
                ToolEffect::Execute {
                    requests,
                    select_result,
                } => {
                    if let Some(result) = self.execute_all(&requests) {
                        if select_result {
                            self.selection = CommandRequest::created_ids(&result);
                        }
                        self.status = format!("{} done", requests[0].command);
                    }
                }
            }
        }
    }

    /// Recompiles after a change, keeping selection and simulator position valid.
    pub(crate) fn refresh(&mut self) {
        self.revision += 1;
        let d = self.session.design();
        self.selection.retain(|id| d.index_of(*id).is_some());
        self.pattern = Compiler::for_design(d).compile(d);
        self.stats = PatternStats::of(&self.pattern);
        self.workspace.estimate = Some(openstitch_design::estimate(&self.pattern, &d.machine));
        if self.simulate.is_some_and(|n| n >= self.stats.stitches) {
            self.simulate = None;
        }
        self.drafts.clear();
    }

    pub fn ensure_render_model(&mut self) {
        let limit = self.simulate.unwrap_or(usize::MAX);
        if self.render_key != Some((self.revision, limit)) {
            self.render = RenderModel::build(&self.pattern, limit);
            self.render_key = Some((self.revision, limit));
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn undo(&mut self) {
        let steps = self.undo_groups.pop().unwrap_or(1);
        let done = self.history("history.undo", steps);
        if done > 0 {
            self.redo_groups.push(done);
        }
    }

    pub fn redo(&mut self) {
        let steps = self.redo_groups.pop().unwrap_or(1);
        let done = self.history("history.redo", steps);
        if done > 0 {
            self.undo_groups.push(done);
        }
    }

    /// Runs `history.undo` / `history.redo` up to `steps` times; returns how many ran.
    fn history(&mut self, command: &'static str, steps: usize) -> usize {
        let mut done = 0;
        while done < steps
            && if command == "history.undo" {
                self.session.can_undo()
            } else {
                self.session.can_redo()
            }
        {
            if self
                .execute(&CommandRequest::new(command, json!({})))
                .is_none()
            {
                break;
            }
            done += 1;
        }
        done
    }

    pub fn select_all(&mut self) {
        self.selection = self
            .session
            .design()
            .objects
            .iter()
            .filter(|o| o.visible)
            .map(|o| o.id)
            .collect();
    }

    pub fn on_selection(&mut self, make: impl Fn(&BTreeSet<u64>) -> CommandRequest) {
        if self.selection.is_empty() {
            return;
        }
        let r = make(&self.selection);
        if let Some(result) = self.execute(&r) {
            let created = CommandRequest::created_ids(&result);
            if !created.is_empty() {
                self.selection = created;
            }
        }
    }

    // MARK: files

    pub fn new_design(&mut self) {
        if self
            .execute(&CommandRequest::new(
                "design.new",
                json!({ "name": "Untitled" }),
            ))
            .is_some()
        {
            self.after_load(None);
        }
    }

    fn after_load(&mut self, path: Option<PathBuf>) {
        self.path = path;
        self.selection.clear();
        self.simulate = None;
        self.tools.cancel();
        self.fit_requested = true;
        self.saved_revision = self.revision;
    }

    pub fn open_dialog(&mut self) {
        let mut exts = vec![openstitch_engine::project::EXTENSION];
        exts.extend(FileType::ALL.iter().map(|t| t.extension()));
        let file = rfd::FileDialog::new()
            .add_filter("Designs and embroidery files", &exts)
            .add_filter(
                "OpenStitch design",
                &[openstitch_engine::project::EXTENSION],
            )
            .add_filter("Embroidery files", &exts[1..])
            .pick_file();
        if let Some(p) = file {
            self.open_path(&p);
        }
    }

    /// `.osd` opens as the design; a machine file opens as a new design holding it as an
    /// imported stitch pattern on a page large enough for it.
    pub fn open_path(&mut self, p: &Path) {
        let path = p.display().to_string();
        if is_project(p) {
            if self
                .execute(&CommandRequest::new(
                    "project.open",
                    json!({ "path": path }),
                ))
                .is_some()
            {
                self.after_load(Some(p.to_path_buf()));
                self.status = format!("Opened {}", p.display());
                self.ui.add_recent(p);
            }
            return;
        }
        let (w, h) = machine_file_size_mm(p).unwrap_or((100.0, 100.0));
        let name = p
            .file_stem()
            .map_or_else(|| "Untitled".into(), |s| s.to_string_lossy().into_owned());
        let page = |v: f64| ((v + 10.0) / 10.0).ceil().max(10.0) * 10.0;
        let steps = [CommandRequest::new(
            "design.new",
            json!({ "name": name, "width": page(w), "height": page(h) }),
        )];
        if self.grouped(|app| app.execute_all(&steps).is_some() && app.import(p)) {
            self.after_load(None);
            self.ui.add_recent(p);
            self.status = format!(
                "Opened {} as a new design (Save to keep it as .osd)",
                p.display()
            );
        }
    }

    pub fn import_dialog(&mut self) {
        let exts: Vec<_> = FileType::ALL.iter().map(|t| t.extension()).collect();
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Embroidery files", &exts)
            .pick_file()
            && self.grouped(|app| app.import(&p))
        {
            self.status = format!("Imported {}", p.display());
        }
    }

    /// File ▸ Import SVG: the artwork's paths become a group of editable shapes, centred.
    pub fn import_svg_dialog(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .add_filter("SVG vector image", &["svg"])
            .pick_file()
        else {
            return;
        };
        let request = CommandRequest::new("import.svg", json!({ "path": p.display().to_string() }));
        if let Some(result) = self.grouped(|app| app.execute(&request)) {
            self.selection = CommandRequest::created_ids(&result);
            self.status = format!("Imported {}", p.display());
        }
    }

    /// File ▸ Background Image: a PNG, JPEG or BMP shown behind the design, fitted to the page.
    pub fn background_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
            .pick_file()
            && self
                .execute(&CommandRequest::new(
                    "background.set",
                    json!({ "path": p.display().to_string() }),
                ))
                .is_some()
        {
            self.status = format!("Background {}", p.display());
        }
    }

    /// File ▸ Background Image ▸ Paste: the clipboard image becomes the background. It is kept
    /// as a PNG in the app's data folder (named by content), since the project only references it.
    pub fn paste_background(&mut self) {
        let image = arboard::Clipboard::new().and_then(|mut c| c.get_image());
        let stored = match image {
            Ok(img) => pasted_images_dir()
                .ok_or_else(|| "no folder to keep pasted images in".to_string())
                .and_then(|dir| {
                    store_pasted_image(
                        &dir,
                        img.width as u32,
                        img.height as u32,
                        img.bytes.into_owned(),
                    )
                }),
            Err(e) => Err(format!("no image on the clipboard ({e})")),
        };
        match stored {
            Ok(path) => {
                if self
                    .execute(&CommandRequest::new(
                        "background.set",
                        json!({ "path": path.display().to_string() }),
                    ))
                    .is_some()
                {
                    self.status = "Background pasted from the clipboard".into();
                }
            }
            Err(e) => self.status = format!("Paste background: {e}"),
        }
    }

    /// Import into design: adds the machine file as a stitch pattern, centred on the page.
    pub fn import(&mut self, p: &Path) -> bool {
        let Some(result) = self.execute(&CommandRequest::new(
            "stitches.import",
            json!({ "path": p.display().to_string() }),
        )) else {
            return false;
        };
        let ids = CommandRequest::created_ids(&result);
        self.execute(&CommandRequest::new(
            "object.center",
            json!({ "ids": CommandRequest::ids(&ids) }),
        ));
        self.selection = ids;
        true
    }

    pub fn save(&mut self, save_as: bool) {
        let path = match (&self.path, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new()
                .add_filter(
                    "OpenStitch design",
                    &[openstitch_engine::project::EXTENSION],
                )
                .set_file_name(format!(
                    "{}.{}",
                    self.session.design().name,
                    openstitch_engine::project::EXTENSION
                ))
                .save_file(),
        };
        let Some(path) = path else { return };
        if self
            .execute(&CommandRequest::new(
                "project.save",
                json!({ "path": path.display().to_string() }),
            ))
            .is_some()
        {
            self.status = format!("Saved {}", path.display());
            self.ui.add_recent(&path);
            self.path = Some(path);
            self.saved_revision = self.revision;
        }
    }

    pub fn export(&mut self, ty: FileType) {
        self.export_with(ty, None);
    }

    /// PES versions offered in File ▸ Export ▸ PES: `pesVersion` value and menu label.
    pub const PES_VERSIONS: [(&'static str, &'static str); 4] = [
        ("6", "Version 6 — RGB colours, names, stitch group"),
        ("6t", "Version 6, stitches only"),
        ("1", "Version 1 — stitch group, chart colours"),
        ("1t", "Version 1, stitches only (smallest)"),
    ];

    /// Exports `ty`; `pes_version` picks the PES variant (`None` = the engine default).
    pub fn export_with(&mut self, ty: FileType, pes_version: Option<&str>) {
        let name = format!("{}.{}", self.session.design().name, ty.extension());
        let Some(path) = rfd::FileDialog::new()
            .add_filter(ty.extension().to_uppercase(), &[ty.extension()])
            .set_file_name(name)
            .save_file()
        else {
            return;
        };
        // A design over the format's limits is written as NAME-1.ext, NAME-2.ext…
        let mut params = json!({
            "path": path.display().to_string(),
            "format": ty.extension(),
            "split": true,
        });
        if let Some(v) = pes_version {
            params["pesVersion"] = json!(v);
        }
        if let Some(r) = self.execute(&CommandRequest::new("design.export", params)) {
            let files = r["paths"].as_array().map_or(1, Vec::len);
            self.status = if files > 1 {
                format!(
                    "Exported {} files ({}-1 …) — the design exceeds one {} file",
                    files,
                    path.with_extension("").display(),
                    ty.extension().to_uppercase()
                )
            } else {
                format!(
                    "Exported {} ({} stitches)",
                    path.display(),
                    r["stats"]["stitches"]
                )
            };
        }
    }

    // MARK: frame

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self.path.as_ref().and_then(|p| p.file_name()).map_or_else(
            || self.session.design().name.clone(),
            |n| n.to_string_lossy().into_owned(),
        );
        let program = if self.database.open {
            Some(openstitch_app::editor::suite::DATABASE.title)
        } else if self.creator.open {
            Some(openstitch_app::editor::suite::CREATOR.title)
        } else if self.sewing.open {
            Some(openstitch_app::editor::suite::FACTORY.title)
        } else {
            None
        };
        let title = match program {
            Some(p) => format!("{p} — OpenStitch"),
            None => format!(
                "{}{} — OpenStitch",
                name,
                if self.is_dirty() { " •" } else { "" }
            ),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_shift = |k| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, k);
        let take = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        // Order matters: the shifted variants first.
        if take(cmd_shift(Key::Z)) || take(cmd(Key::Y)) {
            self.redo();
        } else if take(cmd(Key::Z)) {
            self.undo();
        }
        if take(cmd_shift(Key::S)) {
            self.save(true);
        } else if take(cmd(Key::S)) {
            self.save(false);
        }
        if take(cmd(Key::O)) {
            self.open_dialog();
        }
        if take(cmd_shift(Key::I)) {
            self.import_dialog();
        }
        if take(cmd(Key::N)) {
            self.new_design();
        }
        if take(cmd(Key::Num0)) {
            self.fit_requested = true;
        }
        if take(cmd(Key::Equals)) || take(cmd(Key::Plus)) {
            crate::panels::zoom_center(self, 1.25);
        }
        if take(cmd(Key::Minus)) {
            crate::panels::zoom_center(self, 1.0 / 1.25);
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let take = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        // Copy and Paste arrive as clipboard events, not key presses.
        let (copy, paste) = ctx.input(|i| {
            (
                i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
            )
        });
        if copy {
            crate::ribbon::run(self, openstitch_app::editor::ribbon::Action::Copy, ctx);
        }
        if paste {
            crate::ribbon::run(self, openstitch_app::editor::ribbon::Action::Paste, ctx);
        }
        if take(cmd(Key::A)) {
            self.select_all();
        }
        if take(cmd(Key::D)) {
            self.on_selection(CommandRequest::duplicate);
        }
        if take(cmd_shift(Key::G)) {
            crate::edit_ui::ungroup(self);
        } else if take(cmd(Key::G)) {
            crate::edit_ui::group(self);
        }
        if take(KeyboardShortcut::new(Modifiers::NONE, Key::Delete))
            || take(KeyboardShortcut::new(Modifiers::NONE, Key::Backspace))
        {
            // The Point tool deletes the selected vertex; otherwise the selected objects.
            let mut point_deleted = false;
            self.with_tools(|t, c| {
                let effects = t.delete_point(c);
                point_deleted = effects.is_some();
                effects.unwrap_or_default()
            });
            if !point_deleted {
                self.on_selection(CommandRequest::delete);
            }
        }
        if take(KeyboardShortcut::new(Modifiers::NONE, Key::Escape)) {
            if self.edit.cancel() {
                self.tools.cancel();
                self.status = "Cancelled".into();
            } else if self.workspace.measure.active {
                self.workspace.measure.active = false;
                self.workspace.measure.clear();
            } else if self.tools.is_busy() {
                self.tools.cancel();
            } else {
                self.selection.clear();
            }
        }
        if take(KeyboardShortcut::new(Modifiers::NONE, Key::Enter)) {
            let effects = self.tools.finish();
            self.apply_effects(effects);
        }
        for (key, dx, dy) in [
            (Key::ArrowLeft, -1.0, 0.0),
            (Key::ArrowRight, 1.0, 0.0),
            (Key::ArrowUp, 0.0, -1.0),
            (Key::ArrowDown, 0.0, 1.0),
        ] {
            // Arrow: 0.1 mm, Shift+Arrow: 1 mm.
            for (mods, step) in [(Modifiers::SHIFT, 10.0), (Modifiers::NONE, 1.0)] {
                if take(KeyboardShortcut::new(mods, key)) {
                    self.on_selection(|s| CommandRequest::move_by(s, dx * step, dy * step));
                }
            }
        }
        let typed: Vec<char> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Text(t) if t.chars().count() == 1 => t.chars().next(),
                    _ => None,
                })
                .collect()
        });
        if !ctx.input(|i| i.modifiers.command) {
            for c in typed {
                // Point tool: C curves, S straightens the segment under the pointer.
                if c.eq_ignore_ascii_case(&'m') {
                    self.ui.pan = false;
                    self.toggle_measure();
                } else if c.eq_ignore_ascii_case(&'h') {
                    crate::ribbon::run(self, openstitch_app::editor::ribbon::Action::Pan, ctx);
                } else if self.tools.tool() == Tool::Point && matches!(c, 'c' | 'C' | 's' | 'S') {
                    let curved = c.eq_ignore_ascii_case(&'c');
                    self.with_tools(|t, ctx| t.set_segment_under_pointer(curved, ctx));
                } else if let Some(t) = Tool::from_shortcut(c) {
                    self.tools.set_tool(t);
                    self.workspace.measure.active = false;
                    self.ui.pan = false;
                }
            }
        }
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.screenshot else {
            return;
        };
        if shot.done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        shot.frames += 1;
        ctx.request_repaint();
        let scripted = self.player.as_ref().is_some_and(|p| !p.is_done());
        if shot.frames < 8 || shot.requested || scripted {
            return;
        }
        shot.requested = true;
        let (path, done, repaint) = (shot.path.clone(), shot.done.clone(), ctx.clone());
        ctx.request_screenshot(move |image| {
            let rgba: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|c| c.to_srgba_unmultiplied())
                .collect();
            let [w, h] = image.size;
            match openstitch_app::png::encode_rgba(w as u32, h as u32, &rgba) {
                Some(bytes) => {
                    if let Err(e) = std::fs::write(&path, bytes) {
                        eprintln!("cannot write {}: {e}", path.display());
                    }
                }
                None => eprintln!("screenshot has no pixels"),
            }
            done.store(true, Ordering::SeqCst);
            repaint.request_repaint();
        });
    }

    /// View ▸ Snap to grid on or off.
    pub fn toggle_snap_to_grid(&mut self) {
        self.workspace.grid.snap = !self.workspace.grid.snap;
    }

    /// Measure tool on or off; it takes over the canvas pointer while on.
    pub fn toggle_measure(&mut self) {
        let m = &mut self.workspace.measure;
        m.active = !m.active;
        m.clear();
        self.status = if m.active {
            "Measure: drag on the canvas (Shift = 45° steps) · Esc or M ends".into()
        } else {
            "Ready".into()
        };
    }

    /// Advances the simulator while playing.
    fn tick(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let dt = f64::from(ctx.input(|i| i.stable_dt)).min(0.1);
        self.play_accumulator += dt * self.play_speed;
        let step = self.play_accumulator.floor() as usize;
        self.play_accumulator -= step as f64;
        let n = self.simulate.unwrap_or(0) + step;
        if n >= self.stats.stitches {
            self.simulate = None;
            self.playing = false;
        } else {
            self.simulate = Some(n);
        }
        ctx.request_repaint();
    }
}

impl eframe::App for EditorApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.frames += 1;
        // Start once the canvas has laid out and fitted the page.
        let (Some(player), Some(vp)) = (&mut self.player, self.viewport) else {
            return;
        };
        if self.frames < 4 {
            return;
        }
        if let Some((events, modifiers)) = player.next_frame(&vp) {
            raw.events.push(egui::Event::ModifiersChanged(modifiers));
            raw.events.extend(events);
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.ui.apply_theme(&ctx);
        self.serve_remote();
        if self.database.open || self.creator.open || self.sewing.open {
            // The other suite windows take the whole window, as separate programs do.
            self.update_title(&ctx);
            if self.database.open {
                crate::database_ui::database(self, ui);
            } else if self.creator.open {
                crate::creator_ui::creator(self, ui);
            } else {
                crate::sewing_ui::factory(self, ui);
            }
            self.screenshot_step(&ctx);
            return;
        }
        self.shortcuts(&ctx);
        self.tick(&ctx);
        self.update_title(&ctx);
        egui::Panel::top("ribbon").show(ui, |ui| crate::ribbon::ribbon(self, ui));
        egui::Panel::bottom("status").show(ui, |ui| crate::panes::status_bar(self, ui));
        crate::panes::docked(self, ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| crate::canvas::canvas(self, ui));
        crate::panes::floating_panes(self, &ctx);
        crate::panes::windows(self, &ctx);
        crate::workspace_ui::design_settings_window(self, &ctx);
        crate::print_ui::window(self, &ctx);
        crate::transfer_ui::window(self, &ctx);
        crate::edit_ui::windows(self, &ctx);
        crate::settings_ui::windows(self, &ctx);
        crate::wizard_ui::image_wizard_window(self, &ctx);
        crate::lettering_ui::windows(self, &ctx);
        crate::pattern_wizard_ui::windows(self, &ctx);
        crate::view_ui::windows(self, &ctx);
        let canvas = self.workspace.canvas;
        crate::workspace_ui::reference_window(self, &ctx, canvas);
        self.screenshot_step(&ctx);
    }
}

fn is_project(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(openstitch_engine::project::EXTENSION))
}

/// Width and height (mm) of a machine file, used only to size the page before importing.
fn machine_file_size_mm(p: &Path) -> Option<(f64, f64)> {
    let ty = FileType::from_path(p)?;
    let meta = std::fs::metadata(p).ok()?;
    if meta.len() > 64 << 20 {
        return None;
    }
    let pattern = ty.read(&std::fs::read(p).ok()?).ok()?;
    let b = pattern.bounds();
    Some((b.width() / 10.0, b.height() / 10.0))
}

/// Sample design for `--demo` (documentation screenshots), built with ordinary commands.
pub fn demo_design() -> Vec<CommandRequest> {
    let c = |id: &'static str, p: Value| CommandRequest::new(id, p);
    vec![
        c(
            "design.new",
            json!({ "name": "Badge", "width": 100, "height": 100 }),
        ),
        c(
            "shape.ellipse",
            json!({ "cx": 50, "cy": 50, "rx": 40, "ry": 34, "fill": "#1f4fa0", "outline": "#101010", "name": "Badge" }),
        ),
        c(
            "object.setRegion",
            json!({ "id": 1, "angle": 30, "density": 3 }),
        ),
        c("object.setHoleSewing", json!({ "id": 1, "enabled": true })),
        c(
            "shape.polygon",
            json!({ "name": "Star", "fill": "#f0c419", "points": star(50.0, 47.0, 22.0, 9.5) }),
        ),
        c(
            "object.setRegion",
            json!({ "id": 2, "type": "satin", "angle": 90 }),
        ),
        c(
            "shape.rectangle",
            json!({ "x": 24, "y": 74, "width": 52, "height": 8, "fill": "#c8102e", "name": "Ribbon" }),
        ),
        c(
            "object.setLine",
            json!({ "id": 3, "type": "zigzag", "color": "#ffffff", "width": 1.2 }),
        ),
    ]
}

fn star(cx: f64, cy: f64, r: f64, inner: f64) -> Value {
    let pts: Vec<Value> = (0..10)
        .map(|i| {
            let a = std::f64::consts::PI * (i as f64 / 5.0) - std::f64::consts::FRAC_PI_2;
            let rr = if i % 2 == 0 { r } else { inner };
            json!([
                ((cx + rr * a.cos()) * 100.0).round() / 100.0,
                ((cy + rr * a.sin()) * 100.0).round() / 100.0
            ])
        })
        .collect();
    Value::Array(pts)
}

/// Where pasted background images are kept: the per-user application data folder.
fn pasted_images_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/OpenStitch/backgrounds"))
    } else if cfg!(windows) {
        env("APPDATA").map(|a| a.join("OpenStitch").join("backgrounds"))
    } else {
        env("XDG_DATA_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .map(|d| d.join("openstitch").join("backgrounds"))
    }
}

/// Encodes clipboard RGBA pixels as PNG in `dir`, named by a hash of the content, within the
/// image decoder's size limits. Returns the file's path.
pub fn store_pasted_image(
    dir: &Path,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> Result<PathBuf, String> {
    let raster =
        openstitch_digitize::Raster::from_rgba(width, height, rgba).map_err(|e| e.to_string())?;
    let png = raster.encode_png().map_err(|e| e.to_string())?;
    // FNV-1a: a stable name, so pasting the same image twice reuses the file.
    let hash = png.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("pasted-{hash:016x}.png"));
    std::fs::write(&path, png).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_pes_version_is_one_the_writer_knows() {
        for (v, _) in EditorApp::PES_VERSIONS {
            assert!(
                openstitch_formats::pes::WriteOptions::parse(v).is_some(),
                "{v}"
            );
        }
    }

    #[test]
    fn pasted_pixels_are_stored_as_png_and_become_the_background() {
        let dir = std::env::temp_dir().join(format!("openstitch-paste-{}", std::process::id()));
        let rgba: Vec<u8> = (0..4 * 3 * 2).map(|i| (i * 10) as u8).collect();
        let path = store_pasted_image(&dir, 3, 2, rgba.clone()).expect("store");
        assert_eq!(store_pasted_image(&dir, 3, 2, rgba).expect("again"), path);
        assert!(store_pasted_image(&dir, 3, 3, vec![0; 4]).is_err()); // size mismatch
        assert!(store_pasted_image(&dir, 100_000, 1, vec![0; 400_000]).is_err()); // too wide
        let mut app = EditorApp::new(Launch::default());
        app.execute(&CommandRequest::new(
            "background.set",
            json!({ "path": path.display().to_string() }),
        ))
        .expect("background");
        let b = app.session.design().background.clone().expect("set");
        assert_eq!((b.pixel_width, b.pixel_height), (3, 2));
        assert!(pasted_images_dir().is_some());
    }

    #[test]
    fn demo_design_runs_through_the_engine() {
        let app = EditorApp::new(Launch {
            demo: true,
            ..Launch::default()
        });
        assert_eq!(app.session.design().objects.len(), 3);
        assert!(app.stats.stitches > 1000, "{:?}", app.stats);
        assert_eq!(app.status, "Ready");
        assert!(!app.is_dirty());
    }

    #[test]
    fn opening_a_machine_file_imports_it_into_a_new_design() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../openstitch-engine/tests/fixtures/badge-golden.pes");
        let mut app = EditorApp::new(Launch::default());
        app.open_path(&fixture);
        let d = app.session.design();
        assert_eq!(d.objects.len(), 1, "{}", app.status);
        assert_eq!(d.objects[0].kind, openstitch_design::ObjectKind::Stitches);
        assert!(d.page.width >= 800.0, "page fits the 80 mm design");
        let b = d.objects[0].bounds().expect("bounds");
        assert!(
            (b.center().x - d.page.center().x).abs() < 1.0,
            "centred on the page"
        );
        assert!(
            app.path.is_none(),
            "a machine file is never overwritten by Save"
        );
        app.undo();
        assert!(
            app.session.design().objects.is_empty(),
            "new design + import + centre is one undo step"
        );
        assert!(!app.session.can_undo());
        app.redo();
        assert_eq!(app.session.design().objects.len(), 1, "and one redo step");
    }

    #[test]
    fn corner_scale_fallback_undoes_in_one_step() {
        use openstitch_app::editor::hit::rect_from;
        use openstitch_design::v2;
        let mut app = EditorApp::new(Launch::default());
        app.execute(&CommandRequest::new(
            "shape.rectangle",
            json!({ "x": 10, "y": 10, "width": 20, "height": 10 }),
        ));
        app.execute(&CommandRequest::new(
            "object.setVisible",
            json!({ "id": 1, "visible": true }),
        )); // no-op: no history
        let ids = BTreeSet::from([1]);
        let reqs = CommandRequest::scale_about(
            &ids,
            2.0,
            2.0,
            v2(100.0, 100.0),
            rect_from(v2(100.0, 100.0), v2(300.0, 200.0)),
            false,
        );
        assert_eq!(reqs.len(), 2);
        app.apply_effects(vec![ToolEffect::Execute {
            requests: reqs,
            select_result: false,
        }]);
        assert_eq!(
            app.session.design().objects[0]
                .bounds()
                .map(|b| (b.min_x, b.max_x)),
            Some((100.0, 500.0))
        );
        app.undo();
        let b = app.session.design().objects[0].bounds().expect("bounds");
        assert_eq!(
            (b.min_x, b.max_x),
            (100.0, 300.0),
            "scale and move undone together"
        );
        app.undo();
        assert!(
            app.session.design().objects.is_empty(),
            "the no-op edit took no undo step"
        );
        app.redo();
        app.redo();
        assert_eq!(
            app.session.design().objects[0].bounds().map(|b| b.max_x),
            Some(500.0)
        );
    }

    #[test]
    fn edits_bump_revision_and_failed_commands_report() {
        let mut app = EditorApp::new(Launch::default());
        let r0 = app.revision;
        assert!(
            app.execute(&CommandRequest::new("design.info", json!({})))
                .is_some()
        );
        assert_eq!(app.revision, r0, "reads do not recompile");
        app.execute(&CommandRequest::new(
            "shape.rectangle",
            json!({ "x": 0, "y": 0, "width": 10, "height": 10 }),
        ));
        assert_eq!(app.revision, r0 + 1);
        assert!(app.is_dirty());
        assert!(
            app.execute(&CommandRequest::new("object.delete", json!({ "id": 99 })))
                .is_none()
        );
        assert!(app.status.contains("not found"), "{}", app.status);
        app.simulate = Some(5);
        app.ensure_render_model();
        assert_eq!(app.render.shown, 5);
    }

    #[test]
    fn hoop_change_resizes_page_updates_estimate_and_undoes() {
        let mut app = EditorApp::new(Launch {
            demo: true,
            show: vec!["measure".into(), "norulers".into()],
            ..Default::default()
        });
        assert!(app.workspace.measure.active && !app.workspace.rulers);
        let before = app.workspace.estimate.clone().expect("estimate");
        assert!(before.needle_total() > 1000.0 && before.seconds > 60.0);
        app.execute(&CommandRequest::new(
            "machine.set",
            json!({ "hoop": "130x180", "speed": 400 }),
        ));
        assert_eq!(app.session.design().page.width, 1300.0);
        let slow = app.workspace.estimate.clone().expect("estimate");
        assert!(slow.seconds > before.seconds * 1.5);
        app.undo();
        assert_eq!(app.session.design().page.width, 1000.0);
        assert_eq!(app.workspace.estimate, Some(before));
        app.toggle_measure();
        assert!(!app.workspace.measure.active);
    }
}
