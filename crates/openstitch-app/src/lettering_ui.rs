//! Lettering dialogs (Home ▸ Tools ▸ Text). The forms live in `openstitch_app::editor::lettering`; each
//! action is one engine command, so one Undo.

use crate::app::EditorApp;
use eframe::egui;
use openstitch_app::editor::CommandRequest;
use openstitch_app::editor::lettering::{
    self, FontChoice, FontsForm, MonogramForm, NameDropForm, TemplateWizard, TextForm,
};
use serde_json::json;

/// Dialog state of the Lettering menu.
#[derive(Default)]
pub struct LetteringState {
    pub text: TextForm,
    pub fonts: FontsForm,
    pub monogram: MonogramForm,
    pub name_drop: NameDropForm,
    pub templates: TemplateWizard,
}

/// The fonts `fonts.list` reports for the open design.
pub fn fonts(app: &mut EditorApp) -> Vec<FontChoice> {
    let all = app
        .session
        .execute("fonts.list", Some(&json!({})))
        .map(|v| lettering::font_choices(&v))
        .unwrap_or_default();
    // Text ▸ Font Filter.
    app.ui.prefs.font_filter.apply(all)
}

/// Home ▸ Tools ▸ Text: text, monogram, name drop, templates and fonts.
pub fn text_tools_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    if ui.button("Text…").clicked() {
        app.lettering.text.open = true;
    }
    if ui.button("Monogram…").clicked() {
        app.lettering.monogram.open = true;
    }
    if ui.button("Name Drop List…").clicked() {
        app.lettering.name_drop.open = true;
    }
    if ui.button("Template Wizard…").clicked() {
        open_template_wizard(app);
    }
    if ui.button("Save as Template…").clicked() {
        save_template(app);
    }
    ui.separator();
    if ui.button("Fonts…").clicked() {
        app.lettering.fonts.open = true;
    }
    if ui.button("Load TrueType Font…").clicked() {
        load_truetype(app);
    }
}

/// Loads a TrueType / OpenType font into the design.
pub fn load_truetype(app: &mut EditorApp) {
    if let Some(p) = rfd::FileDialog::new()
        .add_filter("TrueType / OpenType", &["ttf", "otf", "ttc", "otc"])
        .pick_file()
        && let Some(r) = app.execute(&lettering::load_font_request(&p))
    {
        app.status = format!(
            "Loaded font {} ({} glyphs); it is saved with the design",
            r["id"].as_str().unwrap_or_default(),
            r["glyphs"]
        );
    }
}

fn font_combo(ui: &mut egui::Ui, salt: &str, value: &mut String, choices: &[FontChoice]) {
    let label = choices
        .iter()
        .find(|c| &c.id == value)
        .map_or(value.clone(), |c| c.label.clone());
    egui::ComboBox::from_id_salt(salt)
        .selected_text(label)
        .show_ui(ui, |ui| {
            for c in choices {
                ui.selectable_value(value, c.id.clone(), &c.label);
            }
        });
}

pub fn windows(app: &mut EditorApp, ctx: &egui::Context) {
    text_window(app, ctx);
    monogram_window(app, ctx);
    name_drop_window(app, ctx);
    template_window(app, ctx);
    fonts_window(app, ctx);
}

/// What the Fonts window asked for this frame.
enum FontAction {
    Create,
    Convert,
    EditGlyph,
    SetGlyph,
    ChooseImage,
    TraceImage,
    FromImage,
    MapSelection(u64),
    MapFiles,
    Save,
    Open,
    Remove(String),
}

