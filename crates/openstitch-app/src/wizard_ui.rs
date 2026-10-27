//! The Image to Embroidery dialog (Design → Image to Embroidery…). The settings live in
//! [`openstitch_app::editor::wizard::ImageWizard`]; Convert runs its one `digitize.*` command,
//! so the whole conversion is one Undo.

use crate::app::EditorApp;
use eframe::egui;
use openstitch_app::editor::CommandRequest;
use openstitch_app::editor::wizard::{LINE_SEWS, Method, REGION_SEWS};

pub fn image_wizard_window(app: &mut EditorApp, ctx: &egui::Context) {
    let mut open = app.wizard.open;
    let mut convert = false;
    egui::Window::new("Image to Embroidery")
        .open(&mut open)
        .default_width(360.0)
        .resizable(false)
        .show(ctx, |ui| {
            let w = &mut app.wizard;
            crate::art::header(ui, crate::art::Art::WizardImage, |ui| {
                ui.add_space(4.0);
                if ui.button("Choose image…").clicked()
                    && let Some(p) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                        .pick_file()
                {
                    w.path = Some(p);
                }
                let name = w
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map_or("No image chosen".into(), |n| {
                        n.to_string_lossy().into_owned()
                    });
                ui.label(name);
            });
            ui.separator();
            for m in Method::ALL {
                ui.radio_value(&mut w.method, m, m.label())
                    .on_hover_text(m.hint());
            }
            ui.weak(w.method.hint());
            ui.separator();
            egui::Grid::new("wizard").num_columns(2).show(ui, |ui| {
                ui.label("Width");
                ui.horizontal(|ui| {
                    let mut fit = w.width_mm.is_none();
                    if ui.checkbox(&mut fit, "Fit page").changed() {
                        w.width_mm = if fit { None } else { Some(80.0) };
                    }
                    if let Some(v) = &mut w.width_mm {
                        ui.add(egui::DragValue::new(v).range(1.0..=2000.0).suffix(" mm"));
                    }
                });
                ui.end_row();
                let colours = match w.method {
                    Method::DesignCenter => false,
                    Method::PhotoFill | Method::PhotoLines => !w.mono,
                    _ => true,
                };
                if matches!(w.method, Method::PhotoFill | Method::PhotoLines) {
                    ui.label("Mode");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut w.mono, false, "Colour");
                        ui.radio_value(&mut w.mono, true, "Mono");
                    });
                    ui.end_row();
                }
                if colours {
                    ui.label("Colours");
                    ui.add(egui::Slider::new(&mut w.colors, 1..=32));
                    ui.end_row();
                }
                match w.method {
                    Method::AutoPunch => {
                        ui.label("Smallest region");
                        ui.add(egui::Slider::new(&mut w.min_area, 0.0..=50.0).suffix(" mm²"));
                        ui.end_row();
                        ui.label("Satin up to");
                        ui.add(egui::Slider::new(&mut w.satin_max, 1.0..=12.0).suffix(" mm"));
                        ui.end_row();
                    }
                    Method::CrossStitch => {
                        ui.label("Cell size");
                        ui.add(egui::Slider::new(&mut w.cell, 1.0..=10.0).suffix(" mm"));
                        ui.end_row();
                        ui.label("Stitch");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut w.half_cross, false, "Full cross");
                            ui.radio_value(&mut w.half_cross, true, "Half cross");
                        });
                        ui.end_row();
                    }
                    Method::PhotoFill | Method::PhotoLines => {
                        ui.label("Row spacing");
                        ui.horizontal(|ui| {
                            let mut auto = w.row_spacing.is_none();
                            if ui.checkbox(&mut auto, "Default").changed() {
                                w.row_spacing = if auto { None } else { Some(0.6) };
                            }
                            if let Some(v) = &mut w.row_spacing {
                                ui.add(
                                    egui::DragValue::new(v)
                                        .range(0.2..=5.0)
                                        .speed(0.05)
                                        .suffix(" mm"),
                                );
                            }
                        });
                        ui.end_row();
                    }
                    Method::DesignCenter => {
                        ui.label("Line threshold");
                        ui.add(egui::Slider::new(&mut w.threshold, 1..=254));
                        ui.end_row();
                        ui.label("Lines");
                        combo(ui, "lineSew", &mut w.line_sew, &LINE_SEWS);
                        ui.end_row();
                        ui.label("Regions");
                        combo(ui, "regionSew", &mut w.region_sew, &REGION_SEWS);
                        ui.end_row();
                    }
                }
                if matches!(w.method, Method::AutoPunch | Method::CrossStitch) {
                    ui.label("");
                    ui.checkbox(&mut w.skip_background, "Leave out the background colour");
                    ui.end_row();
                }
                if matches!(w.method, Method::AutoPunch | Method::DesignCenter) {
                    ui.label("");
                    ui.checkbox(&mut w.curves, "Fit curves");
                    ui.end_row();
                }
            });
            ui.separator();
            ui.weak("Approximations of PE-DESIGN's wizards; results are editable and one Undo.");
            ui.add_enabled_ui(w.path.is_some(), |ui| {
                convert = ui.button("Convert").clicked();
            });
        });
    app.wizard.open = open;
    if convert {
        run(app);
    }
}

fn combo(ui: &mut egui::Ui, id: &str, value: &mut &'static str, options: &[&'static str]) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(*value)
        .show_ui(ui, |ui| {
            for o in options {
                ui.selectable_value(value, o, *o);
            }
        });
}

/// Runs the conversion, selects what it made and closes the dialog.
pub fn run(app: &mut EditorApp) {
    let Some(request) = app.wizard.request() else {
        return;
    };
    if let Some(result) = app.grouped(|a| a.execute(&request)) {
        app.selection = CommandRequest::created_ids(&result);
        app.status = app.wizard.summary(&result);
        app.wizard.open = false;
    }
}
