//! The Design Database (Options ▸ Design Database): a window of its own with the guide's menu
//! bar (File, Edit, Display, Option, Help), toolbar and status bar
//! ([`openstitch_app::editor::suite::DATABASE`]), a folder tree, the folder's designs as
//! thumbnails or details, a preview with file information, and the Search, Convert Format,
//! catalogue, new folder, rename and delete dialogs. State and file actions live in
//! [`openstitch_app::editor::database`]; this module only draws them.

use crate::app::EditorApp;
use crate::suite_chrome::{self, Host};
use eframe::egui::{self, Color32, RichText, Sense};
use openstitch_app::editor::database::{
    CatalogFormat, Dialog, View, display_path, file_name, format_time, roots, size_text,
};
use openstitch_app::editor::suite::{DATABASE, DbAction};
use openstitch_formats::FileType;
use openstitch_library::catalog::{ColorList, Paper, Style, Units};
use openstitch_library::info::{PROPERTY_KEYS, thread_label};
use openstitch_library::{Kind, thumb};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Designs decoded per frame, so large folders fill in progressively.
const LOAD_PER_FRAME: usize = 8;

/// Paths dragged from the contents onto a folder.
#[derive(Clone, Debug)]
struct DragPaths(Vec<PathBuf>);

/// GPU textures for thumbnails and the preview, dropped when files change.
#[derive(Default)]
pub struct Textures {
    generation: u64,
    thumbs: HashMap<PathBuf, egui::TextureHandle>,
    preview: Option<(PathBuf, egui::TextureHandle)>,
}

fn texture(ctx: &egui::Context, name: &str, img: &thumb::Image) -> egui::TextureHandle {
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [img.width as usize, img.height as usize],
        &img.rgba,
    );
    ctx.load_texture(name, image, egui::TextureOptions::LINEAR)
}

fn thumb_texture(
    app: &mut EditorApp,
    ctx: &egui::Context,
    p: &Path,
) -> Option<egui::TextureHandle> {
    if app.db_textures.generation != app.database.generation {
        app.db_textures = Textures {
            generation: app.database.generation,
            ..Textures::default()
        };
    }
    if let Some(t) = app.db_textures.thumbs.get(p) {
        return Some(t.clone());
    }
    let img = app.database.loaded(p)?.thumb.as_ref()?;
    let t = texture(ctx, &format!("db:{}", p.display()), img);
    app.db_textures.thumbs.insert(p.to_path_buf(), t.clone());
    Some(t)
}

// MARK: frame

/// Draws the whole mode in place of the editor's panels.
pub fn database(app: &mut EditorApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    shortcuts(app, &ctx);
    // Decode what is shown, a few files per frame.
    let order = app.database.items().to_vec();
    let mut order = order;
    if let Some(f) = app.database.focused().cloned() {
        order.insert(0, f);
    }
    if app.database.load_some(&order, LOAD_PER_FRAME) > 0 {
        ctx.request_repaint();
    }
    app.db_chrome.hover = None;
    egui::Panel::top("db-menu").show(ui, |ui| suite_chrome::menu_bar(app, ui, &DATABASE));
    if app.db_chrome.toolbar {
        egui::Panel::top("db-toolbar")
            .show(ui, |ui| suite_chrome::toolbar(app, ui, &DATABASE, labelled));
    }
    if app.db_chrome.status {
        egui::Panel::bottom("db-status").show(ui, |ui| {
            suite_chrome::status_bar(ui, app.db_chrome.hover, &app.database.status, |_| {});
        });
    }
    egui::Panel::left("db-folders")
        .default_size(240.0)
        .show(ui, |ui| folder_tree(app, ui));
    if app.database.show_preview || app.db_chrome.property {
        egui::Panel::right("db-preview")
            .default_size(300.0)
            .show(ui, |ui| preview(app, ui));
    }
    egui::CentralPanel::default().show(ui, |ui| contents(app, ui));
    dialogs(app, &ctx);
    crate::panes::about_window(app, &ctx);
}

fn shortcuts(app: &mut EditorApp, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() || app.database.dialog != Dialog::None {
        return;
    }
    let db = &mut app.database;
    let (events, cmd_a, del, f2, enter, back) = ctx.input_mut(|i| {
        (
            i.events.clone(),
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::A),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Backspace),
            i.consume_key(egui::Modifiers::NONE, egui::Key::F2),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp),
        )
    });
    for e in events {
        match e {
            egui::Event::Copy => db.copy(),
            egui::Event::Cut => db.cut(),
            egui::Event::Paste(_) => {
                db.paste();
            }
            _ => {}
        }
    }
    if cmd_a {
        db.select_all();
    }
    if del {
        db.request_delete();
    }
    if f2 {
        start_rename(app);
    }
    if back {
        go_up(app);
    }
    if enter && let Some(p) = app.database.focused().cloned() {
        open_in_editor(app, &p);
    }
}