fn fonts_window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.lettering.fonts.open {
        return;
    }
    let rows = app
        .session
        .execute("fonts.list", Some(&json!({})))
        .map(|v| lettering::design_fonts(&v))
        .unwrap_or_default();
    if !rows
        .iter()
        .any(|r| r.embroidery && r.id == app.lettering.fonts.font)
        && let Some(r) = rows.iter().find(|r| r.embroidery)
    {
        app.lettering.fonts.font = r.id.clone();
    }
    let single = (app.selection.len() == 1)
        .then(|| app.selection.iter().next().copied())
        .flatten();
    let any_selected = !app.selection.is_empty();
    let has_background = app.session.design().background.is_some();
    let truetype: Vec<FontChoice> = {
        let embroidery: Vec<&str> = rows
            .iter()
            .filter(|r| r.embroidery)
            .map(|r| r.id.as_str())
            .collect();
        fonts(app)
            .into_iter()
            .filter(|c| !embroidery.contains(&c.id.as_str()))
            .collect()
    };
    let mut open = true;
    let mut action = None;
    egui::Window::new("Fonts")
        .open(&mut open)
        .default_width(380.0)
        .show(ctx, |ui| {
            let f = &mut app.lettering.fonts;
            ui.label(egui::RichText::new("Fonts in this design").strong());
            if rows.is_empty() {
                ui.weak("None yet. Load a TrueType font or create an embroidery font.");
            }
            for r in &rows {
                ui.horizontal(|ui| {
                    if r.embroidery {
                        ui.radio_value(&mut f.font, r.id.clone(), &r.id);
                    } else {
                        ui.label(&r.id);
                    }
                    ui.weak(&r.label);
                    if ui.small_button("Remove").clicked() {
                        action = Some(FontAction::Remove(r.id.clone()));
                    }
                });
            }
            if ui.button("Open OpenStitch font (.osfont)…").clicked() {
                action = Some(FontAction::Open);
            }
            ui.separator();
            ui.label(egui::RichText::new("New embroidery font").strong());
            ui.horizontal(|ui| {
                ui.label("Id");
                ui.text_edit_singleline(&mut f.new_id);
            });
            ui.horizontal(|ui| {
                ui.label("Standard character height");
                ui.add(
                    egui::DragValue::new(&mut f.new_height)
                        .range(1.0..=500.0)
                        .suffix(" mm"),
                );
            });
            if ui.button("Create").clicked() {
                action = Some(FontAction::Create);
            }
            ui.separator();
            let editing = rows.iter().any(|r| r.embroidery && r.id == f.font);
            ui.add_enabled_ui(editing, |ui| {
                ui.label(egui::RichText::new("User-mapped characters").strong());
                ui.horizontal(|ui| {
                    ui.label("Character");
                    ui.add(egui::TextEdit::singleline(&mut f.character).desired_width(40.0));
                    if ui
                        .add_enabled(single.is_some(), egui::Button::new("Register selected object"))
                        .on_hover_text("The selected object's stitches become this character")
                        .clicked()
                        && let Some(id) = single
                    {
                        action = Some(FontAction::MapSelection(id));
                    }
                });
                if ui
                    .button("Register stitch files…")
                    .on_hover_text("File names give the characters: u_A, l_a, s_qm, a single character or U+XXXX")
                    .clicked()
                {
                    action = Some(FontAction::MapFiles);
                }
                ui.separator();
                ui.label(egui::RichText::new("Font Creator").strong());
                ui.horizontal(|ui| {
                    ui.label("From TrueType");
                    font_combo(ui, "convert-from", &mut f.convert_from, &truetype);
                });
                ui.add(egui::TextEdit::singleline(&mut f.convert_chars).hint_text("characters"));
                if ui.button("Convert characters").clicked() {
                    action = Some(FontAction::Convert);
                }
                ui.horizontal(|ui| {
                    ui.label("Draw at set line");
                    ui.add(egui::DragValue::new(&mut f.glyph_x).suffix(" mm"));
                    ui.label("baseline");
                    ui.add(egui::DragValue::new(&mut f.glyph_y).suffix(" mm"));
                });
                ui.horizontal(|ui| {
                    if ui
                        .button("Edit character as shape")
                        .on_hover_text("Puts the character's outline on the Design Page")
                        .clicked()
                    {
                        action = Some(FontAction::EditGlyph);
                    }
                    if ui
                        .add_enabled(any_selected, egui::Button::new("Store selection as character"))
                        .on_hover_text("The selected outlines, measured from the set line and baseline above, become the character")
                        .clicked()
                    {
                        action = Some(FontAction::SetGlyph);
                    }
                });
                ui.separator();
                image_characters(ui, f, has_background, &mut action);
                ui.separator();
                if ui.button("Save font as…").clicked() {
                    action = Some(FontAction::Save);
                }
            });
        });
    app.lettering.fonts.open = open;
    let f = app.lettering.fonts.clone();
    let ext = openstitch_engine::fonts::FONT_EXTENSION;
    match action {
        Some(FontAction::Create) => {
            if let Some(r) = app.execute(&f.create()) {
                app.lettering.fonts.font = r["id"].as_str().unwrap_or_default().into();
                app.status = "Created an empty embroidery font".into();
            }
        }
        Some(FontAction::Convert) => {
            if let Some(r) = app.execute(&f.convert()) {
                app.status = format!(
                    "Converted {} character(s){}",
                    r["converted"].as_str().map_or(0, |c| c.chars().count()),
                    match r["missing"].as_str() {
                        Some(m) if !m.is_empty() => format!("; the font has none of {m}"),
                        _ => String::new(),
                    }
                );
            }
        }
        Some(FontAction::EditGlyph) => {
            if let Some(r) = app.execute(&f.edit_glyph()) {
                app.selection = CommandRequest::created_ids(&r);
                app.status = format!(
                    "Editing '{}': baseline {} mm, capital line {} mm, set lines {}–{} mm",
                    f.character, r["baseline"], r["capLine"], r["left"], r["right"]
                );
            }
        }
        Some(FontAction::SetGlyph) => {
            let ids = app.selection.clone();
            if let Some(r) = app.execute(&f.set_glyph(&ids)) {
                app.status = format!("Stored '{}' ({} mm wide)", f.character, r["advanceMM"]);
            }
        }
        Some(FontAction::ChooseImage) => {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                .pick_file()
            {
                app.lettering.fonts.image.path = Some(p);
                app.lettering.fonts.image.found.clear();
            }
        }
        Some(FontAction::TraceImage) => {
            if let Some(r) = app.execute(&f.trace_image()) {
                app.lettering.fonts.set_found(&r);
                app.status = format!(
                    "Found {} character(s) in {} row(s); check the characters, then Create characters",
                    app.lettering.fonts.image.found.len(),
                    r["rows"]
                );
            }
        }
        Some(FontAction::FromImage) => {
            if let Some(r) = f.from_image().and_then(|req| app.execute(&req)) {
                app.status = format!(
                    "Created {} from the image ({} skipped)",
                    r["created"].as_str().unwrap_or_default(),
                    r["skipped"]
                );
                app.lettering.fonts.image.found.clear();
            }
        }
        Some(FontAction::MapSelection(id)) => {
            if let Some(r) = app.execute(&f.map_object(id)) {
                app.status = format!("Registered '{}' ({} stitches)", f.character, r["stitches"]);
            }
        }
        Some(FontAction::MapFiles) => {
            let exts: Vec<&str> = openstitch_formats::FileType::ALL
                .iter()
                .map(|t| t.extension())
                .collect();
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("Embroidery files", &exts)
                .pick_files()
                && let Some(r) = app.execute(&f.map_files(&paths))
            {
                app.status = lettering::map_summary(&r);
            }
        }
        Some(FontAction::Save) => {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("OpenStitch font", &[ext])
                .set_file_name(format!("{}.{ext}", f.font))
                .save_file()
                && app.execute(&f.save(&p)).is_some()
            {
                app.status = format!("Saved {}", p.display());
            }
        }
        Some(FontAction::Open) => {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("OpenStitch font", &[ext])
                .pick_file()
                && let Some(r) = app.execute(&FontsForm::open_file(&p))
            {
                app.lettering.fonts.font = r["id"].as_str().unwrap_or_default().into();
            }
        }
        Some(FontAction::Remove(id)) => {
            app.execute(&FontsForm::remove(&id));
        }
        None => {}
    }
}

