//! Pattern wizard dialogs (Home ▸ Wizards): Background Fill, Appliqué Wizard, Embroidered
//! Patch, Cutwork Wizard. Settings live in
//! [`openstitch_app::editor::pattern_wizards`]; OK sends the wizard's one `wizard.*` command, so
//! the whole result is one Undo and stays editable.

use crate::app::EditorApp;
use crate::art::Art;
use eframe::egui;
use openstitch_app::editor::CommandRequest;
use openstitch_app::editor::pattern_wizards::{
    BackgroundForm, COVERS, FILL_TYPES, LINE_SEWS, MATERIALS, NET_PATTERNS, PATCH_CUTS, TACK_DOWNS,
    WizardKind,
};

pub fn windows(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(kind) = app.pattern_wizards.open else {
        return;
    };
    let mut open = true;
    let mut apply = false;
    egui::Window::new(kind.title())
        .open(&mut open)
        .default_width(380.0)
        .default_pos([260.0, 90.0])
        .resizable(false)
        .show(ctx, |ui| {
            let n = app.selection.len();
            let art = match kind {
                WizardKind::Background => Art::WizardBackground,
                WizardKind::Applique => Art::WizardApplique,
                WizardKind::Patch => Art::WizardPatch,
                WizardKind::Cutwork => Art::WizardCutwork,
            };
            crate::art::header(ui, art, |ui| {
                ui.add_space(14.0);
                ui.label(match n {
                    0 => "Select the patterns first.".to_string(),
                    1 => "1 pattern selected".to_string(),
                    _ => format!("{n} patterns selected"),
                });
            });
            ui.separator();
            match kind {
                WizardKind::Background => background(app, ui),
                WizardKind::Applique => applique(app, ui),
                WizardKind::Patch => patch(app, ui),
                WizardKind::Cutwork => cutwork(app, ui),
            }
            ui.separator();
            ui.horizontal(|ui| {
                let ready = app.pattern_wizards.request(kind, &app.selection).is_some();
                apply = ui.add_enabled(ready, egui::Button::new("OK")).clicked();
                if ui.button("Default").clicked() {
                    match kind {
                        WizardKind::Background => {
                            app.pattern_wizards.background = Default::default()
                        }
                        WizardKind::Applique => app.pattern_wizards.applique = Default::default(),
                        WizardKind::Patch => app.pattern_wizards.patch = Default::default(),
                        WizardKind::Cutwork => app.pattern_wizards.cutwork = Default::default(),
                    }
                }
            });
        });
    if !open {
        app.pattern_wizards.open = None;
    }
    if apply {
        run(app, kind);
        app.pattern_wizards.open = None;
    }
}

/// Applies the wizard to the selection and selects what it made.
pub fn run(app: &mut EditorApp, kind: WizardKind) {
    let Some(request) = app.pattern_wizards.request(kind, &app.selection) else {
        app.status = format!("{}: select the patterns first", kind.title());
        return;
    };
    if let Some(result) = app.grouped(|a| a.execute(&request)) {
        app.selection = CommandRequest::created_ids(&result);
        app.status = format!("{} applied", kind.title());
    }
}

fn mm(ui: &mut egui::Ui, label: &str, v: &mut f64, lo: f64, hi: f64) {
    ui.label(label);
    ui.add(egui::Slider::new(v, lo..=hi).suffix(" mm"));
    ui.end_row();
}