fn go_up(app: &mut EditorApp) {
    if let Some(parent) = app
        .database
        .folder
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
    {
        app.database.navigate(&parent);
    }
}

fn start_rename(app: &mut EditorApp) {
    if let Some(p) = app.database.focused().cloned() {
        let stem = p
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        app.database.dialog = Dialog::Rename(p, stem);
    }
}

/// File ▸ Open in Layout & Editing: the design opens as the editor's design.
fn open_in_editor(app: &mut EditorApp, p: &Path) {
    app.open_path(p);
    app.database.open = false;
}

/// File ▸ Import Into Layout & Editing: added to the current design.
fn import_into_editor(app: &mut EditorApp, paths: &[PathBuf]) {
    let mut n = 0;
    for p in paths {
        if matches!(Kind::of(p), Some(Kind::Machine(_))) && app.grouped(|a| a.import(p)) {
            n += 1;
        }
    }
    app.database.open = false;
    app.status = format!("Imported {n} design(s) from the Design Database");
}

/// Display ▸ Toolbar, Status Bar and Property, and the ScreenTip under the pointer.
pub struct Chrome {
    pub toolbar: bool,
    pub status: bool,
    pub property: bool,
    hover: Option<&'static str>,
}

impl Default for Chrome {
    fn default() -> Self {
        Chrome {
            toolbar: true,
            status: true,
            property: true,
            hover: None,
        }
    }
}

fn catalog(app: &mut EditorApp, f: CatalogFormat) {
    app.database.catalog.format = f;
    app.database.dialog = Dialog::Catalog;
}

impl Host<DbAction> for EditorApp {
    fn enabled(&self, a: DbAction) -> bool {
        use DbAction::*;
        let db = &self.database;
        let any = !db.selection.is_empty();
        let folder = db.folder.is_some();
        match a {
            OpenInEditor | ImportIntoEditor | ConvertFormat | Delete | Rename | Cut | Copy => any,
            Paste => db.clipboard.is_some() && folder,
            NewFolder | Search | SelectAll | Refresh => folder,
            ParentFolder => db.folder.as_deref().and_then(Path::parent).is_some(),
            _ => true,
        }
    }

    fn active(&self, a: DbAction) -> bool {
        use DbAction::*;
        let db = &self.database;
        match a {
            LargeThumbnails => db.view == View::Large,
            SmallThumbnails => db.view == View::Small,
            Details => db.view == View::Details,
            Preview => db.show_preview,
            Property => self.db_chrome.property,
            Toolbar => self.db_chrome.toolbar,
            StatusBar => self.db_chrome.status,
            _ => false,
        }
    }

    fn run(&mut self, a: DbAction, ctx: &egui::Context) {
        use DbAction::*;
        if !suite_chrome::enabled(self, a) {
            return;
        }
        match a {
            OpenInEditor => {
                if let Some(p) = self.database.focused().cloned() {
                    open_in_editor(self, &p);
                }
            }
            ImportIntoEditor => {
                let sel = self.database.selected();
                import_into_editor(self, &sel);
            }
            NewFolder => self.database.dialog = Dialog::NewFolder("New Folder".into()),
            ConvertFormat => self.database.dialog = Dialog::Convert,
            Delete => self.database.request_delete(),
            Rename => start_rename(self),
            Search => self.database.dialog = Dialog::Search,
            PrintSetup | Print => catalog(self, CatalogFormat::Pdf),
            PrintPreview => catalog(self, CatalogFormat::Png),
            ImageHtml => catalog(self, CatalogFormat::Html),
            Csv => catalog(self, CatalogFormat::Csv),
            Exit => self.database.open = false,
            Cut => self.database.cut(),
            Copy => self.database.copy(),
            Paste => {
                self.database.paste();
            }
            SelectAll => self.database.select_all(),
            LargeThumbnails => self.database.view = View::Large,
            SmallThumbnails => self.database.view = View::Small,
            Details => self.database.view = View::Details,
            Preview => self.database.show_preview ^= true,
            Property => self.db_chrome.property ^= true,
            Refresh => self.database.refresh(),
            Toolbar => self.db_chrome.toolbar ^= true,
            StatusBar => self.db_chrome.status ^= true,
            Manual => ctx.open_url(egui::OpenUrl::new_tab(MANUAL)),
            About => self.ui.about_open = true,
            Support => ctx.open_url(egui::OpenUrl::new_tab(SUPPORT)),
            ParentFolder => go_up(self),
            ChooseFolder => {
                if let Some(f) = rfd::FileDialog::new().pick_folder() {
                    self.database.navigate(&f);
                }
            }
            _ => {}
        }
    }