/// Font Creator ▸ Characters from image: read the image, check and assign the characters
/// found, then create them in one step.
fn image_characters(
    ui: &mut egui::Ui,
    f: &mut FontsForm,
    has_background: bool,
    action: &mut Option<FontAction>,
) {
    ui.label(egui::RichText::new("Characters from image").strong());
    let i = &mut f.image;
    ui.horizontal(|ui| {
        let name = i.path.as_ref().map_or_else(
            || {
                if has_background {
                    "Background image".to_string()
                } else {
                    "No image".to_string()
                }
            },
            |p| {
                p.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            },
        );
        ui.label(name);
        if ui.button("Choose image…").clicked() {
            *action = Some(FontAction::ChooseImage);
        }
        if i.path.is_some()
            && has_background
            && ui
                .button("Use background")
                .on_hover_text("Read the background image as it is shown on the Design Page")
                .clicked()
        {
            i.path = None;
            i.found.clear();
        }
    });
    ui.horizontal(|ui| {
        if ui
            .radio_value(&mut i.whole, false, "Split into characters")
            .changed()
            | ui.radio_value(&mut i.whole, true, "Whole image is the character")
                .changed()
        {
            i.found.clear();
        }
    });
    ui.horizontal(|ui| {
        ui.label("Ink darker than");
        ui.add(egui::DragValue::new(&mut i.threshold).range(1..=254));
        if i.whole {
            ui.add_enabled(
                i.path.is_none() && has_background,
                egui::Checkbox::new(&mut i.as_placed, "As placed, from the set line and baseline"),
            )
            .on_hover_text("Use the background image where it lies instead of fitting it to the standard height");
        }
    });
    if !i.whole {
        ui.horizontal(|ui| {
            ui.label("Assign in order");
            ui.text_edit_singleline(&mut i.order);
        });
    }
    let ready = i.path.is_some() || has_background;
    if ui
        .add_enabled(ready, egui::Button::new("Find characters"))
        .clicked()
    {
        *action = Some(FontAction::TraceImage);
    }
    if i.found.is_empty() {
        return;
    }
    ui.weak("Type the character each outline becomes; leave a box empty to skip it.");
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for c in &mut i.found {
                    ui.vertical(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
                        draw_outline(ui, rect, &c.outline);
                        ui.add(egui::TextEdit::singleline(&mut c.assigned).desired_width(44.0))
                            .on_hover_text(format!("Row {}", c.row + 1));
                    });
                }
            });
        });
    if ui.button("Create characters").clicked() {
        *action = Some(FontAction::FromImage);
    }
}

