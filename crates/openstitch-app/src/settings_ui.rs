//! Saved sewing-attribute sets and user thread charts (PE-DESIGN's custom sewing attributes and
//! user thread colour lists). They live in the user settings file, not in the design; each
//! change is one `attributes.*` / `threads.*` command.

use crate::app::EditorApp;
use eframe::egui::{self, Color32};
use openstitch_app::editor::CommandRequest;
use serde_json::json;

#[derive(Default)]
pub struct SettingsState {
    pub sets_open: bool,
    pub charts_open: bool,
    /// Name typed for a new attribute set.
    pub set_name: String,
    /// Selected chart, new chart name, the colour being added and its details.
    pub chart: Option<String>,
    pub chart_name: String,
    pub color: [u8; 3],
    pub code: String,
    pub brand: String,
    pub description: String,
}

fn single(app: &EditorApp) -> Option<u64> {
    (app.selection.len() == 1)
        .then(|| app.selection.iter().next().copied())
        .flatten()
}

pub fn windows(app: &mut EditorApp, ctx: &egui::Context) {
    sets_window(app, ctx);
    charts_window(app, ctx);
}

fn sets_window(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = app.settings.sets_open;
    let mut request = None;
    egui::Window::new("Sewing Attribute Sets")
        .open(&mut open)
        .default_width(320.0)
        .show(ctx, |ui| {
            let sets: Vec<String> = app
                .session
                .user_settings()
                .attribute_sets
                .iter()
                .map(|s| s.name.clone())
                .collect();
            if sets.is_empty() {
                ui.label(egui::RichText::new("No saved sets yet.").weak());
            }
            let any = !app.selection.is_empty();
            for name in sets {
                ui.horizontal(|ui| {
                    ui.label(&name);
                    if ui
                        .add_enabled(any, egui::Button::new("Apply"))
                        .on_hover_text("Apply to the selected objects (their colours stay)")
                        .clicked()
                    {
                        request = Some(CommandRequest::new(
                            "attributes.apply",
                            json!({ "name": name, "ids": CommandRequest::ids(&app.selection) }),
                        ));
                    }
                    if ui.button("Delete").clicked() {
                        request = Some(CommandRequest::new(
                            "attributes.delete",
                            json!({ "name": name }),
                        ));
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut app.settings.set_name);
                let from = single(app);
                if ui
                    .add_enabled(
                        from.is_some() && !app.settings.set_name.trim().is_empty(),
                        egui::Button::new("Save selected"),
                    )
                    .on_hover_text("Save the selected object's sewing attributes under this name")
                    .clicked()
                {
                    request = Some(CommandRequest::new(
                        "attributes.save",
                        json!({ "name": app.settings.set_name.trim(), "from": from }),
                    ));
                }
            });
        });
    if let Some(r) = request
        && app.execute(&r).is_some()
    {
        app.status = format!("{} done", r.command);
    }
    app.settings.sets_open = open;
}

fn charts_window(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = app.settings.charts_open;
    let mut request: Option<CommandRequest> = None;
    egui::Window::new("User Thread Charts")
        .open(&mut open)
        .default_width(380.0)
        .show(ctx, |ui| {
            let charts = app.session.user_settings().thread_charts.clone();
            let st = &mut app.settings;
            if st
                .chart
                .as_ref()
                .is_some_and(|c| !charts.iter().any(|x| &x.name == c))
            {
                st.chart = None;
            }
            if st.chart.is_none() {
                st.chart = charts.first().map(|c| c.name.clone());
            }
            ui.horizontal(|ui| {
                egui::ComboBox::from_label("Chart")
                    .selected_text(st.chart.clone().unwrap_or_else(|| "—".into()))
                    .show_ui(ui, |ui| {
                        for c in &charts {
                            ui.selectable_value(&mut st.chart, Some(c.name.clone()), &c.name);
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut st.chart_name);
                let name = st.chart_name.trim().to_string();
                if ui
                    .add_enabled(!name.is_empty(), egui::Button::new("New"))
                    .clicked()
                {
                    request = Some(CommandRequest::new(
                        "threads.createChart",
                        json!({ "name": name }),
                    ));
                    st.chart = Some(name.clone());
                }
                if let Some(c) = st.chart.clone() {
                    if ui
                        .add_enabled(!name.is_empty(), egui::Button::new("Rename"))
                        .clicked()
                    {
                        request = Some(CommandRequest::new(
                            "threads.renameChart",
                            json!({ "chart": c, "name": name }),
                        ));
                        st.chart = Some(name.clone());
                    }
                    if ui.button("Delete").clicked() {
                        request = Some(CommandRequest::new(
                            "threads.deleteChart",
                            json!({ "chart": c }),
                        ));
                    }
                }
            });
            let Some(name) = st.chart.clone() else {
                return;
            };
            let Some(chart) = charts.iter().find(|c| c.name == name) else {
                return;
            };
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    let n = chart.threads.len();
                    for (i, t) in chart.threads.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let (r, _) = ui
                                .allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                            ui.painter().rect_filled(
                                r,
                                2.0,
                                Color32::from_rgb(t.red, t.green, t.blue),
                            );
                            ui.label(format!(
                                "{}  {}  {}  {}",
                                t.catalog_number.as_deref().unwrap_or(""),
                                t.brand.as_deref().unwrap_or(""),
                                t.name.as_deref().unwrap_or(""),
                                t.hex()
                            ));
                            let mv = |to: usize| {
                                CommandRequest::new(
                                    "threads.moveColor",
                                    json!({ "chart": name, "index": i, "to": to }),
                                )
                            };
                            if ui.add_enabled(i > 0, egui::Button::new("▲")).clicked() {
                                request = Some(mv(i - 1));
                            }
                            if ui.add_enabled(i + 1 < n, egui::Button::new("▼")).clicked() {
                                request = Some(mv(i + 1));
                            }
                            if ui.button("Remove").clicked() {
                                request = Some(CommandRequest::new(
                                    "threads.removeColor",
                                    json!({ "chart": name, "index": i }),
                                ));
                            }
                        });
                    }
                });
            ui.separator();
            egui::Grid::new("new-thread").num_columns(2).show(ui, |ui| {
                ui.label("Colour");
                ui.color_edit_button_srgb(&mut st.color);
                ui.end_row();
                ui.label("Code");
                ui.text_edit_singleline(&mut st.code);
                ui.end_row();
                ui.label("Brand");
                ui.text_edit_singleline(&mut st.brand);
                ui.end_row();
                ui.label("Description");
                ui.text_edit_singleline(&mut st.description);
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if ui.button("Add colour").clicked() {
                    let [r, g, b] = st.color;
                    let mut p =
                        json!({ "chart": name, "color": format!("#{r:02x}{g:02x}{b:02x}") });
                    for (k, v) in [
                        ("code", &st.code),
                        ("brand", &st.brand),
                        ("description", &st.description),
                    ] {
                        if !v.trim().is_empty() {
                            p[k] = json!(v.trim());
                        }
                    }
                    request = Some(CommandRequest::new("threads.addColor", p));
                }
                if ui.button("Import CSV…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                {
                    request = Some(CommandRequest::new(
                        "threads.importCsv",
                        json!({ "chart": name, "path": path.to_string_lossy() }),
                    ));
                }
                if ui
                    .button("Remap design")
                    .on_hover_text("Change every thread to the nearest colour of this chart")
                    .clicked()
                {
                    request = Some(CommandRequest::new(
                        "threads.remap",
                        json!({ "chart": name }),
                    ));
                }
            });
        });
    if let Some(r) = request
        && app.execute(&r).is_some()
    {
        app.status = format!("{} done", r.command);
    }
    app.settings.charts_open = open;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Launch;

    #[test]
    fn settings_file_is_loaded_and_written() {
        let dir = std::env::temp_dir().join(format!(
            "openstitch-app-user-{:?}",
            std::thread::current().id()
        ));
        let file = dir.join("user.json");
        let _ = std::fs::remove_file(&file);
        let mut app = EditorApp::new(Launch {
            user_settings: Some(file.clone()),
            ..Default::default()
        });
        app.execute(&CommandRequest::new(
            "threads.createChart",
            json!({ "name": "Mine" }),
        ));
        assert!(file.exists());
        let again = EditorApp::new(Launch {
            user_settings: Some(file.clone()),
            ..Default::default()
        });
        assert_eq!(again.session.user_settings().thread_charts.len(), 1);
        // A broken file is reported, not fatal.
        std::fs::write(&file, "nonsense").expect("write");
        let broken = EditorApp::new(Launch {
            user_settings: Some(file.clone()),
            ..Default::default()
        });
        assert!(broken.status.contains("user settings"), "{}", broken.status);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