    fn menu(&mut self, a: DbAction, ui: &mut egui::Ui) {
        use DbAction::*;
        let db = &mut self.database;
        match a {
            SystemUnit => {
                ui.radio_value(&mut db.units, Units::Mm, "mm");
                ui.radio_value(&mut db.units, Units::Inch, "inch");
            }
            ApplicationColor => suite_chrome::theme_menu(self, ui),
            DstSettings => {
                ui.horizontal(|ui| {
                    ui.label("Jumps that make a trim");
                    ui.add(egui::DragValue::new(&mut db.convert.dst_trim_jumps).range(1..=8));
                });
            }
            _ => {}
        }
    }

    fn set_hover(&mut self, tip: &'static str) {
        self.db_chrome.hover = Some(tip);
    }
}

const MANUAL: &str = "https://github.com/cristianonescu/OPE-Design11#readme";
const SUPPORT: &str = "https://github.com/cristianonescu/OPE-Design11/issues";

/// Toolbar buttons that show their label (the others are icons, as in the guide's toolbar).
fn labelled(a: DbAction) -> bool {
    use DbAction::*;
    !matches!(
        a,
        Cut | Copy | Paste | LargeThumbnails | SmallThumbnails | Details | Preview | Property
    )
}

// MARK: folders

fn folder_label(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn folder_tree(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.heading("Folders");
    if let Some(f) = &app.database.folder {
        ui.label(RichText::new(display_path(f)).weak().small());
    }
    ui.separator();
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for root in roots() {
                folder_node(app, ui, &root, 0);
            }
        });
}

fn folder_node(app: &mut EditorApp, ui: &mut egui::Ui, dir: &Path, depth: usize) {
    let id = ui.make_persistent_id(("db-folder", dir));
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    if app.database.expanded.remove(dir) {
        state.set_open(true);
    }
    let current = app.database.folder.as_deref() == Some(dir);
    let header = state.show_header(ui, |ui| {
        let r = ui.selectable_label(current, format!("📁 {}", folder_label(dir)));
        drop_target(app, ui, &r, dir);
        if current && app.database.reveal {
            app.database.reveal = false;
            // Scroll vertically only: a sliver far to the left keeps the tree's left edge.
            let x = r.rect.left() - 10_000.0;
            ui.scroll_to_rect(
                egui::Rect::from_x_y_ranges(x..=x + 1.0, r.rect.y_range()),
                Some(egui::Align::Center),
            );
        }
        if r.clicked() {
            app.database.navigate(dir);
        }
    });
    // Deep trees stop at the walk depth limit.
    if depth < app.database.limits.max_depth {
        header.body(|ui| {
            let children = app.database.subfolders(dir).to_vec();
            if children.is_empty() {
                ui.label(RichText::new("(no folders)").weak().small());
            }
            for c in children {
                folder_node(app, ui, &c, depth + 1);
            }
        });
    }
}

/// Designs dropped on a folder move there (same volume) or are copied (other volume);
/// hold Ctrl/Cmd to copy, Shift to move.
fn drop_target(app: &mut EditorApp, ui: &egui::Ui, r: &egui::Response, dir: &Path) {
    if r.dnd_hover_payload::<DragPaths>().is_some() {
        ui.painter().rect_stroke(
            r.rect,
            2.0,
            egui::Stroke::new(2.0, ui.visuals().selection.stroke.color),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(payload) = r.dnd_release_payload::<DragPaths>() {
        let m = ui.input(|i| i.modifiers);
        app.database.drop_on(&payload.0, dir, m.command, m.shift);
    }
}

// MARK: contents

fn contents(app: &mut EditorApp, ui: &mut egui::Ui) {
    if let Some((label, _)) = &app.database.results {
        let label = label.clone();
        ui.horizontal(|ui| {
            ui.strong(label);
            if ui.button("Back to folder").clicked() {
                app.database.clear_results();
            }
        });
        ui.separator();
    }
    let Some(_) = &app.database.folder else {
        ui.centered_and_justified(|ui| ui.label("Choose a folder on the left."));
        return;
    };
    if app.database.items().is_empty() && app.database.listing.folders.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No embroidery designs here.").weak())
        });
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| match app.database.view {
            View::Details => details(app, ui),
            v => tiles(app, ui, v.thumb_size()),
        });
}