/// A found character's outline, fitted into `rect`.
fn draw_outline(ui: &egui::Ui, rect: egui::Rect, outline: &[Vec<[f64; 2]>]) {
    let painter = ui.painter_at(rect);
    painter.rect_stroke(
        rect,
        2.0,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let pts = outline.iter().flatten();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for q in pts {
        (x0, y0, x1, y1) = (x0.min(q[0]), y0.min(q[1]), x1.max(q[0]), y1.max(q[1]));
    }
    if x0 > x1 {
        return;
    }
    let side = (x1 - x0).max(y1 - y0).max(1e-6);
    let scale = f64::from(rect.width() - 8.0) / side;
    let offset = egui::vec2(
        ((f64::from(rect.width()) - (x1 - x0) * scale) / 2.0) as f32,
        ((f64::from(rect.height()) - (y1 - y0) * scale) / 2.0) as f32,
    );
    let stroke = egui::Stroke::new(1.0, ui.visuals().text_color());
    for line in outline {
        let mut points: Vec<egui::Pos2> = line
            .iter()
            .map(|q| {
                rect.min
                    + offset
                    + egui::vec2(((q[0] - x0) * scale) as f32, ((q[1] - y0) * scale) as f32)
            })
            .collect();
        if let Some(&first) = points.first() {
            points.push(first);
        }
        painter.add(egui::Shape::line(points, stroke));
    }
}

fn text_window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.lettering.text.open {
        return;
    }
    let choices = fonts(app);
    let mut open = true;
    let mut add = None;
    egui::Window::new("Text")
        .open(&mut open)
        .resizable(false)
        .show(ctx, |ui| {
            let f = &mut app.lettering.text;
            ui.add(egui::TextEdit::multiline(&mut f.text).desired_rows(2));
            egui::Grid::new("text-form").num_columns(2).show(ui, |ui| {
                ui.label("Font");
                font_combo(ui, "text-font", &mut f.font, &choices);
                ui.end_row();
                ui.label("Size");
                ui.add(
                    egui::DragValue::new(&mut f.size)
                        .range(1.0..=300.0)
                        .suffix(" mm"),
                );
                ui.end_row();
                ui.label("Align");
                ui.horizontal(|ui| {
                    for a in TextForm::ALIGNS {
                        ui.radio_value(&mut f.align, a, a);
                    }
                });
                ui.end_row();
                ui.label("Options");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut f.kerning, "Kerning")
                        .on_hover_text("Use the TrueType font's kerning pairs");
                    ui.checkbox(&mut f.vertical, "Vertical")
                        .on_hover_text("Characters top to bottom, lines as columns right to left");
                });
                ui.end_row();
            });
            ui.weak(
                "CJK, Arabic, Hebrew and other scripts need a TrueType font that has them (Load TrueType Font…); they are shaped and set right to left where needed.",
            );
            if ui.button("Add").clicked() {
                add = f.request();
            }
        });
    app.lettering.text.open = open;
    if let Some(r) = add {
        run(app, &r, lettering::text_summary);
    }
}