fn background(app: &mut EditorApp, ui: &mut egui::Ui) {
    let candidates = BackgroundForm::area_candidates(app.session.design(), &app.selection);
    let f = &mut app.pattern_wizards.background;
    if f.area.is_some_and(|a| !candidates.contains(&a)) {
        f.area = None;
    }
    egui::Grid::new("background").num_columns(2).show(ui, |ui| {
        ui.label("Area to fill");
        let name = |a: Option<u64>| a.map_or("Hoop".to_string(), |id| format!("Shape {id}"));
        egui::ComboBox::from_id_salt("bgArea")
            .selected_text(name(f.area))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut f.area, None, name(None));
                for id in &candidates {
                    ui.selectable_value(&mut f.area, Some(*id), name(Some(*id)));
                }
            });
        ui.end_row();
        ui.label("Fill type");
        let label = FILL_TYPES
            .iter()
            .find(|(k, _)| *k == f.fill_type)
            .map_or("", |(_, l)| *l);
        let mut chosen = f.fill_type;
        egui::ComboBox::from_id_salt("bgType")
            .selected_text(label)
            .show_ui(ui, |ui| {
                for (k, l) in FILL_TYPES {
                    ui.selectable_value(&mut chosen, k, l);
                }
            });
        if chosen != f.fill_type {
            f.set_fill_type(chosen);
        }
        ui.end_row();
        let (lo, hi) = f.spacing_range();
        let spacing_label = if f.fill_type == "echo" {
            "Spacing"
        } else {
            "Spacing between patterns"
        };
        mm(ui, spacing_label, &mut f.spacing, lo, hi);
        match f.fill_type {
            "echo" => {
                ui.label("Echo type");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut f.echo_clipped, true, "Echo clipped");
                    ui.radio_value(&mut f.echo_clipped, false, "Echo");
                });
                ui.end_row();
                if f.echo_clipped {
                    ui.label("");
                    ui.checkbox(&mut f.link_ends, "Link echo ends");
                    ui.end_row();
                }
            }
            "stippling" => mm(ui, "Stippling spacing", &mut f.stipple_spacing, 0.5, 10.0),
            "decorative" => {
                mm(ui, "Pattern width", &mut f.pattern_width, 2.0, 50.0);
                mm(ui, "Pattern height", &mut f.pattern_height, 2.0, 50.0);
                ui.label("");
                ui.checkbox(&mut f.connect_edges, "Connect object edges");
                ui.end_row();
            }
            _ => {
                mm(ui, "Line spacing", &mut f.line_spacing, 7.0, 30.0);
                ui.label("Angle");
                ui.add(egui::Slider::new(&mut f.angle, 45.0..=90.0).suffix("°"));
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut f.link_ends, "Link line ends");
                ui.end_row();
            }
        }
        if f.uses_lines() {
            ui.label("Sew type");
            egui::ComboBox::from_id_salt("bgSew")
                .selected_text(
                    LINE_SEWS
                        .iter()
                        .find(|(k, _)| *k == f.sew)
                        .map_or("", |(_, l)| *l),
                )
                .show_ui(ui, |ui| {
                    for (k, l) in LINE_SEWS {
                        ui.selectable_value(&mut f.sew, k, l);
                    }
                });
            ui.end_row();
            if f.sew == "chain" {
                ui.label("Stitch type");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut f.chain_diamond, false, "Triangle");
                    ui.radio_value(&mut f.chain_diamond, true, "Diamond");
                });
                ui.end_row();
                mm(ui, "Size", &mut f.chain_size, 1.0, 10.0);
            } else {
                mm(ui, "Run pitch", &mut f.pitch, 1.0, 10.0);
            }
            if f.sew != "triple" {
                ui.label("Times");
                ui.add(egui::Slider::new(&mut f.run_times, 1..=5));
                ui.end_row();
            }
        }
        if f.fill_type != "echo" {
            ui.label("");
            ui.checkbox(&mut f.skip_overlap, "Skip overlapped background");
            ui.end_row();
        }
    });
    ui.weak("Fills around the selected patterns; the result is sewn before them.");
}

/// A combo box over `(value, label)` choices.
fn choice(ui: &mut egui::Ui, id: &str, value: &mut &'static str, options: &[(&'static str, &str)]) {
    let label = options
        .iter()
        .find(|(k, _)| k == value)
        .map_or("", |(_, l)| *l);
    egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (k, l) in options {
                ui.selectable_value(value, *k, *l);
            }
        });
}

fn applique(app: &mut EditorApp, ui: &mut egui::Ui) {
    let f = &mut app.pattern_wizards.applique;
    egui::Grid::new("applique").num_columns(2).show(ui, |ui| {
        ui.strong("1 Appliqué material");
        ui.end_row();
        ui.label("Material");
        choice(ui, "apMaterial", &mut f.material, &MATERIALS);
        ui.end_row();
        ui.strong("2 Appliqué position");
        ui.end_row();
        ui.label("");
        ui.weak("Running stitch along the outline (automatic)");
        ui.end_row();
        ui.strong("3 Tack down");
        ui.end_row();
        ui.label("Stitch");
        choice(ui, "apTack", &mut f.tack_down, &TACK_DOWNS);
        ui.end_row();
        match f.tack_down {
            "none" => {}
            "zigzag" => {
                mm(ui, "Width", &mut f.tack_width, 0.5, 10.0);
                ui.label("Density");
                ui.add(egui::Slider::new(&mut f.tack_density, 0.2..=5.0).suffix(" lines/mm"));
                ui.end_row();
            }
            _ => {
                mm(ui, "Stroke", &mut f.tack_width, 0.5, 10.0);
                mm(ui, "Interval", &mut f.tack_interval, 0.5, 10.0);
            }
        }
        ui.strong("4 Covering stitch");
        ui.end_row();
        ui.label("Stitch");
        choice(ui, "apCover", &mut f.cover, &COVERS);
        ui.end_row();
        match f.cover {
            "zigzag" => {
                mm(ui, "Width", &mut f.cover_width, 0.5, 10.0);
                ui.label("Density");
                ui.add(egui::Slider::new(&mut f.cover_density, 1.0..=10.0).suffix(" lines/mm"));
                ui.end_row();
            }
            "chain" => {
                ui.label("Stitch type");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut f.chain_diamond, false, "Triangle");
                    ui.radio_value(&mut f.chain_diamond, true, "Diamond");
                });
                ui.end_row();
                mm(ui, "Size", &mut f.cover_width, 0.5, 10.0);
                ui.label("Times");
                ui.add(egui::Slider::new(&mut f.cover_times, 1..=5));
                ui.end_row();
            }
            _ => {
                mm(ui, "Stroke", &mut f.cover_width, 0.5, 10.0);
                mm(ui, "Interval", &mut f.cover_interval, 0.5, 10.0);
                mm(ui, "Pitch", &mut f.cover_pitch, 1.0, 10.0);
            }
        }
        ui.strong("5 Output pattern");
        ui.end_row();
        ui.label("");
        ui.horizontal(|ui| {
            ui.radio_value(&mut f.replace, false, "Add");
            ui.radio_value(&mut f.replace, true, "Replace");
        });
        ui.end_row();
        if f.replace {
            ui.label("");
            ui.checkbox(&mut f.holes, "Create an appliqué with hole sewing");
            ui.end_row();
        } else {
            mm(ui, "Distance from pattern", &mut f.distance, 0.0, 50.0);
        }
    });
    ui.weak("Machine stops between material, position, tack-down and cover (colour changes).");
}