fn drag_payload(app: &EditorApp, p: &Path) -> DragPaths {
    if app.database.selection.contains(p) {
        DragPaths(app.database.selected())
    } else {
        DragPaths(vec![p.to_path_buf()])
    }
}

/// Click, double-click, drag and context menu shared by tiles and detail rows.
fn item_interaction(app: &mut EditorApp, ui: &egui::Ui, r: &egui::Response, p: &Path) {
    if r.drag_started() {
        r.dnd_set_drag_payload(drag_payload(app, p));
    }
    if r.clicked() {
        let m = ui.input(|i| i.modifiers);
        app.database.click(p, m.command, m.shift);
    }
    if r.double_clicked() {
        open_in_editor(app, p);
        return;
    }
    r.context_menu(|ui| {
        if !app.database.selection.contains(p) {
            app.database.click(p, false, false);
        }
        if ui.button("Open in Layout & Editing").clicked() {
            open_in_editor(app, p);
        }
        if ui.button("Import Into Layout & Editing").clicked() {
            let sel = app.database.selected();
            import_into_editor(app, &sel);
        }
        ui.separator();
        if ui.button("Cut").clicked() {
            app.database.cut();
        }
        if ui.button("Copy").clicked() {
            app.database.copy();
        }
        if ui.button("Rename…").clicked() {
            start_rename(app);
        }
        if ui.button("Delete…").clicked() {
            app.database.request_delete();
        }
        ui.separator();
        if ui.button("Convert Format…").clicked() {
            app.database.dialog = Dialog::Convert;
        }
    });
}

fn tiles(app: &mut EditorApp, ui: &mut egui::Ui, side: f32) {
    let ctx = ui.ctx().clone();
    let folders = if app.database.results.is_none() {
        app.database.listing.folders.clone()
    } else {
        Vec::new()
    };
    let items = app.database.items().to_vec();
    let tile = egui::vec2(side + 16.0, side + 34.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        for f in &folders {
            let (rect, r) = ui.allocate_exact_size(tile, Sense::click());
            let painter = ui.painter_at(rect);
            if r.hovered() {
                painter.rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
            }
            painter.text(
                rect.center_top() + egui::vec2(0.0, side * 0.5 + 4.0),
                egui::Align2::CENTER_CENTER,
                "📁",
                egui::FontId::proportional(side * 0.45),
                ui.visuals().text_color(),
            );
            label_under(ui, &painter, rect, side, &folder_label(f));
            drop_target(app, ui, &r, f);
            let r = r.on_hover_text("Double-click to open");
            if r.double_clicked() {
                app.database.navigate(f);
                return;
            }
        }
        for p in &items {
            let (rect, r) = ui.allocate_exact_size(tile, Sense::click_and_drag());
            let selected = app.database.selection.contains(p);
            let painter = ui.painter_at(rect);
            if selected {
                painter.rect_filled(
                    rect,
                    4.0,
                    ui.visuals().selection.bg_fill.gamma_multiply(0.6),
                );
            } else if r.hovered() {
                painter.rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
            }
            let img_rect =
                egui::Rect::from_min_size(rect.min + egui::vec2(8.0, 6.0), egui::vec2(side, side));
            painter.rect_filled(img_rect, 2.0, Color32::WHITE);
            if let Some(t) = thumb_texture(app, &ctx, p) {
                painter.image(
                    t.id(),
                    img_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else if app.database.loaded(p).is_some() {
                painter.text(
                    img_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "⚠",
                    egui::FontId::proportional(side * 0.3),
                    Color32::from_rgb(200, 120, 0),
                );
            }
            label_under(ui, &painter, rect, side, &file_name(p));
            let hover = app.database.loaded(p).map(|l| match &l.info.summary {
                Ok(s) => format!(
                    "{}\n{}\n{} stitches · {} colours",
                    file_name(p),
                    size_text(app.database.units, s.width_mm, s.height_mm),
                    s.stitches,
                    s.total_colors()
                ),
                Err(e) => format!("{}\n{e}", file_name(p)),
            });
            let r = match hover {
                Some(h) => r.on_hover_text(h),
                None => r,
            };
            item_interaction(app, ui, &r, p);
        }
    });
}

fn label_under(ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, side: f32, text: &str) {
    let max = ((side + 12.0) / 6.5) as usize;
    let shown = if text.chars().count() > max {
        let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    } else {
        text.to_string()
    };
    painter.text(
        egui::pos2(rect.center().x, rect.min.y + side + 18.0),
        egui::Align2::CENTER_CENTER,
        shown,
        egui::FontId::proportional(12.0),
        ui.visuals().text_color(),
    );
}

fn details(app: &mut EditorApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let items = app.database.items().to_vec();
    let units = app.database.units;
    let side = View::Details.thumb_size();
    egui::Grid::new("db-details")
        .striped(true)
        .num_columns(8)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            for h in [
                "", "Name", "Size", "Stitches", "Changes", "Colours", "Format", "Modified",
            ] {
                ui.strong(h);
            }
            ui.end_row();
            for p in &items {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), Sense::hover());
                ui.painter().rect_filled(rect, 2.0, Color32::WHITE);
                if let Some(t) = thumb_texture(app, &ctx, p) {
                    ui.painter().image(
                        t.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                let selected = app.database.selection.contains(p);
                let r = ui.add(
                    egui::Button::selectable(selected, file_name(p)).sense(Sense::click_and_drag()),
                );
                item_interaction(app, ui, &r, p);
                let cells: Vec<String> = match app.database.loaded(p).map(|l| &l.info) {
                    Some(info) => {
                        let mut v = match &info.summary {
                            Ok(s) => vec![
                                size_text(units, s.width_mm, s.height_mm),
                                s.stitches.to_string(),
                                s.color_changes().to_string(),
                                s.total_colors().to_string(),
                            ],
                            Err(_) => vec![
                                "cannot be read".into(),
                                String::new(),
                                String::new(),
                                String::new(),
                            ],
                        };
                        v.push(info.kind.extension().to_uppercase());
                        v.push(info.modified.map(format_time).unwrap_or_default());
                        v
                    }
                    None => vec!["…".into(); 6],
                };
                for c in cells {
                    ui.label(c);
                }
                ui.end_row();
            }
        });
}