/// Runs `r`, selects what it created and reports `summary` of the result.
fn run(app: &mut EditorApp, r: &CommandRequest, summary: fn(&serde_json::Value) -> String) {
    if let Some(result) = app.execute(r) {
        app.selection = CommandRequest::created_ids(&result);
        app.status = summary(&result);
    }
}

fn monogram_window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.lettering.monogram.open {
        return;
    }
    let choices = fonts(app);
    let frames = app
        .session
        .execute("monogram.list", Some(&json!({})))
        .map(|v| lettering::frame_choices(&v))
        .unwrap_or_default();
    // A monogram among the selection can be rebuilt with the form's settings.
    let selected_monogram = {
        let d = app.session.design();
        let ids: Vec<u64> = app.selection.iter().copied().collect();
        d.with_groups(&ids).into_iter().find(|&id| {
            d.index_of(id)
                .is_some_and(|i| d.objects[i].monogram.is_some())
        })
    };
    let mut open = true;
    let (mut add, mut edit, mut decorate) = (false, false, false);
    egui::Window::new("Monogram")
        .open(&mut open)
        .resizable(false)
        .show(ctx, |ui| {
            let f = &mut app.lettering.monogram;
            egui::Grid::new("monogram-form")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Letters");
                    ui.add(egui::TextEdit::singleline(&mut f.letters).desired_width(60.0))
                        .on_hover_text("1-3 letters or digits; upper-cased");
                    ui.end_row();
                    ui.label("Style");
                    ui.horizontal(|ui| {
                        for (id, name) in MonogramForm::STYLES {
                            if ui.radio_value(&mut f.style, id, name).changed() {
                                f.font = None;
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Font");
                    let mut font = f.font.clone().unwrap_or_else(|| {
                        if f.style == "script" {
                            "pacifico"
                        } else {
                            "lato-bold"
                        }
                        .into()
                    });
                    font_combo(ui, "monogram-font", &mut font, &choices);
                    f.font = Some(font);
                    ui.end_row();
                    ui.label("Height");
                    ui.add(
                        egui::DragValue::new(&mut f.size)
                            .range(5.0..=300.0)
                            .suffix(" mm"),
                    );
                    ui.end_row();
                    ui.label("Frame");
                    let label = f
                        .frame
                        .and_then(|id| frames.iter().find(|(i, _)| i == id))
                        .map_or("None".to_string(), |(_, n)| n.clone());
                    egui::ComboBox::from_id_salt("monogram-frame")
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut f.frame, None, "None");
                            for (id, name) in &frames {
                                let id: &'static str = openstitch_design::monogram::frame_info(id)
                                    .map_or("ring", |i| i.id);
                                ui.selectable_value(&mut f.frame, Some(id), name);
                            }
                        });
                    ui.end_row();
                    ui.label("Margin");
                    ui.add(
                        egui::DragValue::new(&mut f.margin)
                            .range(0.0..=50.0)
                            .suffix(" mm"),
                    );
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut f.keep_aspect, "Maintain aspect ratio");
                    ui.end_row();
                });
            ui.horizontal(|ui| {
                add = ui.button("Add").clicked();
                edit = ui
                    .add_enabled(
                        selected_monogram.is_some(),
                        egui::Button::new("Apply to selected monogram"),
                    )
                    .clicked();
                decorate = ui
                    .add_enabled(
                        !app.selection.is_empty() && f.frame.is_some(),
                        egui::Button::new("Frame the selection"),
                    )
                    .on_hover_text("Decorative frame around the selected objects")
                    .clicked();
            });
            ui.weak("Letter styles and frames are OpenStitch's own designs.");
        });
    app.lettering.monogram.open = open;
    let f = app.lettering.monogram.clone();
    let summary =
        |r: &serde_json::Value| format!("Monogram {}", r["letters"].as_str().unwrap_or_default());
    if add {
        run(app, &f.add(), summary);
    } else if edit && let Some(id) = selected_monogram {
        run(app, &f.edit(id), summary);
    } else if decorate && let Some(r) = f.decorate(&app.selection.clone()) {
        run(app, &r, |_| "Added a decorative frame".into());
    }
}