fn patch(app: &mut EditorApp, ui: &mut egui::Ui) {
    let f = &mut app.pattern_wizards.patch;
    egui::Grid::new("patch").num_columns(2).show(ui, |ui| {
        ui.strong("Satin stitch");
        ui.end_row();
        ui.label("Border");
        ui.horizontal(|ui| {
            ui.radio_value(&mut f.border, true, "Yes");
            ui.radio_value(&mut f.border, false, "No");
        });
        ui.end_row();
        if f.border {
            mm(ui, "Width", &mut f.width, 1.0, 10.0);
            ui.label("Density");
            ui.add(egui::Slider::new(&mut f.density, 1.0..=10.0).suffix(" lines/mm"));
            ui.end_row();
            mm(
                ui,
                "Distance from original pattern",
                &mut f.distance,
                0.0,
                50.0,
            );
        }
        ui.strong("Cutting line");
        ui.end_row();
        ui.label("Cut by");
        choice(ui, "patchCut", &mut f.cutting, &PATCH_CUTS);
        ui.end_row();
        if f.cutting != "none" {
            let label = if f.border {
                "Distance from satin stitch"
            } else {
                "Distance from pattern"
            };
            mm(ui, label, &mut f.cut_distance, 0.0, 20.0);
        }
    });
    ui.weak("Border in the current thread; the machine stops before the cutting line.");
}

fn yes_no(ui: &mut egui::Ui, label: &str, v: &mut bool) {
    ui.label(label);
    ui.horizontal(|ui| {
        ui.radio_value(v, true, "Yes");
        ui.radio_value(v, false, "No");
    });
    ui.end_row();
}

fn cutwork(app: &mut EditorApp, ui: &mut egui::Ui) {
    let f = &mut app.pattern_wizards.cutwork;
    egui::Grid::new("cutwork").num_columns(2).show(ui, |ui| {
        yes_no(ui, "1 Stabilize cut edge", &mut f.stabilize);
        ui.label("2 Cutting line");
        ui.horizontal(|ui| {
            ui.radio_value(&mut f.needles, true, "Cutting");
            ui.radio_value(&mut f.needles, false, "Running stitch");
        });
        ui.end_row();
        yes_no(ui, "3 Secure water-soluble stabilizer", &mut f.secure);
        yes_no(ui, "4 Net fill", &mut f.net);
        if f.net {
            ui.label("Pattern");
            egui::ComboBox::from_id_salt("cwNet")
                .selected_text(NET_PATTERNS[(f.net_pattern.clamp(1, 5) - 1) as usize])
                .show_ui(ui, |ui| {
                    for (i, name) in NET_PATTERNS.iter().enumerate() {
                        ui.selectable_value(&mut f.net_pattern, i as u32 + 1, *name);
                    }
                });
            ui.end_row();
            mm(ui, "Spacing", &mut f.net_spacing, 2.0, 10.0);
            ui.label("Direction");
            ui.add(egui::Slider::new(&mut f.net_direction, 0.0..=90.0).suffix("°"));
            ui.end_row();
        }
        yes_no(ui, "5 Covering satin stitch", &mut f.cover);
        if f.cover {
            mm(ui, "Width", &mut f.cover_width, 3.0, 6.0);
            ui.label("Density");
            ui.add(egui::Slider::new(&mut f.cover_density, 5.0..=7.0).suffix(" lines/mm"));
            ui.end_row();
        }
    });
    ui.weak("Each cutwork needle is its own colour block (\"Cutwork needle N\"): the machine stops to change needles.");
}