// MARK: preview and property

fn preview(app: &mut EditorApp, ui: &mut egui::Ui) {
    let show_image = app.database.show_preview;
    if show_image {
        ui.heading("Preview");
    }
    let Some(p) = app.database.focused().cloned() else {
        ui.label(RichText::new("Select a design to see it and its information.").weak());
        return;
    };
    let Some(loaded) = app.database.loaded(&p).cloned() else {
        ui.spinner();
        return;
    };
    let side = (ui.available_width() - 8.0).clamp(120.0, 420.0);
    if show_image && let Some(pattern) = &loaded.pattern {
        let stale =
            app.db_textures.preview.as_ref().is_none_or(|(q, _)| {
                *q != p || app.db_textures.generation != app.database.generation
            });
        if stale {
            let img = thumb::render(pattern, 400, [255, 255, 255]);
            let t = texture(ui.ctx(), "db-preview", &img);
            app.db_textures.preview = Some((p.clone(), t));
        }
        if let Some((_, t)) = &app.db_textures.preview {
            ui.add(egui::Image::new(t).fit_to_exact_size(egui::vec2(side, side)));
        }
    }
    if !app.db_chrome.property {
        return;
    }
    if show_image {
        ui.separator();
    }
    ui.heading("Property");
    let info = &loaded.info;
    let units = app.database.units;
    egui::Grid::new("db-property")
        .num_columns(2)
        .spacing([10.0, 3.0])
        .show(ui, |ui| {
            let mut row = |k: &str, v: String| {
                ui.label(RichText::new(k).weak());
                ui.label(v);
                ui.end_row();
            };
            row("File name", info.file_name());
            row("Format", info.kind.extension().to_uppercase());
            row("File size", format!("{} bytes", info.bytes));
            if let Some(m) = info.modified {
                row("Modified", format_time(m));
            }
            match &info.summary {
                Ok(s) => {
                    row("Size", size_text(units, s.width_mm, s.height_mm));
                    row("Stitches", s.stitches.to_string());
                    row(
                        "Sewing time",
                        format!("about {:.0} min (estimate)", s.sewing_minutes.ceil()),
                    );
                    row("Color changes", s.color_changes().to_string());
                    row("Total colors", s.total_colors().to_string());
                    for (k, label) in PROPERTY_KEYS {
                        if let Some(v) = s.properties.get(k) {
                            row(label, v.clone());
                        }
                    }
                }
                Err(e) => row("Cannot be read", e.clone()),
            }
        });
    if let Ok(s) = &info.summary {
        ui.separator();
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut app.database.color_list,
                ColorList::Changes,
                "Color changes",
            );
            ui.radio_value(
                &mut app.database.color_list,
                ColorList::Totals,
                "Total colors",
            );
        });
        let list = match app.database.color_list {
            ColorList::Changes => &s.sequence,
            ColorList::Totals => &s.totals,
        };
        egui::ScrollArea::vertical()
            .id_salt("db-colours")
            .max_height(ui.available_height() - 40.0)
            .show(ui, |ui| {
                for (i, t) in list.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                        ui.painter().rect(
                            r,
                            2.0,
                            Color32::from_rgb(t.red, t.green, t.blue),
                            egui::Stroke::new(1.0, Color32::from_gray(90)),
                            egui::StrokeKind::Inside,
                        );
                        ui.label(format!("{}. {}", i + 1, thread_label(t, false)));
                        if let Some(n) = t.catalog_number.as_deref() {
                            ui.label(RichText::new(n).weak());
                        }
                    });
                }
            });
    }
    ui.horizontal(|ui| {
        if ui.button("Open").clicked() {
            open_in_editor(app, &p);
        }
        if ui.button("Import").clicked() {
            import_into_editor(app, std::slice::from_ref(&p));
        }
    });
}