/// What the Name Drop window asked for this frame.
enum DropAction {
    Attribute(bool),
    Apply,
    ImportCsv,
    SaveCsv,
    ToFolder,
}

fn name_drop_window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.lettering.name_drop.open {
        return;
    }
    let columns: Vec<String> = app
        .session
        .execute("nameDrop.list", Some(&json!({})))
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|c| {
            c["label"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| format!("Text {}", c["id"]))
        })
        .collect();
    let texts_selected = {
        let d = app.session.design();
        !app.selection.is_empty()
            && app.selection.iter().all(|&id| {
                d.index_of(id)
                    .is_some_and(|i| d.objects[i].kind == openstitch_design::ObjectKind::Text)
            })
    };
    let n = columns.len();
    if app.lettering.name_drop.rows.is_empty() && n > 0 {
        // The list starts with what the design says now.
        let current: Vec<String> = app
            .session
            .execute("nameDrop.list", Some(&json!({})))
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .map(|c| c["text"].as_str().unwrap_or_default().to_string())
            .collect();
        app.lettering.name_drop.rows.push(current);
        app.lettering.name_drop.selected = Some(0);
    }
    app.lettering.name_drop.fit_columns(n);
    let mut open = true;
    let mut action = None;
    egui::Window::new("Name Drop List")
        .open(&mut open)
        .default_width(420.0)
        .show(ctx, |ui| {
            let f = &mut app.lettering.name_drop;
            ui.label(egui::RichText::new("Name Drop attribute").strong());
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.maintain_aspect, "Maintain aspect ratio");
                for a in ["left", "center", "right"] {
                    ui.radio_value(&mut f.align, a, a);
                }
                for a in ["top", "middle", "bottom"] {
                    ui.radio_value(&mut f.valign, a, a);
                }
            });
            ui.horizontal(|ui| {
                ui.add_enabled_ui(texts_selected, |ui| {
                    if ui.button("Apply to selected text").clicked() {
                        action = Some(DropAction::Attribute(true));
                    }
                    if ui.button("Remove from selected text").clicked() {
                        action = Some(DropAction::Attribute(false));
                    }
                });
            });
            ui.separator();
            if n == 0 {
                ui.weak("No text has the Name Drop attribute yet: select text and apply it.");
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    egui::Grid::new("name-drop-grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("");
                            for c in &columns {
                                ui.strong(c);
                            }
                            ui.end_row();
                            for (i, row) in f.rows.iter_mut().enumerate() {
                                if ui
                                    .selectable_label(
                                        f.selected == Some(i),
                                        format!("{:03}", i + 1),
                                    )
                                    .clicked()
                                {
                                    f.selected = Some(i);
                                }
                                for cell in row.iter_mut() {
                                    if ui
                                        .add_sized([150.0, 20.0], egui::TextEdit::singleline(cell))
                                        .gained_focus()
                                    {
                                        f.selected = Some(i);
                                    }
                                }
                                ui.end_row();
                            }
                        });
                });
            ui.horizontal(|ui| {
                if ui
                    .button("+")
                    .on_hover_text("Add a row (500 at most)")
                    .clicked()
                {
                    f.add_row(n);
                }
                if ui.button("Insert").clicked() {
                    f.insert_row(n);
                }
                if ui.button("Delete").clicked() {
                    f.delete_row();
                }
                if ui.button("Import CSV…").clicked() {
                    action = Some(DropAction::ImportCsv);
                }
                if ui
                    .add_enabled(!f.rows.is_empty(), egui::Button::new("Save as CSV…"))
                    .clicked()
                {
                    action = Some(DropAction::SaveCsv);
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(f.selected.is_some(), egui::Button::new("Use selected row"))
                    .on_hover_text("Put the row's text into the design")
                    .clicked()
                {
                    action = Some(DropAction::Apply);
                }
                ui.label("File name");
                ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(90.0));
                egui::ComboBox::from_id_salt("name-drop-format")
                    .selected_text(f.format)
                    .show_ui(ui, |ui| {
                        for t in openstitch_formats::FileType::writable() {
                            ui.selectable_value(&mut f.format, t.extension(), t.extension());
                        }
                    });
                if ui
                    .add_enabled(!f.rows.is_empty(), egui::Button::new("To Folder…"))
                    .on_hover_text("One file per row: name001, name002, …")
                    .clicked()
                {
                    action = Some(DropAction::ToFolder);
                }
            });
        });
    app.lettering.name_drop.open = open;
    let f = app.lettering.name_drop.clone();
    match action {
        Some(DropAction::Attribute(on)) => {
            let ids = app.selection.clone();
            app.execute(&f.set_attribute(&ids, on));
        }
        Some(DropAction::Apply) => {
            if let Some(r) = f.apply() {
                app.execute(&r);
            }
        }
        Some(DropAction::ImportCsv) => {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file()
                && let Some(r) = app.execute(&NameDropForm::read_csv(&p))
            {
                app.lettering.name_drop.set_rows(&r, n);
            }
        }
        Some(DropAction::SaveCsv) => {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .set_file_name("name-drop.csv")
                .save_file()
            {
                app.execute(&f.write_csv(&p));
            }
        }
        Some(DropAction::ToFolder) => {
            if let Some(dir) = rfd::FileDialog::new().pick_folder()
                && let Some(r) = app.execute(&f.export(&dir))
            {
                app.status = format!("Wrote {} file(s) to {}", r["count"], dir.display());
            }
        }
        None => {}
    }
}

pub fn open_template_wizard(app: &mut EditorApp) {
    let w = &mut app.lettering.templates;
    w.open = true;
    w.step = 1;
    let r = w.list();
    if let Some(v) = app.execute(&r) {
        app.lettering.templates.set_templates(&v);
    }
}

pub fn save_template(app: &mut EditorApp) {
    let ext = openstitch_engine::templates::EXTENSION;
    if let Some(p) = rfd::FileDialog::new()
        .add_filter("OpenStitch template", &[ext])
        .set_file_name(format!("{}.{ext}", app.session.design().name))
        .save_file()
        && let Some(r) = app.execute(&CommandRequest::new(
            "template.save",
            json!({ "path": p.display().to_string() }),
        ))
    {
        app.status = format!(
            "Saved template with {} editable text(s)",
            r["slots"].as_array().map_or(0, Vec::len)
        );
    }
}

fn template_window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.lettering.templates.open {
        return;
    }
    let mut open = true;
    let (mut next, mut back, mut folder, mut export, mut done, mut again) =
        (false, false, false, false, false, false);
    egui::Window::new("Template Wizard")
        .open(&mut open)
        .default_width(360.0)
        .show(ctx, |ui| {
            let w = &mut app.lettering.templates;
            match w.step {
                2 => {
                    ui.label(egui::RichText::new("Step 2 · Edit the text").strong());
                    ui.weak("The Design Page shows the template; it updates when you apply.");
                    egui::Grid::new("template-slots")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for slot in &mut w.slots {
                                ui.label(&slot.label);
                                ui.add_sized(
                                    [220.0, 20.0],
                                    egui::TextEdit::singleline(&mut slot.text),
                                );
                                ui.end_row();
                            }
                        });
                    ui.horizontal(|ui| {
                        back = ui.button("Back").clicked();
                        next = ui.button("Apply and continue").clicked();
                    });
                }
                3 => {
                    ui.label(egui::RichText::new("Step 3 · Output").strong());
                    done = ui.button("Edit in Design Page").clicked();
                    export = ui.button("Export PES…").clicked();
                    again = ui.button("Create new embroidery design").clicked();
                }
                _ => {
                    ui.label(egui::RichText::new("Step 1 · Choose a template").strong());
                    ui.horizontal(|ui| {
                        folder = ui.button("Template folder…").clicked();
                        ui.weak(
                            w.dir
                                .as_ref()
                                .map_or("OpenStitch templates".into(), |d| d.display().to_string()),
                        );
                    });
                    let mut picked = None;
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for (category, rows) in w.categories() {
                                ui.label(egui::RichText::new(category).strong());
                                for t in rows {
                                    let chosen = w.chosen.as_deref() == Some(t.id.as_str());
                                    let r = ui.selectable_label(chosen, format!("  {}", t.name));
                                    let r = if t.description.is_empty() {
                                        r
                                    } else {
                                        r.on_hover_text(&t.description)
                                    };
                                    if r.clicked() {
                                        picked = Some(t.id.clone());
                                    }
                                }
                            }
                        });
                    if picked.is_some() {
                        w.chosen = picked;
                    }
                    ui.add_enabled_ui(w.chosen.is_some(), |ui| {
                        next = ui.button("Next").clicked();
                    });
                }
            }
        });
    app.lettering.templates.open = open;
    let step = app.lettering.templates.step.max(1);
    if folder && let Some(d) = rfd::FileDialog::new().pick_folder() {
        app.lettering.templates.dir = Some(d);
        open_template_wizard(app);
    } else if back {
        app.lettering.templates.step = 1;
    } else if next && step == 1 {
        if let Some(r) = app.lettering.templates.open_chosen()
            && let Some(v) = app.execute(&r)
        {
            app.lettering.templates.set_slots(&v);
            app.lettering.templates.step = 2;
            app.selection.clear();
            app.fit_requested = true;
        }
    } else if next && step == 2 {
        let r = app.lettering.templates.fill();
        if let Some(v) = app.execute(&r) {
            app.lettering.templates.set_slots(&v);
            app.lettering.templates.step = 3;
        }
    } else if export {
        app.export(openstitch_formats::FileType::Pes);
    } else if done {
        app.lettering.templates.open = false;
    } else if again {
        app.lettering.templates.step = 1;
    }
}