// MARK: dialogs

fn dialogs(app: &mut EditorApp, ctx: &egui::Context) {
    match app.database.dialog.clone() {
        Dialog::None => {}
        Dialog::Search => search_dialog(app, ctx),
        Dialog::Convert => convert_dialog(app, ctx),
        Dialog::Catalog => catalog_dialog(app, ctx),
        Dialog::NewFolder(name) => {
            if let Some(name) = name_dialog(app, ctx, "Create New Folder", "Folder name", name) {
                match app.database.create_folder(&name) {
                    Ok(_) => app.database.dialog = Dialog::None,
                    Err(e) => app.database.status = e,
                }
            }
        }
        Dialog::Rename(path, name) => {
            let title = format!("Rename {}", file_name(&path));
            if let Some(name) =
                name_dialog(app, ctx, &title, "New name (the extension stays)", name)
            {
                match app.database.rename(&path, &name) {
                    Ok(_) => app.database.dialog = Dialog::None,
                    Err(e) => app.database.status = e,
                }
            }
        }
        Dialog::ConfirmDelete(paths) => confirm_delete(app, ctx, &paths),
    }
}

/// A one-field dialog; returns the text when OK is pressed.
fn name_dialog(
    app: &mut EditorApp,
    ctx: &egui::Context,
    title: &str,
    prompt: &str,
    mut name: String,
) -> Option<String> {
    let mut result = None;
    let mut open = true;
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(prompt);
            let r = ui.text_edit_singleline(&mut name);
            r.request_focus();
            ui.label(RichText::new(&app.database.status).weak().small());
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    result = Some(name.clone());
                }
                if ui.button("Cancel").clicked() {
                    app.database.dialog = Dialog::None;
                }
            });
        });
    if !open {
        app.database.dialog = Dialog::None;
    } else if result.is_none() {
        match &mut app.database.dialog {
            Dialog::NewFolder(n) | Dialog::Rename(_, n) => *n = name,
            _ => {}
        }
    }
    result
}

fn confirm_delete(app: &mut EditorApp, ctx: &egui::Context, paths: &[PathBuf]) {
    let mut open = true;
    egui::Window::new("Delete")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(format!(
                "Move {} item(s) to the {}?",
                paths.len(),
                if cfg!(windows) {
                    "Recycle Bin"
                } else {
                    "Trash"
                }
            ));
            for p in paths.iter().take(8) {
                ui.label(RichText::new(file_name(p)).weak());
            }
            if paths.len() > 8 {
                ui.label(RichText::new(format!("and {} more", paths.len() - 8)).weak());
            }
            ui.horizontal(|ui| {
                if ui.button("Move to Trash").clicked() {
                    let trash = openstitch_library::ops::SystemTrash;
                    if let Err(e) = app.database.delete_confirmed(&trash) {
                        app.database.status = e;
                    }
                }
                if ui.button("Cancel").clicked() {
                    app.database.dialog = Dialog::None;
                }
            });
        });
    if !open {
        app.database.dialog = Dialog::None;
    }
}

fn range_row<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    on: &mut bool,
    label: &str,
    range: &mut (T, T),
    max: T,
    suffix: &str,
) {
    ui.checkbox(on, label);
    ui.add_enabled_ui(*on, |ui| {
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut range.0)
                    .range(T::from_f64(0.0)..=max)
                    .suffix(suffix),
            );
            ui.label("to");
            ui.add(
                egui::DragValue::new(&mut range.1)
                    .range(T::from_f64(0.0)..=max)
                    .suffix(suffix),
            );
        });
    });
    ui.end_row();
}

fn search_dialog(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = true;
    let mut run = false;
    let unit = match app.database.units {
        Units::Mm => " mm",
        Units::Inch => " in",
    };
    egui::Window::new("Search")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            let f = &mut app.database.search;
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.use_name, "File name");
                ui.add_enabled(
                    f.use_name,
                    egui::TextEdit::singleline(&mut f.name)
                        .hint_text("rose*  (* any text, ? one character)"),
                );
            });
            ui.checkbox(&mut f.use_formats, "Format");
            ui.add_enabled_ui(f.use_formats, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.set_max_width(380.0);
                    ui.add_space(22.0);
                    for e in Kind::extensions() {
                        let mut on = f.formats.contains(e);
                        if ui.checkbox(&mut on, e.to_uppercase()).changed() {
                            if on {
                                f.formats.insert(e);
                            } else {
                                f.formats.remove(e);
                            }
                        }
                    }
                });
            });
            ui.add_space(4.0);
            egui::Grid::new("db-search")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    range_row(
                        ui,
                        &mut f.use_stitches,
                        "Stitch count",
                        &mut f.stitches,
                        10_000_000,
                        "",
                    );
                    range_row(
                        ui,
                        &mut f.use_changes,
                        "Color changes",
                        &mut f.changes,
                        1000,
                        "",
                    );
                    range_row(
                        ui,
                        &mut f.use_colors,
                        "Total colors",
                        &mut f.colors,
                        1000,
                        "",
                    );
                    range_row(ui, &mut f.use_width, "Width", &mut f.width, 10_000.0, unit);
                    range_row(
                        ui,
                        &mut f.use_height,
                        "Height",
                        &mut f.height,
                        10_000.0,
                        unit,
                    );
                    ui.checkbox(&mut f.use_date, "Date modified");
                    ui.add_enabled_ui(f.use_date, |ui| {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut f.date.0).desired_width(90.0));
                            ui.label("to");
                            ui.add(egui::TextEdit::singleline(&mut f.date.1).desired_width(90.0));
                        });
                    });
                    ui.end_row();
                    for ((_, label), (on, v)) in PROPERTY_KEYS.iter().zip(f.properties.iter_mut()) {
                        ui.checkbox(on, *label);
                        ui.add_enabled(*on, egui::TextEdit::singleline(v).hint_text("words (PES)"));
                        ui.end_row();
                    }
                });
            ui.checkbox(&mut f.subfolders, "Search subfolders");
            ui.horizontal(|ui| {
                if ui.button("Search").clicked() {
                    run = true;
                }
                if ui.button("Reset").clicked() {
                    app.database.search = Default::default();
                }
            });
        });
    if run {
        match app.database.run_search() {
            Ok(_) => app.database.dialog = Dialog::None,
            Err(e) => app.database.status = e,
        }
    }
    if !open {
        app.database.dialog = Dialog::None;
    }
}