/// Text ▸ Edit Character: offset, rotation and size of the character picked with Point Edit.
/// A value is applied when its drag ends or it is typed, so each change is one Undo.
pub fn character_menu(app: &mut EditorApp, ui: &mut egui::Ui) {
    use openstitch_app::editor::Tool;
    use openstitch_app::editor::tools::characters::{char_edit, set_character};
    let design = app.session.design().clone();
    let picked = app.tools.selected_character(&app.tool_context(&design));
    let Some((id, index)) = picked else {
        ui.label("Pick Point Edit, then click a character of the selected text.");
        if app.tools.tool() != Tool::Point && ui.button("Point Edit").clicked() {
            app.tools.set_tool(Tool::Point);
            app.workspace.measure.active = false;
            app.ui.pan = false;
        }
        return;
    };
    let Some(spec) = design
        .index_of(id)
        .and_then(|i| design.objects[i].text.as_ref())
    else {
        return;
    };
    let ch = spec.text.chars().nth(index).unwrap_or(' ');
    ui.label(format!(
        "Character “{ch}” ({} of {})",
        index + 1,
        spec.text.chars().count()
    ));
    let before = char_edit(spec, index);
    let mut e = before.clone();
    let mut size = e.size.unwrap_or(spec.size);
    let mut commit = false;
    egui::Grid::new("character-edit").show(ui, |ui| {
        let mut row = |ui: &mut egui::Ui, label: &str, v: &mut f64, range, suffix: &str| {
            ui.label(label);
            let r = ui.add(
                egui::DragValue::new(v)
                    .range(range)
                    .speed(0.1)
                    .max_decimals(2)
                    .suffix(suffix),
            );
            commit |= r.drag_stopped() || (r.changed() && !r.dragged());
            ui.end_row();
        };
        row(ui, "Right", &mut e.x, -500.0..=500.0, " mm");
        row(ui, "Down", &mut e.y, -500.0..=500.0, " mm");
        row(ui, "Rotation", &mut e.rotation, -360.0..=360.0, "°");
        row(ui, "Size", &mut size, 1.0..=300.0, " mm");
    });
    if (size - e.size.unwrap_or(spec.size)).abs() > 1e-9 {
        e.size = ((size - spec.size).abs() > 1e-9).then_some(size);
    }
    if commit && e != before {
        app.execute(&set_character(id, &e));
    }
    if ui
        .add_enabled(!before.is_noop(), egui::Button::new("Reset character"))
        .clicked()
    {
        app.execute(&CommandRequest::new(
            "text.setCharacter",
            json!({ "id": id, "index": index, "reset": true }),
        ));
    }
}