fn convert_dialog(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = true;
    let mut run = false;
    let n = app.database.selection.len();
    egui::Window::new("Convert Format")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(format!(
                "{n} design(s) selected. The originals are not changed."
            ));
            let f = &mut app.database.convert;
            egui::Grid::new("db-convert")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Format type");
                    egui::ComboBox::from_id_salt("db-convert-format")
                        .selected_text(f.target.extension().to_uppercase())
                        .show_ui(ui, |ui| {
                            for t in FileType::writable() {
                                ui.selectable_value(&mut f.target, t, t.extension().to_uppercase());
                            }
                        });
                    ui.end_row();
                    ui.label("Output to");
                    ui.vertical(|ui| {
                        let mut same = f.folder.is_none();
                        if ui.radio_value(&mut same, true, "Same folder").clicked() {
                            f.folder = None;
                        }
                        ui.horizontal(|ui| {
                            let browse = ui.radio(!same, "Following folder").clicked()
                                | ui.button("Browse…").clicked();
                            if browse && let Some(d) = rfd::FileDialog::new().pick_folder() {
                                f.folder = Some(d);
                            }
                        });
                        if let Some(d) = &f.folder {
                            ui.label(RichText::new(d.display().to_string()).weak().small());
                        }
                    });
                    ui.end_row();
                    if f.target == FileType::Dst {
                        ui.label("DST jumps per trim");
                        ui.add(egui::DragValue::new(&mut f.dst_trim_jumps).range(1..=8));
                        ui.end_row();
                    }
                });
            ui.label(
                RichText::new("Existing files are never replaced: the new file gets a number.")
                    .weak()
                    .small(),
            );
            if ui
                .add_enabled(n > 0, egui::Button::new("Convert"))
                .clicked()
            {
                run = true;
            }
        });
    if run {
        app.database.run_convert();
        app.database.dialog = Dialog::None;
    }
    if !open {
        app.database.dialog = Dialog::None;
    }
}

fn catalog_dialog(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = true;
    let mut save = false;
    let shown = app.database.items().len();
    let selected = app.database.selection.len();
    egui::Window::new("Catalogue")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            let c = &mut app.database.catalog;
            egui::Grid::new("db-catalog")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Output");
                    egui::ComboBox::from_id_salt("db-cat-format")
                        .selected_text(c.format.label())
                        .show_ui(ui, |ui| {
                            for f in CatalogFormat::ALL {
                                ui.selectable_value(&mut c.format, f, f.label());
                            }
                        });
                    ui.end_row();
                    ui.label("Designs");
                    ui.vertical(|ui| {
                        ui.radio_value(&mut c.selected_only, false, format!("All shown ({shown})"));
                        ui.add_enabled_ui(selected > 0, |ui| {
                            ui.radio_value(
                                &mut c.selected_only,
                                true,
                                format!("Selected ({selected})"),
                            );
                        });
                    });
                    ui.end_row();
                    ui.label("Title");
                    ui.text_edit_singleline(&mut c.setup.title);
                    ui.end_row();
                    if c.format != CatalogFormat::Csv {
                        ui.label("Layout");
                        egui::ComboBox::from_id_salt("db-cat-style")
                            .selected_text(c.setup.style.label())
                            .show_ui(ui, |ui| {
                                for s in Style::ALL {
                                    ui.selectable_value(&mut c.setup.style, s, s.label());
                                }
                            });
                        ui.end_row();
                    }
                    if matches!(c.format, CatalogFormat::Pdf | CatalogFormat::Png) {
                        ui.label("Paper");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut c.setup.paper, Paper::A4, "A4");
                            ui.radio_value(&mut c.setup.paper, Paper::Letter, "Letter");
                            ui.separator();
                            ui.radio_value(&mut c.setup.landscape, false, "Portrait");
                            ui.radio_value(&mut c.setup.landscape, true, "Landscape");
                        });
                        ui.end_row();
                    }
                    ui.label("Color list");
                    ui.horizontal(|ui| {
                        ui.radio_value(
                            &mut c.setup.color_list,
                            ColorList::Changes,
                            "Color changes",
                        );
                        ui.radio_value(&mut c.setup.color_list, ColorList::Totals, "Total colors");
                    });
                    ui.end_row();
                    ui.label("Color description");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut c.setup.brand_numbers, false, "Colour name");
                        ui.radio_value(&mut c.setup.brand_numbers, true, "Brand and number");
                    });
                    ui.end_row();
                });
            if c.format == CatalogFormat::Html {
                ui.label(
                    RichText::new("The images go to a folder named after the HTML file.")
                        .weak()
                        .small(),
                );
            }
            if ui.button("Save…").clicked() {
                save = true;
            }
        });
    if save {
        let f = app.database.catalog.format;
        let name = format!("catalogue.{}", f.extension());
        let mut dialog = rfd::FileDialog::new()
            .add_filter(f.label(), &[f.extension()])
            .set_file_name(name);
        if let Some(dir) = &app.database.folder {
            dialog = dialog.set_directory(dir);
        }
        // The native save dialog asks before replacing an existing file.
        if let Some(path) = dialog.save_file() {
            match app.database.write_catalog(&path) {
                Ok(_) => app.database.dialog = Dialog::None,
                Err(e) => app.database.status = e,
            }
        }
    }
    if !open {
        app.database.dialog = Dialog::None;
    }
}
